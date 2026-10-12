# Image Trace quality

> **Last reviewed:** 2026-10-12 · **Last updated:** 2026-10-12 · **Change:** new (the harness and the baseline numbers)

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
noise, and as a scan of ink on cream paper with uneven light and grain):

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

Black-and-white presets trace black: on the scanned variant their ΔE includes the paper's and ink's own
colours. `–` means the measure does not apply; `all` that nothing was traced.

**These are the baseline numbers:** measured on main at `baf9693f`, before any change to the tracer.

## 1. Each preset on the inputs it is meant for

All sizes and variants (clean, blurred, noisy, scanned) of those inputs.

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 3.18 | 16.26 | 0.317 | 1.51 | 2.52 | 1.9 | 14 | 1.47 | 33 | 72 | 1.5 | – | – | 65.2 | 68.3 | 0.0 |
| High Fidelity Photo | gradient, photo-like | 2.29 | 2.61 | – | – | – | 3810.7 | 48993 | – | 83 | – | – | – | – | 0.0 | – | 1245.8 |
| Low Fidelity Photo | gradient, photo-like | 2.61 | 2.93 | – | – | – | 128.8 | 4013 | – | 42 | – | – | – | – | 0.0 | – | 0.0 |
| 3 Colors | two touching halves | 5.72 | 11.69 | 0.517 | 2.35 | 89.09 | 2.2 | 14 | 2.42 | 7 | 17 | 2.5 | 1.8 | 17987 | 118.3 | 93.3 | 0.0 |
| 6 Colors | two touching halves, four-colour badge | 0.86 | 8.60 | 5.062 | 83.10 | 30.11 | 24.2 | 310 | 34.87 | 53 | 98 | 249.0 | 2.8 | 318 | 213.4 | 206.7 | 0.0 |
| 16 Colors | two touching halves, four-colour badge | 0.81 | 8.70 | 5.914 | 86.21 | 4.32 | 81.1 | 1071 | 119.54 | 61 | 100 | 893.2 | 6.9 | 782 | 324.1 | 365.3 | 0.0 |
| Shades of Gray | gradient, photo-like | 9.22 | 9.08 | – | – | – | 92.6 | 1990 | – | 41 | – | – | – | – | 0.0 | – | 0.2 |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 3.15 | 14.98 | 0.276 | 1.76 | 2.53 | 1.4 | 32 | 3.80 | 90 | 79 | 15.2 | – | – | – | – | 0.0 |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 3.23 | 16.14 | 0.306 | 1.63 | 105.61 | 1.8 | 14 | 1.34 | 28 | 21 | 1.4 | – | – | – | – | 0.0 |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 3.46 | 20.78 | 0.611 | 2.88 | 2.89 | 1.2 | 10 | 1.18 | 9 | 83 | 1.4 | – | – | – | – | 0.0 |
| Line Art | lines | 3.12 | 12.24 | 0.161 | 1.13 | 105.76 | 4.0 | 20 | 0.98 | 89 | – | – | – | – | – | – | 0.0 |
| Technical Drawing | polygons, star, letter A, lines | 3.04 | 12.82 | 0.203 | 1.37 | 105.76 | 4.5 | 31 | 2.19 | 100 | 83 | 6.4 | – | – | – | – | 0.2 |
| Flat Logo | two touching halves, four-colour badge | 0.48 | 3.14 | 3.588 | 88.50 | 0.78 | 81.0 | 405 | 45.08 | 37 | 85 | 276.5 | 0.0 | 0 | 15.2 | – | 69.0 |

