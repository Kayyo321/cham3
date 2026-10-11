"""Static checks for the M1 panel layout and cham3 plasmoid packaging (ADR 0002).

Panel: one vertical panel on the right screen edge, thickness given as a literal
logical-pixel height in [56, 64] (Plasma's panel thickness property), and exactly
the nine ADR 0002 widgets in order
(eight cham3 plasmoids, then the upstream system tray and digital clock).
Plasmoids: each of the eight cham3 plasmoids has metadata.json with KPlugin.Id
equal to its directory name and org.cham3.<snake_case>, plus contents/ui/main.qml.
Look-and-feel and color scheme are checked for presence and a dark default.

These run on desktop/ and FAIL when the tree is missing (M0 convention). The
panel parser ignores JS comments, so a commented-out addWidget() does not count.
"""

import configparser
import json
import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m0_support  # noqa: E402
import m1_support  # noqa: E402

LocationPattern = re.compile(r"""\.location\s*=\s*["'](\w+)["']""")
ThicknessPattern = re.compile(r"\.(width|height)\s*=\s*([^;\n]+)")
WidgetPattern = re.compile(r"""\.addWidget\(\s*["']([^"']+)["']""")
NewPanelPattern = re.compile(r"\bnew\s+Panel\b")
LiteralNumber = re.compile(r"^\s*(\d+(\.\d+)?)\s*$")
PlasmoidStructure = "Plasma/Applet"
LookAndFeelStructure = "Plasma/LookAndFeel"
ApiMinimumVersion = "6"
LookAndFeelId = "org.cham3.desktop"


def layout_problems(js_text):
    """Return problems with the panel layout text (empty means accepted)."""
    code = m1_support.strip_js_comments(js_text)
    problems = []
    if len(NewPanelPattern.findall(code)) != 1:
        problems.append("layout must create exactly one panel")
    locations = LocationPattern.findall(code)
    if locations != ["right"]:
        problems.append("panel must be on the right screen edge, found %r" % locations)
    # Plasma's panel scripting object exposes thickness as `height` for BOTH
    # orientations (there is no `width` property on a scripting Panel); `length`
    # controls the long dimension. So a right-edge panel's thickness is set with
    # `panel.height`. Using `width` would set a non-existent property and leave
    # the default thickness, so it is rejected.
    thickness_values = []
    for name, value in ThicknessPattern.findall(code):
        if name == "width":
            problems.append("panel thickness must be set with height (the Plasma scripting thickness property for both orientations), found width")
        else:
            thickness_values.append(value)
    if len(thickness_values) != 1:
        problems.append("panel thickness (height) must be set exactly once, found %d" % len(thickness_values))
    else:
        literal = LiteralNumber.match(thickness_values[0])
        if not literal:
            problems.append("panel thickness must be a literal logical-pixel value, found %r" % thickness_values[0].strip())
        else:
            value = float(literal.group(1))
            if not m1_support.PanelMinWidth <= value <= m1_support.PanelMaxWidth:
                problems.append(
                    "panel thickness %s outside [%d, %d] logical px"
                    % (literal.group(1), m1_support.PanelMinWidth, m1_support.PanelMaxWidth)
                )
    widgets = tuple(WidgetPattern.findall(code))
    if widgets != m1_support.PanelWidgetOrder:
        problems.append(
            "widget order must be %s, found %s"
            % (", ".join(m1_support.PanelWidgetOrder), ", ".join(widgets) or "none")
        )
    return problems


def plasmoid_problems(plugin_id, directory, metadata, main_qml_present):
    problems = []
    if not isinstance(metadata, dict) or "KPlugin" not in metadata:
        return ["%s metadata.json lacks a KPlugin block" % plugin_id]
    kplugin = metadata["KPlugin"]
    if kplugin.get("Id") != plugin_id:
        problems.append("KPlugin.Id %r must equal %s" % (kplugin.get("Id"), plugin_id))
    if os.path.basename(directory) != plugin_id:
        problems.append("directory %s must equal plugin id %s" % (os.path.basename(directory), plugin_id))
    if not kplugin.get("Name", "").strip():
        problems.append("%s: KPlugin.Name is empty" % plugin_id)
    if metadata.get("KPackageStructure") != PlasmoidStructure:
        problems.append("%s: KPackageStructure must be %s" % (plugin_id, PlasmoidStructure))
    if not str(metadata.get("X-Plasma-API-Minimum-Version", "")).startswith(ApiMinimumVersion):
        problems.append("%s: X-Plasma-API-Minimum-Version must be Plasma 6" % plugin_id)
    if not main_qml_present:
        problems.append("%s: contents/ui/main.qml is missing" % plugin_id)
    return problems


