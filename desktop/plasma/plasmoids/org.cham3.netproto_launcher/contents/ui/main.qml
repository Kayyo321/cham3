// cham3 network protocol launcher - M1 visual foundation, INERT.
// Full-width routing-glyph button. The Network Protocol Editor (Clearnet / Tor /
// Offline tabs) does not exist in M1 and no enforcement service is present. The
// button opens an informational panel only and applies no setting.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    readonly property string state_text: "Network Protocol Editor " + Shared.state_unavailable + " (M4 enforcement not present)"

    toolTipMainText: "Network Protocol Editor"
    toolTipSubText: state_text

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 40
        Layout.preferredHeight: 40
        Layout.maximumHeight: 40

        Accessible.name: "Network Protocol Editor: unavailable. M4 enforcement not present."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            radius: 5
            color: Shared.surface_raised
            border.width: 1
            border.color: compact.activeFocus ? Shared.text_primary : Shared.surface_border

            // Routing glyph: one source node branching to three targets.
            Canvas {
                id: glyph
                anchors.centerIn: parent
                width: 36
                height: 16
                onPaint: {
                    var ctx = getContext("2d")
                    ctx.reset()
                    ctx.strokeStyle = Shared.text_muted
                    ctx.fillStyle = Shared.text_muted
                    ctx.lineWidth = 1.5
                    ctx.beginPath()
                    ctx.moveTo(4, 8)
                    ctx.lineTo(16, 8)
                    ctx.moveTo(16, 8)
                    ctx.lineTo(32, 2)
                    ctx.moveTo(16, 8)
                    ctx.lineTo(32, 8)
                    ctx.moveTo(16, 8)
                    ctx.lineTo(32, 14)
                    ctx.stroke()
                    var dots = [[4, 8], [32, 2], [32, 8], [32, 14]]
                    for (var i = 0; i < dots.length; ++i) {
                        ctx.beginPath()
                        ctx.arc(dots[i][0], dots[i][1], 2.2, 0, 2 * Math.PI)
                        ctx.fill()
                    }
                }
            }
            // Text cue for the state (the glyph alone says nothing about it).
            PlasmaComponents.Label {
                anchors.bottom: parent.bottom
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - 2
                horizontalAlignment: Text.AlignHCenter
                text: Shared.state_unavailable
                font.pixelSize: 8
                fontSizeMode: Text.Fit
                minimumPixelSize: 6
                color: Shared.text_muted
            }
            MouseArea {
                anchors.fill: parent
                onClicked: root.expanded = !root.expanded
            }
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 20
        Layout.minimumHeight: Kirigami.Units.gridUnit * 9
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: "Network Protocol Editor"
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.bold: true
            text: root.state_text
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: "The planned Clearnet, Tor and Offline tabs are not present. No setting is shown, proposed, signed or applied, and no firewall, routing or Tor configuration is changed."
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: Shared.foundation_notice
        }
    }
}
