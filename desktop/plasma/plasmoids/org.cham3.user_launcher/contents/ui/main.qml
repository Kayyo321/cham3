// cham3 user launcher - M1 visual foundation.
// Shows a placeholder avatar and a flyout with (a) a search field and the
// installed applications grouped by menu category and (b) the ordinary Lock /
// Log Out / Reboot / Power Off session actions. The application menu is the
// ordinary KDE one (ADR 0002 component 1; M1 scope): it uses the same Plasma
// models as the stock Kickoff launcher in Debian 13 (Plasma 6.3).
//   Kicker.RootModel   every installed, displayed desktop entry ("All
//                      Applications") and the entries grouped by menu category;
//   Kicker.RunnerModel the KRunner services runner only, which searches the
//                      installed applications (the default runner set also holds
//                      command, session and power runners, which do not belong
//                      in an application search).
// Starting an entry is the standard KDE launch of its desktop entry. There is no
// identity service in M1: the flyout states "key_id: unavailable" and no avatar,
// name or key is shown as an authenticated cham3 identity. Nothing here enrolls,
// authenticates or validates anything. Contract badges and any verified-launch
// state are M5 scope and are not shown: applications started from this flyout
// are not contract-verified, and nothing here fabricates isolation or network
// state.
// org.kde.plasma.private.kicker is a Plasma-private module (external boundary).
// The Debian plasma-workspace package ships it, and cham3-desktop depends on that
// package; it is relied on for the Debian 13 Plasma 6.3 series only. Model role
// names (display, decoration, description), the signal refreshed and the
// PlasmoidItem members expanded and fullRepresentationItem are upstream names.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.extras as PlasmaExtras
import org.kde.plasma.private.kicker as Kicker
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    toolTipMainText: "User launcher"
    toolTipSubText: "key_id: " + Shared.state_unavailable + " (no identity service present)"

    Loader {
        id: session_loader
        source: Qt.resolvedUrl("session_actions.qml")
    }
    readonly property var session_item: session_loader.status === Loader.Ready ? session_loader.item : null

    // Start every opening of the flyout with an empty search on the first category.
    onExpandedChanged: {
        if (root.expanded && root.fullRepresentationItem !== null) {
            root.fullRepresentationItem.reset_menu()
        }
    }

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 52
        Layout.preferredHeight: 52
        Layout.maximumHeight: 52

        Accessible.name: "User launcher. Opens applications and session actions. key_id unavailable."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        // Circular placeholder avatar (generic glyph; not the user's identity).
        Rectangle {
            anchors.centerIn: parent
            width: 40
            height: 40
            radius: width / 2
            color: Shared.surface_raised
            border.width: 2
            border.color: compact.activeFocus ? Shared.text_primary : Shared.surface_border
            Kirigami.Icon {
                anchors.centerIn: parent
                width: 26
                height: 26
                source: "user-identity"
            }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.expanded = !root.expanded
        }
    }

    fullRepresentation: ColumnLayout {
        id: flyout
        // An explicit preferred size keeps the popup from growing to the width of the
        // longest unwrapped label.
        Layout.minimumWidth: Kirigami.Units.gridUnit * 28
        Layout.minimumHeight: Kirigami.Units.gridUnit * 32
        Layout.preferredWidth: Kirigami.Units.gridUnit * 28
        Layout.preferredHeight: Kirigami.Units.gridUnit * 34
        spacing: Kirigami.Units.smallSpacing
        focus: true

        // Selected category: a row of root_model (row 0 is "All Applications").
        property int selected_row: 0
        // Bumped when root_model reports a refresh, which replaces its child models.
        property int refresh_revision: 0

        readonly property bool searching: search_field.text.length > 0
        // refresh_revision is read only to re-evaluate this binding after a refresh:
        // modelForRow() does not notify when the child models are replaced.
        readonly property var category_model: flyout.refresh_revision >= 0 && root_model.count > 0
            ? root_model.modelForRow(flyout.selected_row)
            : null
        // Re-evaluated every time the number of result groups changes.
        readonly property var search_model: runner_model.count > 0 ? runner_model.modelForRow(0) : null
        readonly property var shown_model: flyout.searching ? flyout.search_model : flyout.category_model

        // Start one entry of a list: the standard KDE launch of its desktop entry.
        function launch_entry(view, row) {
            if (!view || !view.model || row < 0 || row >= view.count) {
                return
            }
            if (view.model.trigger(row, "", null)) {
                root.expanded = false
            }
        }

        // Called whenever the flyout opens.
        function reset_menu() {
            search_field.clear()
            flyout.selected_row = 0
            category_view.currentIndex = 0
            search_field.forceActiveFocus()
        }

        Component.onCompleted: {
            root_model.refresh()
            flyout.reset_menu()
        }

        Kicker.RootModel {
            id: root_model
            autoPopulate: false
            appletInterface: root
            flat: true
            sorted: true
            showSeparators: false
            showTopLevelItems: true
            showAllApps: true
            showAllAppsCategorized: false
            showRecentApps: false
            showRecentDocs: false
            showPowerSession: false
            showFavoritesPlaceholder: false
            onRefreshed: flyout.refresh_revision += 1
        }

        Kicker.RunnerModel {
            id: runner_model
            appletInterface: root
            mergeResults: true
            runners: ["krunner_services"]
            query: search_field.text
        }

        Kirigami.Heading {
            level: 3
            text: "cham3 user launcher"
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            font.bold: true
            wrapMode: Text.WordWrap
            text: "key_id: " + Shared.state_unavailable
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "No cham3 identity service is present, so no identity or key is enrolled, authenticated or validated. The avatar is a generic placeholder."
        }

        Kirigami.Separator { Layout.fillWidth: true }

        PlasmaExtras.SearchField {
            id: search_field
            Layout.fillWidth: true
            placeholderText: "Search applications"
            Accessible.name: "Search applications"
            focus: true
            onAccepted: {
                if (flyout.searching) {
                    flyout.launch_entry(apps_view, apps_view.currentIndex)
                }
            }
            Keys.onDownPressed: {
                apps_view.currentIndex = 0
                apps_view.forceActiveFocus()
            }
            Keys.onEscapePressed: {
                if (search_field.text.length > 0) {
                    search_field.clear()
                } else {
                    root.expanded = false
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Kirigami.Units.smallSpacing

            // Categories (hidden while search results are shown).
            PlasmaComponents.ScrollView {
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                visible: !flyout.searching

                ListView {
                    id: category_view
                    clip: true
                    model: root_model
                    keyNavigationEnabled: true
                    Accessible.name: "Application categories"
                    Accessible.role: Accessible.List
                    onCurrentIndexChanged: flyout.selected_row = currentIndex < 0 ? 0 : currentIndex
                    Keys.onRightPressed: {
                        apps_view.currentIndex = 0
                        apps_view.forceActiveFocus()
                    }

                    delegate: PlasmaComponents.ItemDelegate {
                        id: category_entry
                        required property int index
                        required property var model
                        width: ListView.view.width
                        highlighted: ListView.isCurrentItem
                        onClicked: category_view.currentIndex = category_entry.index
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: category_entry.model.decoration
                                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                                implicitHeight: Kirigami.Units.iconSizes.smallMedium
                            }
                            PlasmaComponents.Label {
                                Layout.fillWidth: true
                                text: category_entry.model.display
                                elide: Text.ElideRight
                                maximumLineCount: 1
                            }
                        }
                    }
                }
            }

            // Applications of the selected category, or the search results.
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                PlasmaComponents.ScrollView {
                    anchors.fill: parent

                    ListView {
                        id: apps_view
                        clip: true
                        model: flyout.shown_model
                        keyNavigationEnabled: true
                        Accessible.name: flyout.searching ? "Search results" : "Applications"
                        Accessible.role: Accessible.List
                        // A newly shown model (a category or search results) starts on its first row.
                        onCountChanged: {
                            if (count > 0 && currentIndex < 0) {
                                currentIndex = 0
                            }
                        }
                        Keys.onReturnPressed: flyout.launch_entry(apps_view, apps_view.currentIndex)
                        Keys.onEnterPressed: flyout.launch_entry(apps_view, apps_view.currentIndex)
                        Keys.onLeftPressed: event => {
                            if (flyout.searching) {
                                event.accepted = false
                            } else {
                                category_view.forceActiveFocus()
                            }
                        }

                        delegate: PlasmaComponents.ItemDelegate {
                            id: app_entry
                            required property int index
                            required property var model
                            width: ListView.view.width
                            highlighted: ListView.isCurrentItem && (apps_view.activeFocus || flyout.searching)
                            onClicked: flyout.launch_entry(apps_view, app_entry.index)
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                Kirigami.Icon {
                                    source: app_entry.model.decoration
                                    implicitWidth: Kirigami.Units.iconSizes.medium
                                    implicitHeight: Kirigami.Units.iconSizes.medium
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 0
                                    PlasmaComponents.Label {
                                        Layout.fillWidth: true
                                        text: app_entry.model.display
                                        elide: Text.ElideRight
                                        maximumLineCount: 1
                                    }
                                    PlasmaComponents.Label {
                                        Layout.fillWidth: true
                                        visible: text.length > 0 && text !== app_entry.model.display
                                        text: app_entry.model.description
                                        font: Kirigami.Theme.smallFont
                                        opacity: 0.7
                                        elide: Text.ElideRight
                                        maximumLineCount: 1
                                    }
                                }
                            }
                        }
                    }
                }

                PlasmaExtras.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - Kirigami.Units.gridUnit * 2
                    visible: apps_view.count === 0
                    iconName: "edit-none"
                    text: flyout.searching
                        ? "No matching application"
                        : "Application menu: " + Shared.state_unavailable + " (no application entries found)"
                }
            }
        }

        Kirigami.Separator { Layout.fillWidth: true }

        PlasmaComponents.Label {
            text: "Session"
            font.bold: true
        }
        RowLayout {
            Layout.fillWidth: true
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Lock"
                icon.name: "system-lock-screen"
                enabled: root.session_item !== null && root.session_item.can_lock
                onClicked: root.session_item.do_lock()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Log Out"
                icon.name: "system-log-out"
                enabled: root.session_item !== null && root.session_item.can_logout
                onClicked: root.session_item.do_logout()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Reboot"
                icon.name: "system-reboot"
                enabled: root.session_item !== null && root.session_item.can_reboot
                onClicked: root.session_item.do_reboot()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Power Off"
                icon.name: "system-shutdown"
                enabled: root.session_item !== null && root.session_item.can_shutdown
                onClicked: root.session_item.do_shutdown()
            }
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: root.session_item === null
            text: "Session actions: " + Shared.state_unavailable + " (Plasma session module could not be loaded)."
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "Applications start as from any KDE menu. No contract badges are shown (M5), so applications launched here are not contract-verified: " + Shared.state_unverified + ". " + Shared.foundation_notice
        }
    }
}
