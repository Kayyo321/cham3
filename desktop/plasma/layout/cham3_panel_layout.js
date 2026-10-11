// cham3 default panel layout (M1 visual/package foundation, ADR 0002).
// Plasma 6 layout script, run by plasmashell on first start of a new profile
// through the look-and-feel package org.cham3.desktop (which loads the layout
// template org.cham3.panel whose contents/layout.js is this file).
//
// One vertical panel on the RIGHT screen edge, 60 logical px thick (ADR 0002
// range: 56-64). Components, top to bottom, in ADR 0002 order: user launcher,
// alias wheel, tag pager, contract dock, network protocol launcher, kill switch,
// settings launcher, task manager (eight cham3 plasmoids, ADR 0002 components
// 1-8), then the upstream system tray and digital clock at the bottom (9).
// Components 1-8 are inert visual foundations; backend-dependent ones report
// unavailable. This file configures presentation only: it does not enable any
// service, firewall rule, profile or permission.
// Plasma scripting API property names (location, height, addWidget, ...) are
// upstream spellings and are kept verbatim.

var panel = new Panel;

panel.location = "right";
// For a vertical panel the scripting property "height" is the thickness.
panel.height = 60;
panel.alignment = "left";
panel.hiding = "none";
panel.floating = false;

panel.addWidget("org.cham3.user_launcher");
panel.addWidget("org.cham3.alias_wheel");
panel.addWidget("org.cham3.tag_pager");
panel.addWidget("org.cham3.contract_dock");
panel.addWidget("org.cham3.netproto_launcher");
panel.addWidget("org.cham3.killswitch");
panel.addWidget("org.cham3.settings_launcher");
panel.addWidget("org.cham3.task_manager");

panel.addWidget("org.kde.plasma.systemtray");
panel.addWidget("org.kde.plasma.digitalclock");
