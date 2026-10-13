# Image Trace quality

> **Last reviewed:** 2026-10-13 · **Last updated:** 2026-10-13 · **Change:** minor (Snap Curves To Lines levels lines a degree or two askew: 0 to 91-95 percent of their length exactly level or upright)

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
| askew shapes | a rectangle, a plus sign and two lines, each 1 to 2 degrees off level and upright |
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
| upright % | share of the outline length that is exactly level or upright |
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

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.79 | 17.15 | 0.313 | 1.51 | 2.52 | 1.9 | 14 | 1.45 | 34 | 5 | 76 | 1.4 | – | – | 63.5 | 67.0 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 2.30 | 2.63 | – | – | – | 3652.6 | 46866 | – | 84 | 29 | – | – | – | – | 0.0 | – | 1169.4 | 0.3 |
| Low Fidelity Photo | gradient, photo-like | 2.61 | 2.94 | – | – | – | 128.3 | 3867 | – | 42 | 22 | – | – | – | – | 0.0 | – | 0.0 | 0.7 |
| 3 Colors | two touching halves | 4.93 | 11.38 | 0.482 | 2.35 | 89.09 | 2.1 | 14 | 2.31 | 12 | 8 | 29 | 2.1 | 1.6 | 15418 | 118.0 | 87.3 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.78 | 8.82 | 4.383 | 83.10 | 30.11 | 21.2 | 270 | 30.18 | 51 | 40 | 98 | 213.8 | 2.4 | 272 | 198.8 | 194.6 | 0.0 | 0.3 |
| 16 Colors | two touching halves, four-colour badge | 0.74 | 9.02 | 5.115 | 86.21 | 4.32 | 69.9 | 923 | 102.80 | 58 | 47 | 100 | 765.9 | 5.9 | 670 | 293.7 | 329.9 | 0.0 | 0.5 |
| Shades of Gray | gradient, photo-like | 9.29 | 9.16 | – | – | – | 90.8 | 1921 | – | 42 | 27 | – | – | – | – | 0.0 | – | 0.2 | 0.5 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.76 | 15.96 | 0.270 | 1.76 | 2.53 | 1.4 | 31 | 3.78 | 90 | 17 | 81 | 15.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.85 | 17.04 | 0.302 | 1.63 | 105.63 | 1.8 | 14 | 1.34 | 28 | 8 | 18 | 1.4 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 3.04 | 21.41 | 0.573 | 2.88 | 2.89 | 1.2 | 10 | 1.16 | 9 | 7 | 86 | 1.4 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.77 | 12.51 | 0.158 | 1.13 | 105.76 | 5.4 | 25 | 1.27 | 90 | 6 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.73 | 13.67 | 0.210 | 1.43 | 105.76 | 4.7 | 31 | 2.01 | 100 | 9 | 85 | 5.9 | – | – | – | – | 0.1 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

### Now (all changes up to snapping near-level lines)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.02 | 0.090 | 1.02 | 2.22 | 1.9 | 17 | 1.74 | 29 | 5 | 69 | 0.4 | – | – | 0.5 | 0.6 | 0.0 | 0.0 |
| Auto Color | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.33 | 1.21 | 4.0 | 53 | 3.28 | 38 | 14 | 86 | 5.1 | 0.0 | 0 | 1.4 | 1.1 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.97 | – | – | – | 120.7 | 10001 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.3 | 0.0 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2118 | – | 41 | 36 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 6 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.34 | 1.19 | 4.0 | 50 | 3.13 | 38 | 14 | 86 | 5.2 | 0.0 | 0 | 0.2 | 0.3 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.90 | 0.141 | 1.31 | 1.19 | 4.2 | 62 | 3.79 | 39 | 14 | 86 | 6.4 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 695 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.31 | 0.059 | 1.02 | 2.22 | 1.4 | 29 | 2.88 | 39 | 11 | 69 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 11.98 | 0.186 | 1.00 | 105.61 | 1.8 | 17 | 1.47 | 16 | 7 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.28 | 0.524 | 2.19 | 2.18 | 1.2 | 14 | 1.65 | 11 | 7 | 79 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.62 | 9.44 | 0.055 | 0.96 | 105.82 | 5.4 | 46 | 2.29 | 41 | 0 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.70 | 11.81 | 0.156 | 4.23 | 105.82 | 4.7 | 47 | 3.03 | 71 | 20 | 73 | 0.8 | – | – | – | – | 0.9 | 0.0 |
| Outline | lines | 2.93 | 15.91 | 0.310 | 1.84 | 106.00 | 5.0 | 81 | 4.03 | 15 | 5 | – | – | – | – | – | – | 0.0 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

The sections below are the numbers now (all changes up to snapping near-level lines).

## 1. Each preset on the inputs it is meant for

All sizes and variants (clean, aliased, blurred, noisy, scanned) of those inputs.

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.02 | 0.090 | 1.02 | 2.22 | 1.9 | 17 | 1.74 | 29 | 5 | 69 | 0.4 | – | – | 0.5 | 0.6 | 0.0 | 0.0 |
| Auto Color | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.33 | 1.21 | 4.0 | 53 | 3.28 | 38 | 14 | 86 | 5.1 | 0.0 | 0 | 1.4 | 1.1 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.97 | – | – | – | 120.7 | 10001 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.3 | 0.0 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2118 | – | 41 | 36 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 6 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.34 | 1.19 | 4.0 | 50 | 3.13 | 38 | 14 | 86 | 5.2 | 0.0 | 0 | 0.2 | 0.3 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.90 | 0.141 | 1.31 | 1.19 | 4.2 | 62 | 3.79 | 39 | 14 | 86 | 6.4 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 695 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.31 | 0.059 | 1.02 | 2.22 | 1.4 | 29 | 2.88 | 39 | 11 | 69 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 11.98 | 0.186 | 1.00 | 105.61 | 1.8 | 17 | 1.47 | 16 | 7 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.28 | 0.524 | 2.19 | 2.18 | 1.2 | 14 | 1.65 | 11 | 7 | 79 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.62 | 9.44 | 0.055 | 0.96 | 105.82 | 5.4 | 46 | 2.29 | 41 | 0 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.70 | 11.81 | 0.156 | 4.23 | 105.82 | 4.7 | 47 | 3.03 | 71 | 20 | 73 | 0.8 | – | – | – | – | 0.9 | 0.0 |
| Outline | lines | 2.93 | 15.91 | 0.310 | 1.84 | 106.00 | 5.0 | 81 | 4.03 | 15 | 5 | – | – | – | – | – | – | 0.0 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

