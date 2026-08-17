# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The bundled backend is now committed with its symbol table intact (not
  stripped) and byte-for-byte reproducible from the tracked Rust source
  (pinned toolchain via `rust-toolchain.toml`, remapped build paths, recorded
  SHA-256 in `omarchy/bin/wireview-pro2-qs.sha256`). CI verifies the bundle
  against a fresh build instead of regenerating it.

## [1.0.0] - 2026-08-15

First stable release. One-click install via `omarchy plugin add
https://github.com/LucaNerlich/wireview-pro2-qs.git --enable` with a bundled
static backend; fixes the findings of the 2026-08-15 code audit.

### Fixed

- DBus method calls have a 2 s timeout, so a hung SNI-registered app can no
  longer freeze the widget or `status` indefinitely.
- Process management matches only the real app binary and only the invoking
  user's processes; editors viewing the binary and lookalike processes are no
  longer terminated, and `quit`/`restart` no longer stall on other users'
  processes.
- Termination signals only the processes it scanned and re-verifies their
  identity before SIGKILL, so recycled pids cannot redirect the kill.
- A rapid second click no longer kills a freshly launched app instance.
- Tab now moves to the neighboring bar panel as documented.
- Backend fallback decisions are per-process, tolerate transient spawn
  failures, and re-probe the bundled binary automatically after it is
  restored.
- The watch backend exits when its consumer disappears instead of leaking a
  polling process.
- Non-finite watt readings (NaN/inf) fall back to the `na` state instead of
  re-emitting a JSON line every second.
- Install paths containing spaces or non-ASCII characters now resolve the
  bundled binary correctly.

### Changed

- The source AUR package verifies the release tarball checksum.
- CI refreshes the bundled binary only when the test jobs pass.

## [0.2.0] - 2026-08-15

### Added

- One-click install: the plugin bundles a statically linked x86_64 musl build
  of the backend in `omarchy/bin/`, refreshed by CI on every push to main.
  `omarchy plugin add ... --enable` alone is enough; the widget falls back to
  a `wireview-pro2-qs` binary on PATH (crates.io or AUR) when the bundle
  cannot start, e.g. on other architectures.
- Release artifacts for the AUR `-bin` package now ship the same static
  musl binary, removing the `gcc-libs` dependency.

### Fixed

- The widget now detects failed backend spawns (Quickshell emits neither
  `started` nor `exited` in that case) and retries, instead of silently
  showing `⚡ off` forever.

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

[Unreleased]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v1.0.0
[0.2.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v0.2.0
[0.1.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v0.1.0
