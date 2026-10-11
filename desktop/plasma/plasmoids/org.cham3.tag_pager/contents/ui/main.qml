// cham3 tag pager - M1 visual foundation, INERT.
// Five tag markers 1-5, outline only. No tag state is known in M1 (occupied,
// active and urgent states need cham3 session services and KWin integration),
// so every marker is drawn in the neutral unknown style. Clicking never changes
// the viewed tag and never moves a window.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    readonly property string state_text: Shared.unavailable_text("tag navigation", Shared.reason_session_services_absent)

    toolTipMainText: "Tag pager"
    toolTipSubText: state_text

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 128
        Layout.preferredHeight: 128
        Layout.maximumHeight: 128

        Accessible.name: "Tag pager: tags 1 to 5, state unknown. Tag navigation unavailable."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 2
            spacing: 3
            Repeater {
                model: 5
                delegate: Rectangle {
                    required property int index
                    width: 20
                    height: 20
                    radius: width / 2
                    color: "transparent"
                    border.width: 1
                    border.color: Kirigami.Theme.textColor
                    // Outline only: the "unknown" style. No fill and no dot,
                    // because fill means active and a dot means occupied.
                    PlasmaComponents.Label {
                        anchors.centerIn: parent
                        text: String(index + 1)
                        font.pixelSize: 11
                        color: Kirigami.Theme.textColor
                    }
                }
            }
        }

        PlasmaComponents.Label {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            width: parent.width - 4
            horizontalAlignment: Text.AlignHCenter
            text: Shared.state_unavailable
            font.pixelSize: 9
            fontSizeMode: Text.Fit
            minimumPixelSize: 7
            color: Kirigami.Theme.disabledTextColor
        }

        MouseArea {
            anchors.fill: parent
            onClicked: root.expanded = !root.expanded
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 18
        Layout.minimumHeight: Kirigami.Units.gridUnit * 8
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: "Tag pager"
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
            text: "Tags 1-5 are shown as outlines with state unknown. Changing the viewed tag and moving windows to a tag are not available, and no window is moved by this control."
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: Shared.foundation_notice
        }
    }
}