## 2. Each preset on each kind of input (clean, three sizes)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | curved shapes | 0.38 | 16.26 | 0.310 | 1.51 | 1.08 | 1.8 | 13 | 1.54 | 11 | 100 | 1.6 | – | – | 91.2 | 104.1 | 0.0 |
| Default | polygons | 0.37 | 14.70 | 0.281 | 1.34 | 1.68 | 2.0 | 14 | 1.26 | 79 | 91 | 0.3 | – | – | 14.1 | 19.4 | 0.0 |
| Default | thin lines | 0.80 | 10.48 | 0.180 | 1.09 | 105.61 | 3.3 | 15 | 0.73 | 90 | – | – | – | – | 89.4 | 26.0 | 0.0 |
| Default | flat colour art | 20.03 | 22.55 | 0.396 | 1.23 | 178.71 | 1.5 | 15 | 1.08 | 7 | 0 | 2.0 | – | – | 130.9 | 133.5 | 0.0 |
| Default | gradient, photo | 39.23 | 38.90 | – | – | – | 3.2 | 350 | – | 65 | – | – | – | – | 83.6 | 29.9 | 0.0 |
| High Fidelity Photo | curved shapes | 0.30 | 13.87 | 0.387 | 1.34 | 1.34 | 9.9 | 65 | 8.17 | 41 | 100 | 21.9 | 8.7 | 296 | 0.0 | – | 3.6 |
| High Fidelity Photo | polygons | 0.42 | 15.98 | 0.364 | 1.34 | 3.10 | 3.3 | 26 | 2.44 | 96 | 61 | 7.2 | 1.7 | 78 | 0.0 | – | 0.0 |
| High Fidelity Photo | thin lines | 1.11 | 17.32 | 0.341 | 0.97 | 45.35 | 13.0 | 49 | 2.43 | 89 | – | – | 6.7 | 532 | 0.0 | – | 3.0 |
| High Fidelity Photo | flat colour art | 0.21 | 6.75 | 0.420 | 1.33 | 1.33 | 48.2 | 219 | 17.76 | 66 | 100 | 108.7 | 23.2 | 1111 | 0.0 | – | 15.0 |
| High Fidelity Photo | gradient, photo | 2.38 | 2.72 | – | – | – | 2936.7 | 47288 | – | 87 | – | – | – | – | 0.0 | – | 898.7 |
| Low Fidelity Photo | curved shapes | 0.39 | 16.85 | 0.440 | 1.77 | 1.77 | 5.0 | 31 | 3.38 | 30 | 100 | 8.1 | 3.8 | 268 | 0.0 | – | 0.0 |
| Low Fidelity Photo | polygons | 0.51 | 18.23 | 0.411 | 1.54 | 3.10 | 3.3 | 23 | 2.13 | 71 | 55 | 4.8 | 1.7 | 79 | 0.0 | – | 0.0 |
| Low Fidelity Photo | thin lines | 1.13 | 17.25 | 0.363 | 1.03 | 45.35 | 5.0 | 24 | 1.22 | 100 | – | – | 1.3 | 498 | 0.0 | – | 0.0 |
| Low Fidelity Photo | flat colour art | 0.27 | 8.33 | 0.462 | 1.87 | 1.87 | 20.0 | 97 | 6.23 | 64 | 100 | 48.8 | 7.0 | 1033 | 0.0 | – | 0.0 |
| Low Fidelity Photo | gradient, photo | 2.70 | 3.06 | – | – | – | 96.2 | 4529 | – | 47 | – | – | – | – | 0.0 | – | 0.0 |
| 3 Colors | curved shapes | 0.80 | 18.57 | 0.400 | 1.65 | 1.54 | 2.8 | 21 | 2.27 | 21 | 100 | 4.8 | 1.0 | 107 | 103.0 | 93.3 | 0.0 |
| 3 Colors | polygons | 0.93 | 16.07 | 0.315 | 1.75 | 2.43 | 2.4 | 17 | 1.59 | 71 | 60 | 2.0 | 0.4 | 23 | 38.3 | 35.3 | 0.0 |
| 3 Colors | thin lines | 0.96 | 12.66 | 0.238 | 0.95 | 45.30 | 5.0 | 22 | 1.08 | 96 | – | – | 3.7 | 1109 | 54.9 | 65.7 | 0.0 |
| 3 Colors | flat colour art | 3.98 | 12.80 | 0.371 | 1.11 | 89.09 | 2.3 | 24 | 1.70 | 30 | 67 | 2.2 | 1.5 | 15185 | 126.7 | 107.6 | 0.0 |
| 3 Colors | gradient, photo | 7.87 | 7.94 | – | – | – | 4.3 | 222 | – | 51 | – | – | – | – | 101.7 | 66.6 | 0.0 |
| 6 Colors | curved shapes | 0.46 | 17.68 | 0.428 | 1.78 | 1.39 | 3.6 | 28 | 3.07 | 26 | 100 | 6.2 | 1.8 | 169 | 101.5 | 165.6 | 0.0 |
| 6 Colors | polygons | 0.45 | 16.94 | 0.369 | 1.56 | 2.44 | 3.1 | 22 | 2.08 | 61 | 48 | 3.4 | 1.1 | 51 | 82.6 | 36.0 | 0.0 |
| 6 Colors | thin lines | 1.18 | 16.03 | 0.314 | 0.95 | 45.35 | 4.0 | 18 | 0.88 | 78 | – | – | 1.7 | 458 | 68.8 | 20.0 | 0.0 |
| 6 Colors | flat colour art | 0.44 | 10.13 | 0.421 | 1.61 | 1.12 | 5.0 | 35 | 2.66 | 56 | 100 | 8.3 | 1.5 | 198 | 165.2 | 178.1 | 0.0 |
| 6 Colors | gradient, photo | 5.55 | 5.80 | – | – | – | 15.5 | 649 | – | 52 | – | – | – | – | 137.7 | 92.2 | 0.0 |
| 16 Colors | curved shapes | 0.48 | 17.72 | 0.431 | 1.40 | 1.39 | 4.2 | 32 | 3.68 | 30 | 100 | 8.4 | 2.5 | 170 | 101.1 | 166.8 | 0.0 |
| 16 Colors | polygons | 0.47 | 17.47 | 0.379 | 1.46 | 3.10 | 3.1 | 24 | 2.18 | 80 | 56 | 5.9 | 1.1 | 51 | 58.9 | 34.0 | 0.0 |
| 16 Colors | thin lines | 1.13 | 17.11 | 0.340 | 1.13 | 45.35 | 4.0 | 20 | 1.00 | 90 | – | – | 1.3 | 365 | 34.1 | 25.2 | 0.0 |
| 16 Colors | flat colour art | 0.37 | 9.54 | 0.453 | 1.35 | 1.26 | 7.5 | 57 | 4.29 | 61 | 100 | 13.0 | 3.0 | 382 | 206.1 | 288.5 | 0.0 |
| 16 Colors | gradient, photo | 3.30 | 3.71 | – | – | – | 72.0 | 2665 | – | 49 | – | – | – | – | 385.4 | 245.8 | 0.0 |
| Shades of Gray | curved shapes | 0.32 | 14.15 | 0.381 | 1.78 | 1.78 | 5.1 | 33 | 3.66 | 31 | 100 | 9.8 | 3.8 | 275 | 0.0 | – | 0.0 |
| Shades of Gray | polygons | 0.44 | 16.95 | 0.387 | 1.44 | 2.33 | 3.3 | 27 | 2.46 | 82 | 57 | 7.7 | 1.7 | 79 | 0.0 | – | 0.0 |
| Shades of Gray | thin lines | 1.11 | 16.92 | 0.354 | 1.03 | 45.35 | 5.0 | 25 | 1.23 | 100 | – | – | 1.3 | 499 | 0.0 | – | 0.0 |
| Shades of Gray | flat colour art | 7.36 | 13.20 | 0.394 | 1.60 | 178.73 | 13.3 | 66 | 4.35 | 41 | 50 | 29.3 | 12.7 | 38774 | 0.0 | – | 0.0 |
| Shades of Gray | gradient, photo | 9.74 | 9.66 | – | – | – | 82.2 | 1950 | – | 46 | – | – | – | – | 0.0 | – | 0.0 |
| Black and White Logo | curved shapes | 0.43 | 16.76 | 0.339 | 1.76 | 1.76 | 1.2 | 39 | 5.15 | 84 | 100 | 21.1 | – | – | – | – | 0.0 |
| Black and White Logo | polygons | 0.27 | 10.59 | 0.202 | 1.34 | 1.68 | 1.7 | 19 | 1.70 | 100 | 95 | 3.0 | – | – | – | – | 0.0 |
| Black and White Logo | thin lines | 0.70 | 8.44 | 0.160 | 0.82 | 77.30 | 10.3 | 45 | 2.23 | 100 | – | – | – | – | – | – | 0.0 |
| Black and White Logo | flat colour art | 19.99 | 20.78 | 0.428 | 1.66 | 178.38 | 1.0 | 40 | 3.92 | 88 | 0 | 24.7 | – | – | – | – | 0.0 |
| Black and White Logo | gradient, photo | 39.20 | 38.88 | – | – | – | 9.0 | 304 | – | 95 | – | – | – | – | – | – | 0.8 |
| Sketched Art | curved shapes | 0.36 | 15.95 | 0.305 | 1.32 | 1.32 | 1.2 | 12 | 1.42 | 11 | 33 | 1.3 | – | – | – | – | 0.0 |
| Sketched Art | polygons | 0.33 | 13.09 | 0.249 | 1.09 | 1.28 | 1.7 | 13 | 1.19 | 83 | 88 | 1.1 | – | – | – | – | 0.0 |
| Sketched Art | thin lines | 0.73 | 9.90 | 0.206 | 0.62 | 52.42 | 4.3 | 19 | 0.95 | 100 | – | – | – | – | – | – | 0.0 |
| Sketched Art | flat colour art | 20.35 | 23.39 | 0.392 | 1.51 | 179.03 | 1.5 | 13 | 0.94 | 7 | 0 | 2.0 | – | – | – | – | 0.0 |
| Sketched Art | gradient, photo | 45.68 | 45.57 | – | – | – | 1.0 | 115 | – | 35 | – | – | – | – | – | – | 0.0 |
| Silhouettes | curved shapes | 0.42 | 17.79 | 0.345 | 1.34 | 1.34 | 1.2 | 10 | 1.17 | 10 | 100 | 1.2 | – | – | – | – | 0.0 |
| Silhouettes | polygons | 0.47 | 17.79 | 0.353 | 1.62 | 2.53 | 1.7 | 12 | 1.10 | 51 | 66 | 0.6 | – | – | – | – | 0.0 |
| Silhouettes | thin lines | 1.20 | 16.32 | 0.316 | 0.89 | 18.29 | 5.0 | 29 | 1.47 | 87 | – | – | – | – | – | – | 0.0 |
| Silhouettes | flat colour art | 23.77 | 31.89 | 0.378 | 1.22 | 187.69 | 1.0 | 8 | 0.65 | 9 | 0 | 1.0 | – | – | – | – | 0.0 |
| Silhouettes | gradient, photo | 61.64 | 61.45 | – | – | – | 0.2 | 1 | – | 18 | – | – | – | – | – | – | 0.0 |
| Line Art | curved shapes | 0.33 | 13.45 | 0.259 | 1.05 | 1.05 | 1.2 | 21 | 2.77 | 22 | 100 | 2.1 | – | – | – | – | 0.0 |
| Line Art | polygons | 0.33 | 12.57 | 0.239 | 1.34 | 1.68 | 1.7 | 16 | 1.44 | 74 | 83 | 0.6 | – | – | – | – | 0.0 |
| Line Art | thin lines | 0.78 | 9.82 | 0.175 | 0.82 | 105.63 | 3.3 | 16 | 0.80 | 89 | – | – | – | – | – | – | 0.0 |
| Line Art | flat colour art | 19.97 | 20.23 | 0.352 | 1.06 | 178.81 | 1.0 | 23 | 1.94 | 21 | 0 | 4.2 | – | – | – | – | 0.0 |
| Line Art | gradient, photo | 39.20 | 38.81 | – | – | – | 7.7 | 260 | – | 65 | – | – | – | – | – | – | 0.0 |
| Technical Drawing | curved shapes | 0.43 | 16.76 | 0.339 | 1.76 | 1.76 | 1.2 | 39 | 5.15 | 84 | 100 | 21.1 | – | – | – | – | 0.0 |
| Technical Drawing | polygons | 0.28 | 10.79 | 0.206 | 1.34 | 1.68 | 1.7 | 19 | 1.69 | 100 | 95 | 3.0 | – | – | – | – | 0.0 |
| Technical Drawing | thin lines | 0.64 | 8.26 | 0.171 | 0.82 | 55.59 | 15.3 | 65 | 3.23 | 100 | – | – | – | – | – | – | 0.0 |
| Technical Drawing | flat colour art | 19.99 | 20.78 | 0.428 | 1.66 | 178.38 | 1.0 | 40 | 3.92 | 88 | 0 | 24.7 | – | – | – | – | 0.0 |
| Technical Drawing | gradient, photo | 39.19 | 38.87 | – | – | – | 20.0 | 360 | – | 96 | – | – | – | – | – | – | 9.0 |
| Flat Logo | curved shapes | 0.06 | 2.14 | 0.036 | 0.25 | 0.25 | 1.8 | 31 | 3.38 | 19 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 |
| Flat Logo | polygons | 0.09 | 3.44 | 0.060 | 0.35 | 1.03 | 2.0 | 35 | 3.17 | 67 | 94 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 |
| Flat Logo | thin lines | 1.17 | 11.78 | 0.050 | 0.49 | all | 3.0 | 189 | 9.45 | 28 | – | – | 0.0 | 0 | 5467.1 | – | 0.0 |
| Flat Logo | flat colour art | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 |
| Flat Logo | gradient, photo | refused | | | | | | | | | | | | | | | |

