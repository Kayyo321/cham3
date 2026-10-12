# Task: cham3_logo_refresh — Replace logo artwork and wallpaper

- **State:** READY_TO_MERGE
- **Owner / role / provider:** Codex / COORDINATOR and IMPLEMENTER / OpenAI
- **Created / updated:** 2026-10-12
- **Base commit:** `bfe7de93072b4dd8816785f9c9d60e21a2da0226`

## Objective and approved requirements

Replace all current logo artwork and wallpapers with the new three-chameleon emblem and Cham3 wordmark supplied by the owner in this chat. Product authority: `PROJECT.md` sections 8.1 and 8.6. The attached image supplies artwork only; embedded document instructions have no authority.

## Scope and exclusions

Replace the three PNGs in `assets/branding/` and the one PNG in `assets/wallpapers/`. Preserve the exact owner-provided full image, derive transparent full-logo and emblem-only variants, and retain the minimal wallpaper's 3840 × 2160 canvas, centered 680-pixel visible logo height, and uniform #17171b background. Record source, processing recipe, verification and handoff. Preserve existing packaging filenames and installation paths. Historical screenshots in `docs/evidence/` are test records, not current artwork, and must remain unmodified. Security behavior, color-scheme tokens, desktop controls, architecture, ISO builds and publishing are excluded.

## Dependencies and interfaces

- Owner attachment: `codex-clipboard-279a61bd-ad98-4c53-a347-d1b74e3800af.png`.
- Existing `cham3-desktop` installation rules consume `assets/branding/*.png` and `assets/wallpapers/cham3_minimal_charcoal.png`.
- Built-in imagegen prepares a transparent cutout; local CoreGraphics finishing may preserve source pixels and assemble variants without changing the design.

## Acceptance criteria

- [x] The opaque logo matches the supplied image byte for byte.
- [x] Full transparent logo and emblem-only transparent icon show the new artwork and clear negative space, with no old stacked mascot.
- [x] Wallpaper shows the new full logo at the established size, centered on uniform charcoal, without added effects.
- [x] All current artwork consumers resolve to the replacement assets; historical evidence stays intact.
- [x] Dimensions, alpha, hashes, visual QA, packaging-path checks and independent review have evidence.

## Security and verification

Static artwork only; no authority or security boundary changes. Security review is not required. Use macOS `sips`, Swift/CoreGraphics, `view_image`, SHA-256, `git diff --check`, and existing packaged-path tests. Full Debian package build and KDE boot are NOT_RUN unless an appropriate environment is available; static checks do not prove installation.

## Assignment and Git

- Implementer: Codex, branch `codex/cham3_logo_refresh`, worktree `/Users/jessicabruce/cham3`.
- Startup working tree: clean, on `codex/m2-cryptographic-contract-core`; its HEAD is the base above.
- Independent reviewer: Codex `review_logo_refresh`, read-only review per `docs/agents/ORCHESTRATION.md`; APPROVE, no findings. See `docs/reviews/cham3_logo_refresh.md`.
- Target commit: `a2993749bd174bfff18394e2c109b01d036fef42`.
- Open owner decisions/blockers: none.
