//! Discovery and reads for the WireView2 app's StatusNotifierItem.
//!
//! The app registers two SNI items (`org.kde.StatusNotifierItem-<pid>-0` and
//! `-1`); the second one carries the live reading in its `Title` property.
//! Discovery goes through the StatusNotifierWatcher so no pid is hardcoded.

use std::time::Duration;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::names::BusName;
use zbus::zvariant::OwnedValue;

pub const SNI_INTERFACE: &str = "org.kde.StatusNotifierItem";
pub const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
pub const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// How long a DBus method call may take before it fails. A hung SNI peer
/// (still owning its bus name but not replying) must not stall the 1 Hz
/// watch poll or `status` forever.
const METHOD_TIMEOUT: Duration = Duration::from_secs(2);

/// A registered StatusNotifierItem: the owning service and its object path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRef {
    pub service: String,
    pub path: String,
}

pub fn session() -> zbus::Result<Connection> {
    Builder::session()?.method_timeout(METHOD_TIMEOUT).build()
}

fn get_property(
    conn: &Connection,
    service: &str,
    path: &str,
    interface: &str,
    name: &str,
) -> Option<OwnedValue> {
    let destination = BusName::try_from(service.to_string()).ok()?;
    let reply = conn
        .call_method(
            Some(destination),
            path,
            Some(PROPERTIES_INTERFACE),
            "Get",
            &(interface, name),
        )
        .ok()?;
    reply.body().deserialize::<OwnedValue>().ok()
}

/// Every StatusNotifierItem path currently registered with the watcher.
pub fn registered_items(conn: &Connection) -> Option<Vec<ItemRef>> {
    let value = get_property(
        conn,
        WATCHER_SERVICE,
        WATCHER_PATH,
        WATCHER_SERVICE,
        "RegisteredStatusNotifierItems",
    )?;
    let paths: Vec<String> = value.try_into().ok()?;

    let mut items = Vec::new();
    for entry in paths {
        let Some((service, path)) = entry.split_once('/') else {
            continue;
        };
        items.push(ItemRef {
            service: service.to_string(),
            path: format!("/{path}"),
        });
    }
    Some(items)
}

/// The item's `Title` property, if readable.
pub fn title(conn: &Connection, item: &ItemRef) -> Option<String> {
    let value = get_property(conn, &item.service, &item.path, SNI_INTERFACE, "Title")?;
    value.try_into().ok()
}

/// Outcome of one read of the app's SNI item.
#[derive(Debug)]
pub enum Reading {
    /// A WireView item answered with a title.
    Status(Box<crate::status::Status>),
    /// The watcher answered and lists no WireView item: the app is gone.
    Absent,
    /// The bus or a property call failed; the app's presence is unknown.
    Unavailable,
}

/// Reads the app's SNI item, caching the live item between polls.
///
/// Steady state therefore costs a single property `Get` on the cached item
/// instead of querying every tray application each tick; full discovery
/// runs only when the cache misses or the cached item stops answering.
#[derive(Debug)]
pub struct SniWatcher {
    conn: Connection,
    cached: Option<ItemRef>,
}

impl SniWatcher {
    pub fn new(conn: Connection) -> Self {
        Self { conn, cached: None }
    }

    /// Hands back the underlying connection (used when dropping a broken
    /// one before reconnecting).
    pub fn into_connection(self) -> Connection {
        self.conn
    }

    /// One status read. A cached item whose `Title` read fails or no longer
    /// parses falls through to full discovery, which distinguishes "the app
    /// is gone" ([`Reading::Absent`]) from "the bus is broken"
    /// ([`Reading::Unavailable`]).
    pub fn read_status(&mut self) -> Reading {
        if let Some(item) = self.cached.clone()
            && let Some(text) = title(&self.conn, &item)
            && let Some(status) = crate::status::Status::from_title(Some(&text))
        {
            return Reading::Status(Box::new(status));
        }
        self.discover()
    }

    /// Scan every registered item. Prefers the dynamic item whose title ends
    /// in watts; falls back to the static identity item.
    fn discover(&mut self) -> Reading {
        let Some(items) = registered_items(&self.conn) else {
            return Reading::Unavailable;
        };
        let mut live: Option<(ItemRef, String)> = None;
        let mut fallback: Option<(ItemRef, String)> = None;
        for item in items {
            let Some(text) = title(&self.conn, &item) else {
                continue;
            };
            if text.starts_with("WireView Pro II - ") && text.ends_with(" W") {
                live = Some((item, text));
                break;
            }
            if text.starts_with("WireView Pro II") && fallback.is_none() {
                fallback = Some((item, text));
            }
        }
        match live.or(fallback) {
            None => {
                self.cached = None;
                Reading::Absent
            }
            Some((item, text)) => {
                self.cached = Some(item);
                match crate::status::Status::from_title(Some(&text)) {
                    Some(status) => Reading::Status(Box::new(status)),
                    None => Reading::Absent,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_registered_path() {
        let (service, path) = ":1.4479/StatusNotifierItem".split_once('/').unwrap();
        let item = ItemRef {
            service: service.to_string(),
            path: format!("/{path}"),
        };
        assert_eq!(
            item,
            ItemRef {
                service: ":1.4479".to_string(),
                path: "/StatusNotifierItem".to_string(),
            }
        );
    }
}