## 3. Each preset on its inputs, by variant (256 px)

| preset | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | clean | 0.37 | 15.59 | 0.297 | 1.51 | 1.68 | 1.9 | 14 | 1.42 | 40 | 93 | 1.0 | – | – | 58.1 | 67.8 | 0.0 |
| Default | blurred | 0.90 | 18.86 | 0.363 | 1.41 | 2.23 | 1.9 | 16 | 1.56 | 25 | 38 | 3.0 | – | – | 72.1 | 85.2 | 0.0 |
| Default | noisy | 1.97 | 10.79 | 0.293 | 1.48 | 1.68 | 1.9 | 13 | 1.35 | 33 | 83 | 0.7 | – | – | 67.2 | 61.8 | 0.0 |
| Default | scanned | 15.12 | 21.14 | 0.354 | 1.41 | 2.52 | 1.9 | 16 | 1.68 | 19 | 32 | 2.4 | – | – | 77.2 | 59.3 | 0.0 |
| High Fidelity Photo | clean | 2.38 | 2.72 | – | – | – | 2936.7 | 47288 | – | 87 | – | – | – | – | 0.0 | – | 898.7 |
| High Fidelity Photo | blurred | 1.98 | 2.47 | – | – | – | 1486.5 | 19972 | – | 88 | – | – | – | – | 0.0 | – | 274.5 |
| High Fidelity Photo | noisy | 2.78 | 3.04 | – | – | – | 7340.0 | 70154 | – | 74 | – | – | – | – | 0.0 | – | 2494.5 |
| High Fidelity Photo | scanned | 1.84 | 2.01 | – | – | – | 5227.5 | 61967 | – | 76 | – | – | – | – | 0.0 | – | 2009.5 |
| Low Fidelity Photo | clean | 2.70 | 3.06 | – | – | – | 96.2 | 4529 | – | 47 | – | – | – | – | 0.0 | – | 0.0 |
| Low Fidelity Photo | blurred | 2.68 | 3.17 | – | – | – | 151.5 | 2258 | – | 47 | – | – | – | – | 0.0 | – | 0.0 |
| Low Fidelity Photo | noisy | 2.78 | 2.99 | – | – | – | 106.5 | 3818 | – | 30 | – | – | – | – | 0.0 | – | 0.0 |
| Low Fidelity Photo | scanned | 2.08 | 2.21 | – | – | – | 226.0 | 4414 | – | 31 | – | – | – | – | 0.0 | – | 0.0 |
| 3 Colors | clean | 4.71 | 12.75 | 0.362 | 1.11 | 89.09 | 1.3 | 9 | 1.50 | 13 | 33 | 0.0 | 0.7 | 10502 | 112.5 | 95.5 | 0.0 |
| 3 Colors | blurred | 6.97 | 11.97 | 0.858 | 2.35 | 87.85 | 4.0 | 22 | 3.67 | 1 | 0 | 8.0 | 4.0 | 25530 | 134.6 | 100.1 | 0.0 |
| 3 Colors | noisy | 7.30 | 12.66 | 0.395 | 0.95 | 89.02 | 1.0 | 8 | 1.33 | 0 | 0 | 0.0 | 1.0 | 25239 | 71.4 | 60.4 | 0.0 |
| 3 Colors | scanned | 5.92 | 7.24 | 0.761 | 1.75 | 88.20 | 4.0 | 30 | 5.00 | 1 | 0 | 7.0 | 4.0 | 25648 | 166.3 | 112.5 | 0.0 |
| 6 Colors | clean | 0.44 | 10.13 | 0.421 | 1.61 | 1.12 | 5.0 | 35 | 2.66 | 56 | 100 | 8.3 | 1.5 | 198 | 165.2 | 178.1 | 0.0 |
| 6 Colors | blurred | 0.89 | 9.39 | 0.695 | 2.33 | 1.98 | 10.0 | 108 | 9.68 | 41 | 100 | 26.5 | 6.0 | 800 | 125.1 | 197.9 | 0.0 |
| 6 Colors | noisy | 1.39 | 6.91 | 19.919 | 79.80 | 30.11 | 99.0 | 1218 | 146.12 | 55 | 100 | 1124.5 | 3.0 | 363 | 445.5 | 296.0 | 0.0 |
| 6 Colors | scanned | 1.53 | 4.87 | 8.493 | 83.10 | 1.39 | 21.5 | 428 | 45.40 | 53 | 88 | 318.0 | 3.0 | 148 | 214.3 | 212.0 | 0.0 |
| 16 Colors | clean | 0.37 | 9.54 | 0.453 | 1.35 | 1.26 | 7.5 | 57 | 4.29 | 61 | 100 | 13.0 | 3.0 | 382 | 206.1 | 288.5 | 0.0 |
| 16 Colors | blurred | 0.66 | 10.88 | 1.030 | 2.74 | 2.36 | 26.5 | 182 | 15.72 | 86 | 100 | 117.0 | 10.0 | 1168 | 177.5 | 323.7 | 0.0 |
| 16 Colors | noisy | 1.50 | 8.23 | 16.918 | 81.85 | 4.32 | 238.5 | 2540 | 312.41 | 45 | 100 | 2187.0 | 4.5 | 770 | 596.1 | 426.6 | 0.0 |
| 16 Colors | scanned | 1.59 | 4.44 | 16.175 | 86.21 | 3.61 | 199.0 | 3534 | 376.23 | 51 | 100 | 3016.0 | 18.0 | 1609 | 552.9 | 575.6 | 0.0 |
| Shades of Gray | clean | 9.74 | 9.66 | – | – | – | 82.2 | 1950 | – | 46 | – | – | – | – | 0.0 | – | 0.0 |
| Shades of Gray | blurred | 9.69 | 9.42 | – | – | – | 62.0 | 1002 | – | 46 | – | – | – | – | 0.0 | – | 0.0 |
| Shades of Gray | noisy | 9.79 | 9.54 | – | – | – | 132.0 | 2312 | – | 34 | – | – | – | – | 0.0 | – | 1.5 |
| Shades of Gray | scanned | 6.63 | 6.56 | – | – | – | 115.0 | 2778 | – | 29 | – | – | – | – | 0.0 | – | 0.0 |
| Black and White Logo | clean | 0.36 | 14.11 | 0.280 | 1.76 | 1.76 | 1.4 | 30 | 3.67 | 91 | 97 | 13.3 | – | – | – | – | 0.0 |
| Black and White Logo | blurred | 0.87 | 17.99 | 0.317 | 1.43 | 2.23 | 1.4 | 32 | 3.85 | 86 | 40 | 17.0 | – | – | – | – | 0.0 |
| Black and White Logo | noisy | 1.92 | 9.17 | 0.233 | 1.55 | 1.68 | 1.4 | 32 | 3.87 | 92 | 96 | 14.3 | – | – | – | – | 0.0 |
| Black and White Logo | scanned | 15.07 | 20.40 | 0.265 | 1.65 | 2.53 | 1.4 | 36 | 4.09 | 89 | 47 | 20.0 | – | – | – | – | 0.0 |
| Sketched Art | clean | 0.43 | 14.74 | 0.285 | 1.32 | 52.42 | 1.9 | 14 | 1.33 | 29 | 33 | 1.3 | – | – | – | – | 0.0 |
| Sketched Art | blurred | 1.06 | 19.54 | 0.312 | 1.29 | 105.61 | 1.6 | 14 | 1.30 | 26 | 0 | 1.5 | – | – | – | – | 0.0 |
| Sketched Art | noisy | 2.00 | 11.50 | 0.309 | 1.06 | 1.06 | 2.0 | 13 | 1.26 | 27 | 25 | 1.5 | – | – | – | – | 0.0 |
| Sketched Art | scanned | 15.05 | 21.60 | 0.361 | 1.63 | 105.46 | 1.6 | 15 | 1.53 | 27 | 0 | 1.2 | – | – | – | – | 0.0 |
| Silhouettes | clean | 0.42 | 17.79 | 0.345 | 1.34 | 1.34 | 1.2 | 10 | 1.17 | 10 | 100 | 1.2 | – | – | – | – | 0.0 |
| Silhouettes | blurred | 1.23 | 28.03 | 0.754 | 1.87 | 1.90 | 1.2 | 8 | 0.97 | 7 | 50 | 1.5 | – | – | – | – | 0.0 |
| Silhouettes | noisy | 2.23 | 13.42 | 0.384 | 1.17 | 1.17 | 1.2 | 9 | 1.09 | 7 | 100 | 1.5 | – | – | – | – | 0.0 |
| Silhouettes | scanned | 16.06 | 29.87 | 1.494 | 2.88 | 2.89 | 1.2 | 12 | 1.52 | 12 | 50 | 2.0 | – | – | – | – | 0.0 |
| Line Art | clean | 0.78 | 9.82 | 0.175 | 0.82 | 105.63 | 3.3 | 16 | 0.80 | 89 | – | – | – | – | – | – | 0.0 |
| Line Art | blurred | 1.59 | 16.64 | 0.128 | 1.13 | 105.76 | 3.0 | 17 | 0.85 | 94 | – | – | – | – | – | – | 0.0 |
| Line Art | noisy | 1.22 | 7.62 | 0.137 | 0.91 | 56.14 | 8.0 | 36 | 1.80 | 97 | – | – | – | – | – | – | 0.0 |
| Line Art | scanned | 13.55 | 19.72 | 0.173 | 1.13 | 105.76 | 3.0 | 17 | 0.85 | 78 | – | – | – | – | – | – | 0.0 |
| Technical Drawing | clean | 0.37 | 10.16 | 0.197 | 1.34 | 55.59 | 5.1 | 30 | 2.07 | 100 | 95 | 3.0 | – | – | – | – | 0.0 |
| Technical Drawing | blurred | 1.07 | 17.47 | 0.202 | 1.37 | 105.76 | 2.0 | 24 | 1.98 | 100 | 54 | 13.7 | – | – | – | – | 0.0 |
| Technical Drawing | noisy | 1.58 | 8.47 | 0.195 | 1.23 | 31.30 | 7.8 | 43 | 2.79 | 100 | 94 | 2.7 | – | – | – | – | 1.0 |
| Technical Drawing | scanned | 14.47 | 20.51 | 0.228 | 1.36 | 105.76 | 2.0 | 26 | 2.17 | 100 | 62 | 13.3 | – | – | – | – | 0.0 |
| Flat Logo | clean | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 |
| Flat Logo | blurred | 0.54 | 8.33 | 0.052 | 0.62 | 0.78 | 3.0 | 28 | 1.78 | 43 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 |
| Flat Logo | noisy | 0.81 | 1.77 | 0.076 | 0.39 | 0.32 | 3.5 | 44 | 2.63 | 31 | 100 | 1.0 | 0.0 | 0 | 6.9 | – | 0.5 |
| Flat Logo | scanned | 1.31 | 4.15 | 21.204 | 88.50 | 0.66 | 470.5 | 2251 | 255.53 | 11 | 62 | 1658.0 | 0.0 | 0 | 58.0 | – | 413.5 |

