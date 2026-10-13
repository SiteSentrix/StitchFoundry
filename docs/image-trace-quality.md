# Image Trace quality

> **Last reviewed:** 2026-10-13 · **Last updated:** 2026-10-13 · **Change:** minor (levelling moves no line end more than 1.6 px: Technical Drawing's largest distance 4.21 px to 1.46)

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
| plain shapes | a circle, a rectangle, a square, a rounded rectangle, an ellipse and a hexagon, each alone on white (measured from the Shapes change on, not at the baseline) |
| linear gradients | a two-stop gradient over the whole image at 25 degrees, and a three-stop gradient at 110 degrees in a disc on white: their lines and stops are known (measured from the Gradients change on, not at the baseline) |
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
| centre off, width off, anchors a stroke | stroked centre lines (Create: Strokes) against the thin lines they trace: mean distance (px) from the true line's centre line, mean error (px) of the stroke width, anchors per stroked path |
| shapes % | with Shapes on, the share of the plain shapes that came out as that shape (a shape, not a path, whose outline lies within 0.6 px of the true one) |
| grouped % | with Auto Grouping on, the share of the paths that were put inside another (the picture is painted in the grouped order for every other measure of that row, so a ΔE equal to the row without the option says grouping changed nothing that shows) |
| gradient paths | paths filled with a linear gradient (on inputs that are flat colour, every one would be a gradient that is not there) |
| found %, angle off, ramp off | on the linear-gradient inputs: the share of them in which a gradient was found; and for those found, the largest angle (degrees) between the true gradient line and the traced one, and the largest difference (levels of a channel, 0 to 255) between the true colours and the traced gradient's at 17 points along the true line |
| outside px | how far the trace reaches outside the image: a curve that strays |

Black-and-white presets trace black: on the scanned variant their ΔE includes the paper's and ink's own
colours. `–` means the measure does not apply; `all` that nothing was traced.

## Baseline and now

Each preset on the inputs it is meant for, all sizes and variants.

### Baseline (before any change, main at `baf9693f`)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.79 | 17.15 | 0.313 | 1.51 | 2.52 | 1.9 | 14 | 1.45 | 34 | 5 | 76 | 1.4 | – | – | 63.5 | 67.0 | 0.0 | 0.0 | – | – | – |
| High Fidelity Photo | gradient, photo-like | 2.30 | 2.63 | – | – | – | 3652.6 | 46866 | – | 84 | 29 | – | – | – | – | 0.0 | – | 1169.4 | 0.3 | – | – | – |
| Low Fidelity Photo | gradient, photo-like | 2.61 | 2.94 | – | – | – | 128.3 | 3867 | – | 42 | 22 | – | – | – | – | 0.0 | – | 0.0 | 0.7 | – | – | – |
| 3 Colors | two touching halves | 4.93 | 11.38 | 0.482 | 2.35 | 89.09 | 2.1 | 14 | 2.31 | 12 | 8 | 29 | 2.1 | 1.6 | 15418 | 118.0 | 87.3 | 0.0 | 0.0 | – | – | – |
| 6 Colors | two touching halves, four-colour badge | 0.78 | 8.82 | 4.383 | 83.10 | 30.11 | 21.2 | 270 | 30.18 | 51 | 40 | 98 | 213.8 | 2.4 | 272 | 198.8 | 194.6 | 0.0 | 0.3 | – | – | – |
| 16 Colors | two touching halves, four-colour badge | 0.74 | 9.02 | 5.115 | 86.21 | 4.32 | 69.9 | 923 | 102.80 | 58 | 47 | 100 | 765.9 | 5.9 | 670 | 293.7 | 329.9 | 0.0 | 0.5 | – | – | – |
| Shades of Gray | gradient, photo-like | 9.29 | 9.16 | – | – | – | 90.8 | 1921 | – | 42 | 27 | – | – | – | – | 0.0 | – | 0.2 | 0.5 | – | – | – |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.76 | 15.96 | 0.270 | 1.76 | 2.53 | 1.4 | 31 | 3.78 | 90 | 17 | 81 | 15.3 | – | – | – | – | 0.0 | 0.0 | – | – | – |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.85 | 17.04 | 0.302 | 1.63 | 105.63 | 1.8 | 14 | 1.34 | 28 | 8 | 18 | 1.4 | – | – | – | – | 0.0 | 0.0 | – | – | – |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 3.04 | 21.41 | 0.573 | 2.88 | 2.89 | 1.2 | 10 | 1.16 | 9 | 7 | 86 | 1.4 | – | – | – | – | 0.0 | 0.0 | – | – | – |
| Line Art | lines | 2.77 | 12.51 | 0.158 | 1.13 | 105.76 | 5.4 | 25 | 1.27 | 90 | 6 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.73 | 13.67 | 0.210 | 1.43 | 105.76 | 4.7 | 31 | 2.01 | 100 | 9 | 85 | 5.9 | – | – | – | – | 0.1 | 0.0 | – | – | – |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 | – | – | – |

### Now (with levelling bounded)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke | shapes % | grouped % | gradient paths | found % | angle off | ramp off |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.05 | 0.091 | 1.02 | 2.18 | 1.9 | 17 | 1.73 | 30 | 6 | 76 | 0.4 | – | – | 0.3 | 0.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | two touching halves, four-colour badge | 0.55 | 4.99 | 0.141 | 1.33 | 1.21 | 4.0 | 51 | 3.19 | 39 | 16 | 91 | 6.0 | 0.0 | 0 | 1.5 | 1.2 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9929 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.1 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2134 | – | 43 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 6 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.99 | 0.140 | 1.34 | 1.19 | 4.0 | 49 | 3.07 | 39 | 16 | 91 | 6.1 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.91 | 0.142 | 1.31 | 1.19 | 4.2 | 63 | 3.87 | 40 | 16 | 91 | 7.7 | 0.0 | 0 | 0.5 | 0.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.6 | 700 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.59 | 9.46 | 0.064 | 1.02 | 2.17 | 1.4 | 27 | 2.68 | 42 | 11 | 76 | 0.3 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.02 | 0.185 | 1.00 | 105.61 | 1.8 | 16 | 1.42 | 19 | 8 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.30 | 0.524 | 2.19 | 2.19 | 1.2 | 14 | 1.64 | 12 | 9 | 93 | 0.4 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | lines | 2.62 | 9.49 | 0.056 | 0.99 | 105.72 | 5.4 | 43 | 2.14 | 40 | 0 | – | – | – | – | – | – | 0.1 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.65 | 10.88 | 0.126 | 1.46 | 105.64 | 4.7 | 38 | 2.55 | 81 | 19 | 78 | 0.8 | – | – | – | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | lines | 2.60 | 9.16 | 0.055 | 2.94 | 105.75 | 5.0 | 14 | 0.71 | 95 | 4 | – | – | – | – | – | – | 0.0 | 0.0 | 0.053 | 0.02 | 2.4 | – | – | 0.0 | – | – | – |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | plain shapes | 3.18 | 9.53 | 0.102 | 1.00 | 1.00 | 6.0 | 40 | 1.32 | 41 | 27 | – | – | – | – | 15.8 | 11.5 | 0.0 | 0.0 | – | – | – | 57 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | plain shapes | 0.91 | 6.82 | 0.089 | 0.99 | 0.99 | 6.0 | 43 | 1.43 | 49 | 33 | – | – | 0.0 | 0 | 10.2 | 10.9 | 0.0 | 0.0 | – | – | – | 62 | – | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | two touching halves, four-colour badge | 0.55 | 4.99 | 0.140 | 1.34 | 1.19 | 4.0 | 49 | 3.07 | 39 | 16 | 91 | 6.1 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | 27 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.6 | 700 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 47 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9929 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.1 | 0.0 | – | – | – | – | 69 | 0.0 | – | – | – |
| 6 Colors, no Gradients | two-stop gradient, three-stop gradient in a disc | 2.12 | 3.04 | – | – | – | 6.4 | 469 | – | 21 | 14 | – | – | – | – | 3.9 | 12.7 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Gradients | two-stop gradient, three-stop gradient in a disc | 0.62 | 1.36 | – | – | – | 3.4 | 126 | – | 8 | 4 | – | – | – | – | 1.8 | 2.7 | 0.0 | 0.0 | – | – | – | – | – | 1.1 | 86 | 1.5 | 21 |
| High Fidelity Photo, no Gradients | two-stop gradient, three-stop gradient in a disc | 0.79 | 2.98 | – | – | – | 38.1 | 3827 | – | 31 | 30 | – | – | – | – | 0.0 | – | 1.1 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo with Gradients | two-stop gradient, three-stop gradient in a disc | 0.46 | 1.31 | – | – | – | 20.6 | 759 | – | 9 | 5 | – | – | – | – | 0.0 | – | 2.9 | 0.0 | – | – | – | – | – | 1.0 | 86 | 1.5 | 21 |

The sections below are the numbers now (with levelling bounded).

## 1. Each preset on the inputs it is meant for

All sizes and variants (clean, aliased, blurred, noisy, scanned) of those inputs.

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke | shapes % | grouped % | gradient paths | found % | angle off | ramp off |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.60 | 10.05 | 0.091 | 1.02 | 2.18 | 1.9 | 17 | 1.73 | 30 | 6 | 76 | 0.4 | – | – | 0.3 | 0.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | two touching halves, four-colour badge | 0.55 | 4.99 | 0.141 | 1.33 | 1.21 | 4.0 | 51 | 3.19 | 39 | 16 | 91 | 6.0 | 0.0 | 0 | 1.5 | 1.2 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9929 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.1 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | gradient, photo-like | 2.45 | 2.69 | – | – | – | 37.1 | 2134 | – | 43 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | two touching halves | 0.42 | 3.98 | 0.104 | 0.91 | 0.91 | 2.0 | 11 | 1.81 | 39 | 6 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | two touching halves, four-colour badge | 0.55 | 4.99 | 0.140 | 1.34 | 1.19 | 4.0 | 49 | 3.07 | 39 | 16 | 91 | 6.1 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | two touching halves, four-colour badge | 0.55 | 4.91 | 0.142 | 1.31 | 1.19 | 4.2 | 63 | 3.87 | 40 | 16 | 91 | 7.7 | 0.0 | 0 | 0.5 | 0.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.6 | 700 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | circle, ellipses, badge with a hole, letter D, polygons, star, letter A | 2.59 | 9.46 | 0.064 | 1.02 | 2.17 | 1.4 | 27 | 2.68 | 42 | 11 | 76 | 0.3 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | circle, ellipses, badge with a hole, letter D, lines | 2.71 | 12.02 | 0.185 | 1.00 | 105.61 | 1.8 | 16 | 1.42 | 19 | 8 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | circle, ellipses, badge with a hole, letter D | 2.98 | 19.30 | 0.524 | 2.19 | 2.19 | 1.2 | 14 | 1.64 | 12 | 9 | 93 | 0.4 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | lines | 2.62 | 9.49 | 0.056 | 0.99 | 105.72 | 5.4 | 43 | 2.14 | 40 | 0 | – | – | – | – | – | – | 0.1 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | polygons, star, letter A, lines, askew shapes and lines | 2.65 | 10.88 | 0.126 | 1.46 | 105.64 | 4.7 | 38 | 2.55 | 81 | 19 | 78 | 0.8 | – | – | – | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | lines | 2.60 | 9.16 | 0.055 | 2.94 | 105.75 | 5.0 | 14 | 0.71 | 95 | 4 | – | – | – | – | – | – | 0.0 | 0.0 | 0.053 | 0.02 | 2.4 | – | – | 0.0 | – | – | – |
| Flat Logo | two touching halves, four-colour badge | 0.43 | 3.53 | 3.106 | 88.50 | 0.78 | 69.9 | 352 | 38.92 | 38 | 10 | 80 | 237.0 | 0.0 | 0 | 13.8 | – | 59.1 | 0.2 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | plain shapes | 3.18 | 9.53 | 0.102 | 1.00 | 1.00 | 6.0 | 40 | 1.32 | 41 | 27 | – | – | – | – | 15.8 | 11.5 | 0.0 | 0.0 | – | – | – | 57 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | plain shapes | 0.91 | 6.82 | 0.089 | 0.99 | 0.99 | 6.0 | 43 | 1.43 | 49 | 33 | – | – | 0.0 | 0 | 10.2 | 10.9 | 0.0 | 0.0 | – | – | – | 62 | – | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | two touching halves, four-colour badge | 0.55 | 4.99 | 0.140 | 1.34 | 1.19 | 4.0 | 49 | 3.07 | 39 | 16 | 91 | 6.1 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | 27 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | gradient, photo-like | 9.26 | 9.06 | – | – | – | 16.6 | 700 | – | 51 | 40 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 47 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | gradient, photo-like | 1.75 | 1.98 | – | – | – | 120.7 | 9929 | – | 38 | 36 | – | – | – | – | 0.0 | – | 10.1 | 0.0 | – | – | – | – | 69 | 0.0 | – | – | – |
| 6 Colors, no Gradients | two-stop gradient, three-stop gradient in a disc | 2.12 | 3.04 | – | – | – | 6.4 | 469 | – | 21 | 14 | – | – | – | – | 3.9 | 12.7 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Gradients | two-stop gradient, three-stop gradient in a disc | 0.62 | 1.36 | – | – | – | 3.4 | 126 | – | 8 | 4 | – | – | – | – | 1.8 | 2.7 | 0.0 | 0.0 | – | – | – | – | – | 1.1 | 86 | 1.5 | 21 |
| High Fidelity Photo, no Gradients | two-stop gradient, three-stop gradient in a disc | 0.79 | 2.98 | – | – | – | 38.1 | 3827 | – | 31 | 30 | – | – | – | – | 0.0 | – | 1.1 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo with Gradients | two-stop gradient, three-stop gradient in a disc | 0.46 | 1.31 | – | – | – | 20.6 | 759 | – | 9 | 5 | – | – | – | – | 0.0 | – | 2.9 | 0.0 | – | – | – | – | – | 1.0 | 86 | 1.5 | 21 |

## 2. Each preset on each kind of input (clean, three sizes)

| preset | inputs | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke | shapes % | grouped % | gradient paths | found % | angle off | ramp off |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | curved shapes | 0.09 | 3.40 | 0.060 | 0.38 | 0.38 | 1.8 | 14 | 1.57 | 14 | 14 | 100 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | polygons | 0.06 | 2.37 | 0.041 | 0.47 | 1.26 | 2.0 | 14 | 1.26 | 88 | 5 | 95 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | thin lines | 0.54 | 4.46 | 0.049 | 0.47 | 105.72 | 3.3 | 25 | 1.27 | 40 | 0 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | askew shapes | 0.16 | 3.29 | 0.044 | 0.34 | 36.06 | 3.7 | 23 | 0.94 | 100 | 3 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | plain shapes | 0.13 | 2.63 | 0.055 | 0.35 | 0.35 | 6.0 | 40 | 1.34 | 53 | 48 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Default | flat colour art | 19.91 | 17.97 | 0.172 | 0.51 | 179.34 | 1.5 | 16 | 1.17 | 14 | 14 | 0 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | gradient, photo | 39.03 | 38.64 | – | – | – | 3.2 | 595 | – | 55 | 41 | – | – | – | – | 18.8 | 35.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | linear gradients | 25.64 | 29.86 | – | – | – | 2.0 | 43 | – | 33 | 32 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Auto Color | curved shapes | 0.06 | 2.43 | 0.043 | 0.27 | 0.28 | 1.8 | 16 | 1.77 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | polygons | 0.06 | 2.42 | 0.041 | 0.42 | 1.39 | 2.0 | 16 | 1.43 | 87 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | thin lines | 0.70 | 5.57 | 0.066 | 0.50 | 105.72 | 5.0 | 38 | 1.88 | 43 | 3 | – | – | 4.0 | 3225 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | askew shapes | 0.64 | 3.76 | 0.047 | 0.36 | 9.49 | 4.0 | 24 | 1.00 | 100 | 3 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | plain shapes | 0.10 | 2.07 | 0.043 | 0.30 | 0.26 | 6.0 | 42 | 1.41 | 59 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Auto Color | flat colour art | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | gradient, photo | 2.07 | 2.39 | – | – | – | 159.3 | 4441 | – | 21 | 8 | – | – | – | – | 37.6 | 46.7 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | linear gradients | 1.02 | 1.74 | – | – | – | 15.7 | 1272 | – | 12 | 8 | – | – | – | – | 9.1 | 7.4 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo | curved shapes | 0.04 | 1.63 | 0.026 | 0.15 | 0.15 | 1.2 | 19 | 2.11 | 21 | 21 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | polygons | 0.06 | 2.33 | 0.039 | 0.50 | 1.32 | 1.7 | 17 | 1.47 | 88 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | thin lines | 0.35 | 6.05 | 0.085 | 0.90 | 4.35 | 40.3 | 348 | 17.42 | 35 | 9 | – | – | 27.3 | 4505 | 0.0 | – | 19.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | askew shapes | 0.17 | 3.97 | 0.100 | 0.94 | 1.73 | 8.7 | 157 | 6.54 | 71 | 5 | – | – | 4.3 | 3998 | 0.0 | – | 1.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | plain shapes | 0.08 | 1.66 | 0.035 | 0.25 | 0.28 | 6.0 | 50 | 1.68 | 62 | 51 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| High Fidelity Photo | flat colour art | 0.14 | 4.16 | 0.029 | 0.20 | 0.20 | 3.5 | 73 | 4.59 | 35 | 34 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | gradient, photo | 1.80 | 2.05 | – | – | – | 134.7 | 11212 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | linear gradients | 0.64 | 3.29 | – | – | – | 34.5 | 4318 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Low Fidelity Photo | curved shapes | 0.07 | 2.74 | 0.048 | 0.31 | 0.31 | 1.2 | 13 | 1.44 | 18 | 18 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | polygons | 0.06 | 2.49 | 0.042 | 0.42 | 1.39 | 1.7 | 14 | 1.28 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | thin lines | 0.49 | 5.70 | 0.082 | 0.70 | 45.31 | 7.0 | 56 | 2.80 | 49 | 11 | – | – | 4.0 | 3985 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | askew shapes | 0.23 | 4.65 | 0.059 | 0.70 | 21.55 | 6.0 | 42 | 1.76 | 85 | 3 | – | – | 2.3 | 2630 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | plain shapes | 0.11 | 2.25 | 0.047 | 0.30 | 0.28 | 6.0 | 41 | 1.38 | 58 | 48 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Low Fidelity Photo | flat colour art | 0.15 | 4.25 | 0.044 | 0.28 | 0.28 | 3.5 | 54 | 3.38 | 31 | 29 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | gradient, photo | 2.53 | 2.80 | – | – | – | 42.7 | 2307 | – | 43 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | linear gradients | 0.69 | 3.09 | – | – | – | 24.3 | 1061 | – | 31 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 3 Colors | curved shapes | 0.07 | 2.74 | 0.048 | 0.31 | 0.31 | 1.8 | 16 | 1.67 | 15 | 15 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | polygons | 0.06 | 2.49 | 0.042 | 0.42 | 1.39 | 2.0 | 16 | 1.42 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | thin lines | 0.43 | 4.48 | 0.117 | 1.04 | 45.31 | 5.0 | 43 | 2.13 | 54 | 13 | – | – | 2.0 | 308 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | askew shapes | 0.18 | 3.58 | 0.054 | 0.70 | 14.59 | 4.0 | 25 | 1.04 | 97 | 3 | – | – | 0.7 | 23 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | plain shapes | 0.11 | 2.25 | 0.047 | 0.30 | 0.28 | 6.0 | 41 | 1.38 | 58 | 48 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| 3 Colors | flat colour art | 1.31 | 3.61 | 0.125 | 0.90 | 60.25 | 2.5 | 25 | 1.86 | 45 | 21 | 100 | 0.0 | 1.0 | 9914 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | gradient, photo | 7.49 | 7.60 | – | – | – | 5.7 | 524 | – | 48 | 40 | – | – | – | – | 6.9 | 8.7 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | linear gradients | 4.45 | 5.79 | – | – | – | 3.0 | 78 | – | 28 | 24 | – | – | – | – | 10.3 | 1.9 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors | curved shapes | 0.06 | 2.43 | 0.043 | 0.27 | 0.28 | 1.8 | 16 | 1.77 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | polygons | 0.06 | 2.40 | 0.041 | 0.42 | 1.39 | 2.0 | 16 | 1.45 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | thin lines | 0.49 | 4.76 | 0.108 | 0.70 | 45.31 | 4.0 | 30 | 1.50 | 63 | 19 | – | – | 1.7 | 413 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | askew shapes | 0.19 | 3.72 | 0.058 | 0.73 | 36.06 | 4.0 | 25 | 1.06 | 90 | 3 | – | – | 0.3 | 11 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | plain shapes | 0.10 | 2.07 | 0.043 | 0.30 | 0.26 | 6.0 | 42 | 1.41 | 59 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| 6 Colors | flat colour art | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 829 | – | 37 | 28 | – | – | – | – | 6.9 | 8.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | linear gradients | 2.07 | 3.07 | – | – | – | 6.2 | 266 | – | 20 | 14 | – | – | – | – | 2.4 | 9.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 16 Colors | curved shapes | 0.05 | 2.21 | 0.038 | 0.24 | 0.25 | 1.8 | 17 | 1.83 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | polygons | 0.06 | 2.34 | 0.039 | 0.42 | 1.39 | 2.0 | 16 | 1.48 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | thin lines | 0.39 | 3.90 | 0.113 | 0.70 | 23.53 | 8.7 | 52 | 2.58 | 56 | 17 | – | – | 5.7 | 322 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | askew shapes | 0.18 | 3.65 | 0.062 | 0.73 | 21.55 | 4.3 | 28 | 1.17 | 87 | 3 | – | – | 0.7 | 17 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | plain shapes | 0.09 | 1.92 | 0.041 | 0.30 | 0.26 | 6.0 | 44 | 1.46 | 60 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| 16 Colors | flat colour art | 0.12 | 2.53 | 0.051 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | gradient, photo | 3.00 | 3.30 | – | – | – | 47.2 | 2126 | – | 30 | 16 | – | – | – | – | 15.2 | 19.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | linear gradients | 0.98 | 1.67 | – | – | – | 15.8 | 1340 | – | 8 | 6 | – | – | – | – | 13.9 | 15.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Shades of Gray | curved shapes | 0.07 | 2.70 | 0.047 | 0.31 | 0.31 | 1.2 | 13 | 1.45 | 18 | 18 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | polygons | 0.06 | 2.28 | 0.039 | 0.41 | 1.26 | 1.7 | 13 | 1.18 | 89 | 5 | 95 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | thin lines | 0.56 | 7.08 | 0.088 | 0.68 | 45.31 | 10.3 | 73 | 3.65 | 55 | 11 | – | – | 8.0 | 4319 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | askew shapes | 0.25 | 4.95 | 0.049 | 0.73 | 21.52 | 6.0 | 39 | 1.61 | 95 | 3 | – | – | 2.3 | 2638 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | plain shapes | 0.12 | 2.31 | 0.048 | 0.31 | 0.28 | 6.0 | 42 | 1.39 | 54 | 48 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Shades of Gray | flat colour art | 7.33 | 11.24 | 0.257 | 1.47 | 179.39 | 4.7 | 199 | 18.39 | 22 | 22 | 50 | 0.3 | 3.7 | 173367 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | gradient, photo | 9.71 | 9.55 | – | – | – | 18.3 | 753 | – | 51 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | linear gradients | 5.25 | 8.40 | – | – | – | 9.0 | 477 | – | 31 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Black and White Logo | curved shapes | 0.03 | 1.48 | 0.024 | 0.16 | 0.16 | 1.2 | 20 | 2.12 | 21 | 21 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | polygons | 0.06 | 2.36 | 0.040 | 0.47 | 1.26 | 1.7 | 14 | 1.24 | 93 | 8 | 95 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | thin lines | 0.59 | 6.00 | 0.105 | 0.50 | 77.21 | 10.3 | 44 | 2.18 | 90 | 10 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | askew shapes | 0.49 | 11.14 | 0.234 | 1.46 | 9.63 | 4.0 | 25 | 1.03 | 98 | 70 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | plain shapes | 0.08 | 1.56 | 0.033 | 0.31 | 0.30 | 6.0 | 55 | 1.84 | 58 | 52 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Black and White Logo | flat colour art | 19.90 | 17.76 | 0.157 | 0.30 | 179.51 | 1.0 | 18 | 1.12 | 23 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | gradient, photo | 38.97 | 38.53 | – | – | – | 9.0 | 634 | – | 73 | 54 | – | – | – | – | – | – | 1.2 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | linear gradients | 25.62 | 29.78 | – | – | – | 1.5 | 127 | – | 46 | 33 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Sketched Art | curved shapes | 0.14 | 5.71 | 0.110 | 0.46 | 0.46 | 1.2 | 12 | 1.37 | 16 | 16 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | polygons | 0.12 | 4.71 | 0.085 | 0.37 | 1.08 | 1.7 | 13 | 1.16 | 86 | 5 | 93 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | thin lines | 0.47 | 5.70 | 0.113 | 0.96 | 52.59 | 4.3 | 29 | 1.45 | 64 | 3 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | askew shapes | 0.32 | 6.03 | 0.091 | 0.45 | 35.94 | 3.3 | 23 | 0.94 | 96 | 4 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | plain shapes | 0.30 | 5.84 | 0.107 | 0.47 | 0.47 | 6.0 | 40 | 1.32 | 53 | 47 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Sketched Art | flat colour art | 20.25 | 19.55 | 0.146 | 1.15 | 179.63 | 1.5 | 26 | 1.53 | 16 | 16 | 0 | 0.3 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | gradient, photo | 45.42 | 45.21 | – | – | – | 1.0 | 244 | – | 46 | 35 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | linear gradients | 29.20 | 33.47 | – | – | – | 1.5 | 54 | – | 35 | 34 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Silhouettes | curved shapes | 0.35 | 14.53 | 0.281 | 0.69 | 0.69 | 1.2 | 12 | 1.33 | 16 | 16 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | polygons | 0.34 | 13.44 | 0.251 | 0.64 | 1.67 | 1.7 | 14 | 1.28 | 74 | 6 | 86 | 0.1 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | thin lines | 0.99 | 13.74 | 0.266 | 0.77 | 18.26 | 5.0 | 30 | 1.50 | 67 | 12 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | askew shapes | 0.58 | 14.93 | 0.297 | 0.55 | 0.55 | 4.0 | 24 | 1.01 | 95 | 3 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | plain shapes | 0.76 | 15.17 | 0.291 | 0.69 | 0.69 | 6.0 | 39 | 1.29 | 54 | 46 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Silhouettes | flat colour art | 23.83 | 33.06 | 0.200 | 0.69 | 188.27 | 1.0 | 11 | 0.77 | 25 | 25 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | gradient, photo | 61.57 | 61.32 | – | – | – | 0.2 | 2 | – | 47 | 21 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | linear gradients | 39.85 | 46.48 | – | – | – | 1.0 | 37 | – | 1 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Line Art | curved shapes | 0.04 | 1.83 | 0.031 | 0.22 | 0.22 | 1.2 | 16 | 1.70 | 20 | 20 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | polygons | 0.05 | 2.11 | 0.036 | 0.47 | 1.26 | 1.7 | 14 | 1.26 | 89 | 5 | 95 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | thin lines | 0.52 | 4.09 | 0.040 | 0.47 | 105.72 | 3.3 | 29 | 1.45 | 40 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | askew shapes | 0.12 | 3.13 | 0.044 | 0.48 | 9.60 | 4.0 | 24 | 1.01 | 98 | 3 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | plain shapes | 0.09 | 1.76 | 0.038 | 0.30 | 0.29 | 6.0 | 47 | 1.56 | 56 | 51 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Line Art | flat colour art | 19.90 | 17.83 | 0.162 | 0.42 | 179.43 | 1.0 | 18 | 1.30 | 21 | 21 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | gradient, photo | 38.98 | 38.54 | – | – | – | 7.7 | 472 | – | 58 | 44 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | linear gradients | 25.63 | 29.80 | – | – | – | 1.5 | 95 | – | 33 | 32 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Technical Drawing | curved shapes | 0.03 | 1.48 | 0.024 | 0.16 | 0.16 | 1.2 | 20 | 2.12 | 21 | 21 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | polygons | 0.06 | 2.36 | 0.040 | 0.47 | 1.26 | 1.7 | 14 | 1.23 | 94 | 8 | 95 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | thin lines | 0.50 | 5.46 | 0.109 | 0.50 | 55.93 | 15.3 | 62 | 3.12 | 86 | 14 | – | – | – | – | – | – | 3.3 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | askew shapes | 0.48 | 11.07 | 0.233 | 1.46 | 2.11 | 4.3 | 26 | 1.08 | 98 | 70 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | plain shapes | 0.08 | 1.63 | 0.035 | 0.33 | 0.33 | 6.0 | 55 | 1.82 | 60 | 52 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Technical Drawing | flat colour art | 19.90 | 17.76 | 0.157 | 0.30 | 179.51 | 1.0 | 18 | 1.12 | 23 | 23 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | gradient, photo | 38.94 | 38.51 | – | – | – | 20.0 | 659 | – | 73 | 51 | – | – | – | – | – | – | 11.7 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | linear gradients | 25.62 | 29.80 | – | – | – | 1.5 | 118 | – | 49 | 33 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| Outline | curved shapes | 23.38 | 44.76 | 0.499 | 0.71 | 0.52 | 1.2 | 16 | 1.70 | 20 | 20 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | polygons | 17.94 | 44.26 | 0.494 | 0.94 | 0.76 | 1.7 | 14 | 1.26 | 89 | 5 | 95 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | thin lines | 0.52 | 3.89 | 0.038 | 2.94 | 105.73 | 3.3 | 7 | 0.35 | 100 | 7 | – | – | – | – | – | – | 0.0 | 0.0 | 0.056 | 0.02 | 2.1 | – | – | 0.0 | – | – | – |
| Outline | askew shapes | 23.68 | 36.71 | 0.425 | 1.50 | 6.62 | 4.0 | 20 | 0.83 | 100 | 3 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | plain shapes | 34.32 | 43.23 | 0.495 | 0.73 | 0.51 | 6.0 | 47 | 1.56 | 56 | 51 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Outline | flat colour art | 26.69 | 32.18 | 0.494 | 0.92 | 178.93 | 1.0 | 18 | 1.30 | 21 | 21 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | gradient, photo | 39.38 | 39.18 | – | – | – | 7.5 | 445 | – | 59 | 43 | – | – | – | – | – | – | 3.3 | 0.5 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | linear gradients | 25.08 | 30.63 | – | – | – | 1.5 | 93 | – | 33 | 32 | – | – | – | – | – | – | 0.0 | 0.5 | – | – | – | – | – | 0.0 | 0 | – | – |
| Flat Logo | curved shapes | 0.06 | 2.14 | 0.036 | 0.25 | 0.25 | 1.8 | 31 | 3.38 | 19 | 6 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | polygons | 0.09 | 3.44 | 0.060 | 0.35 | 1.03 | 2.0 | 35 | 3.17 | 67 | 0 | 94 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | thin lines | 1.17 | 11.78 | 0.050 | 0.49 | all | 3.0 | 189 | 9.45 | 28 | 9 | – | – | 0.0 | 0 | 5467.1 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | askew shapes | 0.15 | 3.78 | 0.058 | 0.43 | 0.50 | 4.0 | 114 | 4.76 | 39 | 0 | – | – | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | plain shapes | 0.16 | 3.09 | 0.061 | 0.30 | 0.27 | 6.0 | 70 | 2.32 | 57 | 4 | – | – | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| Flat Logo | flat colour art | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 11 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | gradient, photo | refused | | | | | | | | | | | | | | | | | | | | | | | | | | |
| Flat Logo | linear gradients | 6.77 | 6.85 | – | – | – | 8918.3 | 41974 | – | 7 | 0 | – | – | – | – | 2733.2 | – | 7382.7 | 0.2 | – | – | – | – | – | 0.0 | 0 | – | – |
| Default with Shapes | curved shapes | 0.08 | 3.19 | 0.058 | 0.38 | 0.38 | 1.8 | 13 | 1.44 | 14 | 14 | 100 | 0.0 | – | – | 12.2 | 7.4 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | polygons | 0.06 | 2.34 | 0.040 | 0.47 | 1.26 | 2.0 | 14 | 1.26 | 88 | 5 | 95 | 0.0 | – | – | 0.0 | 2.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | thin lines | 0.54 | 4.53 | 0.049 | 0.52 | 105.72 | 3.3 | 25 | 1.27 | 40 | 0 | – | – | – | – | 0.0 | 18.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | askew shapes | 0.17 | 3.85 | 0.050 | 0.37 | 36.06 | 3.7 | 23 | 0.94 | 100 | 3 | – | – | – | – | 0.0 | 42.1 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | plain shapes | 0.13 | 2.62 | 0.055 | 0.35 | 0.34 | 6.0 | 38 | 1.28 | 55 | 48 | – | – | – | – | 16.4 | 8.2 | 0.0 | 0.0 | – | – | – | 72 | – | 0.0 | – | – | – |
| Default with Shapes | flat colour art | 19.91 | 17.91 | 0.176 | 0.51 | 179.34 | 1.5 | 16 | 1.01 | 14 | 14 | 0 | 0.0 | – | – | 14.0 | 7.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | gradient, photo | 39.03 | 38.64 | – | – | – | 3.2 | 595 | – | 55 | 41 | – | – | – | – | 18.8 | 35.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | linear gradients | 25.64 | 29.86 | – | – | – | 2.0 | 43 | – | 33 | 32 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Shapes | curved shapes | 0.05 | 2.24 | 0.041 | 0.27 | 0.28 | 1.8 | 15 | 1.59 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 9.7 | 5.1 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | polygons | 0.06 | 2.35 | 0.040 | 0.42 | 1.39 | 2.0 | 16 | 1.45 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 6.1 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | thin lines | 0.49 | 4.78 | 0.108 | 0.70 | 45.31 | 4.0 | 30 | 1.50 | 63 | 19 | – | – | 1.7 | 413 | 0.0 | 10.8 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | askew shapes | 0.20 | 4.24 | 0.064 | 0.73 | 36.06 | 4.0 | 25 | 1.03 | 99 | 3 | – | – | 0.3 | 11 | 0.0 | 49.7 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | plain shapes | 0.10 | 2.00 | 0.042 | 0.30 | 0.26 | 6.0 | 40 | 1.33 | 59 | 49 | – | – | 0.0 | 0 | 8.5 | 6.4 | 0.0 | 0.0 | – | – | – | 78 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | flat colour art | 0.13 | 2.71 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.5 | 9.1 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 829 | – | 37 | 28 | – | – | – | – | 7.1 | 8.2 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Shapes | linear gradients | 2.07 | 3.07 | – | – | – | 6.2 | 266 | – | 20 | 14 | – | – | – | – | 2.4 | 9.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Auto Grouping | curved shapes | 0.08 | 3.46 | 0.043 | 0.27 | 0.28 | 1.8 | 16 | 1.77 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 17 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | polygons | 0.07 | 2.87 | 0.041 | 0.42 | 1.39 | 2.0 | 16 | 1.45 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 11 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | thin lines | 0.49 | 4.76 | 0.108 | 0.70 | 45.31 | 4.0 | 30 | 1.50 | 63 | 19 | – | – | 1.7 | 413 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | askew shapes | 0.19 | 3.72 | 0.058 | 0.73 | 36.06 | 4.0 | 25 | 1.06 | 90 | 3 | – | – | 0.3 | 11 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | plain shapes | 0.10 | 2.07 | 0.043 | 0.30 | 0.26 | 6.0 | 42 | 1.41 | 59 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | 0 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | flat colour art | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 30 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 829 | – | 37 | 28 | – | – | – | – | 6.9 | 8.0 | 0.0 | 0.0 | – | – | – | – | 15 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | linear gradients | 2.07 | 3.07 | – | – | – | 6.2 | 266 | – | 20 | 14 | – | – | – | – | 2.4 | 9.3 | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | 0 | – | – |
| Shades of Gray with Auto Grouping | curved shapes | 0.07 | 2.70 | 0.047 | 0.31 | 0.31 | 1.2 | 13 | 1.45 | 18 | 18 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | polygons | 0.06 | 2.28 | 0.039 | 0.41 | 1.26 | 1.7 | 13 | 1.18 | 89 | 5 | 95 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | thin lines | 0.56 | 7.08 | 0.088 | 0.68 | 45.31 | 10.3 | 73 | 3.65 | 55 | 11 | – | – | 8.0 | 4319 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | askew shapes | 0.25 | 4.95 | 0.049 | 0.73 | 21.52 | 6.0 | 39 | 1.61 | 95 | 3 | – | – | 2.3 | 2638 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 3 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | plain shapes | 0.12 | 2.31 | 0.048 | 0.31 | 0.28 | 6.0 | 42 | 1.39 | 54 | 48 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | 0 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | flat colour art | 7.33 | 11.24 | 0.257 | 1.47 | 179.39 | 4.7 | 199 | 18.39 | 22 | 22 | 50 | 0.3 | 3.7 | 173367 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 52 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | gradient, photo | 9.71 | 9.55 | – | – | – | 18.3 | 753 | – | 51 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 53 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | linear gradients | 5.25 | 8.40 | – | – | – | 9.0 | 477 | – | 31 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 68 | 0.0 | 0 | – | – |
| High Fidelity Photo with Auto Grouping | curved shapes | 0.04 | 1.63 | 0.026 | 0.15 | 0.15 | 1.2 | 19 | 2.11 | 21 | 21 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | polygons | 0.06 | 2.33 | 0.039 | 0.50 | 1.32 | 1.7 | 17 | 1.47 | 88 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 0 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | thin lines | 0.35 | 6.05 | 0.085 | 0.90 | 4.35 | 40.3 | 348 | 17.42 | 35 | 9 | – | – | 27.3 | 4505 | 0.0 | – | 19.0 | 0.0 | – | – | – | – | 23 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | askew shapes | 0.17 | 3.97 | 0.100 | 0.94 | 1.73 | 8.7 | 157 | 6.54 | 71 | 5 | – | – | 4.3 | 3998 | 0.0 | – | 1.0 | 0.0 | – | – | – | – | 18 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | plain shapes | 0.08 | 1.66 | 0.035 | 0.25 | 0.28 | 6.0 | 50 | 1.68 | 62 | 51 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | 0 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | flat colour art | 0.14 | 4.16 | 0.029 | 0.20 | 0.20 | 3.5 | 73 | 4.59 | 35 | 34 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 25 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | gradient, photo | 1.80 | 2.05 | – | – | – | 134.7 | 11212 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 | – | – | – | – | 69 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | linear gradients | 0.64 | 3.29 | – | – | – | 34.5 | 4318 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 72 | 0.0 | 0 | – | – |
| 6 Colors, no Gradients | curved shapes | 0.06 | 2.43 | 0.043 | 0.27 | 0.28 | 1.8 | 16 | 1.77 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | polygons | 0.06 | 2.40 | 0.041 | 0.42 | 1.39 | 2.0 | 16 | 1.45 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | thin lines | 0.49 | 4.76 | 0.108 | 0.70 | 45.31 | 4.0 | 30 | 1.50 | 63 | 19 | – | – | 1.7 | 413 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | askew shapes | 0.19 | 3.72 | 0.058 | 0.73 | 36.06 | 4.0 | 25 | 1.06 | 90 | 3 | – | – | 0.3 | 11 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | plain shapes | 0.10 | 2.07 | 0.043 | 0.30 | 0.26 | 6.0 | 42 | 1.41 | 59 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | flat colour art | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | gradient, photo | 5.00 | 5.28 | – | – | – | 13.0 | 829 | – | 37 | 28 | – | – | – | – | 6.9 | 8.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors, no Gradients | linear gradients | 2.07 | 3.07 | – | – | – | 6.2 | 266 | – | 20 | 14 | – | – | – | – | 2.4 | 9.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Gradients | curved shapes | 0.06 | 2.43 | 0.043 | 0.27 | 0.28 | 1.8 | 16 | 1.77 | 16 | 16 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Gradients | polygons | 0.06 | 2.40 | 0.041 | 0.42 | 1.39 | 2.0 | 16 | 1.45 | 86 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Gradients | thin lines | 0.49 | 4.76 | 0.108 | 0.70 | 45.31 | 4.0 | 30 | 1.50 | 63 | 19 | – | – | 1.7 | 413 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Gradients | askew shapes | 0.19 | 3.72 | 0.058 | 0.73 | 36.06 | 4.0 | 25 | 1.06 | 90 | 3 | – | – | 0.3 | 11 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Gradients | plain shapes | 0.10 | 2.07 | 0.043 | 0.30 | 0.26 | 6.0 | 42 | 1.41 | 59 | 49 | – | – | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| 6 Colors with Gradients | flat colour art | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors with Gradients | gradient, photo | 4.52 | 4.86 | – | – | – | 16.3 | 919 | – | 36 | 26 | – | – | – | – | 8.8 | 12.1 | 0.0 | 0.0 | – | – | – | – | – | 0.7 | – | – | – |
| 6 Colors with Gradients | linear gradients | 0.40 | 1.09 | – | – | – | 1.3 | 39 | – | 0 | 0 | – | – | – | – | 0.5 | 1.1 | 0.0 | 0.0 | – | – | – | – | – | 0.8 | 83 | 0.2 | 5 |
| High Fidelity Photo, no Gradients | curved shapes | 0.04 | 1.63 | 0.026 | 0.15 | 0.15 | 1.2 | 19 | 2.11 | 21 | 21 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | polygons | 0.06 | 2.33 | 0.039 | 0.50 | 1.32 | 1.7 | 17 | 1.47 | 88 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | thin lines | 0.35 | 6.05 | 0.085 | 0.90 | 4.35 | 40.3 | 348 | 17.42 | 35 | 9 | – | – | 27.3 | 4505 | 0.0 | – | 19.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | askew shapes | 0.17 | 3.97 | 0.100 | 0.94 | 1.73 | 8.7 | 157 | 6.54 | 71 | 5 | – | – | 4.3 | 3998 | 0.0 | – | 1.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | plain shapes | 0.08 | 1.66 | 0.035 | 0.25 | 0.28 | 6.0 | 50 | 1.68 | 62 | 51 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | flat colour art | 0.14 | 4.16 | 0.029 | 0.20 | 0.20 | 3.5 | 73 | 4.59 | 35 | 34 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | gradient, photo | 1.80 | 2.05 | – | – | – | 134.7 | 11212 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo, no Gradients | linear gradients | 0.64 | 3.29 | – | – | – | 34.5 | 4318 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo with Gradients | curved shapes | 0.04 | 1.63 | 0.026 | 0.15 | 0.15 | 1.2 | 19 | 2.11 | 21 | 21 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | polygons | 0.06 | 2.33 | 0.039 | 0.50 | 1.32 | 1.7 | 17 | 1.47 | 88 | 7 | 93 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | thin lines | 0.35 | 6.05 | 0.085 | 0.90 | 4.35 | 40.3 | 348 | 17.42 | 35 | 9 | – | – | 27.3 | 4505 | 0.0 | – | 19.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | askew shapes | 0.17 | 3.97 | 0.100 | 0.94 | 1.73 | 8.7 | 157 | 6.54 | 71 | 5 | – | – | 4.3 | 3998 | 0.0 | – | 1.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | plain shapes | 0.08 | 1.66 | 0.035 | 0.25 | 0.28 | 6.0 | 50 | 1.68 | 62 | 51 | – | – | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | flat colour art | 0.14 | 4.16 | 0.029 | 0.20 | 0.20 | 3.5 | 73 | 4.59 | 35 | 34 | 100 | 0.0 | 0.0 | 0 | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo with Gradients | gradient, photo | 1.80 | 2.04 | – | – | – | 140.7 | 10844 | – | 41 | 39 | – | – | – | – | 0.0 | – | 10.0 | 0.0 | – | – | – | – | – | 0.8 | – | – | – |
| High Fidelity Photo with Gradients | linear gradients | 0.23 | 1.19 | – | – | – | 6.0 | 252 | – | 0 | 0 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.8 | 83 | 0.2 | 5 |

## 3. Each preset on its inputs, by variant (256 px)

| preset | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke | shapes % | grouped % | gradient paths | found % | angle off | ramp off |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Default | clean | 0.07 | 2.96 | 0.052 | 0.47 | 1.26 | 1.9 | 14 | 1.43 | 46 | 10 | 96 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | aliased | 0.37 | 20.23 | 0.233 | 1.02 | 1.35 | 1.9 | 15 | 1.45 | 21 | 8 | 96 | 0.0 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | blurred | 0.81 | 16.55 | 0.077 | 0.98 | 2.18 | 1.9 | 24 | 2.39 | 7 | 5 | 23 | 1.3 | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | noisy | 1.74 | 4.55 | 0.056 | 0.46 | 1.25 | 1.9 | 15 | 1.48 | 40 | 0 | 96 | 0.0 | – | – | 1.2 | 0.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Default | scanned | 15.06 | 20.16 | 0.118 | 0.74 | 2.14 | 1.9 | 25 | 2.49 | 3 | 0 | 30 | 1.3 | – | – | 1.0 | 3.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | clean | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | blurred | 0.61 | 9.30 | 0.211 | 1.33 | 1.21 | 6.0 | 118 | 6.51 | 41 | 11 | 50 | 21.0 | 0.0 | 0 | 0.2 | 0.4 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | noisy | 0.85 | 2.62 | 0.075 | 0.32 | 0.32 | 3.0 | 28 | 2.04 | 35 | 0 | 100 | 0.0 | 0.0 | 0 | 0.3 | 0.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Auto Color | scanned | 1.73 | 4.43 | 0.230 | 1.20 | 0.94 | 7.0 | 102 | 5.72 | 30 | 0 | 88 | 21.0 | 0.0 | 0 | 9.6 | 7.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | clean | 1.80 | 2.05 | – | – | – | 134.7 | 11212 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | aliased | 1.78 | 2.01 | – | – | – | 73.0 | 8021 | – | 40 | 38 | – | – | – | – | 0.0 | – | 5.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | blurred | 1.59 | 1.95 | – | – | – | 72.0 | 7450 | – | 40 | 39 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | noisy | 2.02 | 2.19 | – | – | – | 155.5 | 10670 | – | 33 | 31 | – | – | – | – | 0.0 | – | 18.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| High Fidelity Photo | scanned | 1.45 | 1.55 | – | – | – | 140.5 | 9723 | – | 36 | 35 | – | – | – | – | 0.0 | – | 24.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | clean | 2.53 | 2.80 | – | – | – | 42.7 | 2307 | – | 43 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | aliased | 2.53 | 2.78 | – | – | – | 36.5 | 1505 | – | 46 | 38 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | blurred | 2.35 | 2.75 | – | – | – | 29.0 | 1990 | – | 44 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | noisy | 2.70 | 2.86 | – | – | – | 32.0 | 1972 | – | 43 | 38 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Low Fidelity Photo | scanned | 1.95 | 2.05 | – | – | – | 34.5 | 2550 | – | 42 | 33 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | clean | 0.06 | 2.00 | 0.070 | 0.31 | 0.30 | 2.0 | 11 | 1.78 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | aliased | 0.13 | 8.77 | 0.242 | 0.91 | 0.91 | 2.0 | 12 | 2.00 | 39 | 39 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | blurred | 0.34 | 7.83 | 0.073 | 0.24 | 0.24 | 2.0 | 9 | 1.50 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | noisy | 0.71 | 2.03 | 0.114 | 0.32 | 0.32 | 2.0 | 11 | 1.83 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 3 Colors | scanned | 1.57 | 3.22 | 0.089 | 0.30 | 0.30 | 2.0 | 12 | 2.00 | 39 | 0 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | clean | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | blurred | 0.62 | 9.31 | 0.208 | 1.34 | 1.19 | 6.0 | 100 | 5.61 | 41 | 11 | 50 | 21.5 | 0.0 | 0 | 0.8 | 0.4 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | noisy | 0.85 | 2.59 | 0.075 | 0.32 | 0.32 | 3.0 | 28 | 2.04 | 35 | 0 | 100 | 0.0 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 6 Colors | scanned | 1.74 | 4.42 | 0.229 | 1.20 | 0.94 | 7.0 | 102 | 5.80 | 30 | 0 | 88 | 21.0 | 0.0 | 0 | 0.8 | 1.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | clean | 0.12 | 2.53 | 0.051 | 0.31 | 0.30 | 3.0 | 28 | 2.11 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | aliased | 0.29 | 10.38 | 0.290 | 1.00 | 1.00 | 3.0 | 31 | 2.25 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | blurred | 0.62 | 9.36 | 0.216 | 1.31 | 1.19 | 7.0 | 139 | 7.53 | 44 | 13 | 50 | 25.5 | 0.0 | 0 | 0.2 | 0.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | noisy | 0.85 | 2.57 | 0.074 | 0.32 | 0.32 | 3.0 | 29 | 2.15 | 34 | 0 | 100 | 0.0 | 0.0 | 0 | 0.2 | 0.2 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| 16 Colors | scanned | 1.72 | 4.44 | 0.257 | 1.09 | 0.96 | 7.5 | 158 | 8.81 | 32 | 0 | 88 | 28.5 | 0.0 | 0 | 3.4 | 3.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | clean | 9.71 | 9.55 | – | – | – | 18.3 | 753 | – | 51 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | aliased | 9.71 | 9.52 | – | – | – | 11.0 | 647 | – | 57 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | blurred | 9.66 | 9.32 | – | – | – | 11.5 | 585 | – | 55 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | noisy | 9.74 | 9.44 | – | – | – | 19.5 | 648 | – | 46 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Shades of Gray | scanned | 6.58 | 6.46 | – | – | – | 19.0 | 764 | – | 47 | 37 | – | – | – | – | 0.0 | – | 1.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | clean | 0.04 | 1.86 | 0.031 | 0.47 | 1.26 | 1.4 | 17 | 1.74 | 52 | 15 | 96 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | aliased | 0.38 | 20.25 | 0.172 | 1.02 | 1.35 | 1.4 | 22 | 2.32 | 53 | 14 | 96 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | blurred | 0.80 | 16.26 | 0.050 | 0.83 | 2.17 | 1.4 | 39 | 3.84 | 22 | 12 | 23 | 1.1 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | noisy | 1.73 | 4.15 | 0.038 | 0.46 | 1.25 | 1.4 | 18 | 1.80 | 45 | 7 | 96 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Black and White Logo | scanned | 15.05 | 20.00 | 0.099 | 0.76 | 2.07 | 1.4 | 57 | 5.56 | 17 | 1 | 30 | 1.1 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | clean | 0.20 | 5.71 | 0.111 | 0.96 | 52.59 | 1.9 | 16 | 1.39 | 26 | 14 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | aliased | 0.47 | 19.58 | 0.213 | 1.00 | 105.54 | 1.6 | 11 | 1.10 | 26 | 9 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | blurred | 1.04 | 19.04 | 0.229 | 0.81 | 105.58 | 1.6 | 17 | 1.49 | 13 | 8 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | noisy | 1.76 | 6.07 | 0.113 | 0.97 | 1.06 | 2.0 | 17 | 1.44 | 17 | 0 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Sketched Art | scanned | 15.10 | 22.33 | 0.406 | 0.78 | 105.61 | 1.6 | 21 | 1.74 | 2 | 0 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | clean | 0.35 | 14.53 | 0.281 | 0.69 | 0.69 | 1.2 | 12 | 1.33 | 16 | 16 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | aliased | 0.39 | 21.69 | 0.289 | 1.00 | 1.00 | 1.2 | 9 | 0.99 | 12 | 9 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | blurred | 1.23 | 28.62 | 0.814 | 1.41 | 1.41 | 1.2 | 14 | 1.64 | 13 | 10 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | noisy | 2.12 | 11.10 | 0.285 | 0.68 | 0.69 | 1.2 | 12 | 1.30 | 9 | 0 | 100 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Silhouettes | scanned | 16.04 | 30.09 | 1.439 | 2.19 | 2.19 | 1.2 | 28 | 3.56 | 3 | 0 | 50 | 2.8 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | clean | 0.52 | 4.09 | 0.040 | 0.47 | 105.72 | 3.3 | 29 | 1.45 | 40 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | aliased | 0.67 | 14.44 | 0.112 | 0.73 | 55.44 | 14.0 | 59 | 2.95 | 87 | 1 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | blurred | 1.56 | 16.06 | 0.061 | 0.99 | 105.72 | 3.0 | 53 | 2.65 | 22 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | noisy | 1.03 | 4.10 | 0.035 | 0.44 | 56.07 | 8.0 | 50 | 2.50 | 43 | 0 | – | – | – | – | – | – | 1.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Line Art | scanned | 13.54 | 19.55 | 0.062 | 0.92 | 105.68 | 3.0 | 51 | 2.55 | 8 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | clean | 0.23 | 4.73 | 0.092 | 1.46 | 55.93 | 4.9 | 26 | 1.58 | 93 | 21 | 95 | 0.0 | – | – | – | – | 0.7 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | aliased | 0.50 | 19.60 | 0.238 | 1.37 | 55.44 | 6.2 | 31 | 1.85 | 96 | 23 | 94 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | blurred | 1.06 | 16.51 | 0.130 | 1.16 | 105.64 | 2.4 | 49 | 3.84 | 47 | 13 | 30 | 2.7 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | noisy | 1.59 | 5.74 | 0.103 | 1.17 | 31.79 | 7.0 | 34 | 1.98 | 93 | 21 | 95 | 0.0 | – | – | – | – | 1.4 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Technical Drawing | scanned | 14.69 | 20.15 | 0.136 | 1.17 | 105.57 | 2.4 | 73 | 5.42 | 49 | 12 | 39 | 3.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Outline | clean | 0.52 | 3.89 | 0.038 | 2.94 | 105.73 | 3.3 | 7 | 0.35 | 100 | 7 | – | – | – | – | – | – | 0.0 | 0.0 | 0.056 | 0.02 | 2.1 | – | – | 0.0 | – | – | – |
| Outline | aliased | 0.62 | 13.33 | 0.169 | 0.88 | 55.55 | 11.0 | 51 | 2.55 | 64 | 9 | – | – | – | – | – | – | 0.0 | 0.0 | 0.154 | 0.01 | 4.6 | – | – | 0.0 | – | – | – |
| Outline | blurred | 1.57 | 16.26 | 0.032 | 0.86 | 105.75 | 3.0 | 6 | 0.30 | 100 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | 0.013 | 0.03 | 2.0 | – | – | 0.0 | – | – | – |
| Outline | noisy | 0.95 | 3.50 | 0.029 | 0.72 | 55.94 | 8.0 | 16 | 0.80 | 100 | 2 | – | – | – | – | – | – | 0.0 | 0.0 | 0.010 | 0.02 | 2.0 | – | – | 0.0 | – | – | – |
| Outline | scanned | 13.53 | 19.37 | 0.044 | 0.88 | 105.64 | 3.0 | 6 | 0.30 | 100 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | 0.027 | 0.03 | 2.0 | – | – | 0.0 | – | – | – |
| Flat Logo | clean | 0.07 | 1.53 | 0.065 | 0.31 | 0.27 | 3.0 | 36 | 3.51 | 47 | 11 | 100 | 0.0 | 0.0 | 0 | 6.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | aliased | 0.13 | 5.87 | 0.218 | 0.65 | 0.65 | 3.0 | 31 | 1.96 | 44 | 19 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | blurred | 0.54 | 8.33 | 0.052 | 0.62 | 0.78 | 3.0 | 28 | 1.78 | 43 | 16 | 50 | 0.0 | 0.0 | 0 | 5.8 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | noisy | 0.81 | 1.77 | 0.076 | 0.39 | 0.32 | 3.5 | 44 | 2.63 | 31 | 0 | 100 | 1.0 | 0.0 | 0 | 6.9 | – | 0.5 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| Flat Logo | scanned | 1.31 | 4.15 | 21.204 | 88.50 | 0.66 | 470.5 | 2251 | 255.53 | 11 | 0 | 62 | 1658.0 | 0.0 | 0 | 58.0 | – | 413.5 | 0.2 | – | – | – | – | – | 0.0 | – | – | – |
| Default with Shapes | clean | 0.13 | 2.62 | 0.055 | 0.35 | 0.34 | 6.0 | 38 | 1.28 | 55 | 48 | – | – | – | – | 16.4 | 8.2 | 0.0 | 0.0 | – | – | – | 72 | – | 0.0 | – | – | – |
| Default with Shapes | aliased | 0.63 | 16.01 | 0.264 | 1.00 | 0.90 | 6.0 | 31 | 1.03 | 45 | 31 | – | – | – | – | 19.2 | 17.7 | 0.0 | 0.0 | – | – | – | 33 | – | 0.0 | – | – | – |
| Default with Shapes | blurred | 1.58 | 15.95 | 0.096 | 0.81 | 1.00 | 6.0 | 51 | 1.70 | 17 | 17 | – | – | – | – | 8.0 | 7.6 | 0.0 | 0.0 | – | – | – | 33 | – | 0.0 | – | – | – |
| Default with Shapes | noisy | 2.63 | 4.94 | 0.063 | 0.38 | 0.38 | 6.0 | 34 | 1.13 | 53 | 0 | – | – | – | – | 25.3 | 22.4 | 0.0 | 0.0 | – | – | – | 83 | – | 0.0 | – | – | – |
| Default with Shapes | scanned | 17.04 | 21.98 | 0.125 | 0.52 | 0.60 | 6.0 | 46 | 1.53 | 8 | 0 | – | – | – | – | 8.8 | 8.2 | 0.0 | 0.0 | – | – | – | 33 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | clean | 0.10 | 2.00 | 0.042 | 0.30 | 0.26 | 6.0 | 40 | 1.33 | 59 | 49 | – | – | 0.0 | 0 | 8.5 | 6.4 | 0.0 | 0.0 | – | – | – | 78 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | aliased | 0.62 | 15.75 | 0.234 | 0.99 | 0.99 | 6.0 | 37 | 1.23 | 46 | 32 | – | – | 0.0 | 0 | 7.5 | 10.1 | 0.0 | 0.0 | – | – | – | 17 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | blurred | 1.55 | 15.53 | 0.114 | 0.57 | 0.61 | 6.0 | 46 | 1.53 | 49 | 49 | – | – | 0.0 | 0 | 12.8 | 23.1 | 0.0 | 0.0 | – | – | – | 67 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | noisy | 1.78 | 3.44 | 0.047 | 0.28 | 0.29 | 6.0 | 36 | 1.20 | 54 | 0 | – | – | 0.0 | 0 | 6.7 | 15.9 | 0.0 | 0.0 | – | – | – | 83 | – | 0.0 | – | – | – |
| 6 Colors with Shapes | scanned | 2.13 | 7.02 | 0.101 | 0.53 | 0.60 | 6.0 | 62 | 2.07 | 17 | 0 | – | – | 0.0 | 0 | 19.3 | 8.2 | 0.0 | 0.0 | – | – | – | 33 | – | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | clean | 0.13 | 2.72 | 0.057 | 0.31 | 0.30 | 3.0 | 28 | 2.00 | 43 | 19 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 30 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | aliased | 0.29 | 10.43 | 0.298 | 1.00 | 1.00 | 3.0 | 27 | 2.05 | 43 | 43 | 100 | 0.0 | 0.0 | 0 | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | 30 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | blurred | 0.62 | 9.31 | 0.208 | 1.34 | 1.19 | 6.0 | 100 | 5.61 | 41 | 11 | 50 | 21.5 | 0.0 | 0 | 0.8 | 0.4 | 0.0 | 0.0 | – | – | – | – | 23 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | noisy | 0.85 | 2.59 | 0.075 | 0.32 | 0.32 | 3.0 | 28 | 2.04 | 35 | 0 | 100 | 0.0 | 0.0 | 0 | 0.3 | 0.3 | 0.0 | 0.0 | – | – | – | – | 30 | 0.0 | – | – | – |
| 6 Colors with Auto Grouping | scanned | 1.74 | 4.42 | 0.229 | 1.20 | 0.94 | 7.0 | 102 | 5.80 | 30 | 0 | 88 | 21.0 | 0.0 | 0 | 0.8 | 1.6 | 0.0 | 0.0 | – | – | – | – | 15 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | clean | 9.71 | 9.55 | – | – | – | 18.3 | 753 | – | 51 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 53 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | aliased | 9.71 | 9.52 | – | – | – | 11.0 | 647 | – | 57 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 54 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | blurred | 9.66 | 9.32 | – | – | – | 11.5 | 585 | – | 55 | 41 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 47 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | noisy | 9.74 | 9.44 | – | – | – | 19.5 | 648 | – | 46 | 37 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | 28 | 0.0 | – | – | – |
| Shades of Gray with Auto Grouping | scanned | 6.58 | 6.46 | – | – | – | 19.0 | 764 | – | 47 | 37 | – | – | – | – | 0.0 | – | 1.0 | 0.0 | – | – | – | – | 37 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | clean | 1.80 | 2.05 | – | – | – | 134.7 | 11212 | – | 39 | 37 | – | – | – | – | 0.0 | – | 7.3 | 0.0 | – | – | – | – | 69 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | aliased | 1.78 | 2.01 | – | – | – | 73.0 | 8021 | – | 40 | 38 | – | – | – | – | 0.0 | – | 5.5 | 0.0 | – | – | – | – | 65 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | blurred | 1.59 | 1.95 | – | – | – | 72.0 | 7450 | – | 40 | 39 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | 65 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | noisy | 2.02 | 2.19 | – | – | – | 155.5 | 10670 | – | 33 | 31 | – | – | – | – | 0.0 | – | 18.5 | 0.0 | – | – | – | – | 72 | 0.0 | – | – | – |
| High Fidelity Photo with Auto Grouping | scanned | 1.45 | 1.55 | – | – | – | 140.5 | 9723 | – | 36 | 35 | – | – | – | – | 0.0 | – | 24.5 | 0.0 | – | – | – | – | 75 | 0.0 | – | – | – |
| 6 Colors, no Gradients | clean | 2.07 | 3.07 | – | – | – | 6.2 | 266 | – | 20 | 14 | – | – | – | – | 2.4 | 9.3 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors, no Gradients | aliased | 2.04 | 3.07 | – | – | – | 6.5 | 500 | – | 22 | 14 | – | – | – | – | 0.0 | 0.6 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors, no Gradients | blurred | 2.08 | 3.69 | – | – | – | 6.0 | 344 | – | 23 | 14 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors, no Gradients | noisy | 2.36 | 2.87 | – | – | – | 8.0 | 864 | – | 19 | 12 | – | – | – | – | 10.5 | 30.5 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors, no Gradients | scanned | 2.16 | 2.41 | – | – | – | 6.0 | 776 | – | 20 | 12 | – | – | – | – | 9.8 | 30.4 | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| 6 Colors with Gradients | clean | 0.40 | 1.09 | – | – | – | 1.3 | 39 | – | 0 | 0 | – | – | – | – | 0.5 | 1.1 | 0.0 | 0.0 | – | – | – | – | – | 0.8 | 83 | 0.2 | 5 |
| 6 Colors with Gradients | aliased | 0.22 | 0.77 | – | – | – | 0.5 | 6 | – | 0 | 0 | – | – | – | – | 0.0 | 0.0 | 0.0 | 0.0 | – | – | – | – | – | 1.0 | 100 | 0.2 | 5 |
| 6 Colors with Gradients | blurred | 0.33 | 2.29 | – | – | – | 3.0 | 161 | – | 8 | 0 | – | – | – | – | 2.3 | 5.1 | 0.0 | 0.0 | – | – | – | – | – | 1.0 | 100 | 0.3 | 6 |
| 6 Colors with Gradients | noisy | 1.01 | 1.36 | – | – | – | 11.0 | 321 | – | 9 | 1 | – | – | – | – | 5.6 | 8.0 | 0.0 | 0.0 | – | – | – | – | – | 2.5 | 100 | 0.2 | 8 |
| 6 Colors with Gradients | scanned | 1.57 | 1.82 | – | – | – | 5.5 | 279 | – | 23 | 14 | – | – | – | – | 3.2 | 2.4 | 0.0 | 0.0 | – | – | – | – | – | 0.5 | 50 | 1.5 | 21 |
| High Fidelity Photo, no Gradients | clean | 0.64 | 3.29 | – | – | – | 34.5 | 4318 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo, no Gradients | aliased | 0.67 | 3.46 | – | – | – | 37.5 | 3184 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo, no Gradients | blurred | 0.72 | 3.25 | – | – | – | 33.5 | 3324 | – | 32 | 31 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo, no Gradients | noisy | 1.15 | 2.67 | – | – | – | 37.5 | 3468 | – | 29 | 28 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo, no Gradients | scanned | 1.05 | 1.62 | – | – | – | 54.5 | 3858 | – | 29 | 28 | – | – | – | – | 0.0 | – | 7.5 | 0.0 | – | – | – | – | – | 0.0 | 0 | – | – |
| High Fidelity Photo with Gradients | clean | 0.23 | 1.19 | – | – | – | 6.0 | 252 | – | 0 | 0 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 0.8 | 83 | 0.2 | 5 |
| High Fidelity Photo with Gradients | aliased | 0.21 | 0.52 | – | – | – | 0.5 | 24 | – | 0 | 0 | – | – | – | – | 0.0 | – | 0.0 | 0.0 | – | – | – | – | – | 1.0 | 100 | 0.2 | 5 |
| High Fidelity Photo with Gradients | blurred | 0.33 | 2.29 | – | – | – | 55.5 | 940 | – | 4 | 0 | – | – | – | – | 0.0 | – | 13.5 | 0.0 | – | – | – | – | – | 1.0 | 100 | 0.3 | 6 |
| High Fidelity Photo with Gradients | noisy | 1.00 | 1.24 | – | – | – | 23.5 | 652 | – | 21 | 7 | – | – | – | – | 0.0 | – | 0.5 | 0.0 | – | – | – | – | – | 2.0 | 100 | 0.2 | 8 |
| High Fidelity Photo with Gradients | scanned | 0.97 | 1.55 | – | – | – | 46.5 | 2942 | – | 18 | 15 | – | – | – | – | 0.0 | – | 6.0 | 0.0 | – | – | – | – | – | 0.5 | 50 | 1.5 | 21 |

## 4. Yardstick

Inkscape 1.4.2's Trace Bitmap, run as a black box through its `object-trace` action (two scans, background
removed, its other settings as they come) on the same one-colour drawings, and its SVG output measured by
the same code. Compare with the black-and-white presets on the same kinds of input in section 2 and 3.

