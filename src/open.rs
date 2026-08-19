//! User-facing app actions: open (launch/focus/restart), restart, quit.

use std::fs;
use std::time::Duration;

use crate::app;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOutcome {
    /// The app was not running; a fresh instance was started.
    Launched,
    /// The app was running with a window; it was focused.
    Focused,
    /// The app was running without a window; it was restarted so the window
    /// reappears (the app has no way to reveal a hidden window on Linux).
    Restarted,
    /// The app is running and no window management is available; left alone.
    LeftAlone,
    /// The app was started moments ago and its window has not appeared yet;
    /// killing it now would discard the fresh instance.
    WaitingForWindow,
}

const TERMINATE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long a freshly launched instance gets to map its window before a
/// windowless `open` falls back to kill-and-relaunch.
const FRESH_INSTANCE_AGE: Duration = Duration::from_secs(5);

fn terminate_all() {
    app::terminate_running(TERMINATE_TIMEOUT);
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

/// The age of the most recently started WireView process, if any.
fn youngest_age() -> Option<Duration> {
    let uptime_secs: f64 = fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let ticks = ticks_per_sec();

    app::running_pids()
        .iter()
        .filter_map(|&pid| {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            age_from_stat(uptime_secs, ticks, &stat)
        })
        .map(Duration::from_secs_f64)
        .min()
}

/// Ensure the app is running and its window is visible.
pub fn open() -> OpenOutcome {
    let pids = app::running_pids();
    if pids.is_empty() {
        app::launch().ok();
        return OpenOutcome::Launched;
    }

    match app::window_address() {
        Some(address) => {
            app::focus_window(&address);
            OpenOutcome::Focused
        }
        None => {
            // No hyprctl/window: if window management is missing entirely we
            // cannot improve on the running app, so leave it alone.
            if !has_hyprctl() {
                return OpenOutcome::LeftAlone;
            }
            // A freshly launched instance needs a moment to map its window;
            // a second click during startup must not kill and relaunch it.
            if youngest_age().is_some_and(|age| age < FRESH_INSTANCE_AGE) {
                return OpenOutcome::WaitingForWindow;
            }
            terminate_all();
            app::launch().ok();
            OpenOutcome::Restarted
        }
    }
}

/// Kill every WireView instance and start a fresh one.
pub fn restart() {
    terminate_all();
    app::launch().ok();
}

/// Kill every WireView instance.
pub fn quit() {
    terminate_all();
}

fn has_hyprctl() -> bool {
    std::process::Command::new("hyprctl")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
