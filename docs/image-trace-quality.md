# Image Trace quality

> **Last reviewed:** 2026-10-13 · **Last updated:** 2026-10-13 · **Change:** minor (shared edges: gaps and overlaps between abutting paths from 22 to 50 px down to under 1)

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

### Now (sub-pixel edges, the palette, photo smoothing, shared edges)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.04 | 0.092 | 1.02 | 2.17 | 1.9 | 18 | 1.78 | 29 | 68 | 0.4 | – | – | 0.8 | 0.5 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.97 | – | – | – | 120.7 | 10001 | – | 38 | – | – | – | – | 0.0 | – | 10.3 | 0.0 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2118 | – | 41 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.34 | 1.19 | 4.0 | 50 | 3.13 | 38 | 86 | 5.2 | 0.0 | 0 | 0.2 | 0.3 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.90 | 0.141 | 1.31 | 1.19 | 4.2 | 62 | 3.79 | 39 | 86 | 6.4 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 695 | – | 51 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.37 | 0.063 | 1.02 | 2.17 | 1.4 | 27 | 2.69 | 39 | 68 | 0.2 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.12 | 0.197 | 1.00 | 105.53 | 1.8 | 17 | 1.48 | 15 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.22 | 0.523 | 2.23 | 2.21 | 1.2 | 14 | 1.64 | 11 | 75 | 0.2 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.65 | 9.88 | 0.093 | 0.91 | 105.75 | 5.4 | 78 | 3.92 | 36 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 2.55 | 9.72 | 0.092 | 1.06 | 105.75 | 4.8 | 56 | 3.67 | 71 | 74 | 0.7 | – | – | – | – | 1.3 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

The sections below are the numbers now (sub-pixel edges, the palette, photo smoothing, shared edges).

## 1. Each preset on the inputs it is meant for

All sizes and variants (clean, aliased, blurred, noisy, scanned) of those inputs.

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.04 | 0.092 | 1.02 | 2.17 | 1.9 | 18 | 1.78 | 29 | 68 | 0.4 | – | – | 0.8 | 0.5 | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.97 | – | – | – | 120.7 | 10001 | – | 38 | – | – | – | – | 0.0 | – | 10.3 | 0.0 |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2118 | – | 41 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.98 | 0.140 | 1.34 | 1.19 | 4.0 | 50 | 3.13 | 38 | 86 | 5.2 | 0.0 | 0 | 0.2 | 0.3 | 0.0 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.90 | 0.141 | 1.31 | 1.19 | 4.2 | 62 | 3.79 | 39 | 86 | 6.4 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.5 | 695 | – | 51 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.58 | 9.37 | 0.063 | 1.02 | 2.17 | 1.4 | 27 | 2.69 | 39 | 68 | 0.2 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.12 | 0.197 | 1.00 | 105.53 | 1.8 | 17 | 1.48 | 15 | 86 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.22 | 0.523 | 2.23 | 2.21 | 1.2 | 14 | 1.64 | 11 | 75 | 0.2 | – | – | – | – | 0.0 | 0.0 |
| Line Art | lines | 2.65 | 9.88 | 0.093 | 0.91 | 105.75 | 5.4 | 78 | 3.92 | 36 | – | – | – | – | – | – | 0.1 | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 2.55 | 9.72 | 0.092 | 1.06 | 105.75 | 4.8 | 56 | 3.67 | 71 | 74 | 0.7 | – | – | – | – | 1.3 | 0.0 |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 |