| inputs | variant | ΔE | edge ΔE | off mean | off max | missed max | paths | anchors | × art | lines % | upright % | corners % | kinks | odd colours | odd px | gap px | overlap px | specks | outside px | centre off | width off | anchors a stroke | shapes % | grouped % | gradient paths | found % | angle off | ramp off |
|:--|:--|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| curved shapes | clean | 0.25 | 11.05 | 0.198 | 0.97 | 0.97 | 1.0 | 15 | 1.98 | 11 | 7 | 83 | 1.8 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| curved shapes | aliased | 0.38 | 20.79 | 0.192 | 0.88 | 0.88 | 1.0 | 16 | 1.88 | 14 | 8 | 50 | 1.8 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| curved shapes | blurred | 0.79 | 16.57 | 0.182 | 0.86 | 1.14 | 1.0 | 15 | 1.84 | 6 | 5 | 0 | 1.8 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| curved shapes | noisy | 2.01 | 8.14 | 0.191 | 0.93 | 0.93 | 1.0 | 17 | 2.06 | 10 | 0 | 75 | 2.8 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| curved shapes | scanned | 15.31 | 20.22 | 0.191 | 0.97 | 1.14 | 1.0 | 16 | 1.98 | 7 | 2 | 0 | 2.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| polygons | clean | 0.16 | 6.45 | 0.124 | 1.26 | 2.36 | 1.0 | 21 | 1.93 | 27 | 4 | 34 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| polygons | aliased | 0.35 | 18.33 | 0.150 | 1.22 | 2.35 | 1.0 | 20 | 1.81 | 20 | 3 | 28 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| polygons | blurred | 0.90 | 17.54 | 0.190 | 1.20 | 2.56 | 1.0 | 19 | 1.75 | 0 | 0 | 0 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| polygons | noisy | 1.62 | 6.05 | 0.119 | 1.22 | 2.32 | 1.0 | 22 | 2.01 | 29 | 4 | 19 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| polygons | scanned | 14.75 | 20.30 | 0.170 | 1.49 | 2.55 | 1.0 | 21 | 1.90 | 4 | 0 | 3 | 0.0 | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| thin lines | clean | 0.44 | 6.05 | 0.109 | 0.98 | 76.81 | 1.0 | 34 | 1.72 | 2 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| thin lines | aliased | 0.74 | 16.37 | 0.099 | 1.17 | 55.97 | 1.0 | 22 | 1.10 | 6 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| thin lines | blurred | 1.72 | 18.80 | 0.254 | 0.81 | 106.00 | 1.0 | 18 | 0.90 | 0 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| thin lines | noisy | 1.02 | 6.25 | 0.112 | 1.00 | 37.89 | 1.0 | 25 | 1.25 | 0 | 0 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| thin lines | scanned | 28.94 | 46.83 | 12.147 | 43.11 | 20.79 | 1.0 | 2763 | 138.15 | 2 | 0 | – | – | – | – | – | – | 0.0 | 0.1 | – | – | – | – | – | 0.0 | – | – | – |
| askew shapes | clean | 0.31 | 7.76 | 0.142 | 1.08 | 3.38 | 1.0 | 45 | 1.89 | 32 | 15 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| askew shapes | aliased | 0.45 | 14.06 | 0.149 | 1.14 | 1.35 | 1.0 | 47 | 1.96 | 30 | 4 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| askew shapes | blurred | 1.19 | 17.05 | 0.195 | 1.20 | 3.65 | 1.0 | 44 | 1.83 | 29 | 11 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| askew shapes | noisy | 2.03 | 6.74 | 0.116 | 0.85 | 1.04 | 1.0 | 62 | 2.58 | 37 | 7 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| askew shapes | scanned | 15.69 | 20.78 | 0.217 | 1.47 | 3.65 | 1.0 | 44 | 1.83 | 26 | 8 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | – | – | 0.0 | – | – | – |
| plain shapes | clean | 0.56 | 11.27 | 0.218 | 1.00 | 1.02 | 1.0 | 46 | 1.54 | 46 | 36 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| plain shapes | aliased | 0.57 | 14.69 | 0.239 | 1.00 | 1.00 | 1.0 | 49 | 1.63 | 47 | 25 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| plain shapes | blurred | 1.68 | 17.28 | 0.262 | 1.14 | 1.14 | 1.0 | 45 | 1.50 | 31 | 8 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| plain shapes | noisy | 2.94 | 9.59 | 0.234 | 0.98 | 0.98 | 1.0 | 53 | 1.77 | 50 | 25 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |
| plain shapes | scanned | 17.09 | 22.56 | 0.281 | 0.99 | 1.10 | 1.0 | 51 | 1.70 | 31 | 8 | – | – | – | – | – | – | 0.0 | 0.0 | – | – | – | 0 | – | 0.0 | – | – | – |

