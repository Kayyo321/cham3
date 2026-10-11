// Display-only window list for the cham3 task manager (M1).
// Reads the upstream KDE task model purely to draw icons. No action is wired
// to any icon (no activate, close, move or pull). Every window is drawn in the
// "unclassified" style because alias, tag and network attribution do not
// exist in M1; a neutral "?" pip stands for unknown and is never a compliant
// color. org.kde.taskmanager is an upstream KDE module (external boundary).
import QtQuick
import org.kde.taskmanager as TaskManager
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

Item {
    id: root

    readonly property int window_count: tasks_model.count

    TaskManager.TasksModel {
        id: tasks_model
        groupMode: TaskManager.TasksModel.GroupDisabled
        sortMode: TaskManager.TasksModel.SortVirtualDesktop
        filterByVirtualDesktop: true
        filterByScreen: false
        filterByActivity: true
    }

    ListView {
        anchors.fill: parent
        clip: true
        spacing: 4
        interactive: contentHeight > height
        model: tasks_model
        delegate: Item {
            id: entry
            required property var model
            width: ListView.view.width
            height: 34
            visible: entry.model.IsWindow === true
            Accessible.name: (entry.model.display || "window") + " - alias, tag and network mode unknown"
            Accessible.role: Accessible.StaticText

            Kirigami.Icon {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
                width: 28
                height: 28
                source: entry.model.decoration
            }
            // Neutral unknown pip: dashed-outline circle with "?" text.
            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.rightMargin: 6
                width: 13
                height: 13
                radius: width / 2
                color: Shared.SurfaceBase
                border.width: 1
                border.color: Shared.TextMuted
                Text {
                    anchors.centerIn: parent
                    text: "?"
                    font.pixelSize: 9
                    color: Shared.TextPrimary
                }
            }
        }
    }
}
