//! Resolve the current status, preferring the hwmon chip when present.

use zbus::blocking::Connection;

use crate::{app, hwmon, sni, status::Status};

/// Resolves the current device status and records whether the application is running.
///
/// Device status is obtained from the WireView hwmon chip when available, with the
/// StatusNotifierItem used as a fallback.
///
/// # Examples
///
/// ```
/// let status = current_status(None);
/// ```
pub fn current_status(conn: Option<&Connection>) -> Status {
    device_status(conn).with_app_running(app::is_running())
}

/// Determines the current device status from discovered hardware sensors or the status notifier service.
///
/// # Examples
///
/// ```
/// let status = device_status(None);
/// ```
fn device_status(conn: Option<&Connection>) -> Status {
    if let Some(sensors) = hwmon::discover() {
        return Status::from_sensors(&sensors);
    }
    conn.and_then(sni::current_status)
        .unwrap_or_else(Status::off)
}