## 2. Each preset on each kind of input (clean, three sizes)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | curved shapes | 0.09 | 3.47 | 0.061 | 0.64 | 0.78 | 1.8 | 15 | 1.62 | 13 | 10 | 83 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | polygons | 0.05 | 2.12 | 0.035 | 0.43 | 1.24 | 2.0 | 14 | 1.26 | 88 | 2 | 89 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | thin lines | 0.53 | 4.28 | 0.045 | 0.47 | 105.51 | 3.3 | 23 | 1.15 | 48 | 0 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | askew shapes | 0.17 | 3.58 | 0.050 | 0.50 | 36.07 | 3.7 | 29 | 1.19 | 86 | 0 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | flat colour art | 19.91 | 17.97 | 0.172 | 0.51 | 179.34 | 1.5 | 16 | 1.17 | 14 | 14 | 0 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | gradient, photo | 39.03 | 38.65 | – | – | – | 3.2 | 601 | – | 54 | 41 | – | – | – | – | 19.1 | 38.4 | 0.0 | 0.0 |
| Auto Color | curved shapes | 0.06 | 2.45 | 0.043 | 0.27 | 0.28 | 1.8 | 17 | 1.78 | 15 | 12 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | polygons | 0.05 | 2.09 | 0.035 | 0.51 | 1.34 | 2.0 | 15 | 1.33 | 88 | 2 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | thin lines | 0.69 | 5.23 | 0.057 | 0.48 | 105.59 | 5.0 | 33 | 1.63 | 57 | 2 | – | – | 4.0 | 3216 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | askew shapes | 0.65 | 4.02 | 0.052 | 0.53 | 9.49 | 4.0 | 32 | 1.35 | 83 | 0 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | flat colour art | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | gradient, photo | 2.07 | 2.38 | – | – | – | 159.3 | 4430 | – | 19 | 8 | – | – | – | – | 22.7 | 32.4 | 0.0 | 0.0 |
| High Fidelity Photo | curved shapes | 0.04 | 1.66 | 0.026 | 0.18 | 0.20 | 1.2 | 19 | 2.12 | 20 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | polygons | 0.05 | 1.88 | 0.031 | 0.51 | 1.34 | 1.7 | 16 | 1.43 | 88 | 3 | 89 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | thin lines | 0.34 | 5.96 | 0.075 | 0.85 | 4.31 | 40.3 | 339 | 16.93 | 37 | 4 | – | – | 27.3 | 4490 | 0.0 | – | 19.0 | 0.0 |
| High Fidelity Photo | askew shapes | 0.17 | 4.07 | 0.102 | 0.94 | 1.73 | 8.7 | 182 | 7.58 | 57 | 3 | – | – | 4.3 | 3998 | 0.0 | – | 1.0 | 0.0 |
| High Fidelity Photo | flat colour art | 0.14 | 4.16 | 0.030 | 0.20 | 0.20 | 3.5 | 73 | 4.62 | 35 | 24 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo | 1.80 | 2.04 | – | – | – | 134.7 | 11321 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 |
| Low Fidelity Photo | curved shapes | 0.06 | 2.65 | 0.047 | 0.31 | 0.31 | 1.2 | 13 | 1.47 | 17 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | polygons | 0.05 | 2.15 | 0.036 | 0.51 | 1.34 | 1.7 | 14 | 1.22 | 88 | 3 | 89 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | thin lines | 0.51 | 5.69 | 0.083 | 0.86 | 45.41 | 7.0 | 56 | 2.80 | 47 | 4 | – | – | 4.0 | 3966 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | askew shapes | 0.25 | 5.00 | 0.064 | 0.70 | 21.54 | 6.0 | 49 | 2.06 | 70 | 3 | – | – | 2.3 | 2635 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | flat colour art | 0.15 | 4.29 | 0.045 | 0.28 | 0.28 | 3.5 | 54 | 3.38 | 31 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | gradient, photo | 2.53 | 2.80 | – | – | – | 42.7 | 2282 | – | 41 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | curved shapes | 0.06 | 2.65 | 0.047 | 0.31 | 0.31 | 1.8 | 16 | 1.70 | 15 | 12 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | polygons | 0.05 | 2.15 | 0.036 | 0.51 | 1.34 | 2.0 | 15 | 1.32 | 88 | 2 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | thin lines | 0.45 | 4.38 | 0.113 | 0.90 | 45.41 | 5.0 | 44 | 2.18 | 50 | 6 | – | – | 2.0 | 290 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | askew shapes | 0.20 | 3.95 | 0.060 | 0.67 | 14.60 | 4.0 | 32 | 1.32 | 80 | 0 | – | – | 0.7 | 22 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | flat colour art | 1.31 | 3.61 | 0.125 | 0.90 | 60.25 | 2.5 | 25 | 1.86 | 45 | 21 | 100 | 0.0 | 1.0 | 9914 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | gradient, photo | 7.49 | 7.60 | – | – | – | 5.7 | 530 | – | 47 | 39 | – | – | – | – | 6.3 | 7.4 | 0.0 | 0.0 |
| 6 Colors | curved shapes | 0.06 | 2.45 | 0.043 | 0.27 | 0.28 | 1.8 | 17 | 1.78 | 15 | 12 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | polygons | 0.05 | 2.08 | 0.035 | 0.51 | 1.34 | 2.0 | 15 | 1.33 | 88 | 2 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | thin lines | 0.50 | 4.49 | 0.102 | 0.86 | 45.41 | 4.0 | 33 | 1.65 | 53 | 6 | – | – | 1.7 | 396 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | askew shapes | 0.19 | 3.81 | 0.058 | 0.70 | 36.07 | 4.0 | 33 | 1.39 | 73 | 3 | – | – | 0.3 | 11 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | flat colour art | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 832 | – | 36 | 28 | – | – | – | – | 6.1 | 8.2 | 0.0 | 0.0 |
| 16 Colors | curved shapes | 0.05 | 2.21 | 0.038 | 0.24 | 0.25 | 1.8 | 17 | 1.85 | 16 | 12 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | polygons | 0.05 | 1.97 | 0.032 | 0.51 | 1.34 | 2.0 | 16 | 1.39 | 88 | 2 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | thin lines | 0.41 | 3.77 | 0.110 | 0.86 | 23.65 | 8.7 | 50 | 2.52 | 52 | 6 | – | – | 5.7 | 305 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | askew shapes | 0.18 | 3.73 | 0.062 | 0.70 | 21.54 | 4.3 | 38 | 1.60 | 70 | 3 | – | – | 0.7 | 15 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | flat colour art | 0.12 | 2.52 | 0.052 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | gradient, photo | 3.00 | 3.30 | – | – | – | 47.2 | 2132 | – | 27 | 16 | – | – | – | – | 12.3 | 18.0 | 0.0 | 0.0 |
| Shades of Gray | curved shapes | 0.06 | 2.67 | 0.047 | 0.31 | 0.31 | 1.2 | 14 | 1.49 | 17 | 15 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | polygons | 0.05 | 2.08 | 0.035 | 0.47 | 1.25 | 1.7 | 14 | 1.22 | 88 | 3 | 90 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | thin lines | 0.55 | 6.55 | 0.083 | 0.86 | 45.40 | 10.3 | 73 | 3.67 | 50 | 4 | – | – | 8.0 | 4272 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | askew shapes | 0.25 | 5.03 | 0.057 | 0.73 | 21.56 | 6.0 | 46 | 1.93 | 78 | 0 | – | – | 2.3 | 2636 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | flat colour art | 7.33 | 11.24 | 0.257 | 1.43 | 179.39 | 4.7 | 200 | 18.45 | 22 | 21 | 50 | 0.3 | 3.7 | 173367 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | gradient, photo | 9.71 | 9.55 | – | – | – | 18.2 | 748 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | curved shapes | 0.03 | 1.49 | 0.024 | 0.51 | 0.59 | 1.2 | 21 | 2.23 | 20 | 20 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | polygons | 0.04 | 1.75 | 0.028 | 0.42 | 1.24 | 1.7 | 16 | 1.42 | 89 | 7 | 89 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | thin lines | 0.63 | 6.34 | 0.118 | 0.50 | 77.21 | 10.3 | 53 | 2.63 | 89 | 7 | – | – | – | – | – | – | 2.3 | 0.0 |
| Black and White Logo | askew shapes | 0.79 | 18.38 | 0.489 | 4.23 | 9.63 | 4.0 | 47 | 1.97 | 92 | 91 | – | – | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | flat colour art | 19.90 | 17.76 | 0.157 | 0.30 | 179.51 | 1.0 | 18 | 1.12 | 23 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | gradient, photo | 38.98 | 38.54 | – | – | – | 9.0 | 573 | – | 70 | 51 | – | – | – | – | – | – | 1.2 | 0.0 |
| Sketched Art | curved shapes | 0.14 | 5.67 | 0.109 | 0.46 | 0.46 | 1.2 | 12 | 1.38 | 16 | 11 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | polygons | 0.12 | 4.65 | 0.084 | 0.53 | 1.06 | 1.7 | 13 | 1.19 | 86 | 3 | 89 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | thin lines | 0.41 | 4.87 | 0.099 | 0.96 | 52.90 | 4.3 | 35 | 1.77 | 38 | 10 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | askew shapes | 0.30 | 5.59 | 0.078 | 0.46 | 35.95 | 3.3 | 29 | 1.21 | 81 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | flat colour art | 20.25 | 19.55 | 0.146 | 1.15 | 179.63 | 1.5 | 26 | 1.53 | 16 | 16 | 0 | 0.3 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | gradient, photo | 45.43 | 45.21 | – | – | – | 1.0 | 240 | – | 44 | 35 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | curved shapes | 0.35 | 14.47 | 0.280 | 0.69 | 0.69 | 1.2 | 12 | 1.33 | 15 | 10 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | polygons | 0.34 | 13.60 | 0.255 | 0.65 | 0.69 | 1.7 | 14 | 1.22 | 77 | 3 | 85 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | thin lines | 1.10 | 15.03 | 0.296 | 1.37 | 17.91 | 5.0 | 37 | 1.83 | 44 | 6 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | askew shapes | 0.58 | 14.76 | 0.292 | 0.56 | 0.56 | 4.0 | 27 | 1.11 | 86 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | flat colour art | 23.83 | 33.06 | 0.200 | 0.69 | 188.27 | 1.0 | 11 | 0.77 | 25 | 25 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | gradient, photo | 61.57 | 61.32 | – | – | – | 0.2 | 2 | – | 47 | 21 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | curved shapes | 0.04 | 1.85 | 0.032 | 0.54 | 0.62 | 1.2 | 16 | 1.76 | 19 | 15 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | polygons | 0.05 | 1.86 | 0.030 | 0.39 | 1.24 | 1.7 | 15 | 1.29 | 88 | 3 | 89 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | thin lines | 0.52 | 3.94 | 0.036 | 0.47 | 105.71 | 3.3 | 28 | 1.38 | 45 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | askew shapes | 0.14 | 3.35 | 0.048 | 0.54 | 9.60 | 4.0 | 34 | 1.40 | 84 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | flat colour art | 19.90 | 17.83 | 0.162 | 0.42 | 179.43 | 1.0 | 18 | 1.30 | 21 | 21 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | gradient, photo | 38.98 | 38.54 | – | – | – | 7.7 | 471 | – | 57 | 44 | – | – | – | – | – | – | 0.2 | 0.0 |
| Technical Drawing | curved shapes | 0.03 | 1.49 | 0.024 | 0.51 | 0.59 | 1.2 | 21 | 2.23 | 20 | 20 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | polygons | 0.04 | 1.76 | 0.028 | 0.42 | 1.24 | 1.7 | 16 | 1.41 | 90 | 7 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | thin lines | 0.53 | 5.81 | 0.121 | 0.50 | 55.69 | 15.3 | 70 | 3.50 | 88 | 7 | – | – | – | – | – | – | 7.3 | 0.0 |
| Technical Drawing | askew shapes | 0.87 | 20.26 | 0.587 | 4.23 | 4.27 | 4.3 | 35 | 1.47 | 95 | 95 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | flat colour art | 19.90 | 17.76 | 0.157 | 0.30 | 179.51 | 1.0 | 18 | 1.12 | 23 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | gradient, photo | 38.94 | 38.51 | – | – | – | 20.0 | 627 | – | 71 | 49 | – | – | – | – | – | – | 11.3 | 0.0 |
| Outline | curved shapes | 23.38 | 44.70 | 0.498 | 0.90 | 0.52 | 1.2 | 16 | 1.76 | 19 | 15 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Outline | polygons | 17.94 | 44.20 | 0.494 | 0.83 | 0.74 | 1.7 | 15 | 1.29 | 88 | 3 | 89 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Outline | thin lines | 0.99 | 14.15 | 0.310 | 1.82 | 106.00 | 3.3 | 88 | 4.40 | 25 | 7 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | askew shapes | 23.75 | 38.13 | 0.467 | 1.62 | 9.82 | 4.0 | 32 | 1.35 | 83 | 7 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | flat colour art | 26.69 | 32.18 | 0.494 | 0.92 | 178.93 | 1.0 | 18 | 1.30 | 21 | 21 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Outline | gradient, photo | 39.57 | 39.32 | – | – | – | 7.5 | 421 | – | 56 | 43 | – | – | – | – | – | – | 0.0 | 0.8 |
| Flat Logo | curved shapes | 0.06 | 2.14 | 0.036 | 0.25 | 0.25 | 1.8 | 31 | 3.38 | 19 | 6 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | polygons | 0.09 | 3.44 | 0.060 | 0.35 | 1.03 | 2.0 | 35 | 3.17 | 67 | 0 | 94 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | thin lines | 1.17 | 11.78 | 0.050 | 0.49 | all | 3.0 | 189 | 9.45 | 28 | 9 | – | – | 0.0 | 0 | 5467.1 | – | 0.0 | 0.0 |
| Flat Logo | askew shapes | 0.15 | 3.78 | 0.058 | 0.43 | 0.50 | 4.0 | 114 | 4.76 | 39 | 0 | – | – | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | flat colour art | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 11 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | gradient, photo | refused | | | | | | | | | | | | | | | | | |

