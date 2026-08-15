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
    actionProc.command = ["wireview-pro2-qs", action]
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
    command: ["wireview-pro2-qs", "watch"]
    stdout: SplitParser {
      onRead: function(line) { root.applyLine(line) }
    }
    onExited: {
      root.statusState = "off"
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
