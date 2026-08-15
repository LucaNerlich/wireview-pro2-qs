//! Process and window management for the WireView2 app.

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How the app is launched on this system (wireview-linux-bin AUR package).
pub const APP_BINARY: &str = "/usr/bin/wireview-linux";
/// Substring that identifies the app in `/proc/<pid>/cmdline`.
pub const PROC_MARKER: &str = "wireview-linux";

/// PIDs of every process whose command line mentions the WireView app.
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
        let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let text = String::from_utf8_lossy(&cmdline);
        if text.split('\0').any(|arg| arg.contains(PROC_MARKER)) {
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
    fn marker_matches_binary_path() {
        assert!(APP_BINARY.contains(PROC_MARKER));
    }

    #[test]
    fn focus_expression_shape() {
        let expr = format!("hl.dsp.focus({{ window = \"address:{}\" }})", "0x1234");
        assert_eq!(expr, "hl.dsp.focus({ window = \"address:0x1234\" })");
    }
}