## 3. Each preset on its inputs, by variant (256 px)

| preset | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | clean | 0.07 | 2.89 | 0.050 | 0.64 | 1.24 | 1.9 | 15 | 1.46 | 45 | 7 | 88 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | aliased | 0.37 | 20.23 | 0.233 | 1.02 | 1.35 | 1.9 | 15 | 1.45 | 21 | 8 | 96 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | blurred | 0.81 | 16.57 | 0.078 | 0.98 | 2.22 | 1.9 | 24 | 2.34 | 5 | 5 | 19 | 1.3 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | noisy | 1.74 | 4.53 | 0.054 | 0.54 | 1.32 | 1.9 | 15 | 1.47 | 41 | 0 | 84 | 0.0 | – | – | 1.9 | 0.3 | 0.0 | 0.0 |
| Default | scanned | 15.06 | 20.17 | 0.114 | 0.79 | 2.14 | 1.9 | 26 | 2.53 | 2 | 0 | 25 | 1.3 | – | – | 1.6 | 4.2 | 0.0 | 0.0 |
| Auto Color | clean | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | blurred | 0.61 | 9.31 | 0.208 | 1.33 | 1.21 | 6.0 | 119 | 6.53 | 39 | 11 | 50 | 15.5 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| Auto Color | noisy | 0.85 | 2.61 | 0.076 | 0.52 | 0.58 | 3.0 | 34 | 2.34 | 28 | 0 | 88 | 0.0 | 0.0 | 0 | 0.3 | 0.5 | 0.0 | 0.0 |
| Auto Color | scanned | 1.73 | 4.42 | 0.230 | 1.20 | 0.94 | 7.0 | 108 | 6.02 | 28 | 0 | 62 | 20.5 | 0.0 | 0 | 9.6 | 7.5 | 0.0 | 0.0 |
| High Fidelity Photo | clean | 1.80 | 2.04 | – | – | – | 134.7 | 11321 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 |
| High Fidelity Photo | aliased | 1.77 | 2.00 | – | – | – | 73.0 | 8004 | – | 40 | 38 | – | – | – | – | 0.0 | – | 5.5 | 0.0 |
| High Fidelity Photo | blurred | 1.59 | 1.95 | – | – | – | 72.0 | 7388 | – | 40 | 39 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| High Fidelity Photo | noisy | 2.02 | 2.18 | – | – | – | 155.5 | 10856 | – | 33 | 31 | – | – | – | – | 0.0 | – | 18.5 | 0.0 |
| High Fidelity Photo | scanned | 1.45 | 1.55 | – | – | – | 140.5 | 9795 | – | 36 | 34 | – | – | – | – | 0.0 | – | 25.5 | 0.0 |
| Low Fidelity Photo | clean | 2.53 | 2.80 | – | – | – | 42.7 | 2282 | – | 41 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | aliased | 2.53 | 2.78 | – | – | – | 36.5 | 1498 | – | 44 | 38 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | blurred | 2.35 | 2.75 | – | – | – | 29.0 | 2005 | – | 42 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | noisy | 2.70 | 2.85 | – | – | – | 32.0 | 1926 | – | 41 | 38 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | scanned | 1.95 | 2.04 | – | – | – | 34.5 | 2548 | – | 39 | 32 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | clean | 0.06 | 2.00 | 0.070 | 0.31 | 0.30 | 2.0 | 11 | 1.78 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | aliased | 0.13 | 8.77 | 0.242 | 0.91 | 0.91 | 2.0 | 12 | 2.00 | 39 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | blurred | 0.34 | 7.83 | 0.073 | 0.24 | 0.24 | 2.0 | 9 | 1.50 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | noisy | 0.71 | 2.03 | 0.114 | 0.32 | 0.32 | 2.0 | 11 | 1.83 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | scanned | 1.57 | 3.22 | 0.089 | 0.30 | 0.30 | 2.0 | 12 | 2.00 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | clean | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | blurred | 0.62 | 9.33 | 0.204 | 1.34 | 1.19 | 6.0 | 98 | 5.46 | 39 | 11 | 50 | 16.0 | 0.0 | 0 | 0.6 | 0.0 | 0.0 | 0.0 |
| 6 Colors | noisy | 0.85 | 2.58 | 0.076 | 0.52 | 0.58 | 3.0 | 34 | 2.34 | 28 | 0 | 88 | 0.0 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 |
| 6 Colors | scanned | 1.74 | 4.42 | 0.228 | 1.20 | 0.94 | 7.0 | 108 | 6.10 | 28 | 0 | 62 | 20.5 | 0.0 | 0 | 0.8 | 1.6 | 0.0 | 0.0 |
| 16 Colors | clean | 0.12 | 2.52 | 0.052 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 14 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | aliased | 0.29 | 10.38 | 0.290 | 1.00 | 1.00 | 3.0 | 31 | 2.25 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | blurred | 0.63 | 9.38 | 0.215 | 1.31 | 1.19 | 7.0 | 132 | 7.21 | 42 | 13 | 50 | 21.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | noisy | 0.85 | 2.56 | 0.075 | 0.52 | 0.58 | 3.0 | 35 | 2.45 | 27 | 0 | 88 | 0.0 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| 16 Colors | scanned | 1.72 | 4.43 | 0.256 | 1.09 | 0.96 | 7.5 | 151 | 8.31 | 33 | 0 | 62 | 24.0 | 0.0 | 0 | 1.2 | 1.2 | 0.0 | 0.0 |
| Shades of Gray | clean | 9.71 | 9.55 | – | – | – | 18.2 | 748 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | aliased | 9.71 | 9.52 | – | – | – | 11.0 | 642 | – | 57 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | blurred | 9.66 | 9.32 | – | – | – | 11.5 | 577 | – | 55 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | noisy | 9.74 | 9.45 | – | – | – | 19.5 | 643 | – | 45 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | scanned | 6.58 | 6.46 | – | – | – | 19.0 | 762 | – | 47 | 37 | – | – | – | – | 0.0 | – | 1.0 | 0.0 |
| Black and White Logo | clean | 0.04 | 1.60 | 0.026 | 0.51 | 1.24 | 1.4 | 19 | 1.89 | 50 | 14 | 88 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | aliased | 0.38 | 20.25 | 0.172 | 1.02 | 1.35 | 1.4 | 22 | 2.32 | 53 | 14 | 96 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | blurred | 0.80 | 16.24 | 0.048 | 0.84 | 2.22 | 1.4 | 40 | 3.92 | 21 | 13 | 19 | 1.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | noisy | 1.72 | 3.86 | 0.028 | 0.49 | 1.32 | 1.4 | 21 | 2.12 | 42 | 5 | 84 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | scanned | 15.05 | 20.00 | 0.090 | 0.79 | 2.10 | 1.4 | 63 | 6.11 | 9 | 1 | 25 | 1.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | clean | 0.19 | 5.51 | 0.107 | 0.96 | 52.90 | 1.9 | 17 | 1.46 | 20 | 11 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | aliased | 0.47 | 19.58 | 0.213 | 1.00 | 105.54 | 1.6 | 11 | 1.10 | 26 | 9 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | blurred | 1.05 | 19.32 | 0.242 | 0.76 | 105.55 | 1.6 | 19 | 1.59 | 9 | 7 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | noisy | 1.76 | 6.13 | 0.116 | 0.90 | 1.06 | 2.0 | 18 | 1.48 | 12 | 0 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | scanned | 15.10 | 22.32 | 0.407 | 0.78 | 105.61 | 1.6 | 21 | 1.75 | 2 | 0 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | clean | 0.35 | 14.47 | 0.280 | 0.69 | 0.69 | 1.2 | 12 | 1.33 | 15 | 10 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | aliased | 0.39 | 21.69 | 0.289 | 1.00 | 1.00 | 1.2 | 9 | 0.99 | 12 | 9 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | blurred | 1.24 | 28.75 | 0.818 | 1.44 | 1.92 | 1.2 | 14 | 1.72 | 10 | 10 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | noisy | 2.11 | 11.05 | 0.284 | 0.68 | 0.69 | 1.2 | 11 | 1.28 | 9 | 0 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | scanned | 16.03 | 30.07 | 1.438 | 2.19 | 2.18 | 1.2 | 28 | 3.55 | 3 | 0 | 0 | 2.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | clean | 0.52 | 3.94 | 0.036 | 0.47 | 105.71 | 3.3 | 28 | 1.38 | 45 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | aliased | 0.67 | 14.44 | 0.112 | 0.73 | 55.44 | 14.0 | 59 | 2.95 | 87 | 1 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | blurred | 1.56 | 16.20 | 0.068 | 0.96 | 105.82 | 3.0 | 53 | 2.65 | 19 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | noisy | 1.02 | 3.98 | 0.030 | 0.50 | 56.08 | 8.0 | 56 | 2.80 | 42 | 0 | – | – | – | – | – | – | 1.0 | 0.0 |
| Line Art | scanned | 13.55 | 19.60 | 0.069 | 0.87 | 105.78 | 3.0 | 70 | 3.50 | 4 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | clean | 0.31 | 6.27 | 0.159 | 4.23 | 55.69 | 4.9 | 31 | 1.84 | 90 | 25 | 90 | 0.0 | – | – | – | – | 1.5 | 0.0 |
| Technical Drawing | aliased | 0.56 | 20.39 | 0.275 | 2.42 | 55.44 | 6.2 | 31 | 1.85 | 96 | 28 | 94 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | blurred | 1.07 | 16.65 | 0.083 | 1.11 | 105.82 | 2.4 | 64 | 4.53 | 24 | 6 | 25 | 2.7 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | noisy | 1.67 | 6.57 | 0.149 | 2.14 | 31.79 | 7.0 | 38 | 2.21 | 88 | 26 | 86 | 0.0 | – | – | – | – | 2.2 | 0.0 |
| Technical Drawing | scanned | 14.70 | 20.26 | 0.110 | 1.13 | 105.79 | 2.4 | 105 | 7.09 | 21 | 4 | 34 | 2.7 | – | – | – | – | 0.0 | 0.0 |
| Outline | clean | 0.99 | 14.15 | 0.310 | 1.82 | 106.00 | 3.3 | 88 | 4.40 | 25 | 7 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | aliased | 0.79 | 17.03 | 0.300 | 1.08 | 55.51 | 11.0 | 93 | 4.65 | 14 | 10 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | blurred | 1.66 | 18.46 | 0.311 | 1.84 | 105.72 | 3.0 | 68 | 3.40 | 0 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | noisy | 1.50 | 12.92 | 0.304 | 1.30 | 56.25 | 8.0 | 74 | 3.70 | 11 | 4 | – | – | – | – | – | – | 0.0 | 0.0 |
| Outline | scanned | 13.60 | 20.48 | 0.323 | 1.79 | 105.55 | 3.0 | 65 | 3.25 | 9 | 1 | – | – | – | – | – | – | 0.0 | 0.0 |
| Flat Logo | clean | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 11 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | aliased | 0.13 | 5.87 | 0.218 | 0.65 | 0.65 | 3.0 | 31 | 1.96 | 44 | 19 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 |
| Flat Logo | blurred | 0.54 | 8.33 | 0.052 | 0.62 | 0.78 | 3.0 | 28 | 1.78 | 43 | 16 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 |
| Flat Logo | noisy | 0.81 | 1.77 | 0.076 | 0.39 | 0.32 | 3.5 | 44 | 2.63 | 31 | 0 | 100 | 1.0 | 0.0 | 0 | 6.9 | – | 0.5 | 0.0 |
| Flat Logo | scanned | 1.31 | 4.15 | 21.204 | 88.50 | 0.66 | 470.5 | 2251 | 255.53 | 11 | 0 | 62 | 1658.0 | 0.0 | 0 | 58.0 | – | 413.5 | 0.2 |