## 5. Cost

One trace at a time on this machine (all cores); memory is the peak allocated above the image itself. A trace of more than 2,000,000 anchors is refused, as in the app.

| preset | image | megapixels | result |
|:--|:--|--:|:--|
| Default | photo-like | 1 | 0.04 s, 29 paths, 6278 anchors, 12 MB |
| Default | four-colour badge | 1 | 0.02 s, 3 paths, 64 anchors, 12 MB |
| Auto Color | photo-like | 1 | 0.16 s, 732 paths, 45265 anchors, 23 MB |
| Auto Color | four-colour badge | 1 | 0.06 s, 5 paths, 84 anchors, 12 MB |
| High Fidelity Photo | photo-like | 1 | 0.25 s, 968 paths, 117041 anchors, 164 MB |
| High Fidelity Photo | four-colour badge | 1 | 0.06 s, 6 paths, 172 anchors, 23 MB |
| Low Fidelity Photo | photo-like | 1 | 0.15 s, 129 paths, 23014 anchors, 104 MB |
| Low Fidelity Photo | four-colour badge | 1 | 0.07 s, 6 paths, 136 anchors, 23 MB |
| 3 Colors | photo-like | 1 | 0.11 s, 9 paths, 3341 anchors, 18 MB |
| 3 Colors | four-colour badge | 1 | 0.07 s, 4 paths, 88 anchors, 13 MB |
| 6 Colors | photo-like | 1 | 0.10 s, 78 paths, 9076 anchors, 24 MB |
| 6 Colors | four-colour badge | 1 | 0.07 s, 5 paths, 84 anchors, 12 MB |
| 16 Colors | photo-like | 1 | 0.18 s, 178 paths, 20470 anchors, 32 MB |
| 16 Colors | four-colour badge | 1 | 0.07 s, 5 paths, 88 anchors, 12 MB |
| Shades of Gray | photo-like | 1 | 0.11 s, 81 paths, 6698 anchors, 40 MB |
| Shades of Gray | four-colour badge | 1 | 0.08 s, 10 paths, 725 anchors, 34 MB |
| Black and White Logo | photo-like | 1 | 0.05 s, 71 paths, 4933 anchors, 8 MB |
| Black and White Logo | four-colour badge | 1 | 0.02 s, 1 paths, 47 anchors, 6 MB |
| Sketched Art | photo-like | 1 | 0.05 s, 5 paths, 2873 anchors, 8 MB |
| Sketched Art | four-colour badge | 1 | 0.02 s, 2 paths, 114 anchors, 6 MB |
| Silhouettes | photo-like | 1 | 0.04 s, 2 paths, 556 anchors, 9 MB |
| Silhouettes | four-colour badge | 1 | 0.02 s, 1 paths, 24 anchors, 6 MB |
| Line Art | photo-like | 1 | 0.05 s, 54 paths, 4495 anchors, 8 MB |
| Line Art | four-colour badge | 1 | 0.02 s, 1 paths, 38 anchors, 6 MB |
| Technical Drawing | photo-like | 1 | 0.05 s, 221 paths, 6258 anchors, 8 MB |
| Technical Drawing | four-colour badge | 1 | 0.02 s, 1 paths, 47 anchors, 6 MB |
| Outline | photo-like | 1 | 0.06 s, 45 paths, 4110 anchors, 16 MB |
| Outline | four-colour badge | 1 | 0.03 s, 1 paths, 38 anchors, 11 MB |
| Flat Logo | photo-like | 1 | refused after 0.11 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 1 | 0.80 s, 5 paths, 78 anchors, 146 MB |
| Default with Shapes | photo-like | 1 | 0.11 s, 29 paths, 6278 anchors, 12 MB |
| Default with Shapes | four-colour badge | 1 | 0.04 s, 3 paths, 63 anchors, 12 MB |
| 6 Colors with Shapes | photo-like | 1 | 0.09 s, 78 paths, 9076 anchors, 24 MB |
| 6 Colors with Shapes | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 12 MB |
| 6 Colors with Auto Grouping | photo-like | 1 | 0.08 s, 78 paths, 9076 anchors, 24 MB |
| 6 Colors with Auto Grouping | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 13 MB |
| Shades of Gray with Auto Grouping | photo-like | 1 | 0.14 s, 81 paths, 6698 anchors, 40 MB |
| Shades of Gray with Auto Grouping | four-colour badge | 1 | 0.07 s, 10 paths, 725 anchors, 35 MB |
| High Fidelity Photo with Auto Grouping | photo-like | 1 | 0.25 s, 968 paths, 117041 anchors, 169 MB |
| High Fidelity Photo with Auto Grouping | four-colour badge | 1 | 0.08 s, 6 paths, 172 anchors, 23 MB |
| 6 Colors, no Gradients | photo-like | 1 | 0.49 s, 78 paths, 9076 anchors, 24 MB |
| 6 Colors, no Gradients | four-colour badge | 1 | 0.05 s, 5 paths, 84 anchors, 12 MB |
| 6 Colors with Gradients | photo-like | 1 | 0.18 s, 78 paths, 9076 anchors, 32 MB |
| 6 Colors with Gradients | four-colour badge | 1 | 0.11 s, 5 paths, 84 anchors, 12 MB |
| High Fidelity Photo, no Gradients | photo-like | 1 | 0.23 s, 968 paths, 117041 anchors, 163 MB |
| High Fidelity Photo, no Gradients | four-colour badge | 1 | 0.07 s, 6 paths, 172 anchors, 23 MB |
| High Fidelity Photo with Gradients | photo-like | 1 | 0.32 s, 968 paths, 117041 anchors, 168 MB |
| High Fidelity Photo with Gradients | four-colour badge | 1 | 0.10 s, 6 paths, 172 anchors, 23 MB |
| Default | photo-like | 4 | 0.21 s, 48 paths, 14438 anchors, 49 MB |
| Default | four-colour badge | 4 | 0.11 s, 3 paths, 72 anchors, 48 MB |
| Auto Color | photo-like | 4 | 0.44 s, 1469 paths, 149389 anchors, 115 MB |
| Auto Color | four-colour badge | 4 | 0.21 s, 5 paths, 92 anchors, 46 MB |
| High Fidelity Photo | photo-like | 4 | 0.78 s, 4479 paths, 337884 anchors, 623 MB |
| High Fidelity Photo | four-colour badge | 4 | 0.29 s, 6 paths, 200 anchors, 97 MB |
| Low Fidelity Photo | photo-like | 4 | 0.41 s, 299 paths, 61601 anchors, 406 MB |
| Low Fidelity Photo | four-colour badge | 4 | 0.24 s, 6 paths, 152 anchors, 97 MB |
| 3 Colors | photo-like | 4 | 0.31 s, 20 paths, 8473 anchors, 73 MB |
| 3 Colors | four-colour badge | 4 | 0.24 s, 4 paths, 122 anchors, 53 MB |
| 6 Colors | photo-like | 4 | 0.34 s, 115 paths, 23295 anchors, 100 MB |
| 6 Colors | four-colour badge | 4 | 0.22 s, 5 paths, 92 anchors, 46 MB |
| 16 Colors | photo-like | 4 | 0.37 s, 353 paths, 52770 anchors, 130 MB |
| 16 Colors | four-colour badge | 4 | 0.22 s, 5 paths, 96 anchors, 46 MB |
| Shades of Gray | photo-like | 4 | 0.50 s, 137 paths, 18823 anchors, 162 MB |
| Shades of Gray | four-colour badge | 4 | 0.33 s, 10 paths, 1256 anchors, 137 MB |
| Black and White Logo | photo-like | 4 | 0.25 s, 208 paths, 13546 anchors, 32 MB |
| Black and White Logo | four-colour badge | 4 | 0.11 s, 1 paths, 57 anchors, 24 MB |
| Sketched Art | photo-like | 4 | 0.24 s, 9 paths, 5387 anchors, 32 MB |
| Sketched Art | four-colour badge | 4 | 0.11 s, 2 paths, 196 anchors, 24 MB |
| Silhouettes | photo-like | 4 | 0.24 s, 2 paths, 1465 anchors, 36 MB |
| Silhouettes | four-colour badge | 4 | 0.09 s, 1 paths, 28 anchors, 24 MB |
| Line Art | photo-like | 4 | 0.22 s, 127 paths, 11330 anchors, 32 MB |
| Line Art | four-colour badge | 4 | 0.09 s, 1 paths, 42 anchors, 24 MB |
| Technical Drawing | photo-like | 4 | 0.21 s, 931 paths, 20628 anchors, 32 MB |
| Technical Drawing | four-colour badge | 4 | 0.08 s, 1 paths, 57 anchors, 24 MB |
| Outline | photo-like | 4 | 0.25 s, 113 paths, 10815 anchors, 64 MB |
| Outline | four-colour badge | 4 | 0.13 s, 1 paths, 42 anchors, 44 MB |
| Flat Logo | photo-like | 4 | refused after 0.10 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 4 | 0.67 s, 5 paths, 69 anchors, 146 MB |
| Default with Shapes | photo-like | 4 | 0.20 s, 48 paths, 14438 anchors, 49 MB |
| Default with Shapes | four-colour badge | 4 | 0.10 s, 3 paths, 72 anchors, 48 MB |
| 6 Colors with Shapes | photo-like | 4 | 0.31 s, 115 paths, 23295 anchors, 100 MB |
| 6 Colors with Shapes | four-colour badge | 4 | 0.21 s, 5 paths, 92 anchors, 46 MB |
| 6 Colors with Auto Grouping | photo-like | 4 | 0.40 s, 115 paths, 23295 anchors, 100 MB |
| 6 Colors with Auto Grouping | four-colour badge | 4 | 0.22 s, 5 paths, 92 anchors, 46 MB |
| Shades of Gray with Auto Grouping | photo-like | 4 | 0.38 s, 137 paths, 18823 anchors, 165 MB |
| Shades of Gray with Auto Grouping | four-colour badge | 4 | 0.29 s, 10 paths, 1256 anchors, 138 MB |
| High Fidelity Photo with Auto Grouping | photo-like | 4 | 1.06 s, 4479 paths, 337884 anchors, 622 MB |
| High Fidelity Photo with Auto Grouping | four-colour badge | 4 | 0.28 s, 6 paths, 200 anchors, 97 MB |
| 6 Colors, no Gradients | photo-like | 4 | 0.36 s, 115 paths, 23295 anchors, 100 MB |
| 6 Colors, no Gradients | four-colour badge | 4 | 0.26 s, 5 paths, 92 anchors, 46 MB |
| 6 Colors with Gradients | photo-like | 4 | 0.73 s, 115 paths, 23295 anchors, 128 MB |
| 6 Colors with Gradients | four-colour badge | 4 | 0.40 s, 5 paths, 92 anchors, 46 MB |
| High Fidelity Photo, no Gradients | photo-like | 4 | 0.98 s, 4479 paths, 337884 anchors, 626 MB |
| High Fidelity Photo, no Gradients | four-colour badge | 4 | 0.29 s, 6 paths, 200 anchors, 97 MB |
| High Fidelity Photo with Gradients | photo-like | 4 | 1.22 s, 4479 paths, 337884 anchors, 629 MB |
| High Fidelity Photo with Gradients | four-colour badge | 4 | 0.43 s, 6 paths, 200 anchors, 97 MB |
| Default | photo-like | 16 | 1.24 s, 127 paths, 34034 anchors, 199 MB |
| Default | four-colour badge | 16 | 1.54 s, 3 paths, 88 anchors, 184 MB |
| Auto Color | photo-like | 16 | 2.44 s, 5390 paths, 454670 anchors, 272 MB |
| Auto Color | four-colour badge | 16 | 1.08 s, 5 paths, 112 anchors, 184 MB |
| High Fidelity Photo | photo-like | 16 | 4.70 s, 31139 paths, 1192715 anchors, 880 MB |
| High Fidelity Photo | four-colour badge | 16 | 1.27 s, 6 paths, 264 anchors, 257 MB |
| Low Fidelity Photo | photo-like | 16 | 2.21 s, 2036 paths, 182778 anchors, 754 MB |
| Low Fidelity Photo | four-colour badge | 16 | 1.25 s, 6 paths, 180 anchors, 257 MB |
| 3 Colors | photo-like | 16 | 1.38 s, 76 paths, 23024 anchors, 290 MB |
| 3 Colors | four-colour badge | 16 | 0.93 s, 4 paths, 190 anchors, 205 MB |
| 6 Colors | photo-like | 16 | 1.38 s, 470 paths, 60831 anchors, 298 MB |
| 6 Colors | four-colour badge | 16 | 0.91 s, 5 paths, 112 anchors, 184 MB |
| 16 Colors | photo-like | 16 | 1.40 s, 1973 paths, 145642 anchors, 304 MB |
| 16 Colors | four-colour badge | 16 | 0.99 s, 5 paths, 114 anchors, 184 MB |
| Shades of Gray | photo-like | 16 | 1.94 s, 686 paths, 53654 anchors, 407 MB |
| Shades of Gray | four-colour badge | 16 | 1.59 s, 9 paths, 2208 anchors, 270 MB |
| Black and White Logo | photo-like | 16 | 1.32 s, 1128 paths, 45273 anchors, 129 MB |
| Black and White Logo | four-colour badge | 16 | 0.54 s, 1 paths, 69 anchors, 88 MB |
| Sketched Art | photo-like | 16 | 1.30 s, 15 paths, 11486 anchors, 128 MB |
| Sketched Art | four-colour badge | 16 | 0.56 s, 2 paths, 361 anchors, 88 MB |
| Silhouettes | photo-like | 16 | 1.94 s, 3 paths, 3350 anchors, 144 MB |
| Silhouettes | four-colour badge | 16 | 0.61 s, 1 paths, 32 anchors, 95 MB |
| Line Art | photo-like | 16 | 1.55 s, 713 paths, 37370 anchors, 129 MB |
| Line Art | four-colour badge | 16 | 0.60 s, 1 paths, 49 anchors, 88 MB |
| Technical Drawing | photo-like | 16 | 1.78 s, 4577 paths, 76559 anchors, 130 MB |
| Technical Drawing | four-colour badge | 16 | 0.59 s, 1 paths, 69 anchors, 88 MB |
| Outline | photo-like | 16 | 1.83 s, 632 paths, 34884 anchors, 256 MB |
| Outline | four-colour badge | 16 | 0.91 s, 1 paths, 49 anchors, 176 MB |
| Flat Logo | photo-like | 16 | refused after 0.20 s (not flat art), 40 MB |
| Flat Logo | four-colour badge | 16 | 1.17 s, 5 paths, 67 anchors, 146 MB |
| Default with Shapes | photo-like | 16 | 1.89 s, 127 paths, 34034 anchors, 199 MB |
| Default with Shapes | four-colour badge | 16 | 1.81 s, 3 paths, 88 anchors, 184 MB |
| 6 Colors with Shapes | photo-like | 16 | 3.30 s, 470 paths, 60831 anchors, 298 MB |
| 6 Colors with Shapes | four-colour badge | 16 | 1.73 s, 5 paths, 112 anchors, 185 MB |
| 6 Colors with Auto Grouping | photo-like | 16 | 2.14 s, 470 paths, 60831 anchors, 299 MB |
| 6 Colors with Auto Grouping | four-colour badge | 16 | 1.50 s, 5 paths, 112 anchors, 185 MB |
| Shades of Gray with Auto Grouping | photo-like | 16 | 2.71 s, 686 paths, 53654 anchors, 407 MB |
| Shades of Gray with Auto Grouping | four-colour badge | 16 | 1.85 s, 9 paths, 2208 anchors, 270 MB |
| High Fidelity Photo with Auto Grouping | photo-like | 16 | 10.82 s, 31139 paths, 1192715 anchors, 880 MB |
| High Fidelity Photo with Auto Grouping | four-colour badge | 16 | 1.69 s, 6 paths, 264 anchors, 257 MB |
| 6 Colors, no Gradients | photo-like | 16 | 1.81 s, 470 paths, 60831 anchors, 299 MB |
| 6 Colors, no Gradients | four-colour badge | 16 | 1.26 s, 5 paths, 112 anchors, 185 MB |
| 6 Colors with Gradients | photo-like | 16 | 4.20 s, 470 paths, 60831 anchors, 512 MB |
| 6 Colors with Gradients | four-colour badge | 16 | 1.76 s, 5 paths, 112 anchors, 185 MB |
| High Fidelity Photo, no Gradients | photo-like | 16 | 5.32 s, 31139 paths, 1192715 anchors, 880 MB |
| High Fidelity Photo, no Gradients | four-colour badge | 16 | 1.66 s, 6 paths, 264 anchors, 257 MB |
| High Fidelity Photo with Gradients | photo-like | 16 | 7.38 s, 31139 paths, 1192715 anchors, 880 MB |
| High Fidelity Photo with Gradients | four-colour badge | 16 | 1.79 s, 6 paths, 264 anchors, 263 MB |