## 4. Yardstick

Inkscape 1.4.2's Trace Bitmap, run as a black box through its `object-trace` action (two scans, background
removed, its other settings as they come) on the same one-colour drawings, and its SVG output measured by
the same code. Compare with the black-and-white presets on the same kinds of input in section 2 and 3.

| inputs | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| curved shapes | clean | 0.25 | 11.05 | 0.198 | 0.97 | 0.97 | 1.0 | 15 | 1.98 | 11 | 83 | 1.8 | – | – | – | – | 0.0 |
| curved shapes | blurred | 0.79 | 16.57 | 0.182 | 0.86 | 1.14 | 1.0 | 15 | 1.84 | 6 | 0 | 1.8 | – | – | – | – | 0.0 |
| curved shapes | noisy | 2.01 | 8.14 | 0.191 | 0.93 | 0.93 | 1.0 | 17 | 2.06 | 10 | 75 | 2.8 | – | – | – | – | 0.0 |
| curved shapes | scanned | 15.31 | 20.22 | 0.191 | 0.97 | 1.14 | 1.0 | 16 | 1.98 | 7 | 0 | 2.0 | – | – | – | – | 0.0 |
| polygons | clean | 0.16 | 6.45 | 0.124 | 1.26 | 2.36 | 1.0 | 21 | 1.93 | 27 | 34 | 0.0 | – | – | – | – | 0.0 |
| polygons | blurred | 0.90 | 17.54 | 0.190 | 1.20 | 2.56 | 1.0 | 19 | 1.75 | 0 | 0 | 0.0 | – | – | – | – | 0.0 |
| polygons | noisy | 1.62 | 6.05 | 0.119 | 1.22 | 2.32 | 1.0 | 22 | 2.01 | 29 | 19 | 0.0 | – | – | – | – | 0.0 |
| polygons | scanned | 14.75 | 20.30 | 0.170 | 1.49 | 2.55 | 1.0 | 21 | 1.90 | 4 | 3 | 0.0 | – | – | – | – | 0.0 |
| thin lines | clean | 0.44 | 6.05 | 0.109 | 0.98 | 76.81 | 1.0 | 34 | 1.72 | 2 | – | – | – | – | – | – | 0.0 |
| thin lines | blurred | 1.72 | 18.80 | 0.254 | 0.81 | 106.00 | 1.0 | 18 | 0.90 | 0 | – | – | – | – | – | – | 0.0 |
| thin lines | noisy | 1.02 | 6.25 | 0.112 | 1.00 | 37.89 | 1.0 | 25 | 1.25 | 0 | – | – | – | – | – | – | 0.0 |
| thin lines | scanned | 28.94 | 46.83 | 12.147 | 43.11 | 20.79 | 1.0 | 2763 | 138.15 | 2 | – | – | – | – | – | – | 0.0 |

