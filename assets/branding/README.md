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

Built-in imagegen was used in background-extraction edit mode with the owner attachment and transparent output. Prompt:

> Remove only the dark background, including dark negative spaces inside the emblem and letter counters. Keep the three curled chameleons exactly as drawn, with their original position, orientation, shape, texture and colors: purple top, coral pink lower left, off-white lower right. Preserve the exact Cham3 wordmark, white Cham and coral pink 3, lettering and layout. Preserve the dark circular eye rings and colored pupil centers. Spaces between animals and within curled tails become transparent. No redesign, approximation, shadow, glow, outline, new shapes, font changes or extra text. Output the complete emblem and wordmark on genuine transparency.

The generated draft had extraction artifacts and changed source colors; it was rejected. Final local finishing uses the supplied raster pixels instead. The opaque source remains byte-identical to the attachment. The generated draft is not a project asset.

Historical screenshots and previous verification hashes in `docs/evidence/` and old handoffs describe their original commits and must not be rewritten to show this artwork.
