// cham3 shared presentation tokens and state vocabulary (M1 visual foundation).
// SOURCE OF TRUTH. Each plasmoid carries a byte-identical copy at
// contents/code/cham3_shared.js because a Plasma applet package must be
// self-contained. debian/rules fails the package build if any copy differs.
//
// Nothing here reads or reports security state. The vocabulary below is the
// only set of state words the M1 plasmoids may show: backend-dependent features
// are reported as unavailable, disconnected or unverified, never as active.
.pragma library

// Accent colors (ADR 0002): Rose = Direct, Purple = Tor, White = Offline.
// A color is never the only carrier of meaning; every use is paired with text.
var rose_direct = "#ff2e7e";
var purple_tor = "#8b4df0";
var white_offline = "#f2f2f2";

// Dark surfaces.
var surface_base = "#17171b";
var surface_raised = "#1f2026";
var surface_border = "#3a3b44";
var text_primary = "#f2f2f2";
var text_muted = "#b4b7c2";

// Panel geometry (logical px); the layout script is authoritative for width.
var panel_width = 60;

// State vocabulary.
var state_unavailable = "unavailable";
var state_disconnected = "disconnected";
var state_unverified = "unverified";
var state_unknown = "unknown";

// Reasons.
var reason_backend_absent = "backend not present (M2+/ADR 0001)";
var reason_session_services_absent = "cham3 session services not present";
var reason_enforcement_absent = "no trusted enforcement present";

// "<what>: unavailable - <why>"
function unavailable_text(what, why) {
    return what + ": " + state_unavailable + " - " + why;
}

// Footer shown in every popup so the foundation status is never implied away.
var foundation_notice = "M1 visual foundation only. This control performs no security action and reports no active security state.";
