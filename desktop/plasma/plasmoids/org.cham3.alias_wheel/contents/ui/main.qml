// cham3 alias wheel - M1 visual foundation, INERT.
// Draws the Rose / Purple / White trifold dial only. It does not select an
// alias, switch profiles, launch anything or change any network mode. A click
// opens an explanatory panel stating that alias selection is unavailable.
// QML note: import aliases (Kirigami, PlasmaComponents, Shared) must start with
// an upper-case letter; this is a QML language boundary, not a cham3 name.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    readonly property string state_text: Shared.unavailable_text("alias selection", Shared.ReasonSessionServicesAbsent)
    // Last clicked segment, for the explanatory panel only. It is NOT a selection.
    property string pressed_segment: ""

    toolTipMainText: "Alias wheel"
    toolTipSubText: state_text

    // Segment table: start angle in degrees (0 = 3 o'clock, clockwise).
    readonly property var segments: [
        { name: "Direct", tier: "Rose", color: Shared.RoseDirect, start: -90 },
        { name: "Tor", tier: "Purple", color: Shared.PurpleTor, start: 30 },
        { name: "Offline", tier: "White", color: Shared.WhiteOffline, start: 150 }
    ]

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 58
        Layout.preferredHeight: 58
        Layout.maximumHeight: 58

        Accessible.name: "Alias wheel: Direct (Rose), Tor (Purple), Offline (White). Selection unavailable."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        Canvas {
            id: dial
            width: 40
            height: 40
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 2
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                var cx = width / 2
                var cy = height / 2
                var outer = width / 2 - 1
                var inner = outer * 0.52
                for (var i = 0; i < root.segments.length; ++i) {
                    var a0 = (root.segments[i].start + 4) * Math.PI / 180
                    var a1 = (root.segments[i].start + 116) * Math.PI / 180
                    ctx.beginPath()
                    ctx.arc(cx, cy, outer, a0, a1, false)
                    ctx.arc(cx, cy, inner, a1, a0, true)
                    ctx.closePath()
                    ctx.fillStyle = root.segments[i].color
                    // Dimmed: the dial is a picture, not a control with state.
                    ctx.globalAlpha = 0.55
                    ctx.fill()
                }
            }
        }

        // Text caption: state is never conveyed by color alone.
        PlasmaComponents.Label {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            width: parent.width - 4
            horizontalAlignment: Text.AlignHCenter
            text: Shared.StateUnavailable
            font.pixelSize: 9
            fontSizeMode: Text.Fit
            minimumPixelSize: 7
            color: Kirigami.Theme.disabledTextColor
        }

        MouseArea {
            anchors.fill: parent
            onClicked: mouse => {
                // Work out which wedge was hit, for the explanatory text only.
                var dx = mouse.x - dial.x - dial.width / 2
                var dy = mouse.y - dial.y - dial.height / 2
                var deg = Math.atan2(dy, dx) * 180 / Math.PI
                if (deg < -90) { deg += 360 }
                var hit = ""
                for (var i = 0; i < root.segments.length; ++i) {
                    if (deg >= root.segments[i].start && deg < root.segments[i].start + 120) {
                        hit = root.segments[i].name + " (" + root.segments[i].tier + ")"
                    }
                }
                root.pressed_segment = hit
                root.expanded = !root.expanded
            }
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 18
        Layout.minimumHeight: Kirigami.Units.gridUnit * 10
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: "Alias wheel"
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: root.state_text
            font.bold: true
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: root.pressed_segment !== ""
            text: "You clicked: " + root.pressed_segment + ". Nothing was selected."
        }
        Repeater {
            model: root.segments
            delegate: RowLayout {
                required property var modelData
                spacing: Kirigami.Units.smallSpacing
                Rectangle {
                    width: Kirigami.Units.gridUnit
                    height: Kirigami.Units.gridUnit
                    radius: width / 2
                    color: modelData.color
                    border.color: Shared.SurfaceBorder
                    Accessible.ignored: true
                }
                PlasmaComponents.Label {
                    text: modelData.name + " alias (" + modelData.tier + "): " + Shared.StateUnavailable
                }
            }
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "Aliases are user-scoped execution contexts, not signing identities or virtual machines. The wheel does not switch profiles or change any network mode."
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: Shared.FoundationNotice
        }
    }
}
