"""Static cross-check: the image-content gate uses the names the packages install (F4).

test_m1_image_contents.py checks file names inside the built ISO. Those names must
be the ones the candidate actually produces, read from these sources:
  - packaging/cham3-desktop/debian/rules: the wallpaper image destination
    (the image is installed as contents/images/3840x2160.png, not the source name);
  - packaging/cham3-desktop/debian/install: the wallpaper metadata destination;
  - the 0086-cham3-sddm-wayland live-config component: the autologin session it
    selects (Session=plasma.desktop), which must be the Plasma session file name.

If a source changes, this test fails, so the gate cannot keep checking a name the
packages no longer produce. It reads text only and does not inspect an ISO; the
real-ISO check is a Codex Remote heavy gate.
"""

import os
import re
import sys
import unittest

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "lib")
)
import m1_support  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_m1_image_contents as image_gate  # noqa: E402

WallpaperDestination = re.compile(r"\$\(pkg_dest\)/(usr/share/wallpapers/[^\s\\]+\.png)")
# A selection names a desktop file. The hook's own sed pattern ('^Session=.*') is not one.
SessionSelection = re.compile(r"Session=([A-Za-z0-9_\-]+\.desktop)\b")
WaylandSessionDir = "usr/share/wayland-sessions"


def rules_wallpaper_destinations(rules_text):
    """Image-package paths that debian/rules installs under usr/share/wallpapers."""
    return WallpaperDestination.findall(rules_text)


def install_destinations(install_text):
    """Destinations of debian/install entries: '<source> <dest-dir>' lines, comments ignored."""
    destinations = []
    for raw in install_text.splitlines():
        fields = raw.split("#", 1)[0].split()
        if len(fields) == 2:
            source, target = fields
            destinations.append(target.rstrip("/") + "/" + os.path.basename(source))
    return destinations


def session_selections(hook_text):
    return SessionSelection.findall(hook_text)


def packaging_problems(rules_text, install_text, hook_text, image, metadata, session):
    """Problems where the image-gate names disagree with the packaging (empty means consistent).

    image, metadata and session are the names the image gate checks for: the
    wallpaper image, the wallpaper metadata, and the Plasma session file.
    """
    problems = []
    if image not in rules_wallpaper_destinations(rules_text):
        problems.append("debian/rules does not install the wallpaper image at " + image)
    if metadata not in install_destinations(install_text):
        problems.append("debian/install does not install the wallpaper metadata at " + metadata)
    selections = session_selections(hook_text)
    if not selections:
        problems.append("the SDDM live-config component selects no session")
    session_name = os.path.basename(session)
    for name in sorted(set(selections)):
        if name != session_name:
            problems.append("SDDM selects session %r, but the image gate expects session file %s" % (name, session))
    if os.path.dirname(session) != WaylandSessionDir:
        problems.append("the image gate session is not a Wayland session file: " + session)
    return problems


class packaged_paths_real_t(unittest.TestCase):
    """Binds the image gate's own constants (test_m1_image_contents) to the packaging."""

    def read_sources(self):
        return (
            m1_support.read_repo(m1_support.WallpaperRules),
            m1_support.read_repo(m1_support.WallpaperInstallList),
            m1_support.read_repo(m1_support.SddmSessionHook),
        )

    def test_image_gate_names_match_packaging(self):
        rules_text, install_text, hook_text = self.read_sources()
        problems = packaging_problems(
            rules_text,
            install_text,
            hook_text,
            image_gate.WallpaperImage,
            image_gate.WallpaperMetadata,
            image_gate.PlasmaSessionFile,
        )
        self.assertEqual(problems, [])

    def test_rules_install_the_resolution_named_wallpaper(self):
        rules_text, _, _ = self.read_sources()
        self.assertIn(image_gate.WallpaperImage, rules_wallpaper_destinations(rules_text))

    def test_superseded_names_are_rejected_against_real_packaging(self):
        # Control: the names the earlier gate used must fail against the real sources.
        rules_text, install_text, hook_text = self.read_sources()
        problems = packaging_problems(
            rules_text,
            install_text,
            hook_text,
            "usr/share/wallpapers/Cham3/contents/images/cham3_minimal_charcoal.png",
            image_gate.WallpaperMetadata,
            "usr/share/wayland-sessions/plasmawayland.desktop",
        )
        self.assertTrue(any("wallpaper image" in p for p in problems), problems)
        self.assertTrue(any("SDDM selects session" in p for p in problems), problems)


class packaged_paths_fixture_t(unittest.TestCase):
    """Positive and negative controls for packaging_problems()."""

    def check(self, rules, install, hook):
        return packaging_problems(
            rules,
            install,
            hook,
            m1_support.PackagedWallpaperImage,
            m1_support.PackagedWallpaperMetadata,
            m1_support.PackagedPlasmaSession,
        )

    def good_rules(self):
        return (
            "override_dh_install:\n"
            "\tinstall -D -m 0644 assets/wallpapers/cham3_minimal_charcoal.png \\\n"
            "\t    $(pkg_dest)/usr/share/wallpapers/Cham3/contents/images/3840x2160.png\n"
        )

    def good_install(self):
        return (
            "# Wallpaper metadata (the image itself is installed by debian/rules).\n"
            "desktop/plasma/wallpapers/cham3/metadata.json usr/share/wallpapers/Cham3/\n"
        )

    def good_hook(self):
        return (
            "apply_component ()\n{\n"
            "\tsed -i 's|^Session=.*|Session=plasma.desktop|' /etc/sddm.conf\n"
            "}\n"
        )

    def test_positive_control_is_consistent(self):
        self.assertEqual(self.check(self.good_rules(), self.good_install(), self.good_hook()), [])

    def test_superseded_wallpaper_name_is_rejected(self):
        rules = self.good_rules().replace("3840x2160.png", "cham3_minimal_charcoal.png")
        problems = self.check(rules, self.good_install(), self.good_hook())
        self.assertTrue(any("wallpaper image" in p for p in problems), problems)

    def test_missing_wallpaper_metadata_install_is_rejected(self):
        problems = self.check(self.good_rules(), "", self.good_hook())
        self.assertTrue(any("wallpaper metadata" in p for p in problems), problems)

    def test_superseded_session_name_is_rejected(self):
        hook = self.good_hook().replace("Session=plasma.desktop", "Session=plasmawayland.desktop")
        problems = self.check(self.good_rules(), self.good_install(), hook)
        self.assertTrue(any("SDDM selects session" in p for p in problems), problems)

    def test_session_without_selection_is_rejected(self):
        problems = self.check(self.good_rules(), self.good_install(), "# no session set\n")
        self.assertTrue(any("selects no session" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
