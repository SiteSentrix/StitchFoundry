# vectorcraft-affinity

An independent, bounded Rust reader for native Affinity documents: `.af` from Affinity 3 and
`.afdesign`, `.afphoto` and `.afpub` from Affinity 1 and 2 (container versions 8 to 12). It reads
the archive, the tagged object stream in `doc.dat` and the part of the document model listed below,
and hands the engine a document in pixels with every transform composed. It does not write
Affinity files.

## What is imported

| Affinity | VectorCraft |
|---|---|
| Pages, Publisher spreads (one or two pages) | artboards, spreads side by side |
| Artboards (`ShpN` with `ABEn`), with name, background fill, any transform | artboards plus a clip group with the background |
| Layers (`Scop`), groups (`Grup`), pass-through or isolated | layers and sublayers, groups |
| Curves (`PCrv`): cubic subpaths, closed flags, live corners (`CnrD`) | paths (multi-subpath curves fill even-odd) |
| Rectangles with corner radii (relative or absolute), ellipses, polygons (smooth too), stars, square stars, pies, triangles, trapezoids | paths |
| Compound shapes (`Comp`, add and subtract) | one even-odd path |
| A shape or curve with children clips them | clip group whose clipping path keeps its fill (behind) and stroke (over) |
| Vector masks (`AdCh` curves and shapes) | clip group |
| Pixel masks (`MRst`, attached or as a layer in a group) | opacity mask |
| Solid fills: RGBA, HSLA, CMYK, grey, Lab (D50) | fill appearance (alpha as fill opacity) |
| Linear, elliptical and radial gradients with Affinity's midpoint bias | gradients (the bias as the midpoint where the blend is half way) |
| Strokes: weight, scale with object, dash pattern and phase, alignment, miter limit, behind the fill | stroke appearance |
| Fill layers (`FRst`) | filled rectangles |
| Artistic and frame text: characters, font family/weight/style, size, tracking, fixed leading, colour, paragraph alignment, first baseline | point type and area type |
| Placed images (`ImgN`): the original JPEG/PNG embedded in the file | embedded images |
| Pixel layers (`Rstr`): RGBA 8/16-bit and CMYK 8-bit tiles, cropped to their content | embedded PNG images |
| Embedded documents and symbols (`EmbN`) | the picture Affinity cached of them |
| Opacity, visibility, lock, names, the blend modes VectorCraft has | the same |

Everything else is reported in the import warnings, one line per kind with a count, never
dropped silently: layer effects, adjustment layers and live filters, brush and pressure strokes,
transparency gradients, fill opacity, bitmap fills, master pages, conical gradients (as radial),
special shapes (cloud, heart, cog, callouts, arrows…, imported as their bounding ellipse), corner
types other than round, stars with rounded points, several fills or strokes on one object (the
active one is used), outlined or scaled text, text fields such as page numbers, frames with
columns or a curved outline, grey/Lab/32-bit pixels, CMYK pixels (converted to RGB without the
document's profile) and Affinity-only blend modes (Add, Linear Light… as Normal).

When the native document can't be read at all, the engine opens the embedded PNG preview instead,
with a warning that names the reason; that preview can't be placed, used as a template or mined
for swatches and styles. Save never writes back over the Affinity file.

## Provenance

The container and object-stream layout was learned from [VMDevCpp/afread](https://github.com/VMDevCpp/afread)
(MIT, at `04b672334a43e3e37ded6b5ffc57af231d589774`, written for container versions 7–11) and
re-described in our own words before this Rust code was written; no code was translated. Its
meaning was then worked out from public documents only, without running Affinity: the semantics of
shapes, paints, text and pixel data were fitted to the thumbnail every Affinity document embeds
(Affinity's own render of it) on 176 public documents saved by Affinity 1.x, 2.x and 3.0/3.1 on
Windows, macOS and iPad. No GPL/AGPL code (such as Inkscape's Affinity extension) was read, and no
Affinity application code, asset or document is part of this repository.

## Validation

* Unit tests on synthetic containers built by the `synth` feature (stored, zlib and zstd entries,
  checksums, budgets, cycles, hostile streams, every truncation) and property tests of random
  mutations.
* `cargo xtask corpus --affinity` fetches 22 public CC0/MIT/Apache-2.0 documents, including the
  four Affinity 3 `.af` files of [samuel-etver/vector-art](https://github.com/samuel-etver/vector-art)
  (CC0), at pinned commits, each checked against `xtask/affinity-corpus.sha256`. `tests/real_files.rs`
  parses them; `engine/tests/affinity_corpus.rs` renders each one and compares it with the
  thumbnail Affinity saved in it (mean difference 0–4.4 of 255 at the thumbnail's size, with a
  ceiling per file).
* `engine/tests/import_fuzz.rs` mutates a synthetic native document's stream and archive and checks
  that whatever opens also renders and exports; `fuzz/` has `cargo-fuzz` targets for the whole
  reader (`container`) and the object stream (`stream`):

  ```sh
  cd crates/affinity
  cargo +nightly fuzz run stream -- -max_total_time=600
  ```

What has not been verified: files from Affinity builds or platforms outside the public set, files
written by other applications, rotated or skewed images against Affinity's render, mask polarity
against a render with and without the mask, and anything listed as a warning above. The `synth`
builders write only what this reader accepts; Affinity has never opened their output, so they are
test fixtures, not an exporter.

## Safety limits

Every size is checked before it is allocated: 256 MiB per archive entry and 1 GiB per import by
default (`Limits`), a 64 MiB zstd window, 4096 saved revisions, array lengths no longer than the
bytes left, 16 777 216 decoded values per document stream (fields and array elements together, counted before
anything is allocated: a value takes about 40 bytes in memory however few it took in the file; the largest of
333 public documents uses 3.3 million), 384 levels of object nesting, 128 levels of layers, 500 000 layers, four million curve
nodes per curve, 64 megapixels per pixel layer (cropped to its content first) and 1024 gradient
stops. Every archive entry's CRC-32 and size must match. Malformed input returns an `Error`; the
crate has no panics outside tests.

## Privacy

Affinity documents can hold the folder they were saved in, user names, original image paths and
XMP metadata. This reader never imports those fields.
