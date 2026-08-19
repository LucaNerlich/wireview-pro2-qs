import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

// Quattro bar entry point for the WireView Pro II widget. All data collection
// lives in the Rust backend (`wireview-pro2-qs watch`); this file owns the bar
// button, the panel routing, and the watch process lifecycle.
BarWidget {
  id: root
  moduleName: "luca.wireview-pro2"

  // The plugin bundles a statically linked x86_64 backend next to this file.
  // Each process keeps its own binary choice: the bundled one first, then a
  // `wireview-pro2-qs` binary on PATH. A single transient spawn failure must
  // not abandon the bundle, so the choice only flips after repeated
  // consecutive failures — and it flips back, so a restored bundle (e.g.
  // after `omarchy plugin update`) is picked up without a shell restart.
  function decodeFileUrl(urlString) {
    // resolvedUrl keeps percent-encoding intact, which QProcess would then
    // treat as part of the file name; decode it so install paths containing
    // spaces or non-ASCII characters work.
    var path = String(urlString).replace(/^file:\/\//, "")
    try {
      return decodeURIComponent(path)
    } catch (e) {
      return path
    }
  }
  readonly property string bundledBinary: root.decodeFileUrl(
    Qt.resolvedUrl("bin/wireview-pro2-qs").toString())
  readonly property int fallbackThreshold: 2
  property bool watchFallback: false
  property bool actionFallback: false
  property int watchFailures: 0
  property int actionFailures: 0
  readonly property string watchBinary: watchFallback ? "wireview-pro2-qs" : bundledBinary
  readonly property string actionBinary: actionFallback ? "wireview-pro2-qs" : bundledBinary
  property string pendingAction: ""

  readonly property var panelItem: panelLoader.item
  readonly property bool opened: panelItem ? panelItem.opened === true : false

  property string statusState: "off"
  property real watts: NaN
  property string title: ""
  property var sensors: null

  readonly property bool hideWhenOff: setting("hideWhenOff", false) === true
  readonly property var status: ({
    state: root.statusState,
    watts: root.watts,
    title: root.title,
    sensors: root.sensors
  })
  readonly property string labelText: Model.labelText(status)
  readonly property string tooltipText: Model.tooltipText(status)

  function open() { if (panelItem) panelItem.open() }
  function close() { if (panelItem) panelItem.close() }
  function toggle() { if (panelItem) panelItem.toggle() }

  function applyLine(line) {
    var parsed = Model.parseLine(String(line || ""))
    if (!parsed) return
    root.statusState = parsed.state
    root.watts = parsed.state === "live" ? parsed.watts : NaN
    root.title = parsed.title
    root.sensors = parsed.sensors || null
  }

  function runAction(action) {
    if (actionProc.running) return
    root.pendingAction = action
    actionProc.retried = false
    actionProc.command = [root.actionBinary, action]
    actionProc.running = true
  }

  function injectPanel() {
    var target = panelItem
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  visible: !hideWhenOff || statusState !== "off"
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()

  Component.onCompleted: watchProc.running = true

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  Process {
    id: watchProc
    command: [root.watchBinary, "watch"]
    // True between a successful start and the process exiting. A `running`
    // false edge without it means the spawn failed: Quickshell then emits
    // neither started nor exited, so onExited would never retry.
    property bool startedOnce: false
    stdout: SplitParser {
      onRead: function(line) { root.applyLine(line) }
    }
    onStarted: {
      watchProc.startedOnce = true
      root.watchFailures = 0
    }
    onExited: {
      root.statusState = "off"
      watchRestartTimer.restart()
    }
    onRunningChanged: {
      if (watchProc.running) return
      var failedStart = !watchProc.startedOnce
      watchProc.startedOnce = false
      if (failedStart) {
        root.statusState = "off"
        root.watchFailures += 1
        if (root.watchFailures >= root.fallbackThreshold) {
          root.watchFailures = 0
          root.watchFallback = !root.watchFallback
        }
      }
      watchRestartTimer.restart()
    }
  }

  Timer {
    id: watchRestartTimer
    interval: 5000
    repeat: false
    onTriggered: watchProc.running = true
  }

  Process {
    id: actionProc
    // Same spawn-failure detection as watchProc. An action is retried once
    // per click on the current binary; only repeated failures across clicks
    // switch the binary, independently of the watch process.
    property bool startedOnce: false
    property bool retried: false
    onStarted: {
      actionProc.startedOnce = true
      root.actionFailures = 0
    }
    onRunningChanged: {
      if (actionProc.running) return
      var failedStart = !actionProc.startedOnce
      actionProc.startedOnce = false
      if (!failedStart || root.pendingAction === "") return
      if (actionProc.retried) {
        actionProc.retried = false
        root.pendingAction = ""
        root.actionFailures += 1
        if (root.actionFailures >= root.fallbackThreshold) {
          root.actionFailures = 0
          root.actionFallback = !root.actionFallback
        }
        return
      }
      actionProc.retried = true
      actionProc.command = [root.actionBinary, root.pendingAction]
      actionProc.running = true
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.labelText
    foreground: Color.bar.text
    activeColor: Color.bar.active
    active: root.statusState === "na"
    horizontalMargin: 8.5
    verticalPadding: 6
    tooltipText: root.tooltipText
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.LeftButton) root.runAction("open")
      else if (buttonCode === Qt.RightButton) root.toggle()
    }
  }
}
