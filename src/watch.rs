//! Long-running `watch` mode: one JSON status line per change on stdout.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::{
    current, sni,
    status::{State, Status},
};

const POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// Poll the app's SNI item forever, printing a JSON line whenever the status
/// changes. Reconnects to the session bus when the connection drops.
pub fn watch() {
    let mut connection = sni::session().ok();
    let mut last: Option<Status> = None;
    let mut consecutive_misses: u32 = 0;

    loop {
        let status = current::current_status(connection.as_ref());
        if status.state == State::Off {
            consecutive_misses += 1;
        } else {
            consecutive_misses = 0;
        }

        if last.as_ref() != Some(&status) {
            let mut stdout = io::stdout().lock();
            let mut broken = false;
            if let Ok(json) = serde_json::to_string(&status) {
                broken = writeln!(stdout, "{json}").is_err();
            }
            // A serialization failure would only mean a bug in Status; the
            // line must still be flushed so the QML parser never stalls.
            broken |= stdout.flush().is_err();
            if broken {
                // The consumer is gone (shell crashed or exited without
                // reaping us); polling the bus forever would leak the
                // process and its session-bus connection.
                return;
            }
            last = Some(status);
        }

        thread::sleep(POLL_INTERVAL);

        // A run of misses usually means the app is gone, but it can also
        // mean the session bus died; reconnect then. hwmon-only mode does not
        // need the bus, but reconnecting is cheap and harmless.
        if connection.is_none() || consecutive_misses >= 3 {
            if let Some(conn) = connection.take() {
                let _ = conn.close();
            }
            connection = sni::session().ok();
            consecutive_misses = 0;
        }
    }
}
