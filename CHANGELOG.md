# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-15

### Added

- Rust backend (`wireview-pro2-qs`) reading the WireView2 app's live power
  reading straight from its StatusNotifierItem over DBus (zbus), bypassing
  the SNI spec violation that makes strict tray hosts reject the app's icon.
- `status`, `watch`, `open`, `restart`, and `quit` subcommands, including
  Hyprland lua-dispatcher window focus and launch/restart handling.
- Omarchy Quattro `bar-widget` plugin: live `⚡ N W` bar label, tooltip, and
  a details panel with Open / Restart / Quit actions.
- Widget setting `hideWhenOff`.
- AUR packaging (`wireview-pro2-qs`, `wireview-pro2-qs-bin`), Makefile, CI
  (fmt, clippy, tests, plugin model tests, MSRV), and unit tests for status
  parsing and app process helpers.

[0.1.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v0.1.0
