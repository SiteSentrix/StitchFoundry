# Image Trace quality

> **Last reviewed:** 2026-10-13 · **Last updated:** 2026-10-13 · **Change:** minor (photos: High Fidelity Photo 3,653 paths to 121 with its colour error down from 2.30 to 1.75)

What Image Trace (`crates/trace`) makes of images whose right answer is known, measured by a harness and
compared with the numbers before each change. Nothing here is a judgement by eye, and nothing is compared
with any other product except the open tracer named under [Yardstick](#4-yardstick).

## How it is measured

`crates/trace/tests/quality` draws its own inputs from vector art (no image files), traces each with every
built-in preset, and measures the result:

```text
TRACE_QUALITY_REPORT=report.md cargo test --release -p vectorcraft-trace --test quality -- --ignored
```

**Inputs** (drawn anti-aliased at 128, 256 and 512 px, and at 256 px also blurred, with compression-like
noise, as a scan of ink on cream paper with uneven light and grain, and aliased: one hard sample per pixel, no
anti-aliasing):

| kind | inputs |
|:--|:--|
| curved shapes | a circle; two ellipses; a rounded badge with a round hole; a letter D with its counter |
| polygons | a triangle, a turned square and a hexagon; a five-pointed star; a letter A with its counter |
| thin lines | five lines 0.5, 1, 2, 3 and 5 units wide (of a 256-unit square), a few degrees off the pixel grid |
| flat colour art | a disc of two halves in different colours that touch; a badge of four colours on white |
| gradient, photo | a smooth two-way gradient with a glow; a photo-like image (soft-edged shaded objects, texture, sensor noise) |

The letters are drawn by hand from lines and curves: the repository has no font to set them in.

**Columns**

| column | meaning |
|:--|:--|
| ΔE | the trace drawn again against the source: mean colour difference in Oklab × 100 (black to white is 100) |
| edge ΔE | the same, weighted by how strong an edge the source has at each pixel |
| off mean, off max | distance (px) from the traced outlines to the art's true outlines: wobble and staircase |
| missed max | largest distance (px) from a true outline to the nearest traced one: what was lost |
| paths, anchors | counts, the background's frame aside |
| × art | anchors per anchor point of the art itself (1.00 is as economical as the original) |
| lines % | share of the outline length made of straight segments (100 is right for polygons, 0 for a circle) |
| corners % | share of the art's sharp corners with a traced corner within 1.5 px |
| kinks | anchors where the outline turns by 10° or more away from any true corner |
| odd colours, odd px | paths (and their area) in a colour the art doesn't have: slivers along anti-aliased edges |
| gap px, overlap px | area no path covers, and area covered twice (abutting traces with a white background only) |
| specks | paths under 6 px of area |
| outside px | how far the trace reaches outside the image: a curve that strays |

Black-and-white presets trace black: on the scanned variant their ΔE includes the paper's and ink's own
colours. `–` means the measure does not apply; `all` that nothing was traced.

## Baseline and now

Each preset on the inputs it is meant for, all sizes and variants.

### Baseline (before any change, main at `baf9693f`)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.79 | 17.15 | 0.313 | 1.51 | 2.52 | 1.9 | 14 | 1.45 | 34 | 76 | 1.4 | – | – | 63.5 | 67.0 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 2.30 | 2.63 | – | – | – | 3652.6 | 46866 | – | 84 | – | – | – | – | 0.0 | – | 1169.4 | 0.3 |
| Low Fidelity Photo | gradient, photo-like | 2.61 | 2.94 | – | – | – | 128.3 | 3867 | – | 42 | – | – | – | – | 0.0 | – | 0.0 | 0.7 |
| 3 Colors | two touching halves | 4.93 | 11.38 | 0.482 | 2.35 | 89.09 | 2.1 | 14 | 2.31 | 12 | 29 | 2.1 | 1.6 | 15418 | 118.0 | 87.3 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.78 | 8.82 | 4.383 | 83.10 | 30.11 | 21.2 | 270 | 30.18 | 51 | 98 | 213.8 | 2.4 | 272 | 198.8 | 194.6 | 0.0 | 0.3 |
| 16 Colors | two touching halves, four-colour badge | 0.74 | 9.02 | 5.115 | 86.21 | 4.32 | 69.9 | 923 | 102.80 | 58 | 100 | 765.9 | 5.9 | 670 | 293.7 | 329.9 | 0.0 | 0.5 |
| Shades of Gray | gradient, photo-like | 9.29 | 9.16 | – | – | – | 90.8 | 1921 | – | 42 | – | – | – | – | 0.0 | – | 0.2 | 0.5 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.76 | 15.96 | 0.270 | 1.76 | 2.53 | 1.4 | 31 | 3.78 | 90 | 81 | 15.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.85 | 17.04 | 0.302 | 1.63 | 105.63 | 1.8 | 14 | 1.34 | 28 | 18 | 1.4 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 3.04 | 21.41 | 0.573 | 2.88 | 2.89 | 1.2 | 10 | 1.16 | 9 | 86 | 1.4 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.77 | 12.51 | 0.158 | 1.13 | 105.76 | 5.4 | 25 | 1.27 | 90 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 2.66 | 13.45 | 0.201 | 1.37 | 105.76 | 4.8 | 32 | 2.23 | 100 | 85 | 5.9 | – | – | – | – | 0.1 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

### Now (sub-pixel edges, the palette, photo smoothing)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.12 | 0.091 | 1.02 | 2.17 | 1.9 | 18 | 1.81 | 29 | 68 | 0.4 | – | – | 22.2 | 22.4 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9818 | – | 43 | – | – | – | – | 0.0 | – | 10.3 | 0.1 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2132 | – | 44 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| 3 Colors | two touching halves | 0.43 | 4.32 | 0.087 | 0.74 | 0.56 | 2.0 | 9 | 1.57 | 39 | 100 | 0.0 | 0.0 | 0 | 48.6 | 29.4 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.56 | 5.01 | 0.137 | 1.09 | 1.14 | 4.0 | 50 | 3.15 | 40 | 88 | 3.6 | 0.0 | 0 | 39.0 | 35.7 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.89 | 0.133 | 1.09 | 1.15 | 4.2 | 66 | 3.97 | 41 | 88 | 4.4 | 0.0 | 0 | 34.5 | 26.4 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 879 | – | 53 | – | – | – | – | 0.0 | – | 0.4 | 0.8 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.39 | 0.064 | 1.00 | 2.18 | 1.4 | 27 | 2.73 | 40 | 67 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.11 | 0.195 | 1.00 | 105.54 | 1.8 | 17 | 1.49 | 16 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.28 | 0.532 | 2.24 | 2.24 | 1.2 | 14 | 1.65 | 12 | 79 | 0.1 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.65 | 9.87 | 0.093 | 0.89 | 105.71 | 5.4 | 77 | 3.86 | 37 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 2.53 | 9.47 | 0.086 | 0.99 | 105.85 | 4.8 | 57 | 3.73 | 69 | 74 | 0.7 | – | – | – | – | 0.3 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

The sections below are the numbers now (sub-pixel edges, the palette, photo smoothing).

## 1. Each preset on the inputs it is meant for

All sizes and variants (clean, aliased, blurred, noisy, scanned) of those inputs.

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.12 | 0.091 | 1.02 | 2.17 | 1.9 | 18 | 1.81 | 29 | 68 | 0.4 | – | – | 22.2 | 22.4 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9818 | – | 43 | – | – | – | – | 0.0 | – | 10.3 | 0.1 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2132 | – | 44 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| 3 Colors | two touching halves | 0.43 | 4.32 | 0.087 | 0.74 | 0.56 | 2.0 | 9 | 1.57 | 39 | 100 | 0.0 | 0.0 | 0 | 48.6 | 29.4 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.56 | 5.01 | 0.137 | 1.09 | 1.14 | 4.0 | 50 | 3.15 | 40 | 88 | 3.6 | 0.0 | 0 | 39.0 | 35.7 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.89 | 0.133 | 1.09 | 1.15 | 4.2 | 66 | 3.97 | 41 | 88 | 4.4 | 0.0 | 0 | 34.5 | 26.4 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 879 | – | 53 | – | – | – | – | 0.0 | – | 0.4 | 0.8 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.39 | 0.064 | 1.00 | 2.18 | 1.4 | 27 | 2.73 | 40 | 67 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.11 | 0.195 | 1.00 | 105.54 | 1.8 | 17 | 1.49 | 16 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.28 | 0.532 | 2.24 | 2.24 | 1.2 | 14 | 1.65 | 12 | 79 | 0.1 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.65 | 9.87 | 0.093 | 0.89 | 105.71 | 5.4 | 77 | 3.86 | 37 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 2.53 | 9.47 | 0.086 | 0.99 | 105.85 | 4.8 | 57 | 3.73 | 69 | 74 | 0.7 | – | – | – | – | 0.3 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

## 2. Each preset on each kind of input (clean, three sizes)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | curved shapes | 0.08 | 3.24 | 0.057 | 0.51 | 0.59 | 1.8 | 16 | 1.72 | 15 | 83 | 0.0 | – | – | 22.7 | 21.1 | 0.0 | 0.0 |
| Default | polygons | 0.05 | 2.11 | 0.036 | 0.46 | 1.23 | 2.0 | 14 | 1.24 | 89 | 90 | 0.0 | – | – | 3.1 | 3.3 | 0.0 | 0.0 |
| Default | thin lines | 0.56 | 4.77 | 0.059 | 0.48 | 105.57 | 3.3 | 22 | 1.12 | 42 | – | – | – | – | 49.1 | 34.0 | 0.0 | 0.0 |
| Default | flat colour art | 19.95 | 18.92 | 0.197 | 0.64 | 179.52 | 1.5 | 22 | 1.32 | 15 | 0 | 0.0 | – | – | 24.3 | 23.0 | 0.0 | 0.0 |
| Default | gradient, photo | 39.13 | 38.78 | – | – | – | 3.2 | 698 | – | 57 | – | – | – | – | 31.2 | 40.7 | 0.0 | 0.3 |
| High Fidelity Photo | curved shapes | 0.04 | 1.74 | 0.026 | 0.16 | 0.16 | 1.2 | 20 | 2.17 | 21 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | polygons | 0.05 | 1.92 | 0.031 | 0.51 | 1.34 | 1.7 | 17 | 1.50 | 87 | 88 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | thin lines | 0.34 | 6.00 | 0.076 | 0.85 | 4.31 | 40.3 | 333 | 16.67 | 37 | – | – | 27.3 | 4489 | 0.0 | – | 19.0 | 0.0 |
| High Fidelity Photo | flat colour art | 0.14 | 4.17 | 0.028 | 0.20 | 0.20 | 3.5 | 71 | 4.44 | 35 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo | 1.80 | 2.05 | – | – | – | 134.7 | 11186 | – | 43 | – | – | – | – | 0.0 | – | 7.7 | 0.1 |
| Low Fidelity Photo | curved shapes | 0.06 | 2.65 | 0.045 | 0.27 | 0.27 | 1.2 | 14 | 1.53 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | polygons | 0.06 | 2.25 | 0.037 | 0.51 | 1.34 | 1.7 | 14 | 1.25 | 86 | 88 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | thin lines | 0.51 | 5.73 | 0.081 | 0.86 | 45.38 | 7.0 | 56 | 2.80 | 47 | – | – | 4.0 | 3974 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | flat colour art | 0.15 | 4.33 | 0.046 | 0.27 | 0.27 | 3.5 | 55 | 3.38 | 32 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | gradient, photo | 2.53 | 2.80 | – | – | – | 42.7 | 2289 | – | 43 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| 3 Colors | curved shapes | 0.06 | 2.65 | 0.046 | 0.33 | 0.24 | 1.8 | 16 | 1.77 | 16 | 100 | 0.0 | 0.0 | 0 | 18.7 | 15.2 | 0.0 | 0.0 |
| 3 Colors | polygons | 0.06 | 2.25 | 0.037 | 0.51 | 1.34 | 2.0 | 15 | 1.35 | 86 | 88 | 0.0 | 0.0 | 0 | 5.1 | 4.7 | 0.0 | 0.0 |
| 3 Colors | thin lines | 0.44 | 4.40 | 0.107 | 0.90 | 45.38 | 5.0 | 44 | 2.22 | 49 | – | – | 2.0 | 290 | 23.8 | 27.4 | 0.0 | 0.0 |
| 3 Colors | flat colour art | 1.32 | 3.85 | 0.113 | 0.90 | 60.22 | 2.5 | 24 | 1.77 | 46 | 100 | 0.0 | 1.0 | 9919 | 30.3 | 31.0 | 0.0 | 0.0 |
| 3 Colors | gradient, photo | 7.49 | 7.60 | – | – | – | 5.7 | 532 | – | 48 | – | – | – | – | 25.5 | 26.9 | 0.0 | 0.2 |
| 6 Colors | curved shapes | 0.06 | 2.38 | 0.042 | 0.28 | 0.24 | 1.8 | 17 | 1.86 | 17 | 100 | 0.0 | 0.0 | 0 | 17.9 | 14.3 | 0.0 | 0.0 |
| 6 Colors | polygons | 0.05 | 2.07 | 0.034 | 0.51 | 1.34 | 2.0 | 16 | 1.42 | 86 | 88 | 0.0 | 0.0 | 0 | 3.9 | 3.9 | 0.0 | 0.0 |
| 6 Colors | thin lines | 0.50 | 4.54 | 0.096 | 0.86 | 45.38 | 4.0 | 32 | 1.60 | 53 | – | – | 1.7 | 398 | 18.5 | 22.8 | 0.0 | 0.0 |
| 6 Colors | flat colour art | 0.12 | 2.66 | 0.044 | 0.25 | 0.20 | 3.0 | 28 | 2.00 | 43 | 100 | 0.0 | 0.0 | 0 | 21.9 | 21.2 | 0.0 | 0.0 |
| 6 Colors | gradient, photo | 5.01 | 5.30 | – | – | – | 13.0 | 833 | – | 36 | – | – | – | – | 42.6 | 37.0 | 0.0 | 0.2 |
| 16 Colors | curved shapes | 0.05 | 2.26 | 0.038 | 0.26 | 0.23 | 1.8 | 18 | 1.93 | 17 | 100 | 0.0 | 0.0 | 0 | 14.9 | 12.5 | 0.0 | 0.0 |
| 16 Colors | polygons | 0.05 | 2.02 | 0.034 | 0.51 | 1.34 | 2.0 | 16 | 1.44 | 86 | 88 | 0.0 | 0.0 | 0 | 3.3 | 4.2 | 0.0 | 0.0 |
| 16 Colors | thin lines | 0.41 | 3.87 | 0.103 | 0.86 | 23.65 | 8.7 | 50 | 2.52 | 51 | – | – | 5.7 | 305 | 18.4 | 25.4 | 0.0 | 0.0 |
| 16 Colors | flat colour art | 0.12 | 2.53 | 0.041 | 0.24 | 0.20 | 3.0 | 28 | 2.07 | 44 | 100 | 0.0 | 0.0 | 0 | 19.8 | 19.5 | 0.0 | 0.0 |
| 16 Colors | gradient, photo | 3.02 | 3.33 | – | – | – | 47.2 | 2154 | – | 29 | – | – | – | – | 89.7 | 80.2 | 0.2 | 0.2 |
| Shades of Gray | curved shapes | 0.06 | 2.72 | 0.046 | 0.48 | 0.65 | 1.2 | 14 | 1.56 | 18 | 92 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | polygons | 0.05 | 1.97 | 0.033 | 0.42 | 1.26 | 1.7 | 14 | 1.21 | 89 | 91 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | thin lines | 0.56 | 6.69 | 0.078 | 0.86 | 45.38 | 10.3 | 71 | 3.57 | 49 | – | – | 8.0 | 4283 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | flat colour art | 7.33 | 11.25 | 0.264 | 1.47 | 179.38 | 4.7 | 214 | 19.86 | 24 | 50 | 0.3 | 3.7 | 173356 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | gradient, photo | 9.71 | 9.55 | – | – | – | 18.2 | 955 | – | 53 | – | – | – | – | 0.0 | – | 0.5 | 0.8 |
| Black and White Logo | curved shapes | 0.04 | 1.52 | 0.024 | 0.50 | 0.60 | 1.2 | 21 | 2.25 | 21 | 75 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | polygons | 0.04 | 1.83 | 0.030 | 0.45 | 1.23 | 1.7 | 15 | 1.34 | 89 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | thin lines | 0.50 | 4.22 | 0.065 | 0.50 | 77.21 | 10.3 | 66 | 3.30 | 65 | – | – | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | flat colour art | 19.94 | 18.72 | 0.192 | 0.31 | 179.52 | 1.0 | 19 | 1.18 | 24 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | gradient, photo | 39.11 | 38.71 | – | – | – | 9.0 | 676 | – | 72 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | curved shapes | 0.13 | 5.54 | 0.107 | 0.41 | 0.41 | 1.2 | 13 | 1.43 | 17 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | polygons | 0.13 | 5.00 | 0.092 | 0.51 | 1.07 | 1.7 | 14 | 1.24 | 82 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | thin lines | 0.38 | 4.99 | 0.102 | 0.80 | 52.57 | 4.3 | 37 | 1.87 | 35 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | flat colour art | 20.32 | 20.95 | 0.175 | 0.89 | 179.64 | 1.5 | 22 | 1.34 | 18 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | gradient, photo | 45.53 | 45.37 | – | – | – | 1.0 | 251 | – | 47 | – | – | – | – | – | – | 0.0 | 0.3 |
| Silhouettes | curved shapes | 0.34 | 14.15 | 0.274 | 0.66 | 0.66 | 1.2 | 12 | 1.36 | 17 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | polygons | 0.35 | 14.03 | 0.263 | 0.67 | 0.70 | 1.7 | 14 | 1.26 | 76 | 85 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | thin lines | 1.05 | 16.01 | 0.308 | 1.09 | 18.10 | 5.0 | 42 | 2.08 | 40 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | flat colour art | 23.86 | 34.06 | 0.226 | 0.77 | 188.38 | 1.0 | 10 | 0.76 | 26 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | gradient, photo | 61.56 | 61.35 | – | – | – | 0.2 | 2 | – | 47 | – | – | – | – | – | – | 0.0 | 0.6 |
| Line Art | curved shapes | 0.05 | 1.93 | 0.033 | 0.53 | 0.63 | 1.2 | 17 | 1.86 | 19 | 75 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | polygons | 0.05 | 1.87 | 0.031 | 0.46 | 1.23 | 1.7 | 14 | 1.27 | 89 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | thin lines | 0.54 | 4.38 | 0.049 | 0.47 | 105.71 | 3.3 | 33 | 1.67 | 42 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | flat colour art | 19.94 | 18.69 | 0.192 | 0.49 | 179.52 | 1.0 | 46 | 2.55 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | gradient, photo | 39.11 | 38.71 | – | – | – | 7.7 | 490 | – | 59 | – | – | – | – | – | – | 0.0 | 0.1 |
| Technical Drawing | curved shapes | 0.04 | 1.52 | 0.024 | 0.50 | 0.60 | 1.2 | 21 | 2.25 | 21 | 75 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | polygons | 0.04 | 1.83 | 0.030 | 0.45 | 1.23 | 1.7 | 15 | 1.34 | 89 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | thin lines | 0.39 | 3.88 | 0.075 | 0.50 | 55.75 | 15.3 | 81 | 4.05 | 76 | – | – | – | – | – | – | 0.3 | 0.0 |
| Technical Drawing | flat colour art | 19.94 | 18.72 | 0.192 | 0.31 | 179.52 | 1.0 | 19 | 1.18 | 24 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | gradient, photo | 39.08 | 38.69 | – | – | – | 20.0 | 964 | – | 79 | – | – | – | – | – | – | 2.0 | 0.1 |
| Flat Logo | curved shapes | 0.06 | 2.14 | 0.036 | 0.25 | 0.25 | 1.8 | 31 | 3.38 | 19 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | polygons | 0.09 | 3.44 | 0.060 | 0.35 | 1.03 | 2.0 | 35 | 3.17 | 67 | 94 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | thin lines | 1.17 | 11.78 | 0.050 | 0.49 | all | 3.0 | 189 | 9.45 | 28 | – | – | 0.0 | 0 | 5467.1 | – | 0.0 | 0.0 |
| Flat Logo | flat colour art | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | gradient, photo | refused | | | | | | | | | | | | | | | | |

## 3. Each preset on its inputs, by variant (256 px)

| preset | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | clean | 0.07 | 2.76 | 0.048 | 0.51 | 1.23 | 1.9 | 15 | 1.52 | 46 | 88 | 0.0 | – | – | 14.3 | 13.5 | 0.0 | 0.0 |
| Default | aliased | 0.39 | 21.13 | 0.229 | 1.02 | 1.34 | 1.9 | 14 | 1.42 | 21 | 98 | 0.0 | – | – | 52.7 | 52.9 | 0.0 | 0.0 |
| Default | blurred | 0.82 | 16.65 | 0.078 | 0.86 | 2.17 | 1.9 | 26 | 2.50 | 7 | 19 | 1.6 | – | – | 23.0 | 23.6 | 0.0 | 0.0 |
| Default | noisy | 1.74 | 4.55 | 0.052 | 0.57 | 1.23 | 1.9 | 16 | 1.64 | 38 | 67 | 0.0 | – | – | 16.2 | 15.1 | 0.0 | 0.0 |
| Default | scanned | 15.06 | 20.24 | 0.134 | 0.87 | 2.17 | 1.9 | 26 | 2.53 | 0 | 25 | 1.0 | – | – | 20.8 | 24.7 | 0.0 | 0.0 |
| High Fidelity Photo | clean | 1.80 | 2.05 | – | – | – | 134.7 | 11186 | – | 43 | – | – | – | – | 0.0 | – | 7.7 | 0.1 |
| High Fidelity Photo | aliased | 1.78 | 2.01 | – | – | – | 73.0 | 7695 | – | 45 | – | – | – | – | 0.0 | – | 5.5 | 0.1 |
| High Fidelity Photo | blurred | 1.59 | 1.95 | – | – | – | 72.0 | 7184 | – | 45 | – | – | – | – | 0.0 | – | 0.5 | 0.1 |
| High Fidelity Photo | noisy | 2.02 | 2.19 | – | – | – | 155.5 | 10562 | – | 38 | – | – | – | – | 0.0 | – | 18.5 | 0.1 |
| High Fidelity Photo | scanned | 1.45 | 1.55 | – | – | – | 140.5 | 9726 | – | 41 | – | – | – | – | 0.0 | – | 24.5 | 0.1 |
| Low Fidelity Photo | clean | 2.53 | 2.80 | – | – | – | 42.7 | 2289 | – | 43 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| Low Fidelity Photo | aliased | 2.53 | 2.78 | – | – | – | 36.5 | 1504 | – | 46 | – | – | – | – | 0.0 | – | 0.0 | 0.1 |
| Low Fidelity Photo | blurred | 2.35 | 2.75 | – | – | – | 29.0 | 2004 | – | 45 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| Low Fidelity Photo | noisy | 2.70 | 2.85 | – | – | – | 32.0 | 1960 | – | 46 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| Low Fidelity Photo | scanned | 1.95 | 2.05 | – | – | – | 34.5 | 2588 | – | 42 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| 3 Colors | clean | 0.07 | 2.53 | 0.051 | 0.24 | 0.14 | 2.0 | 10 | 1.61 | 39 | 100 | 0.0 | 0.0 | 0 | 40.0 | 25.5 | 0.0 | 0.0 |
| 3 Colors | aliased | 0.18 | 9.81 | 0.236 | 0.74 | 0.56 | 2.0 | 7 | 1.17 | 39 | 100 | 0.0 | 0.0 | 0 | 123.2 | 82.4 | 0.0 | 0.0 |
| 3 Colors | blurred | 0.33 | 7.80 | 0.088 | 0.21 | 0.16 | 2.0 | 10 | 1.67 | 39 | 100 | 0.0 | 0.0 | 0 | 29.7 | 18.6 | 0.0 | 0.0 |
| 3 Colors | noisy | 0.71 | 1.81 | 0.049 | 0.23 | 0.08 | 2.0 | 9 | 1.50 | 39 | 100 | 0.0 | 0.0 | 0 | 35.7 | 9.7 | 0.0 | 0.0 |
| 3 Colors | scanned | 1.57 | 3.23 | 0.082 | 0.34 | 0.22 | 2.0 | 11 | 1.83 | 39 | 100 | 0.0 | 0.0 | 0 | 31.9 | 18.9 | 0.0 | 0.0 |
| 6 Colors | clean | 0.12 | 2.66 | 0.044 | 0.25 | 0.20 | 3.0 | 28 | 2.00 | 43 | 100 | 0.0 | 0.0 | 0 | 21.9 | 21.2 | 0.0 | 0.0 |
| 6 Colors | aliased | 0.31 | 10.53 | 0.291 | 1.00 | 1.00 | 3.0 | 26 | 1.91 | 42 | 100 | 0.0 | 0.0 | 0 | 82.6 | 74.0 | 0.0 | 0.0 |
| 6 Colors | blurred | 0.62 | 9.41 | 0.249 | 1.09 | 1.14 | 6.0 | 115 | 6.33 | 42 | 50 | 9.5 | 0.0 | 0 | 32.6 | 48.3 | 0.0 | 0.0 |
| 6 Colors | noisy | 0.85 | 2.50 | 0.048 | 0.26 | 0.18 | 3.0 | 28 | 2.01 | 33 | 100 | 0.0 | 0.0 | 0 | 29.7 | 22.5 | 0.0 | 0.0 |
| 6 Colors | scanned | 1.76 | 4.63 | 0.240 | 1.09 | 0.94 | 7.0 | 101 | 5.81 | 31 | 62 | 15.5 | 0.0 | 0 | 62.7 | 41.7 | 0.0 | 0.0 |
| 16 Colors | clean | 0.12 | 2.53 | 0.041 | 0.24 | 0.20 | 3.0 | 28 | 2.07 | 44 | 100 | 0.0 | 0.0 | 0 | 19.8 | 19.5 | 0.0 | 0.0 |
| 16 Colors | aliased | 0.30 | 10.39 | 0.274 | 1.00 | 1.00 | 3.0 | 34 | 2.34 | 43 | 100 | 0.0 | 0.0 | 0 | 77.3 | 51.7 | 0.0 | 0.0 |
| 16 Colors | blurred | 0.62 | 9.37 | 0.250 | 1.08 | 1.15 | 7.0 | 157 | 8.43 | 44 | 50 | 11.5 | 0.0 | 0 | 27.8 | 38.2 | 0.0 | 0.0 |
| 16 Colors | noisy | 0.85 | 2.34 | 0.042 | 0.27 | 0.22 | 3.0 | 32 | 2.27 | 34 | 100 | 0.0 | 0.0 | 0 | 23.4 | 15.6 | 0.0 | 0.0 |
| 16 Colors | scanned | 1.74 | 4.56 | 0.243 | 1.09 | 0.96 | 7.5 | 156 | 8.53 | 32 | 62 | 19.0 | 0.0 | 0 | 53.3 | 20.8 | 0.0 | 0.0 |
| Shades of Gray | clean | 9.71 | 9.55 | – | – | – | 18.2 | 955 | – | 53 | – | – | – | – | 0.0 | – | 0.5 | 0.8 |
| Shades of Gray | aliased | 9.70 | 9.52 | – | – | – | 11.0 | 788 | – | 58 | – | – | – | – | 0.0 | – | 0.5 | 0.7 |
| Shades of Gray | blurred | 9.66 | 9.32 | – | – | – | 11.5 | 729 | – | 56 | – | – | – | – | 0.0 | – | 0.5 | 0.7 |
| Shades of Gray | noisy | 9.74 | 9.44 | – | – | – | 19.5 | 823 | – | 47 | – | – | – | – | 0.0 | – | 0.0 | 0.2 |
| Shades of Gray | scanned | 6.58 | 6.46 | – | – | – | 19.0 | 947 | – | 49 | – | – | – | – | 0.0 | – | 0.5 | 0.2 |
| Black and White Logo | clean | 0.04 | 1.65 | 0.027 | 0.50 | 1.23 | 1.4 | 18 | 1.86 | 50 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | aliased | 0.39 | 20.41 | 0.176 | 1.00 | 1.34 | 1.4 | 24 | 2.47 | 53 | 98 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | blurred | 0.81 | 16.37 | 0.047 | 0.80 | 2.18 | 1.4 | 39 | 3.78 | 23 | 19 | 1.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | noisy | 1.72 | 3.90 | 0.028 | 0.50 | 1.23 | 1.4 | 21 | 2.14 | 41 | 67 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | scanned | 15.05 | 20.10 | 0.116 | 0.82 | 2.17 | 1.4 | 53 | 5.13 | 11 | 25 | 0.9 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | clean | 0.18 | 5.43 | 0.106 | 0.80 | 52.57 | 1.9 | 18 | 1.52 | 20 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | aliased | 0.49 | 20.73 | 0.237 | 1.00 | 105.54 | 1.6 | 10 | 1.02 | 27 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | blurred | 1.07 | 19.51 | 0.264 | 0.66 | 105.48 | 1.6 | 18 | 1.57 | 13 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | noisy | 1.75 | 5.86 | 0.108 | 0.95 | 1.03 | 2.0 | 18 | 1.53 | 13 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | scanned | 15.11 | 22.41 | 0.438 | 0.85 | 105.28 | 1.6 | 20 | 1.73 | 1 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | clean | 0.34 | 14.15 | 0.274 | 0.66 | 0.66 | 1.2 | 12 | 1.36 | 17 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | aliased | 0.42 | 23.04 | 0.353 | 1.03 | 1.03 | 1.2 | 9 | 1.05 | 9 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | blurred | 1.23 | 28.51 | 0.803 | 1.42 | 1.84 | 1.2 | 17 | 1.91 | 13 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | noisy | 2.10 | 10.67 | 0.274 | 0.64 | 0.64 | 1.2 | 12 | 1.34 | 7 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | scanned | 16.05 | 30.27 | 1.472 | 2.24 | 2.24 | 1.2 | 23 | 3.15 | 3 | 0 | 1.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | clean | 0.54 | 4.38 | 0.049 | 0.47 | 105.71 | 3.3 | 33 | 1.67 | 42 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | aliased | 0.66 | 14.39 | 0.110 | 0.74 | 55.44 | 14.0 | 60 | 3.00 | 87 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | blurred | 1.63 | 17.25 | 0.137 | 0.89 | 105.61 | 3.0 | 139 | 6.95 | 3 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | noisy | 1.03 | 4.27 | 0.043 | 0.48 | 56.06 | 8.0 | 56 | 2.80 | 41 | – | – | – | – | – | – | 1.0 | 0.0 |
| Line Art | scanned | 13.60 | 20.04 | 0.212 | 0.84 | 105.58 | 3.0 | 186 | 9.30 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | clean | 0.13 | 2.34 | 0.041 | 0.50 | 55.75 | 5.1 | 32 | 2.02 | 86 | 90 | 0.0 | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | aliased | 0.43 | 18.55 | 0.206 | 0.99 | 55.44 | 6.8 | 34 | 2.08 | 95 | 97 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | blurred | 1.05 | 16.72 | 0.088 | 0.95 | 105.75 | 2.0 | 97 | 6.62 | 28 | 25 | 2.7 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | noisy | 1.39 | 3.88 | 0.042 | 0.50 | 31.23 | 7.8 | 46 | 2.76 | 81 | 90 | 0.0 | – | – | – | – | 1.8 | 0.0 |
| Technical Drawing | scanned | 14.45 | 20.15 | 0.145 | 0.92 | 105.85 | 2.0 | 126 | 8.57 | 22 | 36 | 2.3 | – | – | – | – | 0.0 | 0.0 |
| Flat Logo | clean | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | aliased | 0.13 | 5.87 | 0.218 | 0.65 | 0.65 | 3.0 | 31 | 1.96 | 44 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 |
| Flat Logo | blurred | 0.54 | 8.33 | 0.052 | 0.62 | 0.78 | 3.0 | 28 | 1.78 | 43 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 |
| Flat Logo | noisy | 0.81 | 1.77 | 0.076 | 0.39 | 0.32 | 3.5 | 44 | 2.63 | 31 | 100 | 1.0 | 0.0 | 0 | 6.9 | – | 0.5 | 0.0 |
| Flat Logo | scanned | 1.31 | 4.15 | 21.204 | 88.50 | 0.66 | 470.5 | 2251 | 255.53 | 11 | 62 | 1658.0 | 0.0 | 0 | 58.0 | – | 413.5 | 0.2 |

## 4. Yardstick

Inkscape 1.4.2's Trace Bitmap, run as a black box through its `object-trace` action (two scans, background
removed, its other settings as they come) on the same one-colour drawings, and its SVG output measured by
the same code. Compare with the black-and-white presets on the same kinds of input in section 2 and 3.