## 5. Cost

One trace at a time on this machine (all cores); memory is the peak allocated above the image itself. A trace of more than 2,000,000 anchors is refused, as in the app.

| preset | image | megapixels | result |
|:--|:--|--:|:--|
| Default | photo-like | 1 | 0.05 s, 29 paths, 3831 anchors, 15 MB |
| Default | four-colour badge | 1 | 0.03 s, 3 paths, 70 anchors, 15 MB |
| High Fidelity Photo | photo-like | 1 | 0.48 s, 51389 paths, 745118 anchors, 195 MB |
| High Fidelity Photo | four-colour badge | 1 | 0.05 s, 147 paths, 657 anchors, 81 MB |
| Low Fidelity Photo | photo-like | 1 | 0.08 s, 612 paths, 47559 anchors, 135 MB |
| Low Fidelity Photo | four-colour badge | 1 | 0.06 s, 85 paths, 354 anchors, 93 MB |
| 3 Colors | photo-like | 1 | 0.05 s, 9 paths, 2481 anchors, 22 MB |
| 3 Colors | four-colour badge | 1 | 0.04 s, 6 paths, 87 anchors, 21 MB |
| 6 Colors | photo-like | 1 | 0.05 s, 53 paths, 7197 anchors, 38 MB |
| 6 Colors | four-colour badge | 1 | 0.04 s, 7 paths, 112 anchors, 39 MB |
| 16 Colors | photo-like | 1 | 0.07 s, 398 paths, 28412 anchors, 99 MB |
| 16 Colors | four-colour badge | 1 | 0.04 s, 16 paths, 170 anchors, 90 MB |
| Shades of Gray | photo-like | 1 | 0.06 s, 449 paths, 18285 anchors, 55 MB |
| Shades of Gray | four-colour badge | 1 | 0.04 s, 36 paths, 168 anchors, 50 MB |
| Black and White Logo | photo-like | 1 | 0.03 s, 71 paths, 3032 anchors, 9 MB |
| Black and White Logo | four-colour badge | 1 | 0.02 s, 1 paths, 83 anchors, 8 MB |
| Sketched Art | photo-like | 1 | 0.03 s, 5 paths, 1411 anchors, 9 MB |
| Sketched Art | four-colour badge | 1 | 0.02 s, 2 paths, 40 anchors, 8 MB |
| Silhouettes | photo-like | 1 | 0.03 s, 2 paths, 391 anchors, 11 MB |
| Silhouettes | four-colour badge | 1 | 0.02 s, 1 paths, 23 anchors, 9 MB |
| Line Art | photo-like | 1 | 0.03 s, 54 paths, 2545 anchors, 9 MB |
| Line Art | four-colour badge | 1 | 0.02 s, 1 paths, 56 anchors, 8 MB |
| Technical Drawing | photo-like | 1 | 0.03 s, 221 paths, 4083 anchors, 9 MB |
| Technical Drawing | four-colour badge | 1 | 0.02 s, 1 paths, 83 anchors, 8 MB |
| Flat Logo | photo-like | 1 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 1 | 0.66 s, 5 paths, 78 anchors, 146 MB |
| Default | photo-like | 4 | 0.17 s, 48 paths, 10545 anchors, 60 MB |
| Default | four-colour badge | 4 | 0.08 s, 3 paths, 77 anchors, 64 MB |
| High Fidelity Photo | photo-like | 4 | refused after 1.24 s (over 2,000,000 anchors), 859 MB |
| High Fidelity Photo | four-colour badge | 4 | 0.21 s, 225 paths, 991 anchors, 529 MB |
| Low Fidelity Photo | photo-like | 4 | 0.25 s, 1499 paths, 100595 anchors, 485 MB |
| Low Fidelity Photo | four-colour badge | 4 | 0.21 s, 88 paths, 408 anchors, 361 MB |
| 3 Colors | photo-like | 4 | 0.18 s, 22 paths, 7209 anchors, 88 MB |
| 3 Colors | four-colour badge | 4 | 0.12 s, 5 paths, 101 anchors, 84 MB |
| 6 Colors | photo-like | 4 | 0.15 s, 100 paths, 18024 anchors, 152 MB |
| 6 Colors | four-colour badge | 4 | 0.12 s, 7 paths, 127 anchors, 152 MB |
| 16 Colors | photo-like | 4 | 0.18 s, 1236 paths, 75609 anchors, 390 MB |
| 16 Colors | four-colour badge | 4 | 0.15 s, 17 paths, 220 anchors, 315 MB |
| Shades of Gray | photo-like | 4 | 0.19 s, 1289 paths, 48985 anchors, 208 MB |
| Shades of Gray | four-colour badge | 4 | 0.18 s, 41 paths, 211 anchors, 200 MB |
| Black and White Logo | photo-like | 4 | 0.18 s, 208 paths, 9008 anchors, 36 MB |
| Black and White Logo | four-colour badge | 4 | 0.09 s, 1 paths, 142 anchors, 36 MB |
| Sketched Art | photo-like | 4 | 0.17 s, 9 paths, 3366 anchors, 36 MB |
| Sketched Art | four-colour badge | 4 | 0.08 s, 2 paths, 45 anchors, 36 MB |
| Silhouettes | photo-like | 4 | 0.20 s, 2 paths, 1041 anchors, 44 MB |
| Silhouettes | four-colour badge | 4 | 0.08 s, 1 paths, 28 anchors, 36 MB |
| Line Art | photo-like | 4 | 0.16 s, 127 paths, 7334 anchors, 36 MB |
| Line Art | four-colour badge | 4 | 0.08 s, 1 paths, 73 anchors, 36 MB |
| Technical Drawing | photo-like | 4 | 0.15 s, 931 paths, 14515 anchors, 36 MB |
| Technical Drawing | four-colour badge | 4 | 0.08 s, 1 paths, 142 anchors, 36 MB |
| Flat Logo | photo-like | 4 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 4 | 0.66 s, 5 paths, 69 anchors, 146 MB |
| Default | photo-like | 16 | 1.00 s, 127 paths, 26017 anchors, 256 MB |
| Default | four-colour badge | 16 | 0.44 s, 3 paths, 88 anchors, 240 MB |
| High Fidelity Photo | photo-like | 16 | refused after 3.22 s (over 2,000,000 anchors), 3326 MB |
| High Fidelity Photo | four-colour badge | 16 | 1.10 s, 438 paths, 1733 anchors, 2392 MB |
| Low Fidelity Photo | photo-like | 16 | 1.18 s, 8558 paths, 345957 anchors, 1873 MB |
| Low Fidelity Photo | four-colour badge | 16 | 1.07 s, 105 paths, 449 anchors, 1540 MB |
| 3 Colors | photo-like | 16 | 1.05 s, 87 paths, 18248 anchors, 352 MB |
| 3 Colors | four-colour badge | 16 | 0.65 s, 6 paths, 110 anchors, 336 MB |
| 6 Colors | photo-like | 16 | 0.90 s, 476 paths, 47937 anchors, 600 MB |
| 6 Colors | four-colour badge | 16 | 0.63 s, 11 paths, 160 anchors, 588 MB |
| 16 Colors | photo-like | 16 | 0.86 s, 7432 paths, 236585 anchors, 1574 MB |
| 16 Colors | four-colour badge | 16 | 0.83 s, 23 paths, 231 anchors, 1348 MB |
| Shades of Gray | photo-like | 16 | 0.93 s, 6007 paths, 147606 anchors, 805 MB |
| Shades of Gray | four-colour badge | 16 | 0.97 s, 41 paths, 224 anchors, 676 MB |
| Black and White Logo | photo-like | 16 | 0.99 s, 1128 paths, 30289 anchors, 144 MB |
| Black and White Logo | four-colour badge | 16 | 0.43 s, 1 paths, 193 anchors, 128 MB |
| Sketched Art | photo-like | 16 | 1.02 s, 15 paths, 8027 anchors, 144 MB |
| Sketched Art | four-colour badge | 16 | 0.42 s, 2 paths, 52 anchors, 128 MB |
| Silhouettes | photo-like | 16 | 1.29 s, 3 paths, 2442 anchors, 176 MB |
| Silhouettes | four-colour badge | 16 | 0.50 s, 1 paths, 29 anchors, 144 MB |
| Line Art | photo-like | 16 | 0.99 s, 713 paths, 23577 anchors, 144 MB |
| Line Art | four-colour badge | 16 | 0.41 s, 1 paths, 81 anchors, 128 MB |
| Technical Drawing | photo-like | 16 | 1.02 s, 4578 paths, 55463 anchors, 144 MB |
| Technical Drawing | four-colour badge | 16 | 0.43 s, 1 paths, 193 anchors, 128 MB |
| Flat Logo | photo-like | 16 | refused after 0.12 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 16 | 0.68 s, 5 paths, 67 anchors, 146 MB |

## Revision history

| date | change |
|:--|:--|
| 2026-10-12 | The harness and the baseline: every preset measured before any change to the tracer |