## 4. Yardstick

Inkscape 1.4.2's Trace Bitmap, run as a black box through its `object-trace` action (two scans, background
removed, its other settings as they come) on the same one-colour drawings, and its SVG output measured by
the same code. Compare with the black-and-white presets on the same kinds of input in section 2 and 3.

| inputs | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| curved shapes | clean | 0.25 | 11.05 | 0.198 | 0.97 | 0.97 | 1.0 | 15 | 1.98 | 11 | 7 | 83 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | aliased | 0.38 | 20.79 | 0.192 | 0.88 | 0.88 | 1.0 | 16 | 1.88 | 14 | 8 | 50 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | blurred | 0.79 | 16.57 | 0.182 | 0.86 | 1.14 | 1.0 | 15 | 1.84 | 6 | 5 | 0 | 1.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | noisy | 2.01 | 8.14 | 0.191 | 0.93 | 0.93 | 1.0 | 17 | 2.06 | 10 | 0 | 75 | 2.8 | – | – | – | – | 0.0 | 0.0 |
| curved shapes | scanned | 15.31 | 20.22 | 0.191 | 0.97 | 1.14 | 1.0 | 16 | 1.98 | 7 | 2 | 0 | 2.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | clean | 0.16 | 6.45 | 0.124 | 1.26 | 2.36 | 1.0 | 21 | 1.93 | 27 | 4 | 34 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | aliased | 0.35 | 18.33 | 0.150 | 1.22 | 2.35 | 1.0 | 20 | 1.81 | 20 | 3 | 28 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | blurred | 0.90 | 17.54 | 0.190 | 1.20 | 2.56 | 1.0 | 19 | 1.75 | 0 | 0 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | noisy | 1.62 | 6.05 | 0.119 | 1.22 | 2.32 | 1.0 | 22 | 2.01 | 29 | 4 | 19 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| polygons | scanned | 14.75 | 20.30 | 0.170 | 1.49 | 2.55 | 1.0 | 21 | 1.90 | 4 | 0 | 3 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| thin lines | clean | 0.44 | 6.05 | 0.109 | 0.98 | 76.81 | 1.0 | 34 | 1.72 | 2 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | aliased | 0.74 | 16.37 | 0.099 | 1.17 | 55.97 | 1.0 | 22 | 1.10 | 6 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | blurred | 1.72 | 18.80 | 0.254 | 0.81 | 106.00 | 1.0 | 18 | 0.90 | 0 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | noisy | 1.02 | 6.25 | 0.112 | 1.00 | 37.89 | 1.0 | 25 | 1.25 | 0 | 0 | – | – | – | – | – | – | 0.0 | 0.0 |
| thin lines | scanned | 28.94 | 46.83 | 12.147 | 43.11 | 20.79 | 1.0 | 2763 | 138.15 | 2 | 0 | – | – | – | – | – | – | 0.0 | 0.1 |
| askew shapes | clean | 0.31 | 7.76 | 0.142 | 1.08 | 3.38 | 1.0 | 45 | 1.89 | 32 | 15 | – | – | – | – | – | – | 0.0 | 0.0 |
| askew shapes | aliased | 0.45 | 14.06 | 0.149 | 1.14 | 1.35 | 1.0 | 47 | 1.96 | 30 | 4 | – | – | – | – | – | – | 0.0 | 0.0 |
| askew shapes | blurred | 1.19 | 17.05 | 0.195 | 1.20 | 3.65 | 1.0 | 44 | 1.83 | 29 | 11 | – | – | – | – | – | – | 0.0 | 0.0 |
| askew shapes | noisy | 2.03 | 6.74 | 0.116 | 0.85 | 1.04 | 1.0 | 62 | 2.58 | 37 | 7 | – | – | – | – | – | – | 0.0 | 0.0 |
| askew shapes | scanned | 15.69 | 20.78 | 0.217 | 1.47 | 3.65 | 1.0 | 44 | 1.83 | 26 | 8 | – | – | – | – | – | – | 0.0 | 0.0 |

