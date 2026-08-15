//! User-facing app actions: open (launch/focus/restart), restart, quit.

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
}

const TERMINATE_TIMEOUT: Duration = Duration::from_secs(3);

fn terminate_all() {
    let pids = app::running_pids();
    if !pids.is_empty() {
        app::terminate(&pids, TERMINATE_TIMEOUT);
    }
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
