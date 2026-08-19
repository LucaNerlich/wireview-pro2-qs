//! Process and window management for the WireView2 app.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How the app is launched on this system (wireview-linux-bin AUR package).
pub const APP_BINARY: &str = "/usr/bin/wireview-linux";

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

/// A pid together with `/proc/<pid>/stat` starttime (field 22), captured
/// at scan time so later signals can refuse a recycled pid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScannedPid {
    pid: i32,
    starttime: u64,
}

/// Clock ticks since boot from field 22 of a `/proc/<pid>/stat` line.
fn starttime_from_stat(stat: &str) -> Option<u64> {
    // Everything after "pid (comm) " starts at field 3 (state), so the
    // 22nd overall field (starttime) sits at index 19 of the remainder.
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

fn read_starttime(pid: i32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    starttime_from_stat(&stat)
}

/// True when `/proc/<pid>` belongs to the current user and its argv[0] is
/// the app binary. Re-checking identity immediately before a signal keeps
/// pid recycling from redirecting the kill at an unrelated process.
fn is_wireview_process(pid: i32) -> bool {
    let proc = format!("/proc/{pid}");
    let Ok(meta) = fs::metadata(&proc) else {
        return false;
    };
    // Other users' processes can never be signalled; skip them so the
    // termination loop does not spin on EPERM until its timeout.
    // SAFETY: getuid has no preconditions.
    if meta.uid() != unsafe { libc::getuid() } {
        return false;
    }
    let Ok(cmdline) = fs::read(format!("{proc}/cmdline")) else {
        return false;
    };
    is_wireview_cmdline(&cmdline)
}

/// Identity plus starttime of `pid` when it still looks like the app.
fn identify(pid: i32) -> Option<ScannedPid> {
    if !is_wireview_process(pid) {
        return None;
    }
    Some(ScannedPid {
        pid,
        starttime: read_starttime(pid)?,
    })
}

/// True when `pid` still names the same WireView process observed earlier.
fn is_same_wireview(scanned: ScannedPid) -> bool {
    identify(scanned.pid) == Some(scanned)
}

fn signal_if_same(scanned: ScannedPid, sig: i32) -> bool {
    if !is_same_wireview(scanned) {
        return false;
    }
    // SAFETY: signalling a verified WireView process with a valid pid.
    unsafe { libc::kill(scanned.pid, sig) };
    true
}

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
pub fn running_pids() -> Vec<i32> {
    running_targets().into_iter().map(|t| t.pid).collect()
}

pub fn is_running() -> bool {
    !running_pids().is_empty()
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
        if signal_if_same(scanned, libc::SIGTERM) {
            targets.push(scanned);
        }
    }

    // Wait on the originally identified processes only; a rescan could pick
    // up a concurrently launched instance and never settle.
    let deadline = Instant::now() + timeout;
    loop {
        let any_alive = targets.iter().copied().any(is_same_wireview);
        if !any_alive || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    for scanned in targets {
        signal_if_same(scanned, libc::SIGKILL);
    }
}

/// Kill every currently running WireView instance. Starttimes are captured
/// during the scan and re-checked immediately before each signal so a
/// recycled pid cannot terminate an unrelated same-user process.
pub fn terminate_running(timeout: Duration) {
    terminate_identified(running_targets(), timeout);
}

/// SIGTERM every pid that still looks like the app, wait up to `timeout`
/// for those same processes to exit, then SIGKILL the stragglers.
///
/// Identity (uid + argv[0]) and starttime are re-checked immediately before
/// each signal so a recycled pid cannot terminate an unrelated process, and
/// so a concurrently launched instance that reused a pid is left alone.
pub fn terminate(pids: &[i32], timeout: Duration) {
    terminate_identified(pids.iter().copied().filter_map(identify).collect(), timeout);
}

/// The address of the app's Hyprland window, if it exists and is mapped.
pub fn window_address() -> Option<String> {
    let output = Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let clients: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    clients.as_array()?.iter().find_map(|client| {
        let class = client.get("class")?.as_str()?;
        if class.to_lowercase().contains("wireview2") {
            client.get("address")?.as_str().map(str::to_string)
        } else {
            None
        }
    })
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
    fn terminate_returns_for_nonexistent_pid() {
        let start = Instant::now();
        terminate(&[i32::MAX], Duration::from_secs(1));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn terminate_does_not_signal_unrelated_same_user_pid() {
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let pid = child.id() as i32;
        let starttime = read_starttime(pid).expect("sleep process has a starttime");

        let start = Instant::now();
        terminate(&[pid], Duration::from_secs(2));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "must not wait out the timeout on an unrelated pid"
        );

        // Identity must fail even when the captured starttime is genuine.
        assert!(identify(pid).is_none());
        assert!(!is_same_wireview(ScannedPid { pid, starttime }));
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "SIGTERM must not hit an unrelated same-user pid"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn terminate_running_returns_without_app() {
        let start = Instant::now();
        terminate_running(Duration::from_secs(1));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn starttime_from_stat_reads_field_22() {
        let stat = "123 (wireview-linux) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9500 0 0";
        assert_eq!(starttime_from_stat(stat), Some(9500));
    }

    #[test]
    fn starttime_from_stat_handles_spaces_in_comm() {
        let stat = "123 (wire view) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9000 0 0";
        assert_eq!(starttime_from_stat(stat), Some(9000));
    }

    #[test]
    fn starttime_from_stat_rejects_malformed() {
        assert!(starttime_from_stat("garbage").is_none());
        assert!(starttime_from_stat("123 (x) S").is_none());
    }

    #[test]
    fn non_wireview_process_is_not_identified() {
        assert!(!is_wireview_process(i32::MAX));
        assert!(!is_wireview_process(1));
        assert!(identify(i32::MAX).is_none());
        assert!(!is_same_wireview(ScannedPid {
            pid: 1,
            starttime: 0
        }));
    }

    #[test]
    fn focus_expression_shape() {
        let expr = format!("hl.dsp.focus({{ window = \"address:{}\" }})", "0x1234");
        assert_eq!(expr, "hl.dsp.focus({ window = \"address:0x1234\" })");
    }
}
