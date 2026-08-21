//! Long-running `watch` mode: one JSON status line per change on stdout.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::{app, current, sni, status::Status};

const POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// Poll forever, printing a JSON line whenever the status changes.
///
/// Transient session-bus failures hold the last known status instead of
/// emitting a false off line and trigger a reconnect; a run of absent-app
/// polls is normal steady state and must not churn connections.
pub fn watch() {
    let mut watcher = sni::session().ok().map(sni::SniWatcher::new);
    let mut last: Option<Status> = None;

    loop {
        let poll = current::poll(watcher.as_mut());
        let bus_broken = matches!(poll, current::Poll::Unavailable);
        let mut broken = false;
        match poll {
            current::Poll::Status(status) => broken |= emit(&mut last, &status),
            // The app is gone: report it once. Bus trouble: hold the last
            // known status so a hiccup does not flash off (or hide the
            // widget when hideWhenOff is set) until the next good read.
            current::Poll::Absent => {
                broken |= emit(
                    &mut last,
                    &Status::off().with_app_running(app::is_running()),
                )
            }
            current::Poll::Unavailable => {}
        }
        if broken {
            // The consumer is gone (shell crashed or exited without
            // reaping us); polling the bus forever would leak the
            // process and its session-bus connection.
            return;
        }

        thread::sleep(POLL_INTERVAL);

        // Reconnect only when the bus itself looks broken: no connection,
        // or the last read failed on it.
        if watcher.is_none() || bus_broken {
            if let Some(old) = watcher.take() {
                let _ = old.into_connection().close();
            }
            watcher = sni::session().ok().map(sni::SniWatcher::new);
        }
    }
}

/// Print `status` unless it equals the previously emitted one. Returns true
/// when stdout broke (consumer gone).
fn emit(last: &mut Option<Status>, status: &Status) -> bool {
    if last.as_ref() == Some(status) {
        return false;
    }
    let mut stdout = io::stdout().lock();
    let mut broken = false;
    if let Ok(json) = serde_json::to_string(status) {
        broken = writeln!(stdout, "{json}").is_err();
    }
    // A serialization failure would only mean a bug in Status; the
    // line must still be flushed so the QML parser never stalls.
    broken |= stdout.flush().is_err();
    if !broken {
        *last = Some(status.clone());
    }
    broken
}