def color_scheme_problems(text):
    """Dark default: [Colors:Window] BackgroundNormal must be dark (rough relative luminance)."""
    parser = configparser.ConfigParser(interpolation=None, strict=False, delimiters=("=",))
    parser.optionxform = str
    try:
        parser.read_string(text)
    except configparser.Error as error:
        return ["color scheme does not parse: %s" % error]
    if not parser.has_section("Colors:Window"):
        return ["color scheme lacks [Colors:Window]"]
    raw = parser.get("Colors:Window", "BackgroundNormal", fallback="")
    try:
        red, green, blue = (int(part) for part in raw.split(",")[:3])
    except ValueError:
        return ["BackgroundNormal is not an r,g,b triple: %r" % raw]
    luminance = (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255.0
    if luminance >= 0.25:
        return ["window background is not dark (luminance %.2f)" % luminance]
    return []


def plasmoid_dirs_by_component():
    result = {}
    for component in m1_support.Cham3PlasmoidComponents:
        plugin_id = m1_support.Cham3Namespace + component
        found = m1_support.plasmoid_dirs(plugin_id)
        if len(found) != 1:
            raise AssertionError("expected exactly one %s directory under desktop/, found %d" % (plugin_id, len(found)))
        result[component] = found[0]
    return result


def cham3_plasmoid_dir_names():
    """Every org.cham3.* directory under desktop/ that is a Plasma applet
    (metadata.json KPackageStructure == Plasma/Applet). This excludes the
    look-and-feel package and the panel layout-template, which are org.cham3.*
    packages but not plasmoids."""
    root = m1_support.desktop_path(m1_support.DesktopTreeDir)
    names = set()
    if not os.path.isdir(root):
        return names
    for dirpath, _, _ in os.walk(root):
        base = os.path.basename(dirpath)
        if not base.startswith(m1_support.Cham3Namespace):
            continue
        metadata_path = os.path.join(dirpath, "metadata.json")
        if not os.path.isfile(metadata_path):
            continue
        try:
            metadata = m1_support.load_json(metadata_path)
        except (ValueError, OSError):
            continue
        if metadata.get("KPackageStructure") == PlasmoidStructure:
            names.add(base)
    return names


class panel_layout_real_t(unittest.TestCase):
    def test_panel_layout_matches_adr_0002(self):
        text = m1_support.read_repo(m1_support.PanelLayoutPath)
        self.assertEqual(layout_problems(text), [])


class panel_layout_fixture_t(unittest.TestCase):
    def good(self):
        widgets = "\n".join(
            'panel.addWidget("%s");' % widget for widget in m1_support.PanelWidgetOrder
        )
        return (
            "var panel = new Panel;\n"
            "panel.location = 'right';\n"
            "panel.height = 60;\n"
            + widgets + "\n"
        )

    def test_positive_control_accepted(self):
        self.assertEqual(layout_problems(self.good()), [])

    def test_left_edge_is_rejected(self):
        problems = layout_problems(self.good().replace("'right'", "'left'"))
        self.assertTrue(any("right screen edge" in p for p in problems), problems)

    def test_thickness_outside_range_is_rejected(self):
        problems = layout_problems(self.good().replace("panel.height = 60", "panel.height = 70"))
        self.assertTrue(any("outside [56, 64]" in p for p in problems), problems)

    def test_width_instead_of_height_is_rejected(self):
        problems = layout_problems(self.good().replace("panel.height = 60", "panel.width = 60"))
        self.assertTrue(any("set with height" in p for p in problems), problems)

    def test_non_literal_thickness_is_rejected(self):
        problems = layout_problems(self.good().replace("panel.height = 60", "panel.height = 40 * scale"))
        self.assertTrue(any("literal" in p for p in problems), problems)

    def test_swapped_order_is_rejected(self):
        widgets = list(m1_support.PanelWidgetOrder)
        widgets[0], widgets[1] = widgets[1], widgets[0]
        text = self.good()
        for widget in m1_support.PanelWidgetOrder:
            text = text.replace('panel.addWidget("%s");\n' % widget, "")
        text += "".join('panel.addWidget("%s");\n' % w for w in widgets)
        problems = layout_problems(text)
        self.assertTrue(any("widget order" in p for p in problems), problems)

    def test_missing_clock_is_rejected(self):
        text = self.good().replace('panel.addWidget("org.kde.plasma.digitalclock");\n', "")
        problems = layout_problems(text)
        self.assertTrue(any("widget order" in p for p in problems), problems)

    def test_extra_backend_widget_is_rejected(self):
        text = self.good() + 'panel.addWidget("org.cham3.ecr_explorer");\n'
        problems = layout_problems(text)
        self.assertTrue(any("widget order" in p for p in problems), problems)

    def test_second_panel_is_rejected(self):
        text = self.good() + "var second = new Panel;\n"
        problems = layout_problems(text)
        self.assertTrue(any("exactly one panel" in p for p in problems), problems)

    def test_commented_out_widget_does_not_count(self):
        text = self.good().replace(
            'panel.addWidget("org.cham3.user_launcher");',
            '// panel.addWidget("org.cham3.user_launcher");',
        )
        problems = layout_problems(text)
        self.assertTrue(any("widget order" in p for p in problems), problems)


class plasmoid_real_t(unittest.TestCase):
    def test_eight_cham3_plasmoids_exist_with_matching_metadata(self):
        problems = []
        for component, directory in plasmoid_dirs_by_component().items():
            plugin_id = m1_support.Cham3Namespace + component
            metadata_path = os.path.join(directory, "metadata.json")
            m0_support.require_file(metadata_path, "plasmoid metadata for " + plugin_id)
            metadata = m1_support.load_json(metadata_path)
            main_qml = os.path.isfile(os.path.join(directory, "contents", "ui", "main.qml"))
            problems += plasmoid_problems(plugin_id, directory, metadata, main_qml)
        self.assertEqual(problems, [])

    def test_no_extra_cham3_plasmoid_is_shipped(self):
        expected = {m1_support.Cham3Namespace + c for c in m1_support.Cham3PlasmoidComponents}
        self.assertEqual(sorted(cham3_plasmoid_dir_names() - expected), [],
                         "unexpected org.cham3.* plasmoid directory under desktop/")
        self.assertEqual(sorted(expected - cham3_plasmoid_dir_names()), [],
                         "contracted org.cham3.* plasmoid directory missing")

    def test_look_and_feel_is_present_and_named_for_cham3(self):
        candidates = [p for p in m1_support.all_desktop_files(("metadata.json",))
                      if os.path.basename(os.path.dirname(p)) == LookAndFeelId]
        self.assertEqual(len(candidates), 1, "expected one %s/metadata.json" % LookAndFeelId)
        metadata = m1_support.load_json(os.path.join(m0_support.repo_root(), candidates[0]))
        self.assertEqual(metadata.get("KPlugin", {}).get("Id"), LookAndFeelId)
        self.assertEqual(metadata.get("KPackageStructure"), LookAndFeelStructure)

    def test_dark_cham3_color_scheme_is_shipped(self):
        schemes = m1_support.all_desktop_files((".colors",))
        self.assertEqual(len(schemes), 1, "expected exactly one color scheme under desktop/")
        self.assertEqual(color_scheme_problems(m1_support.read_repo(schemes[0])), [])

    def test_packaging_installs_contracted_destinations(self):
        texts = []
        debian = os.path.join(m0_support.repo_root(), m1_support.PackageDesktopDir, "debian")
        if not os.path.isdir(debian):
            raise AssertionError("required directory missing: " + m1_support.PackageDesktopDir + "/debian")
        for name in os.listdir(debian):
            path = os.path.join(debian, name)
            if os.path.isfile(path):
                texts.append(m1_support.read_repo(os.path.relpath(path, m0_support.repo_root())))
        combined = "\n".join(texts)
        for destination in (
            "usr/share/plasma/plasmoids",
            "usr/share/plasma/look-and-feel",
            "usr/share/color-schemes",
            "usr/share/wallpapers",
        ):
            self.assertIn(destination, combined, "packaging does not install into " + destination)


class panel_and_plasmoid_fixture_t(unittest.TestCase):
    def test_plasmoid_positive_control_accepted(self):
        meta = {"KPlugin": {"Id": "org.cham3.killswitch", "Name": "Kill switch"},
                "KPackageStructure": "Plasma/Applet", "X-Plasma-API-Minimum-Version": "6.0"}
        self.assertEqual(plasmoid_problems("org.cham3.killswitch", "/x/org.cham3.killswitch", meta, True), [])

    def test_plasmoid_id_mismatch_is_rejected(self):
        meta = {"KPlugin": {"Id": "org.cham3.kill_switch", "Name": "Kill switch"},
                "KPackageStructure": "Plasma/Applet", "X-Plasma-API-Minimum-Version": "6.0"}
        problems = plasmoid_problems("org.cham3.killswitch", "/x/org.cham3.killswitch", meta, True)
        self.assertTrue(any("KPlugin.Id" in p for p in problems), problems)

    def test_plasma5_api_is_rejected(self):
        meta = {"KPlugin": {"Id": "org.cham3.killswitch", "Name": "k"},
                "KPackageStructure": "Plasma/Applet", "X-Plasma-API-Minimum-Version": "5.27"}
        problems = plasmoid_problems("org.cham3.killswitch", "/x/org.cham3.killswitch", meta, True)
        self.assertTrue(any("Plasma 6" in p for p in problems), problems)

    def test_missing_main_qml_is_rejected(self):
        meta = {"KPlugin": {"Id": "org.cham3.killswitch", "Name": "k"},
                "KPackageStructure": "Plasma/Applet", "X-Plasma-API-Minimum-Version": "6.0"}
        problems = plasmoid_problems("org.cham3.killswitch", "/x/org.cham3.killswitch", meta, False)
        self.assertTrue(any("main.qml" in p for p in problems), problems)

    def test_light_color_scheme_is_rejected(self):
        light = "[Colors:Window]\nBackgroundNormal=239,240,241\n"
        dark = "[Colors:Window]\nBackgroundNormal=24,24,28\n"
        self.assertTrue(color_scheme_problems(light))
        self.assertEqual(color_scheme_problems(dark), [])


if __name__ == "__main__":
    unittest.main()
