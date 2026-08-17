//! Resolve the current status, preferring the hwmon chip when present.

use zbus::blocking::Connection;

use crate::{hwmon, sni, status::Status};

/// Prefer the WireView hwmon chip (full per-pin data) when available;
/// otherwise fall back to the app's StatusNotifierItem title (watts only).
pub fn current_status(conn: Option<&Connection>) -> Status {
    if let Some(sensors) = hwmon::discover() {
        return Status::from_sensors(&sensors);
    }
    conn.and_then(sni::current_status)
        .unwrap_or_else(Status::off)
}
