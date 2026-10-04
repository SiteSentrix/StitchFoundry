# VectorCraft Roadmap

VectorCraft is a clean-room, open-source, pure-Rust reimplementation of the Adobe Illustrator workflow. It runs on macOS, Windows, Linux and the web (WASM), and agents can drive it fully over a JSON control channel and MCP.

This file tracks **how far we are and what's left**. Time estimates are wall-clock hours of continuous Claude Opus 5.5 agent work (including builds and the CI gate), given both for **one agent** and for **4–6 parallel agents** on disjoint crates. They are counted from the remaining work (see [Parity estimate](#parity-estimate)), calibrated against measured throughput, and updated as work lands.

_Last updated: 2026-10-04._

## Where we are

| Dimension | Status |
|---|---|
| Infrastructure (engine, command registry, history, render, formats, MCP, web, packaging, tests) | **~90%** |
| Look & feel vs Illustrator 2026 default workspace (measured) | **~75–80%** |
| Feature surface vs full Illustrator (weighted, see below) | **~67%** |
| Parity including interaction fidelity and hardening ("a power user can't tell the difference, but faster") | **~50%** |
| Time to **feature parity** (every menu item, tool, panel, effect and dialog functional) | **~340–540 h** one agent · **~85–155 h** with 4–6 agents |
| Time to **full parity** (feature parity + interaction-fidelity pass + hardening) | **~450–710 h** one agent · **~115–200 h** with 4–6 agents |

### Shipped so far
- **Architecture:** 19+ crates with enforced layering (`cargo xtask layers`). Every action is a command (~400 engine + ~50 UI). Undo is unlimited via structural sharing. `command.batch` runs several commands as one transaction.
- **Automation:**
  - Actions panel (record/playback, persisted), generic parameter dialogs for every "…" command.
  - JSON-lines control channel with real egui pointer and keyboard injection.
  - MCP server with 25 tools (drawing, text, effects, Pathfinder, transforms, graphs, text wrap, export, screenshots, any command), attached to the running app or headless.
  - Headless CLI (`vectorcraft-cli run`, `convert`, `info`, `bench`, `perf`, `mcp`).
  - Actions panel that records and plays back commands.
- **UI:** Illustrator 2026 layout restyled to measured values:
  - Medium Dark theme, categorized and Advanced toolbars, 35 pt document tabs, 33 pt panel tabs.
  - Hint bar, contextual task bar, 19 dock panels with ≡ menus.
  - Native macOS menu bar, vector tool cursors, a ⌘K command palette, middle-button panning with any tool.
  - Save / Don't Save / Cancel before closing or quitting with unsaved documents (tabs, Close, Close All, Quit, window close).
  - Four brightness themes, persistent preferences.
- **Tools:**
  - **Selection:** Selection, Direct/Group Selection, Magic Wand, Lasso.
  - **Drawing:** Pen, Curvature, anchor tools, Pencil, Paintbrush, Blob Brush, Smooth, Path Eraser, Join.
  - **Shapes:** all shape tools (including Flare) and the line, arc, spiral and grid tools.
  - **Cutting:** Eraser, Scissors, Knife.
  - **Transform:** Rotate, Reflect, Scale, Shear (click or Alt-click snaps the reference point to anchors and centres), Reshape, Free Transform (distort/perspective).
  - **Live Corners:** drag a live rectangle's corner widgets (Selection or Direct Selection) to round all corners, with a radius readout.
  - **Graphs:** Column, Stacked Column, Bar, Stacked Bar, Line, Area, Scatter, Pie and Radar graph tools with Graph Data and Graph Type.
  - **Other:** Eyedropper, Gradient annotator, Artboard, Measure, Type, Hand, Zoom, Rotate View.
- **Drawing aids:** Smart Guides and snapping, and Draw Normal / Behind / Inside modes.
- **Geometry and effects:**
  - Pathfinder (10 exact curve booleans), Offset, Outline Stroke, Simplify, Clean Up, Split Into Grid, Divide Objects Below.
  - Live effects with previewing dialogs: Distort & Transform, Path, Convert to Shape, 15 Warp styles, Round Corners, Scribble, Effect → Pathfinder (all 10 operations, live on groups), and raster drop shadow, glows and feather. SVG and PDF export keep live effects (geometry baked; SVG raster effects as filters).
