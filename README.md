# wireview-pro2-qs

[![GitHub Release](https://img.shields.io/github/v/release/LucaNerlich/wireview-pro2-qs)](https://github.com/LucaNerlich/wireview-pro2-qs/releases)

Omarchy Quattro bar widget showing the live GPU power draw from the
[Thermal Grizzly WireView Pro II](https://www.thermal-grizzly.com/en/wireview-pro-ii-gpu/s-tg-wv-p2)
(e.g. `⚡ 57 W`), with a details panel for app actions.

<img width="464" height="180" alt="image" src="https://github.com/user-attachments/assets/9a9f600a-c641-4f4d-8816-bd1b347c47fc" />

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

- **The WireView2 app is required.** This widget reads the app's SNI item
  over DBus; it does not talk to the device itself. Install it first
  (`wireview-linux-bin` on AUR). Without it the widget shows `⚡ off` and the
  open action has nothing to launch.
- https://github.com/emaspa/wireview-linux  
- `hyprctl` (ships with Hyprland) for window focus.

## Architecture

- **Rust backend** (`wireview-pro2-qs` binary): talks to the session bus with
  zbus, discovers the app's SNI item through the StatusNotifierWatcher, reads
  the live `Title`, and manages the app process (launch / focus / restart /
  quit, including Hyprland's lua dispatcher). No shelling out to busctl or
  dbus-send; the only external command is `hyprctl` for window management.
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

The plugin bundles a statically linked x86_64 build of its backend
(`omarchy/bin/wireview-pro2-qs`, built on musl, refreshed by CI on every push
to main). If the bundled binary cannot start — non-x86_64 machine, missing
exec bit, whatever — the widget falls back to a `wireview-pro2-qs` binary on
PATH, so these still work:

```bash
# Binary (pick one)
cargo install wireview-pro2-qs            # from crates.io
yay -S wireview-pro2-qs                   # AUR source build
yay -S wireview-pro2-qs-bin               # AUR prebuilt binary
```

Omarchy clones the plugin into `~/.config/omarchy/plugins/` and adds the
widget to the bar (right section by default). Update or remove it with the
normal plugin commands:

```bash
omarchy plugin update luca.wireview-pro2
omarchy plugin remove luca.wireview-pro2
```

## Usage

- **Bar**: left-click opens the app window — launches it when not running,
  focuses it when a window exists, and restarts it when the process runs
  windowless (the app has no SNI `Activate` method and an empty dbusmenu in
  v1.2.0.0, so a hidden window cannot be revealed any other way).
  Right-click opens the details panel.
- **Panel**: current power draw and app state, plus Open / Restart / Quit
  actions. Enter opens the app window, Tab moves to the neighboring bar
  panel, Esc closes.
- **Shell**: `omarchy-shell shell summon luca.wireview-pro2 '{}'` opens the
  panel, `omarchy-shell shell hide luca.wireview-pro2` closes it.

## Settings

Widget settings live in `~/.config/omarchy/shell.json`:

```bash
omarchy bar set luca.wireview-pro2 hideWhenOff true
```

| Key | Default | Description |
| --- | --- | --- |
| `hideWhenOff` | false | Hide the widget entirely when the app is not running. |

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
{"state":"live","watts":43.2,"title":"WireView Pro II - 43.2 W"}
{"state":"na","title":"WireView Pro II"}
{"state":"off"}
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

`make bundle` rebuilds the statically linked backend into `omarchy/bin/`
(requires the `x86_64-unknown-linux-musl` target: `rustup target add
x86_64-unknown-linux-musl`). CI refreshes the committed binary on every push
to main.

Saving files under an installed user plugin triggers Quattro's plugin hot
reload. Rerun `omarchy plugin validate .` after changing the manifest or
entry points. Note: on quickshell-git 0.3.0 `Qt.clearComponentCache` is
unavailable, so plugin QML changes only take effect after a full
`omarchy restart shell`.

## License

Apache-2.0. This project is not affiliated with Thermal Grizzly or ElmorLabs;
"WireView" is their trademark. The app it monitors is the unofficial
[wireview-linux](https://github.com/emaspa/wireview-linux) port.
