//! Backend for the Omarchy Quattro WireView Pro II bar widget.
//!
//! The WireView2 app (Avalonia) publishes its power reading in the `Title`
//! property of a StatusNotifierItem on the session bus. Strict SNI hosts such
//! as Quickshell reject the item because the app also writes the reading into
//! the SNI `Status` property, which the spec reserves for `Active` / `Passive`
//! / `NeedsAttention`. This crate reads the item over DBus directly and drives
//! the app's process lifecycle for the QML frontend.

pub mod app;
pub mod open;
pub mod sni;
pub mod status;
pub mod watch;