## 2. Each preset on each kind of input (clean, three sizes)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | curved shapes | 0.09 | 3.51 | 0.062 | 0.64 | 0.78 | 1.8 | 15 | 1.64 | 13 | 83 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | polygons | 0.05 | 2.13 | 0.036 | 0.44 | 1.24 | 2.0 | 14 | 1.26 | 89 | 90 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | thin lines | 0.56 | 4.77 | 0.061 | 0.47 | 105.57 | 3.3 | 24 | 1.18 | 43 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | flat colour art | 19.95 | 19.00 | 0.199 | 0.53 | 179.52 | 1.5 | 19 | 1.18 | 14 | 0 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | gradient, photo | 39.14 | 38.78 | – | – | – | 3.2 | 593 | – | 55 | – | – | – | – | 18.6 | 30.6 | 0.0 | 0.0 |
| High Fidelity Photo | curved shapes | 0.04 | 1.66 | 0.026 | 0.18 | 0.20 | 1.2 | 19 | 2.12 | 20 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | polygons | 0.05 | 1.88 | 0.031 | 0.51 | 1.34 | 1.7 | 16 | 1.43 | 88 | 89 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | thin lines | 0.34 | 5.96 | 0.075 | 0.85 | 4.31 | 40.3 | 339 | 16.93 | 37 | – | – | 27.3 | 4490 | 0.0 | – | 19.0 | 0.0 |
| High Fidelity Photo | flat colour art | 0.14 | 4.16 | 0.030 | 0.20 | 0.20 | 3.5 | 73 | 4.62 | 35 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| High Fidelity Photo | gradient, photo | 1.80 | 2.04 | – | – | – | 134.7 | 11321 | – | 39 | – | – | – | – | 0.0 | – | 7.3 | 0.0 |
| Low Fidelity Photo | curved shapes | 0.06 | 2.65 | 0.047 | 0.31 | 0.31 | 1.2 | 13 | 1.47 | 17 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | polygons | 0.05 | 2.15 | 0.036 | 0.51 | 1.34 | 1.7 | 14 | 1.22 | 88 | 89 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | thin lines | 0.51 | 5.69 | 0.083 | 0.86 | 45.41 | 7.0 | 56 | 2.80 | 47 | – | – | 4.0 | 3966 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | flat colour art | 0.15 | 4.29 | 0.045 | 0.28 | 0.28 | 3.5 | 54 | 3.38 | 31 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | gradient, photo | 2.53 | 2.80 | – | – | – | 42.7 | 2282 | – | 41 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | curved shapes | 0.06 | 2.65 | 0.047 | 0.31 | 0.31 | 1.8 | 16 | 1.70 | 15 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | polygons | 0.05 | 2.15 | 0.036 | 0.51 | 1.34 | 2.0 | 15 | 1.32 | 88 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | thin lines | 0.45 | 4.38 | 0.113 | 0.90 | 45.41 | 5.0 | 44 | 2.18 | 50 | – | – | 2.0 | 290 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | flat colour art | 1.31 | 3.61 | 0.125 | 0.90 | 60.25 | 2.5 | 25 | 1.86 | 45 | 100 | 0.0 | 1.0 | 9914 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | gradient, photo | 7.49 | 7.60 | – | – | – | 5.7 | 530 | – | 47 | – | – | – | – | 6.3 | 7.4 | 0.0 | 0.0 |
| 6 Colors | curved shapes | 0.06 | 2.45 | 0.043 | 0.27 | 0.28 | 1.8 | 17 | 1.78 | 15 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | polygons | 0.05 | 2.08 | 0.035 | 0.51 | 1.34 | 2.0 | 15 | 1.33 | 88 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | thin lines | 0.50 | 4.49 | 0.102 | 0.86 | 45.41 | 4.0 | 33 | 1.65 | 53 | – | – | 1.7 | 396 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | flat colour art | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 832 | – | 36 | – | – | – | – | 6.1 | 8.2 | 0.0 | 0.0 |
| 16 Colors | curved shapes | 0.05 | 2.21 | 0.038 | 0.24 | 0.25 | 1.8 | 17 | 1.85 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | polygons | 0.05 | 1.97 | 0.032 | 0.51 | 1.34 | 2.0 | 16 | 1.39 | 88 | 89 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | thin lines | 0.41 | 3.77 | 0.110 | 0.86 | 23.65 | 8.7 | 50 | 2.52 | 52 | – | – | 5.7 | 305 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | flat colour art | 0.12 | 2.52 | 0.052 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | gradient, photo | 3.00 | 3.30 | – | – | – | 47.2 | 2132 | – | 27 | – | – | – | – | 12.3 | 18.0 | 0.0 | 0.0 |
| Shades of Gray | curved shapes | 0.06 | 2.67 | 0.047 | 0.31 | 0.31 | 1.2 | 14 | 1.49 | 17 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | polygons | 0.05 | 2.08 | 0.035 | 0.47 | 1.25 | 1.7 | 14 | 1.22 | 88 | 90 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | thin lines | 0.55 | 6.55 | 0.083 | 0.86 | 45.40 | 10.3 | 73 | 3.67 | 50 | – | – | 8.0 | 4272 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | flat colour art | 7.33 | 11.24 | 0.257 | 1.43 | 179.39 | 4.7 | 200 | 18.45 | 22 | 50 | 0.3 | 3.7 | 173367 | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | gradient, photo | 9.71 | 9.55 | – | – | – | 18.2 | 748 | – | 51 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Black and White Logo | curved shapes | 0.04 | 1.51 | 0.024 | 0.50 | 0.59 | 1.2 | 21 | 2.29 | 20 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | polygons | 0.04 | 1.83 | 0.030 | 0.43 | 1.24 | 1.7 | 16 | 1.39 | 90 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | thin lines | 0.60 | 6.10 | 0.111 | 0.50 | 77.21 | 10.3 | 48 | 2.38 | 83 | – | – | – | – | – | – | 3.3 | 0.0 |
| Black and White Logo | flat colour art | 19.94 | 18.71 | 0.192 | 0.32 | 179.52 | 1.0 | 20 | 1.22 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | gradient, photo | 39.11 | 38.71 | – | – | – | 9.0 | 574 | – | 67 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | curved shapes | 0.14 | 5.77 | 0.112 | 0.47 | 0.47 | 1.2 | 12 | 1.38 | 16 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | polygons | 0.12 | 4.81 | 0.088 | 0.52 | 1.05 | 1.7 | 13 | 1.18 | 85 | 88 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | thin lines | 0.40 | 5.10 | 0.106 | 0.80 | 52.93 | 4.3 | 39 | 1.93 | 35 | – | – | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | flat colour art | 20.32 | 21.13 | 0.179 | 0.87 | 179.64 | 1.5 | 24 | 1.43 | 16 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | gradient, photo | 45.54 | 45.39 | – | – | – | 1.0 | 237 | – | 43 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | curved shapes | 0.36 | 14.54 | 0.282 | 0.71 | 0.71 | 1.2 | 12 | 1.34 | 15 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | polygons | 0.35 | 13.93 | 0.261 | 0.67 | 0.69 | 1.7 | 14 | 1.26 | 76 | 85 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | thin lines | 1.03 | 15.78 | 0.304 | 1.09 | 18.10 | 5.0 | 47 | 2.33 | 34 | – | – | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | flat colour art | 23.86 | 33.99 | 0.224 | 0.77 | 188.38 | 1.0 | 11 | 0.78 | 24 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | gradient, photo | 61.57 | 61.34 | – | – | – | 0.2 | 2 | – | 42 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | curved shapes | 0.04 | 1.86 | 0.032 | 0.54 | 0.62 | 1.2 | 16 | 1.75 | 19 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | polygons | 0.05 | 1.90 | 0.031 | 0.40 | 1.24 | 1.7 | 14 | 1.26 | 90 | 90 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | thin lines | 0.54 | 4.39 | 0.049 | 0.47 | 105.75 | 3.3 | 35 | 1.77 | 41 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | flat colour art | 19.94 | 18.73 | 0.193 | 0.50 | 179.52 | 1.0 | 43 | 2.40 | 22 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Line Art | gradient, photo | 39.12 | 38.72 | – | – | – | 7.7 | 448 | – | 57 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | curved shapes | 0.04 | 1.51 | 0.024 | 0.50 | 0.59 | 1.2 | 21 | 2.29 | 20 | 83 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | polygons | 0.04 | 1.83 | 0.030 | 0.43 | 1.24 | 1.7 | 16 | 1.38 | 91 | 91 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | thin lines | 0.53 | 5.83 | 0.125 | 0.50 | 55.75 | 15.3 | 61 | 3.03 | 88 | – | – | – | – | – | – | 8.3 | 0.0 |
| Technical Drawing | flat colour art | 19.94 | 18.71 | 0.192 | 0.32 | 179.52 | 1.0 | 20 | 1.22 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | gradient, photo | 39.08 | 38.68 | – | – | – | 20.0 | 593 | – | 68 | – | – | – | – | – | – | 2.7 | 0.0 |
| Flat Logo | curved shapes | 0.06 | 2.14 | 0.036 | 0.25 | 0.25 | 1.8 | 31 | 3.38 | 19 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | polygons | 0.09 | 3.44 | 0.060 | 0.35 | 1.03 | 2.0 | 35 | 3.17 | 67 | 94 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | thin lines | 1.17 | 11.78 | 0.050 | 0.49 | all | 3.0 | 189 | 9.45 | 28 | – | – | 0.0 | 0 | 5467.1 | – | 0.0 | 0.0 |
| Flat Logo | flat colour art | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 |
| Flat Logo | gradient, photo | refused | | | | | | | | | | | | | | | | |

