//! Resolve the current status, preferring the hwmon chip when present.

use zbus::blocking::Connection;

use crate::{app, hwmon, sni, status::Status};

/// Prefer the WireView hwmon chip (full per-pin data) when available;
/// otherwise fall back to the app's StatusNotifierItem title (watts only).
///
/// [`Status::app_running`] is filled independently so a live chip does not
/// imply that the WireView2 GUI is running.
pub fn current_status(conn: Option<&Connection>) -> Status {
    device_status(conn).with_app_running(app::is_running())
}

fn device_status(conn: Option<&Connection>) -> Status {
    if let Some(sensors) = hwmon::discover() {
        return Status::from_sensors(&sensors);
    }
    conn.and_then(sni::current_status)
        .unwrap_or_else(Status::off)
}