## Revision history

| date | change |
|:--|:--|
| 2026-10-13 | Snap Curves To Lines moves no end of a line by more than 1.6 px: long lines a degree or two off stay as the image has them (Technical Drawing: largest distance 4.21 px to 1.46) |
| 2026-10-13 | Gradients (off unless asked for): areas that are a linear gradient are found before the palette is made and traced as one path with a gradient fill; linear-gradient inputs with known lines and stops, and four columns for them |
| 2026-10-13 | Auto Grouping (off unless asked for): each path that has others inside it is grouped with them; a "grouped %" column, and three presets measured with the option on |
| 2026-10-13 | Shapes (off unless asked for): filled outlines that are a circle, an ellipse, a rectangle or a regular polygon come out as that shape; corners whose pixel is mostly outside the shape are corners; plain-shape inputs and a "shapes %" column |
| 2026-10-13 | Create: Strokes: centre lines placed in the middle of their ink and as wide as it, fitted as outlines are; centre-line distance, width error and anchors per stroke measured against the thin lines' truth |
| 2026-10-13 | Snap Curves To Lines levels lines within 2.5 degrees of level or upright; askew inputs and an "upright %" column, so the baseline was measured again with them (the tracer of main at `baf9693f`) |
| 2026-10-13 | Ignore Color; the Auto Color and Outline presets, measured from here on (Outline is the first measure of Create: Strokes) |
| 2026-10-13 | Thin lines: black and white read against the image's darkest black and the paper near each edge |
| 2026-10-13 | Memory: layers traced within the box of their pixels and a few at a time (16 megapixel photo: 3.9 GB to 0.9 GB) |
| 2026-10-13 | Shared edges: outlines are cut at their junctions and each piece is fitted once, the same from either side |
| 2026-10-13 | Photos: continuous-tone images are smoothed within their tones before quantising, and stacked layers go by lightness |
| 2026-10-13 | The palette: made of the picture's own colours, noise-made colours merged, blend pixels settled, cuts where colours differ most, colours compared in Oklab |
| 2026-10-13 | Sub-pixel edges and the outline fitter in Black and White, Grayscale and Color; the harness gains an aliased variant and an "outside px" column, so the baseline was measured again with it (same tracer, main at `baf9693f`) |
| 2026-10-12 | The harness and the baseline: every preset measured before any change to the tracer |
