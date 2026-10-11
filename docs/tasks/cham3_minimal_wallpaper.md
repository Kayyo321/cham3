# Task: cham3_minimal_wallpaper — Centered logo wallpaper

- **State:** IMPLEMENTED
- **Owner / role / provider:** Codex / IMPLEMENTER / OpenAI
- **Created / updated:** 2026-10-11
- **Base commit:** `24e4bdfddd60339cc4981bb60741471c725ca671`

## Objective and approved requirements

Create a minimalistic Cham3 OS wallpaper in `assets/wallpapers`, using the main transparent logo at the center. The owner's request excludes glow and a generic decorative technology aesthetic. `PROJECT.md` section 8.1 requires restrained visual effects and usability over decorative complexity.

Owner refinement: reduce the logo slightly while keeping it centered on the same flat charcoal background. Use a 15% reduction in width and height (800 to 680 pixels visible height), preserving the 3840 × 2160 canvas and source artwork.

## Scope and dependencies

One landscape PNG wallpaper, its generation prompt, and verification/handoff evidence. Source: `assets/branding/cham3-logo-transparent.png`, currently owner-provided and untracked. Preserve the source and all unrelated dirty files. Do not change desktop defaults, packaging, security behavior, or architecture. Local finishing within this scope uses macOS CoreGraphics to assemble the original transparent logo on an exactly uniform 3840 × 2160 charcoal canvas; generated drafts retained background texture and redrew the logo. This preserves the owner's supplied artwork and requires no architecture change.

## Acceptance criteria

- [x] A usable landscape PNG is saved in `assets/wallpapers`.
- [x] The main Cham3 logo and wordmark occupy the center with generous negative space.
- [x] Background and composition have no glow, decorative effects, or extra text.
- [x] The saved image is inspected visually and its dimensions are verified.

## Security and verification

Security impact: none; a static wallpaper grants no authority. Security review is not required. Verify the saved PNG dimensions with `sips` and inspect the saved file with `view_image` on macOS. OS installation and KDE integration tests are outside this asset-only task.

## Assignment and Git

- Implementer: Codex.
- Branch/worktree: `codex/cham3_minimal_wallpaper`, `/Users/jessicabruce/cham3`.
- Exact target: record asset SHA-256 in the handoff; no unrelated files will be staged.
- Independent review: pending; author inspection is asset QA, not independent approval.
- Open owner decisions/blockers: none for asset creation.

## Evidence

See `docs/handoffs/cham3_minimal_wallpaper_handoff.md`. All asset acceptance checks passed. Independent repository review and integration remain pending; this task is not marked DONE.
