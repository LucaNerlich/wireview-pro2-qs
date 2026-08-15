//! Long-running `watch` mode: one JSON status line per change on stdout.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::{sni, status::Status};

const POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// Poll the app's SNI item forever, printing a JSON line whenever the status
/// changes. Reconnects to the session bus when the connection drops.
pub fn watch() {
    let mut connection = sni::session().ok();
    let mut last: Option<Status> = None;
    let mut consecutive_misses: u32 = 0;

    loop {
        let status = match &connection {
            Some(conn) => match sni::current_status(conn) {
                Some(status) => {
                    consecutive_misses = 0;
                    status
                }
                None => {
                    consecutive_misses += 1;
                    Status::off()
                }
            },
            None => Status::off(),
        };

        if last.as_ref() != Some(&status) {
            let mut stdout = io::stdout().lock();
            // A serialization failure would only mean a bug in Status; the
            // line must still be flushed so the QML parser never stalls.
            if let Ok(json) = serde_json::to_string(&status) {
                let _ = writeln!(stdout, "{json}");
            }
            let _ = stdout.flush();
            last = Some(status);
        }

        thread::sleep(POLL_INTERVAL);

        // Missing reads usually mean the app is gone, but a run of them can
        // also mean the bus connection died; reconnect then.
        if connection.is_none() || consecutive_misses >= 3 {
            if let Some(conn) = connection.take() {
                let _ = conn.close();
            }
            connection = sni::session().ok();
            consecutive_misses = 0;
        }
    }
}
