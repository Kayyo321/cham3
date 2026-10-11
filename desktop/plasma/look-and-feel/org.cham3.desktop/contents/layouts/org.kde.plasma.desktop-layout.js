// Layout entry point read by plasmashell for the look-and-feel package
// org.cham3.desktop. The file name is fixed by Plasma (external boundary).
// The panel itself lives in the layout template org.cham3.panel, installed from
// desktop/plasma/layout/cham3_panel_layout.js (see debian/rules).
loadTemplate("org.cham3.panel");

var desktops_array = desktopsForActivity(currentActivity());
for (var desktop_index = 0; desktop_index < desktops_array.length; desktop_index++) {
    desktops_array[desktop_index].wallpaperPlugin = "org.kde.image";
    desktops_array[desktop_index].currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
    desktops_array[desktop_index].writeConfig("Image", "file:///usr/share/wallpapers/Cham3");
}
