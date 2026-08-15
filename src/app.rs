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

/// PIDs of every running WireView app process owned by the current user.
pub fn running_pids() -> Vec<i32> {
    let mut pids = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return pids;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let proc = format!("/proc/{pid}");
        // Other users' processes can never be signalled; skip them so the
        // termination loop does not spin on EPERM until its timeout.
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let Ok(meta) = fs::metadata(&proc) else {
            continue;
        };
        if meta.uid() != uid {
            continue;
        }
        let Ok(cmdline) = fs::read(format!("{proc}/cmdline")) else {
            continue;
        };
        if is_wireview_cmdline(&cmdline) {
            pids.push(pid);
        }
    }

    pids
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

/// SIGTERM every pid, wait up to `timeout` for them to exit, then SIGKILL
/// the stragglers.
pub fn terminate(pids: &[i32], timeout: Duration) {
    for &pid in pids {
        // SAFETY: signalling another process with a valid pid.
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }

    let deadline = Instant::now() + timeout;
    loop {
        let alive = running_pids();
        if alive.is_empty() || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    for &pid in running_pids().iter() {
        // SAFETY: signalling another process with a valid pid.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
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
    fn focus_expression_shape() {
        let expr = format!("hl.dsp.focus({{ window = \"address:{}\" }})", "0x1234");
        assert_eq!(expr, "hl.dsp.focus({ window = \"address:0x1234\" })");
    }
}
