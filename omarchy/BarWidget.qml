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
  property bool appRunning: false
  property real watts: NaN
  property string title: ""
  property var sensors: null

  readonly property bool hideWhenOff: setting("hideWhenOff", false) === true
  readonly property var status: ({
    state: root.statusState,
    watts: root.watts,
    title: root.title,
    appRunning: root.appRunning,
    sensors: root.sensors
  })
  readonly property string labelText: Model.labelText(status)
  readonly property string tooltipText: Model.tooltipText(status)
  readonly property bool hasLiveFault: Model.hasLiveFault(status)
  readonly property color urgent: bar ? bar.urgent : Color.urgent

  function open() { if (panelItem) panelItem.open() }
  function close() { if (panelItem) panelItem.close() }
  function toggle() { if (panelItem) panelItem.toggle() }

  property string lastFaultKey: ""
  // Hysteresis against threshold flapping: a fault must stay clear for this
  // many consecutive readings before the same fault may notify again, and
  // any two notifications are separated by at least notifyCooldownMs.
  readonly property int faultClearStreak: 3
  readonly property int notifyCooldownMs: 60000
  // Debounced imbalance alerts: a Current Imbalance must persist for this many
  // consecutive readings (~seconds at the watcher's fixed 1 Hz poll) before it
  // may raise a notification, so momentary load transients stay silent.
  readonly property int imbalanceConfirmStreak: 5
  property int faultFreeStreak: 0
  property double lastNotifyAt: 0
  property int imbalanceStreak: 0
  property var pendingNotify: null

  function applyLine(line) {
    var parsed = Model.parseLine(String(line || ""))
    if (!parsed) return
    root.statusState = parsed.state
    root.appRunning = parsed.appRunning === true
    root.watts = parsed.state === "live" ? parsed.watts : NaN
    root.title = parsed.title
    root.sensors = parsed.sensors || null
    root.maybeNotify({
      state: root.statusState,
      watts: root.watts,
      title: root.title,
      appRunning: root.appRunning,
      sensors: root.sensors
    })
  }

  function maybeNotify(status) {
    var currentState = {
      lastFaultKey: root.lastFaultKey,
      faultFreeStreak: root.faultFreeStreak,
      lastNotifyAt: root.lastNotifyAt,
      imbalanceStreak: root.imbalanceStreak
    }
    var newState = Model.computeNotifyState(
      status,
      currentState,
      root.faultClearStreak,
      root.notifyCooldownMs,
      Date.now(),
      root.imbalanceConfirmStreak
    )
    root.lastFaultKey = newState.lastFaultKey
    root.faultFreeStreak = newState.faultFreeStreak
    root.lastNotifyAt = newState.lastNotifyAt
    root.imbalanceStreak = newState.imbalanceStreak
    if (newState.shouldNotify) {
      root.pendingNotify = newState.alert
      root.flushNotify()
    } else if (!Model.faultAlert(status)) {
      // Clear pending notification when no fault is present
      root.pendingNotify = null
    }
  }

  function flushNotify() {
    if (notifyProc.running || !root.pendingNotify) return
    var alert = root.pendingNotify
    var argv = Model.notifyCommand(alert)
    if (!argv.length) {
      root.pendingNotify = null
      return
    }
    notifyProc.command = argv
    notifyProc.running = true
    if (notifyProc.running) {
      // Accepted for delivery; the queue slot is free again.
      root.pendingNotify = null
    } else {
      // omarchy-notification-send is missing or refused to spawn: fall back
      // to plain notify-send so a critical fault alert is not lost, and log
      // if even that path is unavailable.
      root.deliverFallbackNotify(alert)
    }
  }

  function deliverFallbackNotify(alert) {
    var argv = [
      "notify-send",
      "-u", "critical",
      "-a", "WireView Pro II",
      String(alert.headline || "WireView Pro II fault"),
      String(alert.body || "")
    ]
    notifyProc.command = argv
    notifyProc.running = true
    root.pendingNotify = null
    if (!notifyProc.running)
      console.warn("wireview-pro2: cannot deliver fault notification:", alert.body || "")
  }

  function warnActionFailed(action, exitCode) {
    if (notifyProc.running) return
    notifyProc.command = [
      "notify-send",
      "-u", "normal",
      "-a", "WireView Pro II",
      "WireView Pro II",
      action + " failed (exit " + exitCode + ")"
    ]
    notifyProc.running = true
  }

  function clearStatus() {
    root.statusState = "off"
    root.appRunning = false
    root.watts = NaN
    root.sensors = null
    // Preserve notification state (lastNotifyAt, faultFreeStreak, lastFaultKey,
    // imbalanceStreak) across transient watcher exits and failed starts. They
    // are only reset during genuine first-time initialization
    // (Component.onCompleted path).
    root.pendingNotify = null
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

  visible: !hideWhenOff || root.appRunning
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
      root.clearStatus()
      watchRestartTimer.restart()
    }
    onRunningChanged: {
      if (watchProc.running) return
      var failedStart = !watchProc.startedOnce
      watchProc.startedOnce = false
      if (failedStart) {
        root.clearStatus()
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
    onExited: function(exitCode) {
      // The backend exits non-zero when an action could not be carried out
      // (app missing, spawn refused); surface it instead of failing silently.
      var action = root.pendingAction
      root.pendingAction = ""
      if (exitCode === 0 || action === "") return
      root.warnActionFailed(action, exitCode)
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

  Process {
    id: notifyProc
    running: false
    onRunningChanged: {
      if (notifyProc.running) return
      root.flushNotify()
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.labelText
    foreground: root.hasLiveFault ? root.urgent : Color.bar.text
    activeColor: Color.bar.active
    active: root.statusState === "na" || root.hasLiveFault
    horizontalMargin: 8.5
    verticalPadding: 6
    tooltipText: root.tooltipText
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.LeftButton) root.runAction("open")
      else if (buttonCode === Qt.RightButton) root.toggle()
    }
  }
}