| inputs | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| curved shapes | clean | 0.25 | 11.05 | 0.198 | 0.97 | 0.97 | 1.0 | 15 | 1.98 | 11 | 83 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | aliased | 0.38 | 20.79 | 0.192 | 0.88 | 0.88 | 1.0 | 16 | 1.88 | 14 | 50 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | blurred | 0.79 | 16.57 | 0.182 | 0.86 | 1.14 | 1.0 | 15 | 1.84 | 6 | 0 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | noisy | 2.01 | 8.14 | 0.191 | 0.93 | 0.93 | 1.0 | 17 | 2.06 | 10 | 75 | 2.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | scanned | 15.31 | 20.22 | 0.191 | 0.97 | 1.14 | 1.0 | 16 | 1.98 | 7 | 0 | 2.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | clean | 0.16 | 6.45 | 0.124 | 1.26 | 2.36 | 1.0 | 21 | 1.93 | 27 | 34 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | aliased | 0.35 | 18.33 | 0.150 | 1.22 | 2.35 | 1.0 | 20 | 1.81 | 20 | 28 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | blurred | 0.90 | 17.54 | 0.190 | 1.20 | 2.56 | 1.0 | 19 | 1.75 | 0 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | noisy | 1.62 | 6.05 | 0.119 | 1.22 | 2.32 | 1.0 | 22 | 2.01 | 29 | 19 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | scanned | 14.75 | 20.30 | 0.170 | 1.49 | 2.55 | 1.0 | 21 | 1.90 | 4 | 3 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| thin lines | clean | 0.44 | 6.05 | 0.109 | 0.98 | 76.81 | 1.0 | 34 | 1.72 | 2 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | aliased | 0.74 | 16.37 | 0.099 | 1.17 | 55.97 | 1.0 | 22 | 1.10 | 6 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | blurred | 1.72 | 18.80 | 0.254 | 0.81 | 106.00 | 1.0 | 18 | 0.90 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | noisy | 1.02 | 6.25 | 0.112 | 1.00 | 37.89 | 1.0 | 25 | 1.25 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | scanned | 28.94 | 46.83 | 12.147 | 43.11 | 20.79 | 1.0 | 2763 | 138.15 | 2 | – | – | – | – | – | – | 0.0 | 0.1 |

