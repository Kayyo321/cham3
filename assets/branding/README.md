# Cham3 artwork

The owner supplied the current three-chameleon emblem and Cham3 wordmark on 2026-10-12. Purple is at the top, rose at lower left, and white at lower right.

| Asset | Purpose |
|---|---|
| `cham3-logo.png` | Original owner attachment, copied byte for byte; 1254 × 1254 with charcoal background. |
| `cham3-logo-transparent.png` | Full emblem and wordmark on a transparent 1254 × 1254 canvas. |
| `cham3-logo-icon-transparent.png` | Emblem only, centered in a padded transparent square. |
| `../wallpapers/cham3_minimal_charcoal.png` | 3840 × 2160 wallpaper; full logo centered at 680 pixels visible height on uniform #17171b. |

The existing hyphenated PNG filenames are preserved because they are established asset interfaces. New source identifiers and filenames use the repository naming convention. `packaging/cham3-desktop/debian/install` installs the three PNGs; `debian/rules` installs the wallpaper under KDE's resolution-named path. Existing desktop defaults already select that wallpaper package.

## Rebuilding the derived assets

From the repository root on macOS:

```sh
swift assets/branding/refresh_artwork.swift
```

This source-specific CoreGraphics recipe retains all opaque foreground colors from the owner image, clears the charcoal background and negative spaces, preserves the dark eye rings, removes the charcoal matte from partial edge pixels, crops the emblem, and assembles the wallpaper. It checks unchanged opaque colors, wallpaper opacity, placement, and background uniformity. This is raster extraction, not newly drawn artwork. A different source image needs a new reviewed recipe; the source dimensions, eye coordinates and wordmark separation are specific to this image.

## Imagegen prompt and disposition

Built-in imagegen was used in background-extraction edit mode with the owner attachment and transparent output. Exact prompt:

```text
Use case: background-extraction
Asset type: cham3 OS canonical transparent logo, full emblem and wordmark.
Input image 1: EDIT TARGET and exact owner approved artwork. Remove only the dark background, including dark negative spaces inside the emblem and letter counters and eye rings. Keep the three flat curled chameleons EXACTLY as drawn, in their same position, orientation, shape, texture and colors: purple top, coral pink lower left, off-white lower right. Preserve the exact wordmark 'Cham3' including white Cham and coral pink 3, exact lettering and layout. Preserve circular dark eye rings as opaque near-black artwork, with colored pupil centers unchanged; spaces between animals and within their curled tails become transparent. No redesign, no vector approximation, no shadow, no glow, no outline, no new shapes, no change of font, no added text. Output the complete emblem plus wordmark on genuine transparent background, tightly bounded with small clear padding, high quality clean edges. Only background changes.
```

The generated draft had extraction artifacts and changed source colors; it was rejected. Final local finishing uses the supplied raster pixels instead. The opaque source remains byte-identical to the attachment. The generated draft is not a project asset.

Historical screenshots and previous verification hashes in `docs/evidence/` and old handoffs describe their original commits and must not be rewritten to show this artwork.
