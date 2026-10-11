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
var RoseDirect = "#ff2e7e";
var PurpleTor = "#8b4df0";
var WhiteOffline = "#f2f2f2";

// Dark surfaces.
var SurfaceBase = "#17171b";
var SurfaceRaised = "#1f2026";
var SurfaceBorder = "#3a3b44";
var TextPrimary = "#f2f2f2";
var TextMuted = "#b4b7c2";

// Panel geometry (logical px); the layout script is authoritative for width.
var PanelWidth = 60;

// State vocabulary.
var StateUnavailable = "unavailable";
var StateDisconnected = "disconnected";
var StateUnverified = "unverified";
var StateUnknown = "unknown";

// Reasons.
var ReasonBackendAbsent = "backend not present (M2+/ADR 0001)";
var ReasonSessionServicesAbsent = "cham3 session services not present";
var ReasonEnforcementAbsent = "no trusted enforcement present";

// "<what>: unavailable - <why>"
function unavailable_text(what, why) {
    return what + ": " + StateUnavailable + " - " + why;
}

// Footer shown in every popup so the foundation status is never implied away.
var FoundationNotice = "M1 visual foundation only. This control performs no security action and reports no active security state.";