## 5. Cost

One trace at a time on this machine (all cores); memory is the peak allocated above the image itself. A trace of more than 2,000,000 anchors is refused, as in the app.

| preset | image | megapixels | result |
|:--|:--|--:|:--|
| Default | photo-like | 1 | 0.05 s, 29 paths, 7947 anchors, 15 MB |
| Default | four-colour badge | 1 | 0.03 s, 3 paths, 106 anchors, 15 MB |
| High Fidelity Photo | photo-like | 1 | 0.25 s, 968 paths, 113205 anchors, 238 MB |
| High Fidelity Photo | four-colour badge | 1 | 0.06 s, 6 paths, 192 anchors, 34 MB |
| Low Fidelity Photo | photo-like | 1 | 0.13 s, 129 paths, 22788 anchors, 141 MB |
| Low Fidelity Photo | four-colour badge | 1 | 0.05 s, 6 paths, 144 anchors, 36 MB |
| 3 Colors | photo-like | 1 | 0.07 s, 9 paths, 3391 anchors, 26 MB |
| 3 Colors | four-colour badge | 1 | 0.05 s, 4 paths, 79 anchors, 21 MB |
| 6 Colors | photo-like | 1 | 0.08 s, 78 paths, 9239 anchors, 42 MB |
| 6 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 89 anchors, 33 MB |
| 16 Colors | photo-like | 1 | 0.11 s, 178 paths, 21069 anchors, 102 MB |
| 16 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 92 anchors, 33 MB |
| Shades of Gray | photo-like | 1 | 0.09 s, 82 paths, 8511 anchors, 65 MB |
| Shades of Gray | four-colour badge | 1 | 0.06 s, 10 paths, 780 anchors, 56 MB |
| Black and White Logo | photo-like | 1 | 0.05 s, 71 paths, 9067 anchors, 9 MB |
| Black and White Logo | four-colour badge | 1 | 0.02 s, 1 paths, 58 anchors, 8 MB |
| Sketched Art | photo-like | 1 | 0.05 s, 5 paths, 3207 anchors, 9 MB |
| Sketched Art | four-colour badge | 1 | 0.02 s, 2 paths, 145 anchors, 8 MB |
| Silhouettes | photo-like | 1 | 0.04 s, 2 paths, 851 anchors, 11 MB |
| Silhouettes | four-colour badge | 1 | 0.02 s, 1 paths, 29 anchors, 9 MB |
| Line Art | photo-like | 1 | 0.05 s, 54 paths, 4751 anchors, 9 MB |
| Line Art | four-colour badge | 1 | 0.02 s, 1 paths, 243 anchors, 8 MB |
| Technical Drawing | photo-like | 1 | 0.06 s, 221 paths, 10646 anchors, 9 MB |
| Technical Drawing | four-colour badge | 1 | 0.02 s, 1 paths, 58 anchors, 8 MB |
| Flat Logo | photo-like | 1 | refused after 0.09 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 1 | 0.64 s, 5 paths, 78 anchors, 146 MB |
| Default | photo-like | 4 | 0.19 s, 48 paths, 20717 anchors, 60 MB |
| Default | four-colour badge | 4 | 0.10 s, 3 paths, 148 anchors, 64 MB |
| High Fidelity Photo | photo-like | 4 | 0.88 s, 4475 paths, 344985 anchors, 964 MB |
| High Fidelity Photo | four-colour badge | 4 | 0.22 s, 6 paths, 216 anchors, 152 MB |
| Low Fidelity Photo | photo-like | 4 | 0.38 s, 304 paths, 61608 anchors, 592 MB |
| Low Fidelity Photo | four-colour badge | 4 | 0.23 s, 6 paths, 169 anchors, 152 MB |
| 3 Colors | photo-like | 4 | 0.28 s, 20 paths, 8647 anchors, 104 MB |
| 3 Colors | four-colour badge | 4 | 0.18 s, 4 paths, 122 anchors, 84 MB |
| 6 Colors | photo-like | 4 | 0.27 s, 115 paths, 24075 anchors, 165 MB |
| 6 Colors | four-colour badge | 4 | 0.19 s, 5 paths, 116 anchors, 128 MB |
| 16 Colors | photo-like | 4 | 0.33 s, 353 paths, 54215 anchors, 380 MB |
| 16 Colors | four-colour badge | 4 | 0.18 s, 5 paths, 124 anchors, 128 MB |
| Shades of Gray | photo-like | 4 | 0.29 s, 137 paths, 22704 anchors, 232 MB |
| Shades of Gray | four-colour badge | 4 | 0.27 s, 10 paths, 1234 anchors, 220 MB |
| Black and White Logo | photo-like | 4 | 0.21 s, 208 paths, 25916 anchors, 36 MB |
| Black and White Logo | four-colour badge | 4 | 0.08 s, 1 paths, 64 anchors, 36 MB |
| Sketched Art | photo-like | 4 | 0.21 s, 9 paths, 6875 anchors, 36 MB |
| Sketched Art | four-colour badge | 4 | 0.08 s, 2 paths, 279 anchors, 36 MB |
| Silhouettes | photo-like | 4 | 0.18 s, 2 paths, 2133 anchors, 44 MB |
| Silhouettes | four-colour badge | 4 | 0.10 s, 1 paths, 32 anchors, 36 MB |
| Line Art | photo-like | 4 | 0.21 s, 127 paths, 12595 anchors, 36 MB |
| Line Art | four-colour badge | 4 | 0.08 s, 1 paths, 465 anchors, 36 MB |
| Technical Drawing | photo-like | 4 | 0.22 s, 931 paths, 33946 anchors, 36 MB |
| Technical Drawing | four-colour badge | 4 | 0.08 s, 1 paths, 64 anchors, 36 MB |
| Flat Logo | photo-like | 4 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 4 | 0.66 s, 5 paths, 69 anchors, 146 MB |
| Default | photo-like | 16 | 1.08 s, 127 paths, 46199 anchors, 256 MB |
| Default | four-colour badge | 16 | 0.46 s, 3 paths, 284 anchors, 240 MB |
| High Fidelity Photo | photo-like | 16 | 3.48 s, 31131 paths, 1254672 anchors, 3954 MB |
| High Fidelity Photo | four-colour badge | 16 | 1.07 s, 6 paths, 248 anchors, 576 MB |
| Low Fidelity Photo | photo-like | 16 | 1.57 s, 2036 paths, 185091 anchors, 2050 MB |
| Low Fidelity Photo | four-colour badge | 16 | 1.08 s, 6 paths, 188 anchors, 576 MB |
| 3 Colors | photo-like | 16 | 1.27 s, 76 paths, 23365 anchors, 416 MB |
| 3 Colors | four-colour badge | 16 | 0.80 s, 4 paths, 179 anchors, 336 MB |
| 6 Colors | photo-like | 16 | 1.17 s, 470 paths, 63030 anchors, 673 MB |
| 6 Colors | four-colour badge | 16 | 0.84 s, 5 paths, 118 anchors, 512 MB |
| 16 Colors | photo-like | 16 | 1.30 s, 1973 paths, 149717 anchors, 1633 MB |
| 16 Colors | four-colour badge | 16 | 0.84 s, 5 paths, 120 anchors, 504 MB |
| Shades of Gray | photo-like | 16 | 1.32 s, 688 paths, 65052 anchors, 850 MB |
| Shades of Gray | four-colour badge | 16 | 1.28 s, 9 paths, 2296 anchors, 800 MB |
| Black and White Logo | photo-like | 16 | 1.04 s, 1128 paths, 77629 anchors, 144 MB |
| Black and White Logo | four-colour badge | 16 | 0.45 s, 1 paths, 103 anchors, 128 MB |
| Sketched Art | photo-like | 16 | 1.15 s, 15 paths, 17817 anchors, 144 MB |
| Sketched Art | four-colour badge | 16 | 0.48 s, 2 paths, 478 anchors, 128 MB |
| Silhouettes | photo-like | 16 | 1.40 s, 3 paths, 4891 anchors, 176 MB |
| Silhouettes | four-colour badge | 16 | 0.55 s, 1 paths, 44 anchors, 144 MB |
| Line Art | photo-like | 16 | 1.21 s, 713 paths, 64481 anchors, 144 MB |
| Line Art | four-colour badge | 16 | 0.45 s, 1 paths, 786 anchors, 128 MB |
| Technical Drawing | photo-like | 16 | 1.20 s, 4578 paths, 114292 anchors, 144 MB |
| Technical Drawing | four-colour badge | 16 | 0.44 s, 1 paths, 103 anchors, 128 MB |
| Flat Logo | photo-like | 16 | refused after 0.11 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 16 | 0.66 s, 5 paths, 67 anchors, 146 MB |

## Revision history

| date | change |
|:--|:--|
| 2026-10-13 | Photos: continuous-tone images are smoothed within their tones before quantising, and stacked layers go by lightness |
| 2026-10-13 | The palette: made of the picture's own colours, noise-made colours merged, blend pixels settled, cuts where colours differ most, colours compared in Oklab |
| 2026-10-13 | Sub-pixel edges and the outline fitter in Black and White, Grayscale and Color; the harness gains an aliased variant and an "outside px" column, so the baseline was measured again with it (same tracer, main at `baf9693f`) |
| 2026-10-12 | The harness and the baseline: every preset measured before any change to the tracer |