- **Type:** Text Wrap, Type on a Path effects (Rainbow/Skew/3D Ribbon/Stair Step/Gravity), Character and Paragraph Styles (override-preserving redefine), Area Type Options (rows/columns/inset/first baseline), threaded text across any closed shapes, Fit Headline.
- **Transparency:** opacity masks (clip/invert/disable/link), exported as SVG `<mask>` and PDF soft masks.
- **Advanced art:** live Blends (steps/distance/smooth colour, spine), Envelope Distort (warp/mesh/top object), Gradient Mesh, Shape Builder, Live Paint, Image Trace (12 presets), pattern swatches with pattern editing mode, live Repeat (radial/grid/mirror).
- **Paint and appearance (M3):**
  - **Swatches:** names follow the colour model and are unique; Swatch Options (process/spot, Global, Gray/RGB/HSB/CMYK/Web, live preview); editing a global or spot swatch recolours every linked fill, stroke and text run in one undo step; multi-select and delete with confirmation; New Swatch and New Color Group dialogs (a group from the selected artwork's colours); swatches in colour groups convert, separate and count like any other. The Swatches panel selects ranges and whole groups, has a find field, and drags swatches to reorder, regroup or paint art; Add Used Colors, Select All Unused, Merge, Ungroup and Sort by Kind; a CMYK document keeps applied colours in CMYK.
  - **Colour:** a Color Picker (colour field, H/S/B/R/G/B channel slider, HSB/RGB/Lab/CMYK fields, hex, web-only snapping, out-of-gamut correction); the Color panel follows each colour's own model and Alt-click paints the other proxy; proxies look through groups and show "?" for mixed paint; Invert, Complement, Apply Last Color (`,`) and Apply Last Gradient (`.`) keep the colour model; every paint command feeds the recent colours.
  - **Gradients:** lossless, validated gradient params; gradients follow rotate, reflect, shear, non-uniform scale and distortions; the Gradient tool edits the fill or stroke of the active proxy, type included; an interactive on-canvas annotator (move, length/angle, rotate, add/move/delete/duplicate stops, midpoints, a stop popover, Delete and arrow keys). The Gradient panel has the Fill/Stroke proxy, a gradient-swatch dropdown with Save to Swatches, midpoint locations, Alt-drag to copy or swap stops, swatch drops on the ramp and a stop eyedropper; copied appearances and graphic styles place gradients on each target's own bounds; gradient handles snap and honour Constrain Angle.
  - **Strokes:** one stroke-geometry module shared by the canvas, SVG, PDF and Outline Stroke; dotted lines (zero-length dashes draw dots or squares); arrowheads with hollow outlines, tip-on-end or extend-past-end alignment and one opacity for line and head. SVG and PDF export arrowheads, width profiles, brushes and aligned strokes exactly as the canvas draws them; Outline Stroke and the live Outline Stroke effect outline every stroke item as drawn (dashes, heads, profiles, alignment); dashes can be fitted to corners and path ends; width profiles carry through dashes, and their corners take the join.
  - **Appearance:** the Appearance panel's selected row drives every paint, stroke, gradient and transparency edit (`item` on the commands); effects apply to one fill or stroke; per-item Opacity popups; Mixed Appearances, Layers target dots and basic-appearance rules. Effects can be reordered, moved between items and copied by dragging; Show All Hidden Attributes; clicking an effect edits it in place; the object thumbnail drags onto art; effects render on type, images, symbols, blends, envelopes, meshes and repeats, and per-item raster effects export.
  - **Masks and clipping:** opacity-mask controls work while the mask is being edited; transparency commands take ids and work without a selection; clipping sets clip by compound paths, even-odd paths, text and groups on screen and in raster export, matching SVG/PDF.
  - **Transparency:** three-state Knockout Group (on, neutral, off), Opacity & Mask Define Knockout Shape, Page Isolated Blending and Page Knockout Group, on screen and in SVG/PDF.
  - **Graphic styles:** styles keep opacity and blend mode, capture groups and type, apply on top with Alt, and link to the objects using them; Redefine, Break Link, Graphic Style Options, Select All Unused, Sort by Name, and Select > Same Graphic Style / Appearance Attribute.
  - **Neutral wording:** labels, MCP tool text, docs and packaging use neutral names, and `cargo xtask brands` (part of `cargo xtask ci`) fails on vendor names.
- **Colour, type and file workflows:** Recolor Artwork (dialog with harmonies), Edit Colors, Find & Replace, Change Case, Smart Punctuation, Guides, Lock/Hide Above, Transform Each, Rasterize.
- **Formats:**
  - `.vectorcraft` (lossless JSON), SVG import/export, PDF export/import (including PDF-compatible `.ai`).
  - PNG, JPEG and WebP export, Export for Screens, Place.
- **Performance:** 20k shapes + 1k texts render in 27 ms per full-retina frame (7.8 ms zoomed), 7× faster than the first version. The UI thread never blocks. The web build is 7.1 MB gzipped.
- **Tests:** ~1,430 automated tests: model-based property tests, a junk-parameter sweep over every command, golden renders, and MCP end-to-end tests over stdio.

## Milestones and estimates

| # | Milestone | Status | Est. remaining, one agent (h) |
|---|---|---|---|
| M0 | Skeleton + vertical slice | ✅ done | — |
| M1 | Selection, transform, layers, MCP | ✅ mostly done (transform reference point snaps to anchors/centres; rotated persistent bbox pending) | 4–6 |
| M2 | Drawing tools + smart guides | ✅ mostly done (Flare, Reshape, Live Corners widget dragging landed; Shaper, Pen modifier nuances) | 15–20 |
| M3 | Paint & appearance (swatches, color, gradient, stroke, appearance, transparency, styles) | 🟡 M3.7–M3.39 and M3.42–M3.48 done: swatch options, global/spot edits, colour groups and Swatches panel depth (drag and drop, find, panel commands, document colour mode); Color Picker and Color panel behaviour; proxy commands; gradients that follow every transform, an interactive annotator, Gradient panel depth and rebased gradients; shared stroke geometry, dotted lines, arrowheads, SVG/PDF stroke parity, Outline Stroke fidelity, corner-fitted dashes, profiles through dashes; Appearance-panel targeting, per-item effects and opacity, effect drag/edit, effects on every object kind; three-state knockout and page groups; graphic style links and management; opacity-mask editing reach; clipping by any shape. Pending (M3.40–M3.41, M3.49–M3.98): stroke on type, overprint and the Attributes panel, swatch and style libraries, tints and linked gradient stops, freeform gradients and gradient on strokes, width profile library and Width tool depth, Scale Strokes and preview bounds, container appearance, target circles and Expand Appearance, blend accuracy and isolation, layer clipping masks, Colour Guide and Recolor depth, Expand, mask view, CMYK blending, Flatten Transparency | 45–85 |
| M4 | Files (native, SVG, PDF, raster, Export for Screens, clipboard interop) | 🟡 one engine loader/encoder for every frontend (desktop, web, CLI, control channel, headless MCP open native/SVG/SVGZ/PDF/.ai/.ait/PNG/JPEG/GIF/WebP/TIFF/BMP; `document.formats`; MCP `export` takes artboard/range/options; one PDF page per artboard in Export for Screens; template layers left out of raster exports); Export for Screens (PNG/JPG/WebP/SVG/PDF × scales) done; headless CLI/MCP export every format; system clipboard: copy puts SVG markup on it, paste takes SVG from other apps (Ctrl/Cmd+C/X/V now also work off macOS); live effects now survive SVG/PDF/clipboard export (geometry baked, SVG filters for shadows/glows/blur/feather); Save / Don't Save / Cancel before closing modified documents; PDF raster effects, PNG/PDF clipboard flavours, EPS/DXF pending | 52–73 |
| M5 | Performance | 🟡 background render + caches + MT done; `vectorcraft-cli bench` and `vectorcraft-cli perf` (budget suite); file format v2 opens 3× faster (50k paths: 722 → 244 ms); raster effects (glows, shadows, blur, feather) no longer force the whole frame single-threaded (filtered offscreen per effect, verified equal to the single-threaded reference); effect-heavy demos need a clean-machine benchmark; dirty-region rendering, GPU backend spike pending | 15–25 |
| M6 | Path operations (Pathfinder, Shape Builder, offset…) | ✅ mostly done (Shape Builder edge erase, large-offset bug open) | 3–6 |
| M7 | Type (point/area/path, editing, styles, OpenType, threading, glyphs) | 🟡 Character/Paragraph Styles, Area Type Options, threaded text, Fit Headline, Glyphs, OpenType panel, Find Font, Text Wrap (offset, invert, both sides of an object; follows edits), Type on a Path effects, tab stops + Tabs panel done; tab leaders, spell check, vertical type pending | 45–60 |
| M8 | Transform & distort (Puppet Warp, Liquify tools, Envelopes, Blends, Perspective Grid) | 🟡 live Blends, Envelopes (warp/mesh/top object), Width tool, Liquify tools, Puppet Warp and Perspective Grid landed; fidelity pass pending | 8–12 |
| M9 | Live effects (+ 3D & Materials) | 🟡 2D effects done incl. Effect → Pathfinder; SVG Filters, Document Raster Effects Settings, 3D pending | 86–135 |
| M10 | Brushes, symbols, patterns, Repeat | 🟡 pattern swatches (5 tile types, Pattern Options, editing mode, SVG `<pattern>`/PDF export) and live Repeat (radial/grid/mirror) done; brushes/symbols in progress | 23–37 |
| M11 | Artboards & views (artboard panel/tool done, Trim View, middle-button pan; print tiling, multiple windows, presentation polish) | 🟡 | 20–33 |
| M12 | Advanced color & art (CMYK/ICC, separations, Gradient Mesh, Live Paint, Image Trace, Graphs) | 🟡 Gradient Mesh, Live Paint, Image Trace (12 presets, 18 ms/1k² image), Recolor Artwork, colour management (ICC, soft proofing, separations preview), Graphs (all 9 tools, Graph Data/Type, regenerate in place) done; graph Design/Column/Marker designs pending | 6–10 |
| M13 | Automation (Actions ✅ record/playback, persisted; variables, scripting, batch) | 🟡 | 18–27 |
| M14 | 1.0 polish (preferences, shortcut editor, workspaces, accessibility, packaging for all OSes) | 🟡 Preferences, shortcut editor, workspaces done; accessibility, Windows/Linux packaging pending | 20–30 |
| — | Interaction fidelity pass (every tool's modifiers, Properties panel per context, isolation, nuance) | ⬜ | 60–90 |
| — | Hardening at scale (big-file corpus, fuzzing, cross-platform + browser QA) | 🟡 | 50–80 |
| | **Total to full parity** (one agent; sum of the rows above — matches [Parity estimate](#parity-estimate)) | | **~450–710** |

## Parity estimate

_Method (2026-10-02, M3 rows re-derived 2026-10-04):_ Illustrator's feature surface is split into 22 areas, weighted by how much of the app (and of
real users' work) each represents. Each area is scored by depth of behaviour, not by presence of a menu item: an area
is 100% only when every feature in it behaves like Illustrator. Feature parity = Σ weight × score / Σ weight. Remaining
time is counted per area from what is missing, calibrated on measured throughput: in the last session one agent landed
about 20 medium features (Text Wrap, Graphs, Effect → Pathfinder, export baking…) in ~16 h wall clock including builds
and CI on a heavily loaded machine — about 0.8 h per medium feature; large subsystems (3D, raster filters, vertical type)
are counted bottom-up. The M3 rows (colour, strokes, appearance) come from a detailed 92-task M3 plan whose review
found more missing depth than the first estimate; they are calibrated on its first two waves, where 13 packages planned at
~120 h took roughly 25–45 agent-hours.

| Area | Weight | Done | Missing (main items) | One agent (h) |
|---|---:|---:|---|---:|
| Selection, transform & align tools | 6 | 85% | rotated persistent bounding box, Start Global Edit, transform nuances | 4–6 |
| Drawing tools | 7 | 85% | Shaper Groups (merge/punch overlapping shapes), pen/pencil modifier nuances, Touch Type | 10–15 |
| Path operations, Pathfinder, Shape Builder, Live Paint | 5 | 85% | Live Paint gap options, Shape Builder edge cases | 3–6 |
| Colour, swatches, gradients, patterns, mesh, recolor | 7 | 85% | swatch libraries (original), tints, linked gradient stops, freeform gradients, gradient on strokes, Expand, Colour Guide and Recolor depth, overprint, Tile Edge Color | 23–44 |
| Strokes, brushes, width profiles | 5 | 80% | stroke on type, width profile library, Width tool depth, Scale Strokes, brush options depth, brush libraries (generated in code) | 6–12 |
| Appearance, transparency, graphic styles, masks | 5 | 91% | container appearance, target circles, Expand Appearance, graphic style libraries, blend accuracy and isolation, Flatten Transparency, mask view (Alt-click) | 15–29 |
| Live vector effects | 5 | 85% | Outline Object, Pathfinder Hard/Soft Mix and Trap, SVG Filters | 6–10 |
| Raster effects (Effect Gallery, Document Raster Effects Settings) | 4 | 15% | ~55 filters (Artistic, Brush Strokes, Distort, Pixelate, Sketch, Texture…), resolution setting, raster effects in PDF | 30–45 |
| 3D and Materials | 4 | 0% | Extrude & Bevel, Revolve, Inflate, Rotate, lighting, materials (software renderer) | 50–80 |
| Type core | 9 | 78% | composer/hyphenation options, Optical Margin Alignment, hidden characters | 15–20 |
| Type advanced | 4 | 35% | vertical type/CJK, tab leaders, spell check (open dictionary), Touch Type, Retype | 26–35 |
| Symbols, blends, envelopes, Repeat, perspective | 5 | 75% | symbol libraries (original), dynamic symbols, perspective edge cases | 8–12 |
| Image Trace, graphs, image tools | 3 | 70% | graph Design/Column/Marker, Create Object Mosaic, Crop Image polish | 6–10 |
| Layers, artboards, document setup | 5 | 80% | Layers panel options depth, artboard presets/rearrange polish | 6–10 |
| View & navigation | 3 | 70% | New View/Edit Views, multiple windows/arrange, print tiling, Snap to Pixel/Glyph | 10–15 |
| Guides, grids, smart guides, snapping, rulers | 3 | 75% | global/video rulers, smart-guide preference depth | 4–8 |
| File formats | 6 | 60% | EPS in/out, DXF/DWG, PSD placement, PDF raster effects/presets | 40–55 |
| Export for Screens, Asset Export, slices, Save for Web | 3 | 45% | Asset Export panel, slices, Save for Web | 12–18 |
| Print, colour management, separations, flattener | 3 | 40% | Print dialog (platform), flattener presets, print presets | 20–30 |
| Automation | 3 | 60% | Variables (data merge), scripting surface, batch | 8–12 |
| UI chrome (panels, contextual Properties, workspaces, prefs) | 6 | 75% | ~14 panels (Tabs, Links, Attributes, Asset Export, SVG Interactivity, Variables, CSS Properties…), Properties per context | 20–30 |
| Libraries, Links, Package | 2 | 15% | Links panel, Package, local libraries | 10–15 |
| **Feature parity** | **103** | **~67%** | | **~340–540** |
| Interaction-fidelity pass (side by side with Illustrator: every tool modifier, cursor, dialog, Properties context) | | | | 60–90 |
| Hardening (big-file corpus, fuzzing, cross-platform and browser QA, accessibility, packaging) | | | | 50–80 |
| **Full parity** | | **~51%** | | **~450–710** |

With 4–6 agents working on disjoint crates (as the layering allows) the wall-clock time divides by roughly 3.5–4
(integration, review and shared files such as `menus.rs` serialize some work): **~85–155 h** to feature parity,
**~115–200 h** to full parity.

_Inventories (2026-10-02):_ 426 engine commands + ~60 UI commands; 316 menu items wired, ~70 still disabled; 69 of 79
tools implemented (missing: Slice ×2, Touch Type, vertical type ×3, Print Tiling); 32 panels of ~45; 44 live
effects of ~110 (Illustrator effects 44/54, Photoshop-style raster effects 1/56); ~1,050 tests (~1,430 on 2026-10-04).

_Where we already beat Illustrator:_ exact curve booleans, off-thread multithreaded rendering, undo that never runs out,
lossless SVG/PDF export of live effects with SVG filters, a documented JSON format, the same app on the web, and every
command, gesture and dialog drivable by agents (MCP, CLI, control channel).

## Out of scope (by design or by law)
- **Native `.ai` private data:** it's undocumented. We read the PDF-compatible part, so Illustrator-only live objects arrive as appearance.
- **Adobe cloud services** (Libraries sync, Adobe Fonts, Firefly/generative): these are pluggable provider APIs, not built in.
- **Adobe's bundled assets** (swatch/brush/symbol libraries, presets, icons): ours are original.

## Where we aim to be better than Illustrator
- **Speed:** off-thread multithreaded rendering, instant startup, a responsive UI on huge files.
- **Robustness:** exact curve booleans (no "cannot perform operation"), property-tested undo/redo and file round trips.
- **Openness:** a documented native format, first-class SVG, and the same app in the browser.
- **Automation:** every command, gesture, dialog and widget is drivable by agents (JSON channel + MCP), with headless batch mode.

## How to update this file
After each milestone task lands, update the status column and remaining estimates, and move items into "Shipped so far". Keep estimates honest: re-derive them from what remains, never from wishful velocity.
