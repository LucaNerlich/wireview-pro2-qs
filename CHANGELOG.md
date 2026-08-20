# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.6] - 2026-08-20

### Fixed

- `restart`/`quit` deliver SIGTERM and SIGKILL with `pidfd_send_signal` on
  the scanned `/proc/<pid>` directory fd. Identity is read through that
  same fd, so a pid recycled between a `/proc` check and `kill(2)` cannot
  redirect the signal at an unrelated same-user process.
- `open` reads process age through that same directory fd, so a recycled
  pid cannot look like a fresh WireView instance and skip a needed restart.
- Window focus requires the Hyprland client's pid to still identify as the
  app binary, so a lookalike class is not focused.
- hwmon discovery re-reads the chip `name` through the opened sysfs
  directory so a reused `hwmonN` node cannot feed another chip's sensors.
- The QML panel no longer binds the watch-stream `title` into `PanelHero`
  (Qt Text may treat HTML as rich text). `parseLine` also drops titles that
  contain `<`, `>`, or `&`.

## [1.1.5] - 2026-08-19

### Fixed

- Rebuild the bundled musl backend so it matches the tracked source. A
  comment-only edit of `src/app.rs` left the v1.1.4 ELF in place; rustc
  hashes comments into symbol names, so marketplace review's exact-SHA
  rebuild diverged.

### Changed

- `make verify-bundle` now fingerprints `src/`, `Cargo.toml`, `Cargo.lock`,
  and `rust-toolchain.toml` (`omarchy/bin/wireview-pro2-qs.srcid`) so a
  stale bundle fails before the musl rebuild. Comments count.

### Removed

- AUR PKGBUILD templates and install instructions. This project is not
  published to the AUR; install via `omarchy plugin add`.

## [1.1.4] - 2026-08-19

### Fixed

- `restart` and `quit` re-verify process identity and starttime before the
  initial SIGTERM (not only before SIGKILL), so a recycled pid cannot
  terminate an unrelated same-user process.

## [1.1.3] - 2026-08-19

### Changed

- `make verify-bundle` now also requires the committed ELF to be non-stripped,
  to match its recorded SHA-256, and to report the same version as
  `Cargo.toml` / `manifest.json`. Pushing a `v*.*.*` tag re-runs those checks
  and only then publishes the GitHub Release, so a later release cannot skip
  the marketplace attestation.

## [1.1.2] - 2026-08-19

### Fixed

- CI installs rustfmt and clippy on the pinned 1.97.1 toolchain so the format
  check can actually run, and the bundle reproducibility job no longer depends
  on that check. A style-tooling failure can no longer skip the byte-for-byte
  verify that marketplace review requires.

## [1.1.1] - 2026-08-17

### Changed

- Document the optional `wireview-hwmon` dependency (full sensor panel) and
  its install steps in the README.

## [1.1.0] - 2026-08-17

### Added

- Read the full per-pin sensor data from the `wireview` hwmon chip when the
  WireView daemon exposes one: voltage and current for all six 12VHPWR pins,
  the four temperature channels, fault status/log, and the PSU rating. The
  right-click panel shows these when available, and falls back to the app's
  SNI title (watts only) otherwise.

## [1.0.1] - 2026-08-17

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

[1.1.6]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/LucaNerlich/wireview-pro2-qs/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v1.0.0
[0.2.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v0.2.0
[0.1.0]: https://github.com/LucaNerlich/wireview-pro2-qs/releases/tag/v0.1.0
