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

/// Ensure the app is running and its window is visible.
///
/// Returns `Err` when a fresh instance could not be spawned (e.g. the app
/// binary is missing), so callers can surface the failure.
pub fn open() -> std::io::Result<OpenOutcome> {
    if !app::is_running() {
        app::launch()?;
        return Ok(OpenOutcome::Launched);
    }

    match app::window_address() {
        Some(address) => {
            app::focus_window(&address);
            Ok(OpenOutcome::Focused)
        }
        None => {
            // No hyprctl/window: if window management is missing entirely we
            // cannot improve on the running app, so leave it alone.
            if !has_hyprctl() {
                return Ok(OpenOutcome::LeftAlone);
            }
            // A freshly launched instance needs a moment to map its window;
            // a second click during startup must not kill and relaunch it.
            if app::youngest_age().is_some_and(|age| age < FRESH_INSTANCE_AGE) {
                return Ok(OpenOutcome::WaitingForWindow);
            }
            terminate_all();
            app::launch()?;
            Ok(OpenOutcome::Restarted)
        }
    }
}

/// Kill every WireView instance and start a fresh one.
pub fn restart() -> std::io::Result<()> {
    terminate_all();
    app::launch()
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
