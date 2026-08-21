//! Resolve the current status, preferring the hwmon chip when present.

use zbus::blocking::Connection;

use crate::{app, hwmon, sni, status::Status};

/// Outcome of one resolution pass.
#[derive(Debug)]
pub enum Poll {
    /// A source reported a reading (or the app's no-reading state).
    Status(Box<Status>),
    /// No hwmon chip and no WireView SNI item: the app is gone.
    Absent,
    /// The session bus failed; the device state is unknown this tick.
    Unavailable,
}

/// One-shot resolution for `status`: an unknown bus state collapses to
/// `off`, and the app-running flag is overlaid on every outcome.
///
/// # Examples
///
/// ```
/// let status = current_status(None);
/// ```
pub fn current_status(conn: Option<&Connection>) -> Status {
    let mut watcher = conn.map(|c| sni::SniWatcher::new(c.clone()));
    match resolve(watcher.as_mut()) {
        Poll::Status(status) => *status,
        Poll::Absent | Poll::Unavailable => Status::off(),
    }
    .with_app_running(app::is_running())
}

/// Streaming resolution for `watch`: distinguishes "the app is gone"
/// ([`Poll::Absent`]) from "the bus is broken" ([`Poll::Unavailable`]) so a
/// transient DBus error does not have to emit a false off line.
pub fn poll(watcher: Option<&mut sni::SniWatcher>) -> Poll {
    resolve(watcher)
}

fn resolve(watcher: Option<&mut sni::SniWatcher>) -> Poll {
    if let Some(sensors) = hwmon::discover() {
        return Poll::Status(Box::new(Status::from_sensors(&sensors)));
    }
    let Some(watcher) = watcher else {
        return Poll::Unavailable;
    };
    match watcher.read_status() {
        sni::Reading::Status(status) => Poll::Status(status),
        sni::Reading::Absent => Poll::Absent,
        sni::Reading::Unavailable => Poll::Unavailable,
    }
}
