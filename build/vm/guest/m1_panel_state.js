// Plasma desktop scripting (evaluated by plasmashell through its D-Bus method
// org.kde.PlasmaShell.evaluateScript): prints the running shell's panels, their
// widgets and geometry, and the desktop wallpaper as JSON. Read-only: it changes
// no configuration. Used by build/vm/run_m1_vm_checks.py to check the rendered
// right-edge panel. Scripting API names (panels, widgets, geometry, location,
// formFactor, readConfig, ...) are upstream spellings; the printed keys are cham3's.
var result = { panels: [], desktops: [], screens: [] };
panels().forEach(function (panel) {
    var widgets = panel.widgets().map(function (widget) {
        var geometry = widget.geometry;
        return {
            type: widget.type,
            id: widget.id,
            geometry: geometry ? { x: geometry.x, y: geometry.y, width: geometry.width, height: geometry.height } : null
        };
    });
    result.panels.push({
        location: panel.location,
        form_factor: panel.formFactor,
        height: panel.height,
        length: panel.length,
        alignment: panel.alignment,
        hiding: panel.hiding,
        offset: panel.offset,
        screen: panel.screen,
        widgets: widgets
    });
});
desktops().forEach(function (desktop) {
    desktop.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
    result.desktops.push({ wallpaper_plugin: desktop.wallpaperPlugin, image: desktop.readConfig("Image"), screen: desktop.screen });
});
for (var index = 0; index < screenCount; ++index) {
    var screen = screenGeometry(index);
    result.screens.push({ x: screen.x, y: screen.y, width: screen.width, height: screen.height });
}
print(JSON.stringify(result));
