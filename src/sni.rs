//! Discovery and reads for the WireView2 app's StatusNotifierItem.
//!
//! The app registers two SNI items (`org.kde.StatusNotifierItem-<pid>-0` and
//! `-1`); the second one carries the live reading in its `Title` property.
//! Discovery goes through the StatusNotifierWatcher so no pid is hardcoded.

use zbus::blocking::Connection;
use zbus::names::BusName;
use zbus::zvariant::OwnedValue;

pub const SNI_INTERFACE: &str = "org.kde.StatusNotifierItem";
pub const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
pub const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// A registered StatusNotifierItem: the owning service and its object path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRef {
    pub service: String,
    pub path: String,
}

pub fn session() -> zbus::Result<Connection> {
    Connection::session()
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

/// The WireView item carrying the live power reading, preferring the dynamic
/// one whose title ends in watts; falls back to the static identity item.
pub fn find_live_item(conn: &Connection) -> Option<ItemRef> {
    let mut fallback: Option<ItemRef> = None;

    for item in registered_items(conn)? {
        let text = title(conn, &item).unwrap_or_default();
        if text.starts_with("WireView Pro II - ") && text.ends_with(" W") {
            return Some(item);
        }
        if text.starts_with("WireView Pro II") && fallback.is_none() {
            fallback = Some(item);
        }
    }

    fallback
}

/// The current status read straight from the app's SNI item.
pub fn current_status(conn: &Connection) -> Option<crate::status::Status> {
    let item = find_live_item(conn)?;
    let text = title(conn, &item);
    crate::status::Status::from_title(text.as_deref())
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
