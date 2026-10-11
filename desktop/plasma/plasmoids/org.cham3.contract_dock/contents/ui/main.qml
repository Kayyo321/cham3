// cham3 contract dock - M1 visual foundation, INERT.
// 2x2 grid: Contract Handler, ECR Explorer, Active Contracts Monitor, Hot
// Contract View. None of the backends exist in M1, so every button is drawn
// disabled-looking and opens only an explanatory panel. No contract is read,
// shown, verified, revoked or inspected, and no window is selected.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    property int shown_index: 0

    // Display data only. ecr_note is the ADR 0001 dependency statement.
    readonly property var tools: [
        { name: "Contract Handler", utility: "cham3_contract_manager", icon: "document-edit",
          note: "unavailable - backend not present (M2+/ADR 0001). Needs the contract and policy services." },
        { name: "ECR Explorer", utility: "cham3_ecr_explorer", icon: "view-history",
          note: "unavailable pending ADR 0001 approval/implementation. No history or DAG is shown." },
        { name: "Active Contracts Monitor", utility: "cham3_active_monitor", icon: "utilities-system-monitor",
          note: "unavailable - backend not present (M2+/ADR 0001). No process or sandbox state is shown and nothing can be revoked." },
        { name: "Hot Contract View", utility: "hot contract view", icon: "edit-find",
          note: "unavailable - backend not present (M2+/ADR 0001). The inspection reticle is not armed and no window is attributed." }
    ]

    toolTipMainText: "Contract dock"
    toolTipSubText: "All four contract tools: " + Shared.StateUnavailable + " - backend not present (M2+/ADR 0001)"

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 88
        Layout.preferredHeight: 88
        Layout.maximumHeight: 88

        GridLayout {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 2
            columns: 2
            rowSpacing: 4
            columnSpacing: 4
            Repeater {
                model: root.tools
                delegate: Rectangle {
                    id: tile
                    required property var modelData
                    required property int index
                    Layout.preferredWidth: 26
                    Layout.preferredHeight: 26
                    radius: 5
                    color: Shared.SurfaceRaised
                    border.width: 1
                    border.color: tile.activeFocus ? Shared.TextPrimary : Shared.SurfaceBorder
                    activeFocusOnTab: true
                    Accessible.name: modelData.name + ": unavailable"
                    Accessible.role: Accessible.Button
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            root.shown_index = tile.index
                            root.expanded = true
                        }
                    }
                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: 16
                        height: 16
                        source: modelData.icon
                        opacity: 0.8
                    }
                    // Strike-through mark: second, non-color cue for "unavailable".
                    Rectangle {
                        anchors.centerIn: parent
                        width: 20
                        height: 1
                        rotation: -45
                        color: Shared.TextMuted
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            root.shown_index = tile.index
                            root.expanded = true
                        }
                    }
                }
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
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 20
        Layout.minimumHeight: Kirigami.Units.gridUnit * 10
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: root.tools[root.shown_index].name
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.bold: true
            text: root.tools[root.shown_index].note
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "Utility: " + root.tools[root.shown_index].utility + " (not installed)"
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: "This dock shows no contract, key, signature or process state. Package presence is not evidence of enforcement."
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: Shared.FoundationNotice
        }
    }
}
