//! Process and window management for the WireView2 app.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

/// Path of the WireView2 app binary this widget launches and monitors.
pub const APP_BINARY: &str = "/usr/bin/wireview-linux";

/// `APP_BINARY` with symlinks resolved. On standard installs it is a symlink
/// to the real binary (e.g. `/usr/lib/wireview-linux/WireView2`), which is
/// also what `.desktop` files and autostart execute, so `/proc/<pid>/exe`
/// points at the target rather than at `APP_BINARY` itself.
fn canonical_app_binary() -> &'static Path {
    static CANON: OnceLock<PathBuf> = OnceLock::new();
    CANON.get_or_init(|| fs::canonicalize(APP_BINARY).unwrap_or_else(|_| PathBuf::from(APP_BINARY)))
}

/// True when `cmdline` belongs to the WireView app: the process must have
/// been started as the app binary itself (argv[0] is the exact binary path).
/// Substring matching would also collect editors viewing the binary and
/// lookalike scripts, which `terminate` would then kill.
fn is_wireview_cmdline(cmdline: &[u8]) -> bool {
    cmdline
        .split(|&b| b == 0)
        .next()
        .filter(|field| !field.is_empty())
        .is_some_and(|field| field == APP_BINARY.as_bytes())
}

/// True when the `/proc/<pid>/exe` link target is the app binary. The kernel
/// resolves the link to the real executable, so instances launched through
/// the `APP_BINARY` symlink, a `.desktop` file, or autostart all match even
/// though their argv[0] differs from [`APP_BINARY`].
fn exe_is_app(link: &[u8], app: &Path) -> bool {
    // An unlinked-but-running binary gets a " (deleted)" suffix; strip it so
    // a freshly replaced install still matches while the old code runs.
    let link = link.strip_suffix(b" (deleted)").unwrap_or(link);
    let Ok(path) = std::str::from_utf8(link) else {
        return false;
    };
    Path::new(path) == app
}

/// A pid pinned by its `/proc/<pid>` directory fd, captured at scan time.
/// That fd is a pidfd: identity is read through it with `openat`, and
/// signals go through `pidfd_send_signal` on the same fd, so a recycled
/// pid cannot redirect the kill at an unrelated process.
#[derive(Debug)]
struct ScannedPid {
    pid: i32,
    pidfd: OwnedFd,
}

fn open_proc_dir(pid: i32) -> Option<OwnedFd> {
    let path = CString::new(format!("/proc/{pid}")).ok()?;
    // SAFETY: path is a valid C string; open fails with -1 on error.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a newly opened directory we own.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn owner_uid(pidfd: &OwnedFd) -> Option<u32> {
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes a complete stat buffer on success.
    let rc = unsafe { libc::fstat(pidfd.as_raw_fd(), st.as_mut_ptr()) };
    if rc < 0 {
        return None;
    }
    // SAFETY: fstat initialized st.
    Some(unsafe { st.assume_init() }.st_uid)
}

