# cham3 desktop source tree (M1)

This tree is the canonical source for the `cham3-desktop` Debian package
(`packaging/cham3-desktop/`). It is an **inert M1 visual and package
foundation**, not a security feature.

## What this is

- Eight Plasma 6 plasmoids for the default right-edge panel (ADR 0002), in
  `plasma/plasmoids/org.cham3.<id>/`:
  `user_launcher`, `alias_wheel`, `tag_pager`, `contract_dock`,
  `netproto_launcher`, `killswitch`, `settings_launcher`, `task_manager`.
  Component 9 of ADR 0002 is the upstream Plasma system tray and digital clock,
  placed by the layout script; it is not a cham3 plasmoid.
- The panel layout script `plasma/layout/cham3_panel_layout.js`: one vertical
  panel on the right screen edge, 60 logical px thick, with the components in
  ADR 0002 order and the tray and clock at the bottom. It is installed as the
  layout template `org.cham3.panel` and loaded by the look-and-feel package.
- The look-and-feel package `plasma/look-and-feel/org.cham3.desktop/`, the
  color scheme `plasma/color-schemes/Cham3Dark.colors` (dark surfaces; the
  Rose, Purple and White alias accents are recorded in its `Cham3Accents`
  group), and the wallpaper metadata `plasma/wallpapers/cham3/` (installed as the KDE wallpaper package `Cham3`). The wallpaper
  image and logos are installed from `assets/` by the package.

## What this is not (inertness)

No component performs a security action or reports active security state.
There is no identity, alias, tag, contract, network-enforcement or
kill-switch backend in M1 (M2 to M5). Therefore:

| Component | M1 behavior |
|---|---|
| user_launcher | Placeholder avatar; shows `key_id: unavailable`; the flyout has a search field and the installed applications grouped by menu category (the ordinary KDE application menu, from the Plasma application models), plus the ordinary KDE Lock / Log Out / Reboot / Power Off requests. No contract badges (M5); applications launched here are marked unverified. |
| alias_wheel | Draws the Rose/Purple/White dial. Clicking selects nothing and switches no profile; it states that alias selection is unavailable. |
| tag_pager | Draws markers 1 to 5 in the unknown (outline) style. No tag change, no window move. |
| contract_dock | Four disabled-looking tools; each states it is unavailable. ECR Explorer states: unavailable pending ADR 0001 approval/implementation. |
| netproto_launcher | Opens an information panel: Network Protocol Editor unavailable (M4 enforcement not present). Applies nothing. |
| killswitch | Rocker picture with no position state. States Full Isolation Kill is unavailable (no trusted enforcement). Cannot alter the firewall, network or radios and cannot simulate a kill. |
| settings_launcher | Opens KDE System Settings. cham3 security sections are not present. |
| task_manager | Shows open windows icon-only, all unclassified with a "?" (unknown) pip. No alias, tag or network attribution; no click action on any window. |

State is always conveyed by text (`unavailable`, `unverified`, `unknown`), never
by color alone. There is no animation, so reduced-motion settings are
satisfied by construction.

No `.desktop` launchers are shipped by cham3. The user launcher lists the
desktop entries of the applications that are installed (the `cham3-desktop`
dependencies and the `cham3-full` suite), as the KDE application menu does.

## Maintenance notes

- Each plasmoid carries a copy of `plasma/shared/cham3_shared.js` in its own
  `contents/code/` because a Plasma applet package must be self-contained.
  `debian/rules` fails the package build if a copy differs from the source of
  truth. To refresh the copies after editing the shared file:
  `for d in desktop/plasma/plasmoids/org.cham3.*; do cp desktop/plasma/shared/cham3_shared.js "$d/contents/code/"; done`
- Plasma-private modules used (external boundary, pinned to Debian 13 Plasma
  6.3): `org.kde.plasma.private.sessions` (session actions, loaded through a
  Loader so only the session buttons degrade if it is missing),
  `org.kde.plasma.private.kicker` (the application menu and search of the user
  launcher: `RootModel` for the categories, `RunnerModel` restricted to the
  KRunner services runner; shipped by `plasma-workspace`, imported directly in
  `user_launcher/contents/ui/main.qml`) and the `apps` data engine of
  `org.kde.plasma.plasma5support` (launching System Settings, in
  `settings_launcher`). The menu needs the session to export
  `XDG_MENU_PREFIX=plasma-` as `startplasma` does; without menu data the
  launcher states that the application menu is unavailable.
- Naming: all project identifiers, including the constants in the shared
  JavaScript, are `snake_case` (the repo naming test does not exempt
  constants). The following are external spellings kept as required: QML import
  aliases (`Kirigami`, `PlasmaComponents`, `PlasmaExtras`, `Kicker`,
  `P5Support`, `Shared` must start upper-case in QML), the Kicker model role and
  signal names (`display`, `decoration`, `description`, `refreshed`) and
  PlasmoidItem members (`expanded`, `fullRepresentationItem`), Qt/KDE/Plasma API names, KDE JSON keys (`KPlugin`,
  `X-Plasma-API-Minimum-Version`), the KDE color-scheme file format and the
  file name `Cham3Dark.colors`, the KDE wallpaper package id `Cham3`, and the
  Plasma-fixed layout file name `org.kde.plasma.desktop-layout.js`.
