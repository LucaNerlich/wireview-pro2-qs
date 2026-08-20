import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

// Native Quattro popup for the WireView widget: current status plus app
// actions. State flows in from BarWidget.qml (fed by the Rust watch stream).
Panel {
  id: root
  moduleName: "luca.wireview-pro2"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  // The bar tracks the widget mounted in its slot — BarWidget.qml — not this
  // nested panel, so everything the bar identifies a panel by must be that
  // widget (same pattern as the built-in clock panel).
  readonly property var barIdentity: hostWidget || root
  readonly property var watcher: hostWidget || root
  readonly property bool hasWatcher: watcher !== null && watcher !== root

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property string statusState: hasWatcher ? String(watcher.statusState || "off") : "off"
  readonly property real watts: hasWatcher ? Number(watcher.watts) : NaN
  readonly property string title: hasWatcher ? String(watcher.title || "") : ""
  readonly property var sensors: hasWatcher ? (watcher.sensors || null) : null

  readonly property var status: ({
    state: root.statusState,
    watts: root.watts,
    title: root.title,
    sensors: root.sensors
  })
  readonly property string stateLine: Model.stateLine(status)
  readonly property bool hasSensors: Model.hasSensors(status)

  function runAction(action) {
    if (!hasWatcher || typeof watcher.runAction !== "function") return
    watcher.runAction(action)
    root.close()
  }

  // The base Panel passes `root` (this Panel) to bar.switchPanelFrom, which
  // only matches slot.activeItem — the BarWidget. Route through barIdentity
  // so Tab actually moves to the neighboring bar panel.
  function switchPanel(direction) {
    if (root.bar && typeof root.bar.switchPanelFrom === "function")
      return root.bar.switchPanelFrom(root.barIdentity, direction)
    return false
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(420))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onActivateRequested: root.runAction("open")
      onTabRequested: function(direction) { root.switchPanel(direction) }
    }

    Column {
      id: column
      width: parent.width
      spacing: Style.space(10)

      PanelHero {
        width: parent.width
        title: "WireView Pro II"
        meta: root.stateLine
        // Do not bind the watch-stream `title` here. Qt Text may interpret
        // HTML as rich text (AutoText); watts already appear in `meta`.
        detail: "Thermal Grizzly GPU power monitor"
        foreground: root.foreground
        fontFamily: root.fontFamily

        iconComponent: Component {
          Text {
            text: "\u26A1"
            color: root.statusState === "live" ? root.foreground : root.urgent
            font.family: root.fontFamily
            font.pixelSize: Style.font.display
          }
        }

        trailingControl: Component {
          Row {
            spacing: Style.space(4)

            PanelActionButton {
              iconText: "\uF455"
              tooltipText: "Open app window"
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.runAction("open")
            }

            PanelActionButton {
              iconText: "\uF4AD"
              tooltipText: "Restart app"
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.runAction("restart")
            }

            PanelActionButton {
              iconText: "\uF4C5"
              tooltipText: "Quit app"
              foreground: root.urgent
              fontFamily: root.fontFamily
              onClicked: root.runAction("quit")
            }
          }
        }
      }

      Row {
        width: parent.width
        spacing: Style.space(10)

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "Power draw"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: root.statusState === "live" ? Model.formatWatts(root.watts) + " W" : "\u2014"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }
        }

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "App"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: root.statusState === "off" ? "not running" : "running"
            color: root.statusState === "off" ? root.urgent : root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }
        }
      }

      // Only instantiated when the backend read the hwmon chip, so the
      // bindings below never evaluate a null sensors object.
      Loader {
        width: parent.width
        active: root.hasSensors
        sourceComponent: sensorsSection
      }
    }
  }

  Component {
    id: sensorsSection

    Column {
      width: parent.width
      spacing: Style.space(8)

      Text {
        text: "Sensors"
        color: Qt.darker(root.foreground, 1.5)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      Row {
        width: parent.width
        spacing: Style.space(10)

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "Total current"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmt(root.sensors.sumCurrentA, 2) + " A"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }
        }

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "PSU rating"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmt(root.sensors.psuCapW, 0) + " W"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }
        }
      }

      Row {
        width: parent.width
        spacing: Style.space(10)

        Column {
          width: (parent.width - 3 * parent.spacing) / 4
          spacing: Style.space(2)

          Text {
            text: "Temp in"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtTemp(root.sensors.tempInC)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        Column {
          width: (parent.width - 3 * parent.spacing) / 4
          spacing: Style.space(2)

          Text {
            text: "Temp out"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtTemp(root.sensors.tempOutC)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        Column {
          width: (parent.width - 3 * parent.spacing) / 4
          spacing: Style.space(2)

          Text {
            text: "Ext 1"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtTemp(root.sensors.ext1C)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        Column {
          width: (parent.width - 3 * parent.spacing) / 4
          spacing: Style.space(2)

          Text {
            text: "Ext 2"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtTemp(root.sensors.ext2C)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }
      }

      Row {
        width: parent.width
        spacing: Style.space(10)

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "Fault status"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtFault(root.sensors.faultStatus)
            color: root.sensors.faultStatus ? root.urgent : root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        Column {
          width: (parent.width - parent.spacing) / 2
          spacing: Style.space(2)

          Text {
            text: "Fault log"
            color: Qt.darker(root.foreground, 1.5)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            text: Model.fmtFault(root.sensors.faultLog)
            color: root.sensors.faultLog ? root.urgent : root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }
      }

      Column {
        width: parent.width
        spacing: Style.space(2)

        Text {
          text: "12VHPWR pins"
          color: Qt.darker(root.foreground, 1.5)
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Repeater {
          model: 6

          delegate: Row {
            width: parent.width
            spacing: 0

            Text {
              width: parent.width * 0.3
              text: "Pin " + (index + 1)
              color: Qt.darker(root.foreground, 1.5)
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }

            Text {
              width: parent.width * 0.35
              text: Model.fmt(root.sensors.voltageV[index], 3) + " V"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
            }

            Text {
              width: parent.width * 0.35
              text: Model.fmt(root.sensors.currentA[index], 3) + " A"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
            }
          }
        }
      }
    }
  }
}