## 3. Each preset on its inputs, by variant (256 px)

| preset | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | clean | 0.07 | 2.92 | 0.051 | 0.64 | 1.24 | 1.9 | 15 | 1.48 | 46 | 89 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | aliased | 0.37 | 20.23 | 0.233 | 1.02 | 1.35 | 1.9 | 15 | 1.45 | 21 | 96 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | blurred | 0.82 | 16.64 | 0.078 | 0.97 | 2.17 | 1.9 | 25 | 2.42 | 5 | 21 | 1.4 | – | – | 0.0 | 0.0 | 0.0 | 0.0 |
| Default | noisy | 1.74 | 4.50 | 0.050 | 0.54 | 1.24 | 1.9 | 16 | 1.64 | 39 | 67 | 0.0 | – | – | 2.3 | 0.6 | 0.0 | 0.0 |
| Default | scanned | 15.06 | 20.19 | 0.132 | 0.86 | 2.17 | 1.9 | 25 | 2.50 | 0 | 25 | 1.1 | – | – | 3.6 | 2.9 | 0.0 | 0.0 |
| High Fidelity Photo | clean | 1.80 | 2.04 | – | – | – | 134.7 | 11321 | – | 39 | – | – | – | – | 0.0 | – | 7.3 | 0.0 |
| High Fidelity Photo | aliased | 1.77 | 2.00 | – | – | – | 73.0 | 8004 | – | 40 | – | – | – | – | 0.0 | – | 5.5 | 0.0 |
| High Fidelity Photo | blurred | 1.59 | 1.95 | – | – | – | 72.0 | 7388 | – | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| High Fidelity Photo | noisy | 2.02 | 2.18 | – | – | – | 155.5 | 10856 | – | 33 | – | – | – | – | 0.0 | – | 18.5 | 0.0 |
| High Fidelity Photo | scanned | 1.45 | 1.55 | – | – | – | 140.5 | 9795 | – | 36 | – | – | – | – | 0.0 | – | 25.5 | 0.0 |
| Low Fidelity Photo | clean | 2.53 | 2.80 | – | – | – | 42.7 | 2282 | – | 41 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | aliased | 2.53 | 2.78 | – | – | – | 36.5 | 1498 | – | 44 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | blurred | 2.35 | 2.75 | – | – | – | 29.0 | 2005 | – | 42 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | noisy | 2.70 | 2.85 | – | – | – | 32.0 | 1926 | – | 41 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Low Fidelity Photo | scanned | 1.95 | 2.04 | – | – | – | 34.5 | 2548 | – | 39 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| 3 Colors | clean | 0.06 | 2.00 | 0.070 | 0.31 | 0.30 | 2.0 | 11 | 1.78 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | aliased | 0.13 | 8.77 | 0.242 | 0.91 | 0.91 | 2.0 | 12 | 2.00 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | blurred | 0.34 | 7.83 | 0.073 | 0.24 | 0.24 | 2.0 | 9 | 1.50 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | noisy | 0.71 | 2.03 | 0.114 | 0.32 | 0.32 | 2.0 | 11 | 1.83 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 3 Colors | scanned | 1.57 | 3.22 | 0.089 | 0.30 | 0.30 | 2.0 | 12 | 2.00 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | clean | 0.12 | 2.70 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 6 Colors | blurred | 0.62 | 9.33 | 0.204 | 1.34 | 1.19 | 6.0 | 98 | 5.46 | 39 | 50 | 16.0 | 0.0 | 0 | 0.6 | 0.0 | 0.0 | 0.0 |
| 6 Colors | noisy | 0.85 | 2.58 | 0.076 | 0.52 | 0.58 | 3.0 | 34 | 2.34 | 28 | 88 | 0.0 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 |
| 6 Colors | scanned | 1.74 | 4.42 | 0.228 | 1.20 | 0.94 | 7.0 | 108 | 6.10 | 28 | 62 | 20.5 | 0.0 | 0 | 0.8 | 1.6 | 0.0 | 0.0 |
| 16 Colors | clean | 0.12 | 2.52 | 0.052 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | aliased | 0.29 | 10.38 | 0.290 | 1.00 | 1.00 | 3.0 | 31 | 2.25 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | blurred | 0.63 | 9.38 | 0.215 | 1.31 | 1.19 | 7.0 | 132 | 7.21 | 42 | 50 | 21.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 |
| 16 Colors | noisy | 0.85 | 2.56 | 0.075 | 0.52 | 0.58 | 3.0 | 35 | 2.45 | 27 | 88 | 0.0 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 |
| 16 Colors | scanned | 1.72 | 4.43 | 0.256 | 1.09 | 0.96 | 7.5 | 151 | 8.31 | 33 | 62 | 24.0 | 0.0 | 0 | 1.2 | 1.2 | 0.0 | 0.0 |
| Shades of Gray | clean | 9.71 | 9.55 | – | – | – | 18.2 | 748 | – | 51 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | aliased | 9.71 | 9.52 | – | – | – | 11.0 | 642 | – | 57 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | blurred | 9.66 | 9.32 | – | – | – | 11.5 | 577 | – | 55 | – | – | – | – | 0.0 | – | 0.5 | 0.0 |
| Shades of Gray | noisy | 9.74 | 9.45 | – | – | – | 19.5 | 643 | – | 45 | – | – | – | – | 0.0 | – | 0.0 | 0.0 |
| Shades of Gray | scanned | 6.58 | 6.46 | – | – | – | 19.0 | 762 | – | 47 | – | – | – | – | 0.0 | – | 1.0 | 0.0 |
| Black and White Logo | clean | 0.04 | 1.64 | 0.027 | 0.50 | 1.24 | 1.4 | 19 | 1.90 | 50 | 89 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | aliased | 0.39 | 20.35 | 0.172 | 1.02 | 1.35 | 1.4 | 22 | 2.32 | 53 | 96 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | blurred | 0.80 | 16.29 | 0.045 | 0.82 | 2.15 | 1.4 | 38 | 3.62 | 21 | 21 | 0.9 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | noisy | 1.72 | 3.90 | 0.028 | 0.51 | 1.24 | 1.4 | 22 | 2.18 | 40 | 67 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Black and White Logo | scanned | 15.05 | 20.10 | 0.115 | 0.86 | 2.17 | 1.4 | 51 | 5.02 | 11 | 25 | 0.9 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | clean | 0.19 | 5.64 | 0.111 | 0.80 | 52.93 | 1.9 | 18 | 1.49 | 19 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | aliased | 0.46 | 19.57 | 0.213 | 1.00 | 105.53 | 1.6 | 11 | 1.10 | 26 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | blurred | 1.07 | 19.72 | 0.274 | 0.70 | 105.49 | 1.6 | 17 | 1.53 | 10 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | noisy | 1.76 | 6.16 | 0.119 | 0.95 | 1.03 | 2.0 | 19 | 1.58 | 12 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Sketched Art | scanned | 15.12 | 22.50 | 0.442 | 0.86 | 105.27 | 1.6 | 20 | 1.65 | 2 | 50 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | clean | 0.36 | 14.54 | 0.282 | 0.71 | 0.71 | 1.2 | 12 | 1.34 | 15 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | aliased | 0.39 | 21.69 | 0.289 | 1.00 | 1.00 | 1.2 | 9 | 0.99 | 12 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | blurred | 1.21 | 28.11 | 0.785 | 1.45 | 1.92 | 1.2 | 15 | 1.82 | 11 | 25 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | noisy | 2.11 | 10.96 | 0.284 | 0.69 | 0.69 | 1.2 | 11 | 1.27 | 7 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Silhouettes | scanned | 16.05 | 30.19 | 1.458 | 2.23 | 2.21 | 1.2 | 25 | 3.37 | 3 | 0 | 1.2 | – | – | – | – | 0.0 | 0.0 |
| Line Art | clean | 0.54 | 4.39 | 0.049 | 0.47 | 105.75 | 3.3 | 35 | 1.77 | 41 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | aliased | 0.67 | 14.44 | 0.112 | 0.73 | 55.44 | 14.0 | 59 | 2.95 | 87 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | blurred | 1.63 | 17.15 | 0.136 | 0.89 | 105.57 | 3.0 | 143 | 7.15 | 4 | – | – | – | – | – | – | 0.0 | 0.0 |
| Line Art | noisy | 1.04 | 4.35 | 0.045 | 0.48 | 56.06 | 8.0 | 53 | 2.65 | 38 | – | – | – | – | – | – | 1.0 | 0.0 |
| Line Art | scanned | 13.60 | 20.06 | 0.214 | 0.91 | 105.64 | 3.0 | 188 | 9.40 | 1 | – | – | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | clean | 0.17 | 2.83 | 0.054 | 0.50 | 55.75 | 5.1 | 27 | 1.79 | 90 | 91 | 0.0 | – | – | – | – | 2.1 | 0.0 |
| Technical Drawing | aliased | 0.43 | 18.51 | 0.204 | 1.02 | 55.44 | 6.8 | 33 | 2.06 | 95 | 94 | 0.0 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | blurred | 1.04 | 16.59 | 0.082 | 1.06 | 105.75 | 2.0 | 114 | 7.50 | 24 | 28 | 2.3 | – | – | – | – | 0.0 | 0.0 |
| Technical Drawing | noisy | 1.42 | 4.30 | 0.055 | 0.50 | 31.23 | 7.8 | 38 | 2.40 | 85 | 90 | 0.0 | – | – | – | – | 2.8 | 0.0 |
| Technical Drawing | scanned | 14.45 | 20.15 | 0.144 | 0.91 | 105.74 | 2.0 | 123 | 8.33 | 23 | 34 | 2.3 | – | – | – | – | 0.0 | 0.0 |
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
| Default | photo-like | 1 | 0.04 s, 29 paths, 6888 anchors, 15 MB |
| Default | four-colour badge | 1 | 0.02 s, 3 paths, 96 anchors, 15 MB |
| High Fidelity Photo | photo-like | 1 | 0.21 s, 968 paths, 114383 anchors, 238 MB |
| High Fidelity Photo | four-colour badge | 1 | 0.06 s, 6 paths, 172 anchors, 36 MB |
| Low Fidelity Photo | photo-like | 1 | 0.11 s, 129 paths, 22929 anchors, 144 MB |
| Low Fidelity Photo | four-colour badge | 1 | 0.05 s, 6 paths, 136 anchors, 36 MB |
| 3 Colors | photo-like | 1 | 0.08 s, 9 paths, 3367 anchors, 26 MB |
| 3 Colors | four-colour badge | 1 | 0.05 s, 4 paths, 88 anchors, 21 MB |
| 6 Colors | photo-like | 1 | 0.08 s, 78 paths, 9008 anchors, 42 MB |
| 6 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 33 MB |
| 16 Colors | photo-like | 1 | 0.10 s, 178 paths, 20466 anchors, 102 MB |
| 16 Colors | four-colour badge | 1 | 0.05 s, 5 paths, 88 anchors, 33 MB |
| Shades of Gray | photo-like | 1 | 0.08 s, 81 paths, 6698 anchors, 65 MB |
| Shades of Gray | four-colour badge | 1 | 0.06 s, 10 paths, 733 anchors, 56 MB |
| Black and White Logo | photo-like | 1 | 0.04 s, 71 paths, 5465 anchors, 9 MB |
| Black and White Logo | four-colour badge | 1 | 0.02 s, 1 paths, 52 anchors, 8 MB |
| Sketched Art | photo-like | 1 | 0.04 s, 5 paths, 2971 anchors, 9 MB |
| Sketched Art | four-colour badge | 1 | 0.02 s, 2 paths, 131 anchors, 8 MB |
| Silhouettes | photo-like | 1 | 0.04 s, 2 paths, 791 anchors, 11 MB |
| Silhouettes | four-colour badge | 1 | 0.02 s, 1 paths, 25 anchors, 9 MB |
| Line Art | photo-like | 1 | 0.04 s, 54 paths, 4421 anchors, 9 MB |
| Line Art | four-colour badge | 1 | 0.02 s, 1 paths, 232 anchors, 8 MB |
| Technical Drawing | photo-like | 1 | 0.04 s, 221 paths, 7680 anchors, 9 MB |
| Technical Drawing | four-colour badge | 1 | 0.02 s, 1 paths, 52 anchors, 8 MB |
| Flat Logo | photo-like | 1 | refused after 0.09 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 1 | 0.64 s, 5 paths, 78 anchors, 146 MB |
| Default | photo-like | 4 | 0.18 s, 48 paths, 17801 anchors, 60 MB |
| Default | four-colour badge | 4 | 0.09 s, 3 paths, 178 anchors, 64 MB |
| High Fidelity Photo | photo-like | 4 | 0.72 s, 4475 paths, 341272 anchors, 981 MB |
| High Fidelity Photo | four-colour badge | 4 | 0.24 s, 6 paths, 200 anchors, 152 MB |
| Low Fidelity Photo | photo-like | 4 | 0.38 s, 304 paths, 61093 anchors, 568 MB |
| Low Fidelity Photo | four-colour badge | 4 | 0.23 s, 6 paths, 152 anchors, 152 MB |
| 3 Colors | photo-like | 4 | 0.28 s, 20 paths, 8453 anchors, 104 MB |
| 3 Colors | four-colour badge | 4 | 0.18 s, 4 paths, 122 anchors, 84 MB |
| 6 Colors | photo-like | 4 | 0.37 s, 115 paths, 23128 anchors, 168 MB |
| 6 Colors | four-colour badge | 4 | 0.19 s, 5 paths, 92 anchors, 128 MB |
| 16 Colors | photo-like | 4 | 0.32 s, 353 paths, 52749 anchors, 408 MB |
| 16 Colors | four-colour badge | 4 | 0.19 s, 5 paths, 96 anchors, 128 MB |
| Shades of Gray | photo-like | 4 | 0.29 s, 138 paths, 18794 anchors, 224 MB |
| Shades of Gray | four-colour badge | 4 | 0.27 s, 10 paths, 1265 anchors, 220 MB |
| Black and White Logo | photo-like | 4 | 0.20 s, 208 paths, 17188 anchors, 36 MB |
| Black and White Logo | four-colour badge | 4 | 0.09 s, 1 paths, 64 anchors, 36 MB |
| Sketched Art | photo-like | 4 | 0.20 s, 9 paths, 6884 anchors, 36 MB |
| Sketched Art | four-colour badge | 4 | 0.09 s, 2 paths, 236 anchors, 36 MB |
| Silhouettes | photo-like | 4 | 0.19 s, 2 paths, 1951 anchors, 44 MB |
| Silhouettes | four-colour badge | 4 | 0.10 s, 1 paths, 32 anchors, 36 MB |
| Line Art | photo-like | 4 | 0.19 s, 127 paths, 12091 anchors, 36 MB |
| Line Art | four-colour badge | 4 | 0.09 s, 1 paths, 419 anchors, 36 MB |
| Technical Drawing | photo-like | 4 | 0.20 s, 931 paths, 23132 anchors, 36 MB |
| Technical Drawing | four-colour badge | 4 | 0.08 s, 1 paths, 64 anchors, 36 MB |
| Flat Logo | photo-like | 4 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 4 | 0.65 s, 5 paths, 69 anchors, 146 MB |
| Default | photo-like | 16 | 1.04 s, 127 paths, 43656 anchors, 256 MB |
| Default | four-colour badge | 16 | 0.47 s, 3 paths, 250 anchors, 240 MB |
| High Fidelity Photo | photo-like | 16 | 2.85 s, 31125 paths, 1175671 anchors, 3856 MB |
| High Fidelity Photo | four-colour badge | 16 | 1.11 s, 6 paths, 274 anchors, 576 MB |
| Low Fidelity Photo | photo-like | 16 | 1.49 s, 2036 paths, 181832 anchors, 1972 MB |
| Low Fidelity Photo | four-colour badge | 16 | 1.07 s, 6 paths, 187 anchors, 576 MB |
| 3 Colors | photo-like | 16 | 1.22 s, 76 paths, 22878 anchors, 416 MB |
| 3 Colors | four-colour badge | 16 | 0.82 s, 4 paths, 190 anchors, 336 MB |
| 6 Colors | photo-like | 16 | 1.13 s, 470 paths, 60539 anchors, 672 MB |
| 6 Colors | four-colour badge | 16 | 0.82 s, 5 paths, 126 anchors, 500 MB |
| 16 Colors | photo-like | 16 | 1.18 s, 1973 paths, 145173 anchors, 1625 MB |
| 16 Colors | four-colour badge | 16 | 0.84 s, 5 paths, 128 anchors, 504 MB |
| Shades of Gray | photo-like | 16 | 1.48 s, 686 paths, 53872 anchors, 852 MB |
| Shades of Gray | four-colour badge | 16 | 1.40 s, 9 paths, 2216 anchors, 800 MB |
| Black and White Logo | photo-like | 16 | 1.15 s, 1128 paths, 52547 anchors, 144 MB |
| Black and White Logo | four-colour badge | 16 | 0.47 s, 1 paths, 95 anchors, 128 MB |
| Sketched Art | photo-like | 16 | 1.16 s, 15 paths, 16298 anchors, 144 MB |
| Sketched Art | four-colour badge | 16 | 0.47 s, 2 paths, 414 anchors, 128 MB |
| Silhouettes | photo-like | 16 | 1.34 s, 3 paths, 4516 anchors, 176 MB |
| Silhouettes | four-colour badge | 16 | 0.53 s, 1 paths, 40 anchors, 144 MB |
| Line Art | photo-like | 16 | 1.04 s, 713 paths, 43982 anchors, 144 MB |
| Line Art | four-colour badge | 16 | 0.45 s, 1 paths, 707 anchors, 128 MB |
| Technical Drawing | photo-like | 16 | 1.21 s, 4578 paths, 84475 anchors, 144 MB |
| Technical Drawing | four-colour badge | 16 | 0.46 s, 1 paths, 95 anchors, 128 MB |
| Flat Logo | photo-like | 16 | refused after 0.11 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 16 | 0.69 s, 5 paths, 67 anchors, 146 MB |

## Revision history

| date | change |
|:--|:--|
| 2026-10-13 | Shared edges: outlines are cut at their junctions and each piece is fitted once, the same from either side |
| 2026-10-13 | Photos: continuous-tone images are smoothed within their tones before quantising, and stacked layers go by lightness |
| 2026-10-13 | The palette: made of the picture's own colours, noise-made colours merged, blend pixels settled, cuts where colours differ most, colours compared in Oklab |
| 2026-10-13 | Sub-pixel edges and the outline fitter in Black and White, Grayscale and Color; the harness gains an aliased variant and an "outside px" column, so the baseline was measured again with it (same tracer, main at `baf9693f`) |
| 2026-10-12 | The harness and the baseline: every preset measured before any change to the tracer |