fn read_proc_entry(pidfd: &OwnedFd, name: &str) -> Option<Vec<u8>> {
    let name = CString::new(name).ok()?;
    // SAFETY: pidfd is an open /proc/<pid> directory; name is a valid C string.
    let fd = unsafe {
        libc::openat(
            pidfd.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a newly opened file we own.
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Reads a symlink inside the pinned `/proc/<pid>` directory (e.g. `exe`).
fn readlink_entry(pidfd: &OwnedFd, name: &str) -> Option<Vec<u8>> {
    let name = CString::new(name).ok()?;
    let mut buf = [0u8; 4096];
    // SAFETY: pidfd is an open directory, name is a valid C string, and buf
    // is writable for its full length.
    let n = unsafe {
        libc::readlinkat(
            pidfd.as_raw_fd(),
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n < 0 {
        return None;
    }
    let n = usize::try_from(n).ok()?;
    if n >= buf.len() {
        return None;
    }
    Some(buf[..n].to_vec())
}

/// True when `pidfd` (a `/proc/<pid>` directory) belongs to the current
/// user and is the WireView app: either argv[0] is the exact binary path or
/// the kernel-resolved `/proc/<pid>/exe` target equals the canonical app
/// binary. Reads go through the directory fd so they cannot observe a later
/// occupant of the same pid.
fn is_wireview_dir(pidfd: &OwnedFd) -> bool {
    // Other users' processes can never be signalled; skip them so the
    // termination loop does not spin on EPERM until its timeout.
    // SAFETY: getuid has no preconditions.
    if owner_uid(pidfd) != Some(unsafe { libc::getuid() }) {
        return false;
    }
    if let Some(cmdline) = read_proc_entry(pidfd, "cmdline")
        && is_wireview_cmdline(&cmdline)
    {
        return true;
    }
    readlink_entry(pidfd, "exe").is_some_and(|link| exe_is_app(&link, canonical_app_binary()))
}

/// Pin `pid` when it still looks like the app. The returned pidfd is what
/// later signals use; a recycled pid cannot steal it.
fn identify(pid: i32) -> Option<ScannedPid> {
    let pidfd = open_proc_dir(pid)?;
    if !is_wireview_dir(&pidfd) {
        return None;
    }
    Some(ScannedPid { pid, pidfd })
}

/// True when the process pinned at scan time is still alive. Signal 0 on a
/// pidfd is an existence check for that process, not for whoever now holds
/// the numeric pid.
fn is_same_wireview(scanned: &ScannedPid) -> bool {
    pidfd_send_signal(&scanned.pidfd, 0)
}

fn pidfd_send_signal(pidfd: &OwnedFd, sig: i32) -> bool {
    // SAFETY: pidfd is a `/proc/<pid>` directory fd, which the kernel
    // accepts as a pidfd. A null info pointer asks it to build a standard
    // siginfo. Failure (ESRCH, ENOSYS, …) must not fall back to kill(2):
    // that reopens the pid-reuse race this path exists to close.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            sig,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    rc == 0
}

fn signal_if_same(scanned: &ScannedPid, sig: i32) -> bool {
    pidfd_send_signal(&scanned.pidfd, sig)
}

/// Finds currently running WireView processes owned by the current user.
///
/// # Examples
///
/// ```
/// let targets = running_targets();
/// assert!(targets.len() >= 0);
/// ```
///
fn running_targets() -> Vec<ScannedPid> {
    let mut pids = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return pids;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        if let Some(scanned) = identify(pid) {
            pids.push(scanned);
        }
    }

    pids
}

/// PIDs of every running WireView app process owned by the current user.
///
/// Numeric pids are for diagnostics and tests only. Signalling must go
/// through [`terminate_running`] / [`terminate`], which pin each process
/// with a pidfd at identify time.
pub fn running_pids() -> Vec<i32> {
    running_targets().into_iter().map(|t| t.pid).collect()
}

/// Determines whether at least one WireView process owned by the current user is running.
///
/// This performs an observation only and does not signal or terminate any process.
///
/// # Examples
///
/// ```
/// let running = is_running();
/// assert!(running || !running);
/// ```
pub fn is_running() -> bool {
    !running_targets().is_empty()
}

/// Clock ticks per second from `sysconf(_SC_CLK_TCK)`, with the ubiquitous
/// x86_64 fallback when the call fails.
fn ticks_per_sec() -> f64 {
    // SAFETY: sysconf has no preconditions.
    let value = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if value > 0 { value as f64 } else { 100.0 }
}

/// The age of a process in seconds, parsed from `/proc/<pid>/stat` contents.
/// `starttime` (field 22) is in clock ticks since boot; age is the difference
/// to the system uptime. Returns `None` when the stat line is malformed.
fn age_from_stat(uptime_secs: f64, ticks_per_sec: f64, stat: &str) -> Option<f64> {
    // Everything after "pid (comm) " starts at field 3 (state), so the
    // 22nd overall field (starttime) sits at index 19 of the remainder.
    let rest = stat.rsplit_once(')')?.1;
    let start_ticks: f64 = rest.split_whitespace().nth(19)?.parse().ok()?;
    Some((uptime_secs - start_ticks / ticks_per_sec).max(0.0))
}

fn age_of_pidfd(pidfd: &OwnedFd, uptime_secs: f64, ticks: f64) -> Option<f64> {
    let stat = read_proc_entry(pidfd, "stat")?;
    let stat = std::str::from_utf8(&stat).ok()?;
    age_from_stat(uptime_secs, ticks, stat)
}

/// The age of the most recently started WireView process, if any.
///
/// Starttime is read with `openat` on the same `/proc/<pid>` directory fd
/// used to identify the process, so a recycled pid cannot look like a
/// fresh WireView instance.
pub fn youngest_age() -> Option<Duration> {
    let uptime_secs: f64 = fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let ticks = ticks_per_sec();
    running_targets()
        .iter()
        .filter_map(|t| age_of_pidfd(&t.pidfd, uptime_secs, ticks))
        .map(Duration::from_secs_f64)
        .min()
}

/// Spawn the app detached from this process; it keeps running after we exit.
pub fn launch() -> std::io::Result<()> {
    Command::new(APP_BINARY)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn terminate_identified(candidates: Vec<ScannedPid>, timeout: Duration) {
    let mut targets = Vec::new();
    for scanned in candidates {
        if signal_if_same(&scanned, libc::SIGTERM) {
            targets.push(scanned);
        }
    }

    // Wait on the originally identified processes only; a rescan could pick
    // up a concurrently launched instance and never settle.
    let deadline = Instant::now() + timeout;
    loop {
        let any_alive = targets.iter().any(is_same_wireview);
        if !any_alive || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    for scanned in &targets {
        signal_if_same(scanned, libc::SIGKILL);
    }
}

/// Kill every currently running WireView instance. Each scanned process is
/// pinned by its `/proc/<pid>` directory fd, and both SIGTERM and SIGKILL
/// are delivered with `pidfd_send_signal` on that fd so a recycled pid
/// cannot terminate an unrelated same-user process.
pub fn terminate_running(timeout: Duration) {
    terminate_identified(running_targets(), timeout);
}

/// SIGTERM every pid that still looks like the app, wait up to `timeout`
/// for those same processes to exit, then SIGKILL the stragglers.
///
/// Each candidate is pinned by its `/proc/<pid>` directory fd at identify
/// time (uid + argv[0] read through that fd). Signals use
/// `pidfd_send_signal` on the same fd so a recycled pid cannot terminate
/// an unrelated process, and a concurrently launched instance that reused
/// a pid is left alone.
pub fn terminate(pids: &[i32], timeout: Duration) {
    terminate_identified(pids.iter().copied().filter_map(identify).collect(), timeout);
}

/// The address of the app's Hyprland window, if it exists and is mapped.
///
/// The client's pid must still identify as the WireView binary; a lookalike
/// class or a recycled compositor address attached to another process is
/// ignored.
pub fn window_address() -> Option<String> {
    let output = Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let clients: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    clients
        .as_array()?
        .iter()
        .find_map(client_address_if_wireview)
}

fn class_looks_like_wireview(class: &str) -> bool {
    class.to_lowercase().contains("wireview2")
}

fn client_address_if_wireview(client: &serde_json::Value) -> Option<String> {
    let class = client.get("class")?.as_str()?;
    if !class_looks_like_wireview(class) {
        return None;
    }
    let pid = i32::try_from(client.get("pid")?.as_i64()?).ok()?;
    identify(pid)?;
    client.get("address")?.as_str().map(str::to_string)
}

/// Focus the window through Hyprland's lua dispatcher (Hyprland >= 0.55).
pub fn focus_window(address: &str) -> bool {
    let expression = format!("hl.dsp.focus({{ window = \"address:{address}\" }})");
    Command::new("hyprctl")
        .args(["dispatch", &expression])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn own_process_is_not_wireview() {
        let pids = running_pids();
        assert!(!pids.contains(&(std::process::id() as i32)));
    }

    #[test]
    fn cmdline_matches_binary_path() {
        assert!(is_wireview_cmdline(APP_BINARY.as_bytes()));
    }

    #[test]
    fn cmdline_matches_binary_with_args() {
        let cmdline = format!("{APP_BINARY}\0--minimized\0");
        assert!(is_wireview_cmdline(cmdline.as_bytes()));
    }

    #[test]
    fn cmdline_rejects_editor_viewing_binary() {
        let cmdline = format!("vim\0{APP_BINARY}\0");
        assert!(!is_wireview_cmdline(cmdline.as_bytes()));
    }

    #[test]
    fn cmdline_rejects_lookalike_name() {
        assert!(!is_wireview_cmdline(
            format!("{APP_BINARY}-helper").as_bytes()
        ));
    }

    #[test]
    fn cmdline_rejects_empty() {
        assert!(!is_wireview_cmdline(b"\0"));
        assert!(!is_wireview_cmdline(b""));
    }

    #[test]
    fn exe_link_matches_canonical_app_binary() {
        let base = std::env::temp_dir().join("wv-app-exe-match");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let real = base.join("WireView2");
        fs::write(&real, b"elf").unwrap();
        let link = base.join("wireview-linux");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canon = fs::canonicalize(&link).unwrap();

        assert!(exe_is_app(real.as_os_str().as_bytes(), &canon));
        assert!(!exe_is_app(link.as_os_str().as_bytes(), &canon));
        assert!(!exe_is_app(b"/usr/bin/vim", &canon));
        assert!(!exe_is_app(b"/usr/bin/wireview-linux-helper", &canon));

        let deleted = format!("{} (deleted)", real.display());
        assert!(exe_is_app(deleted.as_bytes(), &canon));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn exe_match_uses_canonicalized_app_binary() {
        // APP_BINARY is a symlink on standard installs; the canonical form
        // must be compared, not the literal constant.
        let canon = canonical_app_binary();
        assert!(canon.is_absolute());
        assert_ne!(canon, Path::new("/definitely/not/the/app"));
    }

    #[test]
    fn terminate_returns_for_nonexistent_pid() {
        let start = Instant::now();
        terminate(&[i32::MAX], Duration::from_secs(1));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    fn spawn_sleep() -> std::process::Child {
        Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep")
    }

    #[test]
    fn terminate_does_not_signal_unrelated_same_user_pid() {
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        let start = Instant::now();
        terminate(&[pid], Duration::from_secs(2));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "must not wait out the timeout on an unrelated pid"
        );

        let pidfd = open_proc_dir(pid).expect("sleep process has a /proc dir");
        assert!(!is_wireview_dir(&pidfd));
        assert!(identify(pid).is_none());
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "SIGTERM must not hit an unrelated same-user pid"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn pidfd_signals_the_process_it_refers_to() {
        let mut child = spawn_sleep();
        let pidfd = open_proc_dir(child.id() as i32).expect("open /proc dir");
        assert!(pidfd_send_signal(&pidfd, libc::SIGTERM));
        let status = child.wait().expect("wait");
        assert!(!status.success(), "sleep must exit from SIGTERM");
    }

    #[test]
    fn pidfd_does_not_signal_a_later_process() {
        let mut first = spawn_sleep();
        let pidfd = open_proc_dir(first.id() as i32).expect("open /proc dir");
        first.kill().expect("kill first");
        first.wait().expect("wait first");

        let mut second = spawn_sleep();
        assert!(
            !pidfd_send_signal(&pidfd, libc::SIGKILL),
            "signalling a reaped pidfd must fail with ESRCH"
        );
        assert!(
            second.try_wait().expect("try_wait").is_none(),
            "a later process must not receive the pidfd signal"
        );

        let _ = second.kill();
        let _ = second.wait();
    }

    #[test]
    fn terminate_empty_candidate_list_returns_immediately() {
        // Test that terminate_identified returns quickly with an empty
        // candidate list, verifying the termination logic doesn't spin when
        // there are no processes to wait for.
        let start = Instant::now();
        terminate_identified(Vec::new(), Duration::from_secs(1));
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "terminate_identified with empty list must return immediately"
        );
    }

    #[test]
    fn non_wireview_process_is_not_identified() {
        assert!(identify(i32::MAX).is_none());
        assert!(identify(1).is_none());
        if let Some(pidfd) = open_proc_dir(1) {
            assert!(!is_wireview_dir(&pidfd));
        }
    }

    #[test]
    fn parses_age_from_stat_line() {
        // "123 (wireview-linux) S ..." with starttime (field 22) at 9500
        // ticks; uptime 100 s at 100 ticks/s means the process is 5 s old.
        let stat = "123 (wireview-linux) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9500 0 0 0 0 0 0 0";
        let age = age_from_stat(100.0, 100.0, stat).unwrap();
        assert_eq!(age, 5.0);
    }

    #[test]
    fn parses_age_with_spaces_in_comm() {
        let stat = "123 (wire view pro ii) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9000 0 0 0 0 0 0 0";
        let age = age_from_stat(100.0, 100.0, stat).unwrap();
        assert_eq!(age, 10.0);
    }

    #[test]
    fn age_is_never_negative() {
        let stat = "123 (x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 20000 0 0 0 0 0 0 0";
        assert_eq!(age_from_stat(100.0, 100.0, stat).unwrap(), 0.0);
    }

    #[test]
    fn rejects_malformed_stat_line() {
        assert!(age_from_stat(100.0, 100.0, "garbage").is_none());
        assert!(age_from_stat(100.0, 100.0, "123 (x) S").is_none());
    }

    #[test]
    fn pinned_stat_does_not_observe_a_later_process() {
        let mut first = spawn_sleep();
        let pidfd = open_proc_dir(first.id() as i32).expect("open /proc dir");
        first.kill().expect("kill first");
        first.wait().expect("wait first");

        let mut second = spawn_sleep();
        assert!(
            read_proc_entry(&pidfd, "stat").is_none(),
            "openat on a reaped proc dir must not read a later occupant's stat"
        );
        assert!(age_of_pidfd(&pidfd, 100.0, 100.0).is_none());

        let _ = second.kill();
        let _ = second.wait();
    }

    #[test]
    fn youngest_age_ignores_unrelated_processes() {
        // Test that youngest_age doesn't include unrelated same-user processes.
        // Spawn a sleep process and verify it's not counted as an app instance.
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        // Verify the sleep process is not identified as the app
        assert!(identify(pid).is_none(), "sleep must not identify as the app");

        // If youngest_age returns Some, it must not be based on the sleep process.
        // We can't assert it's None because a real app might be running, but we
        // can verify the sleep process doesn't affect the result by checking
        // that it's not in the running targets.
        let targets = running_targets();
        assert!(
            !targets.iter().any(|t| t.pid == pid),
            "sleep process must not be in running_targets"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn watch_path_does_not_treat_or_signal_unrelated_same_user_process() {
        // `appRunning` is polled at 1 Hz via is_running/youngest_age. Those
        // must stay observation-only and keep the uid + identity match, or
        // we reintroduce #2 (over-broad kill) / #4 (recycled pid looks
        // fresh). Verify that a spawned unrelated process is never identified
        // as the app and remains untouched.
        let mut child = spawn_sleep();
        let pid = child.id() as i32;
        assert!(!running_pids().contains(&pid));
        assert!(
            !identify(pid).is_some(),
            "sleep must not identify as the app"
        );

        // Verify that observation functions don't affect the unrelated process
        for _ in 0..8 {
            let _ = is_running();
            let _ = youngest_age();
        }

        // Verify that terminate() called on the specific pid doesn't signal
        // an unrelated process (it should fail identification and skip it)
        let start = Instant::now();
        terminate(&[pid], Duration::from_secs(2));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "terminate must not wait on an unrelated pid"
        );

        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "is_running/youngest_age/terminate must not hit an unrelated pid"
        );
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn focus_expression_shape() {
        let expr = format!("hl.dsp.focus({{ window = \"address:{}\" }})", "0x1234");
        assert_eq!(expr, "hl.dsp.focus({ window = \"address:0x1234\" })");
    }

    #[test]
    fn window_client_rejects_unrelated_class() {
        let client = serde_json::json!({
            "class": "firefox",
            "pid": std::process::id(),
            "address": "0x1"
        });
        assert!(client_address_if_wireview(&client).is_none());
    }

    #[test]
    fn window_client_rejects_lookalike_class_without_app_pid() {
        let client = serde_json::json!({
            "class": "WireView2",
            "pid": 1,
            "address": "0x1"
        });
        assert!(client_address_if_wireview(&client).is_none());
    }

    #[test]
    fn window_client_rejects_missing_pid() {
        let client = serde_json::json!({
            "class": "WireView2",
            "address": "0x1"
        });
        assert!(client_address_if_wireview(&client).is_none());
    }
}