## 5. Cost

One trace at a time on this machine (all cores); memory is the peak allocated above the image itself. A trace of more than 2,000,000 anchors is refused, as in the app.

| preset | image | megapixels | result |
|:--|:--|--:|:--|
| Default | photo-like | 1 | 0.04 s, 29 paths, 6701 anchors, 12 MB |
| Default | four-colour badge | 1 | 0.02 s, 3 paths, 64 anchors, 12 MB |
| Auto Color | photo-like | 1 | 0.16 s, 732 paths, 45410 anchors, 21 MB |
| Auto Color | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 12 MB |
| High Fidelity Photo | photo-like | 1 | 0.23 s, 968 paths, 114383 anchors, 168 MB |
| High Fidelity Photo | four-colour badge | 1 | 0.06 s, 6 paths, 172 anchors, 23 MB |
| Low Fidelity Photo | photo-like | 1 | 0.12 s, 129 paths, 22929 anchors, 105 MB |
| Low Fidelity Photo | four-colour badge | 1 | 0.07 s, 6 paths, 136 anchors, 22 MB |
| 3 Colors | photo-like | 1 | 0.09 s, 9 paths, 3367 anchors, 18 MB |
| 3 Colors | four-colour badge | 1 | 0.05 s, 4 paths, 88 anchors, 13 MB |
| 6 Colors | photo-like | 1 | 0.10 s, 78 paths, 9008 anchors, 23 MB |
| 6 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 12 MB |
| 16 Colors | photo-like | 1 | 0.11 s, 178 paths, 20466 anchors, 32 MB |
| 16 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 88 anchors, 12 MB |
| Shades of Gray | photo-like | 1 | 0.09 s, 81 paths, 6698 anchors, 46 MB |
| Shades of Gray | four-colour badge | 1 | 0.06 s, 10 paths, 733 anchors, 34 MB |
| Black and White Logo | photo-like | 1 | 0.04 s, 71 paths, 5047 anchors, 8 MB |
| Black and White Logo | four-colour badge | 1 | 0.02 s, 1 paths, 47 anchors, 6 MB |
| Sketched Art | photo-like | 1 | 0.04 s, 5 paths, 2880 anchors, 8 MB |
| Sketched Art | four-colour badge | 1 | 0.02 s, 2 paths, 114 anchors, 6 MB |
| Silhouettes | photo-like | 1 | 0.03 s, 2 paths, 573 anchors, 9 MB |
| Silhouettes | four-colour badge | 1 | 0.02 s, 1 paths, 24 anchors, 6 MB |
| Line Art | photo-like | 1 | 0.04 s, 54 paths, 4579 anchors, 8 MB |
| Line Art | four-colour badge | 1 | 0.02 s, 1 paths, 38 anchors, 6 MB |
| Technical Drawing | photo-like | 1 | 0.04 s, 221 paths, 6443 anchors, 8 MB |
| Technical Drawing | four-colour badge | 1 | 0.02 s, 1 paths, 47 anchors, 6 MB |
| Outline | photo-like | 1 | 0.05 s, 45 paths, 3875 anchors, 16 MB |
| Outline | four-colour badge | 1 | 0.03 s, 1 paths, 38 anchors, 11 MB |
| Flat Logo | photo-like | 1 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 1 | 0.65 s, 5 paths, 78 anchors, 146 MB |
| Default | photo-like | 4 | 0.23 s, 48 paths, 15720 anchors, 49 MB |
| Default | four-colour badge | 4 | 0.09 s, 3 paths, 72 anchors, 48 MB |
| Auto Color | photo-like | 4 | 0.44 s, 1469 paths, 148161 anchors, 111 MB |
| Auto Color | four-colour badge | 4 | 0.19 s, 5 paths, 92 anchors, 47 MB |
| High Fidelity Photo | photo-like | 4 | 0.82 s, 4475 paths, 341272 anchors, 620 MB |
| High Fidelity Photo | four-colour badge | 4 | 0.28 s, 6 paths, 200 anchors, 96 MB |
| Low Fidelity Photo | photo-like | 4 | 0.55 s, 304 paths, 61093 anchors, 402 MB |
| Low Fidelity Photo | four-colour badge | 4 | 0.29 s, 6 paths, 152 anchors, 97 MB |
| 3 Colors | photo-like | 4 | 0.40 s, 20 paths, 8453 anchors, 72 MB |
| 3 Colors | four-colour badge | 4 | 0.23 s, 4 paths, 122 anchors, 53 MB |
| 6 Colors | photo-like | 4 | 0.35 s, 115 paths, 23128 anchors, 100 MB |
| 6 Colors | four-colour badge | 4 | 0.21 s, 5 paths, 92 anchors, 47 MB |
| 16 Colors | photo-like | 4 | 0.37 s, 353 paths, 52749 anchors, 132 MB |
| 16 Colors | four-colour badge | 4 | 0.19 s, 5 paths, 96 anchors, 46 MB |
| Shades of Gray | photo-like | 4 | 0.29 s, 138 paths, 18794 anchors, 160 MB |
| Shades of Gray | four-colour badge | 4 | 0.32 s, 10 paths, 1265 anchors, 137 MB |
| Black and White Logo | photo-like | 4 | 0.25 s, 208 paths, 13717 anchors, 32 MB |
| Black and White Logo | four-colour badge | 4 | 0.09 s, 1 paths, 57 anchors, 24 MB |
| Sketched Art | photo-like | 4 | 0.24 s, 9 paths, 5358 anchors, 32 MB |
| Sketched Art | four-colour badge | 4 | 0.09 s, 2 paths, 196 anchors, 24 MB |
| Silhouettes | photo-like | 4 | 0.24 s, 2 paths, 1457 anchors, 36 MB |
| Silhouettes | four-colour badge | 4 | 0.13 s, 1 paths, 28 anchors, 24 MB |
| Line Art | photo-like | 4 | 0.35 s, 127 paths, 11461 anchors, 32 MB |
| Line Art | four-colour badge | 4 | 0.14 s, 1 paths, 42 anchors, 24 MB |
| Technical Drawing | photo-like | 4 | 0.35 s, 931 paths, 21061 anchors, 32 MB |
| Technical Drawing | four-colour badge | 4 | 0.12 s, 1 paths, 57 anchors, 24 MB |
| Outline | photo-like | 4 | 0.40 s, 113 paths, 10455 anchors, 64 MB |
| Outline | four-colour badge | 4 | 0.16 s, 1 paths, 42 anchors, 44 MB |
| Flat Logo | photo-like | 4 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 4 | 0.67 s, 5 paths, 69 anchors, 146 MB |
| Default | photo-like | 16 | 1.10 s, 127 paths, 34766 anchors, 199 MB |
| Default | four-colour badge | 16 | 0.50 s, 3 paths, 88 anchors, 184 MB |
| Auto Color | photo-like | 16 | 1.63 s, 5391 paths, 449395 anchors, 283 MB |
| Auto Color | four-colour badge | 16 | 0.82 s, 5 paths, 126 anchors, 184 MB |
| High Fidelity Photo | photo-like | 16 | 3.92 s, 31125 paths, 1175671 anchors, 880 MB |
| High Fidelity Photo | four-colour badge | 16 | 1.07 s, 6 paths, 274 anchors, 257 MB |
| Low Fidelity Photo | photo-like | 16 | 1.80 s, 2036 paths, 181832 anchors, 754 MB |
| Low Fidelity Photo | four-colour badge | 16 | 1.07 s, 6 paths, 187 anchors, 257 MB |
| 3 Colors | photo-like | 16 | 1.21 s, 76 paths, 22878 anchors, 290 MB |
| 3 Colors | four-colour badge | 16 | 0.84 s, 4 paths, 190 anchors, 205 MB |
| 6 Colors | photo-like | 16 | 1.16 s, 470 paths, 60539 anchors, 298 MB |
| 6 Colors | four-colour badge | 16 | 0.85 s, 5 paths, 126 anchors, 184 MB |
| 16 Colors | photo-like | 16 | 1.20 s, 1973 paths, 145173 anchors, 303 MB |
| 16 Colors | four-colour badge | 16 | 0.91 s, 5 paths, 128 anchors, 184 MB |
| Shades of Gray | photo-like | 16 | 1.69 s, 686 paths, 53872 anchors, 407 MB |
| Shades of Gray | four-colour badge | 16 | 1.41 s, 9 paths, 2216 anchors, 270 MB |
| Black and White Logo | photo-like | 16 | 1.17 s, 1128 paths, 45117 anchors, 129 MB |
| Black and White Logo | four-colour badge | 16 | 0.44 s, 1 paths, 69 anchors, 88 MB |
| Sketched Art | photo-like | 16 | 1.19 s, 15 paths, 11568 anchors, 128 MB |
| Sketched Art | four-colour badge | 16 | 0.41 s, 2 paths, 361 anchors, 88 MB |
| Silhouettes | photo-like | 16 | 1.41 s, 3 paths, 3358 anchors, 144 MB |
| Silhouettes | four-colour badge | 16 | 0.45 s, 1 paths, 32 anchors, 95 MB |
| Line Art | photo-like | 16 | 1.16 s, 713 paths, 36212 anchors, 129 MB |
| Line Art | four-colour badge | 16 | 0.39 s, 1 paths, 49 anchors, 88 MB |
| Technical Drawing | photo-like | 16 | 1.26 s, 4578 paths, 79049 anchors, 130 MB |
| Technical Drawing | four-colour badge | 16 | 0.37 s, 1 paths, 69 anchors, 88 MB |
| Outline | photo-like | 16 | 1.29 s, 632 paths, 31715 anchors, 256 MB |
| Outline | four-colour badge | 16 | 0.60 s, 1 paths, 49 anchors, 176 MB |
| Flat Logo | photo-like | 16 | refused after 0.14 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 16 | 0.87 s, 5 paths, 67 anchors, 146 MB |

