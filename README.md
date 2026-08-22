# wireview-pro2-qs

[![GitHub Release](https://img.shields.io/github/v/release/LucaNerlich/wireview-pro2-qs)](https://github.com/LucaNerlich/wireview-pro2-qs/releases)
[Omarchy marketplace](https://omarchyplugins.com/plugin.html?id=luca.wireview-pro2)

Omarchy Quattro bar widget showing the live GPU power draw from the
[Thermal Grizzly WireView Pro II](https://www.thermal-grizzly.com/en/wireview-pro-ii-gpu/s-tg-wv-p2)
(e.g. `⚡ 57 W`), with a details panel for app actions.

<img width="464" height="180" alt="image" src="https://github.com/user-attachments/assets/9a9f600a-c641-4f4d-8816-bd1b347c47fc" />

<img width="748" height="880" alt="image" src="https://github.com/user-attachments/assets/bf3cbef6-8f08-4ef5-b5cd-6ea2a6b7a4a0" />

## Why this exists

The WireView2 app (Avalonia) publishes its power reading in the `Title`
property of a StatusNotifierItem, but also writes the same text into the SNI
`Status` property — which the spec reserves for `Active` / `Passive` /
`NeedsAttention`. Strict tray hosts reject the entire item:

```
quickshell.dbus.properties: Nonconformant StatusNotifierItem Status: WireView Pro II - 43 W
quickshell.service.sni.item: Received invalid status update
```

So the app never shows up in the Omarchy tray, and combined with the app's
"start minimized" setting the process runs with no visible window at all.
This widget reads the item over DBus directly and takes the tray icon's place.

## Requirements

- **The WireView2 app** (`wireview-linux`) — provides the
  Open / Restart / Quit actions and the wattage-only fallback. Install it to
  control the app; without it the widget shows `⚡ off` and the open action
  has nothing to launch. https://github.com/emaspa/wireview-linux
- **wireview-hwmon** (optional, recommended) — enables the full per-pin,
  temperature, and fault panel. See “Full sensor data” below.
- `hyprctl` (ships with Hyprland) for window focus.

## Architecture

- **Rust backend** (`wireview-pro2-qs` binary): prefers the `wireview` hwmon
  chip (full per-pin data) when present and otherwise reads the app's SNI
  `Title` (watts) over the session bus with zbus. Also manages the app process
  (launch / focus / restart / quit, including Hyprland's lua dispatcher). The
  JSON status reports `appRunning` independently of the device reading, so the
  widget can show live watts from `wireviewd` without claiming the GUI is up.
  No shelling out to busctl or dbus-send; the only external command is `hyprctl`
  for window management.
- **QML frontend** (`omarchy/`): a `bar-widget` plugin. `BarWidget.qml` runs
  `wireview-pro2-qs watch` once and updates from its JSON lines; `Panel.qml`
  shows status and app actions. All data collection stays in Rust; the QML is
  pure presentation.

```
wireview-pro2-qs watch ──(JSON lines, 1 Hz)──▶ SplitParser ─▶ BarWidget ─▶ Panel
```

## Install

One command:

```bash
omarchy plugin add https://github.com/LucaNerlich/wireview-pro2-qs.git --enable
```

Update to the latest version the same way you would any marketplace plugin:

```bash
omarchy plugin update luca.wireview-pro2
```

The plugin bundles a statically linked x86_64 build of its backend
(`omarchy/bin/wireview-pro2-qs`, built on musl). The binary is committed
without stripping, so its symbol table maps to the Rust source and can be
inspected with `nm`. It is also a byte-for-byte reproducible build of the
source in this repository: the toolchain is pinned by `rust-toolchain.toml`,
machine-specific paths are remapped, and the expected SHA-256 is recorded in
`omarchy/bin/wireview-pro2-qs.sha256`. Anyone can confirm it against this
exact checkout with `make verify-bundle`; CI fails the build if the committed
binary ever drifts from the tracked source. If the bundled binary cannot
start — non-x86_64 machine, missing exec bit, whatever — the widget falls
back to a `wireview-pro2-qs` binary on PATH (`cargo install wireview-pro2-qs`).

Omarchy clones the plugin into `~/.config/omarchy/plugins/` and adds the
widget to the bar (right section by default). Update or remove it with the
normal plugin commands:

```bash
omarchy plugin update luca.wireview-pro2
omarchy plugin remove luca.wireview-pro2
```

## Full sensor data (optional)

The bar always shows watts. To also show per-pin voltage, current, and power,
fan duty, the four temperature channels, named fault status/log, current
imbalance, and the PSU rating in the right-click panel, install the companion
hwmon driver and daemon:

```bash
yay -S wireview-hwmon wireview-hwmon-dkms   # or: paru -S
sudo modprobe wireview_hwmon
sudo systemctl enable --now wireviewd
```

`wireviewd` reads the device over serial and exposes it as a `wireview` hwmon
chip under `/sys/class/hwmon/`. The serial port is single-owner, so quit the
WireView2 app before starting `wireviewd`, then relaunch it — the app switches
to reading through hwmon, and the widget reads the same chip alongside it. The
widget also works **without the GUI**: if only `wireviewd` is running the bar
still shows live watts, the panel App row reads "daemon only", and Open launches
the app. The widget keeps the SNI-title fallback (watts only) when no chip is
present.

## Usage

- **Bar**: left-click opens the app window — launches it when not running,
  focuses it when a window exists, and restarts it when the process runs
  windowless (the app has no SNI `Activate` method and an empty dbusmenu in
  v1.2.0.0, so a hidden window cannot be revealed any other way).
  Right-click opens the details panel.
- **Panel**: current power draw and app state (running, daemon only, or not
  running), plus Open / Restart / Quit. Restart and Quit hide when the GUI is
  not running. When the hwmon chip is present the panel also shows per-pin
  voltage, current, and watts, an imbalance caption (highlighting the hottest
  pin when a pin is ≥6 A and the spread exceeds 40%), fan duty, temperatures,
  named fault status/log, and the PSU rating. A live fault paints the bar in
  the urgent color and sends a critical Omarchy notification (click it to
  open this panel); an imbalance-only alert notifies after it has persisted
  across five consecutive readings (~5 s) so momentary load transients stay
  silent. Enter opens the app window, Tab moves to the neighboring
  bar panel, Esc closes.
- **Shell**: `omarchy-shell shell summon luca.wireview-pro2 '{}'` opens the
  panel, `omarchy-shell shell hide luca.wireview-pro2` closes it.

## Settings

Widget settings live in `~/.config/omarchy/shell.json`:

```bash
omarchy bar set luca.wireview-pro2 hideWhenOff true
```

| Key | Default | Description |
| --- | --- | --- |
| `hideWhenOff` | false | Hide the widget entirely when the WireView2 GUI is not running, even if `wireviewd` is still feeding the hwmon chip. |

## CLI

```bash
wireview-pro2-qs status    # one status report as a single JSON line
wireview-pro2-qs watch     # stream status lines, one per change (1 Hz)
wireview-pro2-qs open      # ensure the app runs and its window is visible
wireview-pro2-qs restart   # kill every instance and start a fresh one
wireview-pro2-qs quit      # kill every instance
```

Status lines:

```json
{"state":"live","watts":43.2,"appRunning":true,"title":"WireView Pro II - 43.2 W"}
{"state":"live","watts":108.05,"appRunning":false,"sensors":{"voltageV":[12.0,12.1],"currentA":[1.5,1.6],"powerW":[18.0,19.36],"sumCurrentA":3.1,"sumPowerW":108.05,"tempInC":34.5,"tempOutC":null,"ext1C":null,"ext2C":null,"fanDuty":75,"faultStatus":0,"faultLog":0,"psuCapW":600}}
{"state":"na","appRunning":true,"title":"WireView Pro II"}
{"state":"off","appRunning":false}
```

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
node omarchy/model.test.mjs
omarchy plugin validate .
qmllint -I "$OMARCHY_PATH/shell" omarchy/BarWidget.qml omarchy/Panel.qml
```

`make bundle` rebuilds the statically linked backend into `omarchy/bin/` and
`make verify-bundle` is the marketplace gate: the committed ELF must be
non-stripped (`nm` can map symbols to the Rust source), its SHA-256 file
must match the bytes in git, `--version` must match `Cargo.toml` /
`manifest.json`, the `.srcid` fingerprint must match `src/` + `Cargo.toml` +
`Cargo.lock` + `rust-toolchain.toml`, and a fresh musl rebuild must be
byte-identical (needs `rustup target add x86_64-unknown-linux-musl`). The
toolchain is pinned in `rust-toolchain.toml` (including rustfmt and clippy).
CI runs the format check and this verify as independent jobs so a missing
rustfmt component cannot skip the attestation. Pushing a `vX.Y.Z` tag
re-runs the same script and publishes the GitHub Release tarball only if it
passes.

Any edit under `src/`, `Cargo.toml`, `Cargo.lock`, or `rust-toolchain.toml`
changes the ELF — including comments. rustc hashes this crate's source into
symbol names (the crate disambiguator), so a leftover binary from an earlier
commit fails marketplace review's exact-SHA rebuild. Run `make bundle` in
the same change as the Rust edit; do not merge while the marketplace bundle
job is red.

### Releasing

1. Bump `Cargo.toml`, `Cargo.lock`, `manifest.json`, and `CHANGELOG.md`.
2. Run `make bundle` then `make verify-bundle`.
3. Open a PR and wait until the **marketplace bundle** CI job is green on
   that SHA. Do not tag a commit where that job was skipped or failed.
4. Merge, then tag `vX.Y.Z` on the merged commit (the tag must match the
   crate version). The Release workflow repeats the bundle checks and
   publishes `wireview-pro2-qs-X.Y.Z-linux-x86_64.tar.gz` from the
   committed ELF.

Saving files under an installed user plugin triggers Quattro's plugin hot
reload. Rerun `omarchy plugin validate .` after changing the manifest or
entry points. Note: on quickshell-git 0.3.0 `Qt.clearComponentCache` is
unavailable, so plugin QML changes only take effect after a full
`omarchy restart shell`.

## License

Apache-2.0. This project is not affiliated with Thermal Grizzly or ElmorLabs;
"WireView" is their trademark. The app it monitors is the unofficial
[wireview-linux](https://github.com/emaspa/wireview-linux) port.
