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
  // If it cannot start (other architecture, missing exec bit, ...), fall back
  // to a `wireview-pro2-qs` binary on PATH (crates.io or AUR install).
  // Quickshell emits neither `started` nor `exited` when a spawn fails, only
  // a `running` false edge, so a failed start shows up as runningChanged
  // without a preceding started signal.
  readonly property string bundledBinary: Qt.resolvedUrl("bin/wireview-pro2-qs")
    .toString().replace(/^file:\/\//, "")
  property bool binaryFallback: false
  readonly property string backendBinary: binaryFallback ? "wireview-pro2-qs" : bundledBinary
  property string pendingAction: ""

  readonly property var panelItem: panelLoader.item
  readonly property bool opened: panelItem ? panelItem.opened === true : false

  property string statusState: "off"
  property real watts: NaN
  property string title: ""

  readonly property bool hideWhenOff: setting("hideWhenOff", false) === true
  readonly property var status: ({
    state: root.statusState,
    watts: root.watts,
    title: root.title
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
  }

  function runAction(action) {
    if (actionProc.running) return
    root.pendingAction = action
    actionProc.command = [root.backendBinary, action]
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
    command: [root.backendBinary, "watch"]
    // True between a successful start and the process exiting. A `running`
    // false edge without it means the spawn failed: Quickshell then emits
    // neither started nor exited, so onExited would never retry.
    property bool startedOnce: false
    stdout: SplitParser {
      onRead: function(line) { root.applyLine(line) }
    }
    onStarted: watchProc.startedOnce = true
    onExited: {
      root.statusState = "off"
      watchRestartTimer.restart()
    }
    onRunningChanged: {
      if (watchProc.running) return
      var failedStart = !watchProc.startedOnce
      watchProc.startedOnce = false
      if (failedStart && !root.binaryFallback) {
        root.binaryFallback = true
        root.statusState = "off"
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
    // Same spawn-failure detection as watchProc; on failure retry the action
    // once with a PATH binary instead of silently dropping the click.
    property bool startedOnce: false
    onStarted: actionProc.startedOnce = true
    onRunningChanged: {
      if (actionProc.running) return
      var failedStart = !actionProc.startedOnce
      actionProc.startedOnce = false
      if (!failedStart || root.pendingAction === "") return
      if (!root.binaryFallback) {
        root.binaryFallback = true
        actionProc.command = [root.backendBinary, root.pendingAction]
        actionProc.running = true
      }
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
