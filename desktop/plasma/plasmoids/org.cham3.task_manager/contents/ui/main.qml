// cham3 task manager - M1 visual foundation, INERT.
// Two scoped sections separated by a subtle divider:
//   Section A (active-tag apps) and Section B (apps on other tags).
// Alias and tag attribution and per-application network status come from cham3
// session services that do not exist in M1. So:
//   - every window is shown unclassified in Section A, with a "?" pip meaning
//     unknown (never a compliant Rose/Purple/White color);
//   - Section B is empty and states that it is unavailable;
//   - no icon activates, moves, pulls or closes a window.
// A window appearing here carries no claim about isolation, contract or
// network mode.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    readonly property string state_text: Shared.unavailable_text("alias, tag and network attribution", Shared.reason_session_services_absent)

    toolTipMainText: "Task manager"
    toolTipSubText: state_text

    preferredRepresentation: compactRepresentation

    compactRepresentation: Item {
        id: compact
        // Takes the vertical space left between the fixed components and the tray.
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumHeight: 96
        Layout.preferredHeight: 200

        Accessible.name: "Task manager. Alias, tag and network attribution unavailable. Windows shown unclassified."

        ColumnLayout {
            anchors.fill: parent
            anchors.topMargin: 2
            spacing: 2

            PlasmaComponents.Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: "tag: n/a"
                font.pixelSize: 8
                fontSizeMode: Text.Fit
                minimumPixelSize: 6
                color: Kirigami.Theme.disabledTextColor
            }

            // Section A: active-tag apps (unclassified in M1).
            Loader {
                id: view_loader
                Layout.fillWidth: true
                Layout.fillHeight: true
                source: Qt.resolvedUrl("task_view.qml")
            }

            // Subtle divider between sections.
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: 10
                Layout.rightMargin: 10
                height: 1
                color: Shared.surface_border
            }

            // Section B: other-tag apps. Dimmed and empty: no tag data exists.
            PlasmaComponents.Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                opacity: 0.7
                text: "other tags: " + Shared.state_unavailable
                font.pixelSize: 8
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.disabledTextColor
            }
            PlasmaComponents.Label {
                Layout.fillWidth: true
                visible: view_loader.status !== Loader.Ready
                horizontalAlignment: Text.AlignHCenter
                text: "window list: " + Shared.state_unavailable
                font.pixelSize: 8
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }
}