## Revision history

| date | change |
|:--|:--|
| 2026-10-13 | Snap Curves To Lines levels lines within 2.5 degrees of level or upright; askew inputs and an "upright %" column, so the baseline was measured again with them (the tracer of main at `baf9693f`) |
| 2026-10-13 | Ignore Color; the Auto Color and Outline presets, measured from here on (Outline is the first measure of Create: Strokes) |
| 2026-10-13 | Thin lines: black and white read against the image's darkest black and the paper near each edge |
| 2026-10-13 | Memory: layers traced within the box of their pixels and a few at a time (16 megapixel photo: 3.9 GB to 0.9 GB) |
| 2026-10-13 | Shared edges: outlines are cut at their junctions and each piece is fitted once, the same from either side |
| 2026-10-13 | Photos: continuous-tone images are smoothed within their tones before quantising, and stacked layers go by lightness |
| 2026-10-13 | The palette: made of the picture's own colours, noise-made colours merged, blend pixels settled, cuts where colours differ most, colours compared in Oklab |
| 2026-10-13 | Sub-pixel edges and the outline fitter in Black and White, Grayscale and Color; the harness gains an aliased variant and an "outside px" column, so the baseline was measured again with it (same tracer, main at `baf9693f`) |
| 2026-10-12 | The harness and the baseline: every preset measured before any change to the tracer |
