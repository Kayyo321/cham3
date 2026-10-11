// cham3 kill switch - M1 visual foundation, INERT.
// Draws the rocker only. It has no position state: it cannot be toggled, it
// issues no request, it never alters the firewall, interfaces, routes or
// radios, and it never shows Full Isolation Kill as pending, engaged or
// confirmed. Clicking opens an explanatory panel. Only a future trusted
// enforcement service may report enforcement state.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    readonly property string state_text: "Full Isolation Kill " + Shared.StateUnavailable + " - " + Shared.ReasonEnforcementAbsent

    toolTipMainText: "Full Isolation Kill"
    toolTipSubText: state_text

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 68
        Layout.preferredHeight: 68
        Layout.maximumHeight: 68

        Accessible.name: "Full Isolation Kill rocker: unavailable. No trusted enforcement present. Does nothing."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        Rectangle {
            id: rocker
            width: 26
            height: 44
            radius: 6
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 2
            color: Shared.SurfaceRaised
            border.width: 1
            border.color: compact.activeFocus ? Shared.TextPrimary : Shared.SurfaceBorder
            opacity: 0.7

            // Both halves are identical: there is no "on" or "off" position to read.
            Rectangle {
                x: 3; y: 3
                width: parent.width - 6
                height: parent.height / 2 - 4
                radius: 3
                color: Shared.SurfaceBase
                border.width: 1
                border.color: Shared.SurfaceBorder
            }
            Rectangle {
                x: 3
                y: parent.height / 2 + 1
                width: parent.width - 6
                height: parent.height / 2 - 4
                radius: 3
                color: Shared.SurfaceBase
                border.width: 1
                border.color: Shared.SurfaceBorder
            }
            PlasmaComponents.Label {
                anchors.centerIn: parent
                text: "?"
                font.bold: true
                font.pixelSize: 14
                color: Shared.TextPrimary
            }
        }

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
            // Opens information only. There is deliberately no other handler.
            onClicked: root.expanded = !root.expanded
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 20
        Layout.minimumHeight: Kirigami.Units.gridUnit * 10
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: "Full Isolation Kill"
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
            text: "Requested position: none. Enforcement state: " + Shared.StateUnknown + " (" + Shared.StateUnavailable + "). This control does not change the firewall, network interfaces or radios and cannot start or simulate a kill. Network connectivity is not affected by this panel."
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: Shared.FoundationNotice
        }
    }
}
