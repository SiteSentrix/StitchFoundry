# VectorCraft MCP server

`vectorcraft-cli mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io) server on stdio
(newline-delimited JSON-RPC 2.0, protocol `2025-06-18`; `2025-03-26` and `2024-11-05` also accepted). Agents use it to
draw, inspect and look at VectorCraft documents.

It has two backends:

| Mode | How | What works |
|---|---|---|
| **Remote** | `vectorcraft-cli mcp --connect 127.0.0.1:7979` (the app must run with `vectorcraft --control 7979`) | Everything. Tool calls are forwarded over the [control protocol](control-protocol.md), so you watch the app change live. |
| **Headless** | `vectorcraft-cli mcp --headless` | An in-process engine session with a CPU renderer. Everything except the UI-only tools (`inspect_ui`, `type_text`, `open_panel`, `screenshot {window:true}`). |

With no flag, the server tries `127.0.0.1:7979` and falls back to headless. Logs go to stderr; stdout carries
only protocol messages.

## Build and register

```sh
cargo build --release -p vectorcraft-cli
claude mcp add vectorcraft -- "$PWD/target/release/vectorcraft-cli" mcp
# or pin a mode:
claude mcp add vectorcraft-headless -- "$PWD/target/release/vectorcraft-cli" mcp --headless
claude mcp add vectorcraft-app -- "$PWD/target/release/vectorcraft-cli" mcp --connect 127.0.0.1:7979
```

Other clients use the same command in their JSON config:

```json
{"mcpServers": {"vectorcraft": {"command": "/abs/path/target/release/vectorcraft-cli", "args": ["mcp"]}}}
```

For a live session, start the app first: `cargo run --release -p vectorcraft -- --control 7979`.

## Tools

Coordinates are points in document space: y points down, the origin is the first artboard's top-left, and a new
document is 612 × 792 (US Letter). New objects become the selection. Most commands act on the selection or on
explicit `ids`.

Paint values (`fill`, `stroke`) accept `"#rrggbb"`, `"none"`, `[r,g,b]` (0..1), `{"c","m","y","k"}`, `{"gray"}`,
or a full `paint.setFill` params object (`{"gradient": …}`, `{"swatch": "name"}`).
The Fill/Stroke proxy commands run through `run_command`: `paint.invert` and `paint.complement` recolour the active
proxy keeping each colour's model, `paint.lastColor` / `paint.lastGradient` re-apply the last solid colour or gradient,
and `paint.recent` returns the recent colours that every paint command (and the eyedropper) feeds.
`paint.proxies` returns what the proxies show: the fill and stroke, which one is active, and whether the selected
objects' fills or strokes differ (`fillMixed` / `strokeMixed`, drawn as a "?" proxy).

| Tool | Arguments | Notes |
|---|---|---|
| `list_commands` | `{filter?, enabledOnly?}` | The command catalogue: id, label, menu, shortcut, params doc, enablement. |
| `run_command` | `{command, params?}` | Runs any command. Use it for everything without a dedicated tool. |
| `inspect_document` | `{}` | Artboards, layer tree (ids, kinds, bounds, paint), selection, history, tool. |
| `inspect_ui` | `{}` | UI state. Remote mode only. |
| `select_tool` | `{tool}` | `selection`, `directSelection`, `pen`, `rectangle`, `ellipse`, `polygon`, `star`, `lineSegment`, … |
| `pointer_gesture` | `{events:[{kind,x,y,mods?}], tool?, mods?}` | `kind` is one of `down`, `drag`, `up`, `move`, `doubleclick`. Events go through the same path as the mouse. |
| `draw_path` | `{points \| d, closed?, fill?, stroke?, strokeWidth?}` | `points` is `[[x,y],…]` or `[{x,y,in?,out?,smooth?},…]`. `d` is SVG path data. |
| `draw_shape` | `{shape, …geometry, fill?, stroke?, strokeWidth?}` | `rectangle`/`ellipse`: `x,y,width,height` (plus `radius` for corners). `polygon`: `cx,cy,radius,sides`. `star`: `cx,cy,radius1,radius2,points`. `line`: `x1,y1,x2,y2`. |
| `set_paint` | `{fill?, stroke?, strokeWidth?, ids?}` | Applies to the selection (or `ids`) and becomes the default for new art. |
| `press_key` | `{key, mods?}` | Remote: a real key event. Headless: runs the command or tool bound to that shortcut, or sends the key to the busy tool. |
| `type_text` | `{text}` | Remote only. |
| `invoke_menu` | `{command, params?}` | Invokes a menu item by command id. Includes UI commands such as `view.*` and `window.*` in remote mode. |
| `open_panel` | `{panel}` | Remote only. |
| `screenshot` | `{path?, scale?, artboard?, window?}` | Returns MCP image content (`image/png`, base64) plus a text block. Renders the artboard; `window:true` captures the app window (remote only). |
| `open_file` | `{path}` | Opens any readable file as a new active document: `.vectorcraft`/`.drawcraft`, `.svg`/`.svgz`, `.pdf`/`.ai`, `.ait`, PNG, JPEG, GIF, WebP, TIFF, BMP (an image opens as a document of its pixel size). Templates open as a new untitled document. `run_command document.formats` lists the formats. |
| `save_file` | `{path?}` | Saves in the native `.vectorcraft` format. |
| `export` | `{path?, format?, scale?, artboard?, range?, selection?, outlineText?, options?}` | `svg`, `pdf`, `png`, `jpg`, `webp` or `vectorcraft` (the list comes from `document.formats`). When `format` is omitted, it comes from the path's extension. PDF writes one page per artboard: all of them, or `artboard` (0-based) / `range` (`"1-3, 5"`, 1-based); the other formats write one artboard. `options` carries more format options (e.g. `{"quality": 80}` for JPEG). `selection: true` exports the selected objects cropped to their bounds; `outlineText: true` writes SVG text as paths. Template layers are left out, live effects are kept, and exporting `vectorcraft` never changes the document's path. Without `path` the bytes come back as `dataBase64`. Both backends run the same `document.export` call. |
| `add_text` | `{text, x?, y?, width?, height?, path?, mode?, pathEffect?, size?, font?, color?}` | Point type at (x, y); area type with `width`/`height`; or `path` + `mode` (`area`/`onPath`) to flow text in or along a path, with `pathEffect` (`rainbow`, `skew`, `3dRibbon`, `stairStep`, `gravity`). |
| `apply_effect` | `{effect?, params?, ids?}` | Appends a live effect. Without `effect`, returns the effect catalogue with parameters and defaults. |
| `pathfinder` | `{operation, ids?}` | `unite`, `minusFront`, `intersect`, `exclude`, `divide`, `trim`, `merge`, `crop`, `outline`, `minusBack`. For a live version, apply the `pathfinder.*` effect to a group. |
| `transform` | `{ids?, dx?, dy?, rotate?, scale?, scaleX?, scaleY?, reflect?, shear?, origin?, copy?}` | Runs move, rotate, scale, reflect, shear in that order. With `copy`, the first step duplicates. |
| `create_graph` | `{type?, x, y, width, height, csv? \| series?, categories?, rows?}` | The nine Illustrator graph types: column, stacked column, bar, stacked bar, line, area, scatter, pie and radar. Edit later with `graph.setData` / `graph.setType` via `run_command`. |
| `text_wrap` | `{ids?, offset?, invert?, release?}` | Area type below the objects (same layer) flows around them. |
| `undo` / `redo` | `{}` | |

Appearance stacks: an object can carry several fills and strokes (`appearance.addFill`, `appearance.addStroke`), indexed
in paint order (0 is painted first, the bottom row of the Appearance panel). `paint.setFill`, `paint.setStroke`,
`stroke.set`, `stroke.setAdvanced`, `paint.editGradient`, `paint.setGradientGeom` and `transparency.set` take
`item` to edit one of them; `run_command appearance.setActiveItem {"index": n}` makes that row the target of later
calls that omit `item` (as clicking the row in the Appearance panel does) until the selection changes; the proxy of
its kind (`paint.proxies`) and the Gradient tool's annotator then show that row.
`inspect_document` reports it as `paint.appearanceItem`. Live effects take the same `item` to apply to one fill or
stroke instead of the whole object: `run_command effect.apply {"effect": "path.offsetPath", "item": 0}`, and
`effect.remove`, `effect.setParams` (`visible` toggles one) and `effect.duplicate` address that item's effects;
`apply_effect` uses the active item. `effect.list` reports each object's item effects under `applied[].items`.

Errors (unknown tool, bad arguments, a disabled or failing command) come back as a normal result with
`isError: true` and a message. The model can read the message and retry.

Swatches go through `run_command`: `swatch.list` lists every swatch with its colour group, kind and colour;
`swatch.edit {name, newName?, color?, mode?, global?, spot?}` edits one (fills, strokes and text linked to a
global or spot swatch follow its colour and name, as one undo step); `swatch.delete {names, unlink?}` deletes
swatches and colour groups in one undo step (art using a deleted global swatch keeps its colour, unlinked).
`swatch.new {color?, mode?, spot?, global?, group?}` saves a colour (into a colour group with `group`), gradient
or pattern; `swatch.newGroup {fromArtwork: true, toGlobal?, includeTints?}` makes a colour group of the selected
art's colours (by default as global swatches the art links to).
`swatch.move {names, to?, group?}` reorders swatches or moves them into or out of a colour group (as dragging
them in the Swatches panel does); give colour group names instead to reorder the groups. Dropping a swatch on art
in the app runs `paint.setFill` (or `paint.setStroke`, the active proxy) with `ids: [the object under the pointer]`.
The Swatches panel menu's commands: `swatch.addUsedColors {selection?, global?}`, `swatch.unused` (a query: the
names Select All Unused selects), `swatch.merge {names}` (the first is kept), `swatch.ungroup {name}` and
`swatch.sortByKind`.
Document colour mode: `file.new {colorMode: "cmyk"}` starts a CMYK document with CMYK default swatches, and RGB
colours applied to it (`paint.setFill`/`setStroke` colours and gradient stops, `swatch.new`) are stored as CMYK;
Gray stays Gray, and `keepModel: true` keeps a colour as given. RGB documents keep colours as given. Harmonies, Edit
Colors blends, inversions and Recolor Artwork keep each colour's model (a blend between models takes the document's).
`swatch.new {colors: [...]}` saves several colours as swatches in one undo step. `color.harmony {color, rule, steps?, variation?, amount?}` answers what the Color
Guide panel shows: the harmony rule's colours, base first, and per colour its row of `2·steps+1` variations
(shades, cool or muted on the left, tints, warm or vivid on the right). With `limitTo` (a swatch library id or name,
or `"document"` for the document's swatches) every colour snaps to the library's nearest colour (ΔE 2000), as the
panel's Limit to Library does (`ui.colorGuideLimit`); Recolor Artwork's `limitTo` takes the same values.

Color Themes (local only: no online service) keeps five-colour themes in the preferences. `colorTheme.save
{colors: [...]}` saves up to five colours as they are, `colorTheme.save {color, rule}` saves the five-colour theme a
harmony rule makes from a base colour (base first; `name?` defaults to "Theme N", `replace: "<name>"` overwrites
that theme in place). `colorTheme.list` answers `{themes: [{name, colors: [hex], keys: [colour keys], rule?}]}`,
`colorTheme.delete {name}` removes one and `colorTheme.addToSwatches {name}` adds one to the Swatches panel as a
colour group (one undo step).

```json
{"name":"run_command","arguments":{"command":"colorTheme.save","params":{"color":"#2266aa","rule":"splitComplementary","name":"Harbor"}}}
{"name":"run_command","arguments":{"command":"colorTheme.addToSwatches","params":{"name":"Harbor"}}}
```

SVG Options: `export` to SVG takes them in `options`, flat or as `{"svg": {…}}`: `styling` (`presentation`,
`style`, `entities`, `css`), `outlineText`, `images` (`embed`, or `link`: embedded images are written next to the
SVG, or returned as `linked`), `objectIds` (`layerNames`, `minimal`, `unique`), `decimals` (1–7), `minify`,
`responsive`, `useArtboards`, `range: "all"` (one SVG per artboard, listed in `files`), `preserveEditing` (the SVG
reopens as the full document) and `metadata`. Unknown keys inside `svg` are rejected; `run_command document.formats`
lists every option with its default. `run_command document.save {path: "x.svg", svg: {…}}` saves as SVG.

## Resources

| URI | Content |
|---|---|
| `vectorcraft://document` | `document.inspect` summary (JSON) |
| `vectorcraft://document/json` | The complete document model (JSON) |

## Examples

Draw, look, export:

```json
{"name":"draw_shape","arguments":{"shape":"rectangle","x":72,"y":72,"width":200,"height":120,"radius":12,"fill":"#1e88e5","stroke":"none"}}
{"name":"draw_shape","arguments":{"shape":"star","cx":400,"cy":300,"radius1":90,"radius2":40,"points":5,"fill":"#ffc107","stroke":"#000000","strokeWidth":2}}
{"name":"draw_path","arguments":{"d":"M72 400 C 150 300 250 500 330 400","stroke":"#e53935","strokeWidth":4,"fill":"none"}}
{"name":"screenshot","arguments":{"scale":0.5}}
{"name":"export","arguments":{"path":"/tmp/art.svg"}}
```

Long-tail commands:

```json
{"name":"list_commands","arguments":{"filter":"align"}}
{"name":"run_command","arguments":{"command":"select.all"}}
{"name":"run_command","arguments":{"command":"object.align","params":{"align":"left"}}}
{"name":"run_command","arguments":{"command":"object.group"}}
{"name":"run_command","arguments":{"command":"document.exportPdf","params":{"path":"/tmp/art.pdf","range":"1, 3","compatibility":"1.5"}}}
{"name":"run_command","arguments":{"command":"document.pdfSettings","params":{"marks":{"trim":true},"includeDocument":true}}}
```

PDF files take the Save PDF dialog's options: `preset`, `standard`, `compatibility`, the General toggles and the
`compression`, `marks`, `bleed`, `output`, `advanced` and `security` sections (`list_commands` with filter `exportPdf`
documents every field). `document.exportPdf` and `export` (format `pdf`, the options in `options`) return `warnings`:
options accepted but not applied yet, and features approximated or left out. PDF/X, passwords and PDF/A-2b at 2.0 are refused.
`document.pdfSettings` lists the options that differ from the defaults and the warnings without writing a file.

Drive a tool like a mouse:

```json
{"name":"pointer_gesture","arguments":{"tool":"ellipse","events":[
  {"kind":"down","x":100,"y":100},{"kind":"drag","x":150,"y":140},{"kind":"up","x":200,"y":180}]}}
```

Edit a gradient with the Gradient tool's annotator (the bar runs along the vector; stop chips sit 10 px under
it, midpoint diamonds 7 px above): a click on the bar adds a stop, dragging a chip moves it (drag it off the bar to
delete it, hold Alt to copy it), dragging the end handle changes the vector. The selected stop
(`gradient.selectStop`) takes Delete and ←/→:

```json
{"name":"run_command","arguments":{"command":"paint.setFill","params":{"gradient":{"start":[100,150],"end":[200,150]}}}}
{"name":"pointer_gesture","arguments":{"tool":"gradient","events":[{"kind":"down","x":130,"y":150},{"kind":"up","x":130,"y":150}]}}
{"name":"pointer_gesture","arguments":{"events":[{"kind":"down","x":130,"y":160},{"kind":"drag","x":160,"y":160},{"kind":"up","x":160,"y":160}]}}
{"name":"press_key","arguments":{"key":"Delete"}}
```

A radial gradient's annotator also draws its extent: a dashed ellipse around the centre (the start, drawn as a ring)
with a dot on it across the bar (drag it to change the aspect ratio); dragging the ellipse elsewhere rotates it.
The dot inside the centre ring is the focal point, where the first stop sits: drag it for an off-centre radial, back
onto the centre to centre it. `paint.setGradientGeom` sets the same things directly (`aspect` in %, `focal` in
document coordinates or `null`; `start`/`end` may be left out), and they export as SVG `fx`/`fy` and PDF two-point
radial shadings:

```json
{"name":"run_command","arguments":{"command":"paint.setGradientGeom","params":{"aspect":60,"focal":[130,140]}}}
```

Freeform gradients: `paint.editGradient {kind: "freeform"}` places four or more colour points inside each selected
object (coloured along the stops; `mode: "points"|"lines"` is the Draw toggle). `paint.freeform.get` lists the points
(document coordinates), lines and the selected point; `paint.freeform.addPoint {at, color?, opacity?, spread?,
line?}`, `setPoint {index?, …}`, `deletePoint {index?}`, `addLine {points}`, `splitLine {line, segment, t?}` and
`selectPoint {index|null}` edit them (`index` defaults to the selected point; each edit is one undo step). With the
Gradient tool on a freeform gradient a click on a point selects it (drag to move it), a click on a line adds a point
on it, a click elsewhere on the art adds a point (in Lines mode joined to the selected one), and Delete removes the
selected point:

```json
{"name":"run_command","arguments":{"command":"paint.editGradient","params":{"kind":"freeform","mode":"lines"}}}
{"name":"run_command","arguments":{"command":"paint.freeform.addPoint","params":{"at":[150,150],"color":"#ff3366","spread":20}}}
{"name":"pointer_gesture","arguments":{"tool":"gradient","events":[{"kind":"down","x":180,"y":170},{"kind":"up","x":180,"y":170}]}}
```

On the canvas the selected point also shows its spread as a dashed ring with a handle 16 px (or the spread, if larger)
to its right: drag the ring or handle to change the spread. Dragging a point out of the object removes it, and
double-clicking one opens its popover (dialog `gradientStop`, which on a freeform gradient edits the selected point:
set `color`, `opacity` and `spread` and confirm). In Lines mode successive clicks on the art draw one smooth line
through the points they add; a click on an existing point first continues the line from it, and `press_key` Escape
(or a click off the art) ends it:

```json
{"name":"pointer_gesture","arguments":{"tool":"gradient","events":[{"kind":"down","x":110,"y":190},{"kind":"up","x":110,"y":190},{"kind":"down","x":150,"y":170},{"kind":"up","x":150,"y":170}]}}
{"name":"press_key","arguments":{"key":"Escape"}}
```

Raw protocol (for debugging):

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"sh","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"draw_shape","arguments":{"shape":"ellipse","x":10,"y":10,"width":100,"height":80,"fill":"#ff0000"}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"inspect_document","arguments":{}}}' \
  | target/release/vectorcraft-cli mcp --headless
```

## Headless batch CLI

```sh
vectorcraft-cli commands                          # command catalogue (JSON)
vectorcraft-cli run --in in.svg \
  --cmd select.all --cmd paint.setFill --params '{"color":"#ff0000"}' \
  --export out.svg --export out.png --scale 2
vectorcraft-cli run --cmd file.new --params '{"width":800,"height":600}' \
  --cmd shape.star --params '{"cx":400,"cy":300,"radius1":200,"radius2":90}' \
  --export star.vectorcraft
```

`run` prints one JSON line per step (`open`, `cmd`, `export`) and exits non-zero on the first failure. `--params`
applies to the `--cmd` just before it. `run` also accepts the host commands `file.open`, `file.save`, `file.export`,
`file.exportForScreens` and `tool.select`. `run --in`, `convert` and `info` read every format `document.open` reads
(`vectorcraft-cli --help` lists them).

## Transparency and opacity masks

Transparency and opacity-mask commands take `ids` (or `id`), so they need no selection. Opacity is a percentage
(0..100) in every command (`transparency.set`, `object.setProps`, `appearance.setItem`). `transparency.info` returns
the Transparency panel's values, with `null` where the objects differ. Making a mask from one object gives it an
empty mask and enters mask editing: art drawn then becomes the mask, and the mask commands act on the masked object
until `transparency.stopEditingOpacityMask`. Saves and exports never include the editing layer.

```json
{"name":"run_command","arguments":{"command":"transparency.set","params":{"ids":[12],"opacity":40,"blend":"Multiply"}}}
{"name":"run_command","arguments":{"command":"transparency.makeOpacityMask","params":{"ids":[12,15],"invert":true}}}
{"name":"run_command","arguments":{"command":"transparency.setOpacityMask","params":{"id":12,"clip":false}}}
{"name":"run_command","arguments":{"command":"transparency.info","params":{"ids":[12,20]}}}
```

Knockout Group has three states: `transparency.set {knockout: "on"|"off"|"neutral"}` (`true` = on, `false` =
neutral, the default). In a knockout group each child hides what it covers of the children below it; neutral groups
pass the enclosing group's setting to their children, off groups never knock out. `knockoutShape: true` makes the
object's opacity and opacity mask scale how much it knocks out. `transparency.togglePageKnockoutGroup` and
`transparency.togglePageIsolatedBlending` (`{value?}`) treat the whole page as a knockout or isolated group; they are
saved with the document and undoable, and `transparency.info` reports them. PDF and SVG write knockout groups as
soft-masked groups with the same look (the PDF export reports a warning).

Groups are not isolated unless Isolate Blending is on: a blend mode inside a group with opacity, a blend mode, an
opacity mask, knockout or a clip reaches the art below the group, on screen and in PDF (a knockout group's elements
then composite against the art below it). SVG has no non-isolated groups, so there such groups stay isolated.

`transparency.viewOpacityMask {on?, id?}` is Alt-clicking the mask thumbnail: the canvas shows only the mask of `id`
(default: the mask being edited, else the first selected masked object) as greyscale coverage (white = opaque) and the
mask is edited; called again it shows the artwork, still editing. It is view state of the open document: the canvas
(and `ui.screenshot`) shows it, exports never do, and it ends when mask editing ends.

`view.transparencyGrid {on?}` (View → Show Transparency Grid; default: toggle) is view state of each open document,
like the mask view: the grid shows behind the artboards of the documents that turn it on, on the canvas and in
`ui.screenshot`, never in exports. It returns `{on}`, isn't undoable and doesn't mark the document changed.

## Clipping masks

`object.clippingMask.make` clips the selected objects by the topmost one, which may be a path, a compound path or a
text object (it loses its paint). Compound holes, even-odd fills, glyph outlines and the union of a group's members
clip alike on screen, in raster export and in SVG and PDF. `object.clippingMask.release` turns the clip group into a
plain group and keeps the clipping path with whatever paint it has.

A clipping path can be painted after Make (`paint.setFill`, `paint.setStroke`, `stroke.set` with its `ids`): its fill
paints behind the clipped art and its stroke over it, not clipped, on screen and in SVG and PDF.

```json
{"name":"run_command","arguments":{"command":"select.set","params":{"ids":[12,15]}}}
{"name":"run_command","arguments":{"command":"object.clippingMask.make","params":{}}}
```

`layer.clippingMask.toggle {id?}` is the Layers panel's clipping mask button: the top object of the layer `id` (default:
the one selected group, else the current layer) becomes its clipping path (unpainted, moved to the bottom of the layer,
so art added later is clipped too); called again it releases the mask. It returns `{clip}`, and the Layers panel
underlines clipping-path names.

## Graphic styles

A graphic style holds an appearance (fills, strokes, effects) plus opacity, blend mode, isolate and knockout.
`graphicStyle.new {name?, id?}` captures an object (a group without its own fills or strokes lends its topmost
object's, type its characters'). `graphicStyle.apply {name, ids?}` gives the objects the style and links them;
`add: true` adds the style on top of the existing appearance instead. Linked objects stay linked while they keep the
style's look: editing their appearance or transparency breaks the link, and `graphicStyle.redefine {name?, id?}`
updates only the objects still linked. `graphicStyle.list` returns each style with the ids linked to it and the
style of the first selected object; `select.same.graphicStyle` selects an object's fellow users.
Styles keep placed gradients relative to the bounds of the object they were made from, and each object a
style is applied to gets them at the same place relative to its own bounds.

```json
{"name":"run_command","arguments":{"command":"graphicStyle.new","params":{"id":12,"name":"Glow"}}}
{"name":"run_command","arguments":{"command":"graphicStyle.apply","params":{"name":"Glow","ids":[15,20]}}}
{"name":"run_command","arguments":{"command":"graphicStyle.redefine","params":{"id":12}}}
{"name":"run_command","arguments":{"command":"graphicStyle.list","params":{}}}
```

`graphicStyle.merge {names, name?}` adds a style stacking the fills, strokes and effects of two or more styles
(each on top of the ones before it) with the first one's transparency; `graphicStyle.move {name, to}` reorders the
list (what dragging a style in the panel does). Override Character Color (`graphicStyle.setOptions
{overrideCharColor}`, the preference `overrideCharColor`, on by default) makes a style applied to type replace its
characters' fill and stroke with the style's fills and strokes; off, the characters keep their colour under them.

## Editing appearance stacks

`effect.move {from, to, fromItem?, toItem?, copy?}` reorders an effect or moves it between the object's effects
(`null`) and a fill's or stroke's (an item index), as dragging its row in the Appearance panel does; `copy: true` copies
it (Alt-drag). `appearance.duplicateItem {index, to?}` / `{indices}` and `appearance.removeItem {index | indices}`
act on several fills/strokes at once, and `appearance.showAllHidden` makes every hidden fill, stroke and effect
visible again. In remote mode, `effect.dialog {effect, index, item}` opens an applied effect's dialog prefilled
(`ui.dialog.confirm` runs `effect.setParams`); see the control protocol for the "already applied" question.

## Overprint

Each fill and stroke (and each character's fill and stroke) can overprint: its inks print over the inks below instead
of knocking them out. `object.setOverprint {fill?, stroke?, item?, ids?}` sets it (groups set their contents, type its
characters too; `item` aims at one appearance item) and `attributes.info {ids?}` reads it back (`null` where the
objects differ). Overprint Preview (`view.overprintPreview`) and Separations Preview show it; rendering approximates
it by multiplying. Older files that listed Overprint Black objects in the document get the flags on load.

```json
{"name":"run_command","arguments":{"command":"object.setOverprint","params":{"ids":[12],"stroke":true}}}
{"name":"run_command","arguments":{"command":"attributes.info","params":{"ids":[12]}}}
{"name":"run_command","arguments":{"command":"view.overprintPreview","params":{"on":true}}}
```

## Edit Colors and Recolor Artwork

`edit.colors.invert`, `edit.colors.toCMYK`, `edit.colors.toGrayscale`, `edit.colors.toRGB`, `edit.colors.saturate`,
`edit.colors.adjustBalance` and `recolor.apply` recolour everything inside the selection (or `ids`) in one undo step:
fills, strokes, text, gradient stops, gradient-mesh points, embedded images (a recoloured copy of the image; linked
images are left alone) and the tiles of pattern fills and strokes (a recoloured copy saved as a new pattern swatch,
e.g. "Dots 2"; the original pattern is untouched). `includeImages: false` / `includePatterns: false` leave images or
patterns out. The Blend commands also grade gradient meshes, which keep their shading.

```json
{"name":"run_command","arguments":{"command":"recolor.colors","params":{}}}
{"name":"run_command","arguments":{"command":"recolor.apply","params":{"map":{"#ff0000":"#0055ff"},"includeImages":false}}}
{"name":"run_command","arguments":{"command":"edit.colors.adjustBalance","params":{"mode":"cmyk","m":-20,"k":10}}}
```

Recolor Artwork keys colours by identity, not hex: `recolor.colors` lists each colour's `key` (`"rgb 255 0 0"`,
`"cmyk 0 100 100 0"`, `"gray 40"`, `"lab 55 60 -40"`; RGB in 0–255 levels, CMYK and Gray in percent), so CMYK,
Lab and RGB colours that look alike stay apart, and every colour parameter also accepts a key.
`recolor.reduce {colors?, method?, preserve?, limitTo?}` groups the selection's colours into rows of similar colours (k-means in Lab, weighted by use;
tints of a global swatch share its row; White and Black are preserved by default) and returns a map of rows
`[{from: [keys], to: key}]` to edit and pass to `recolor.apply {map, method?, limitTo?, group?, groupColors?, rename?}`.
`method` says how a row's colours take its new colour: `exact`, `preserveTints` (tints of the row's darkest colour
stay tints), `scaleTints` (every colour becomes a tint as light, relative to the darkest), `tintsShades` (lighter and
darker than the row's average become tints and shades) or `hueShift` (the most saturated colour takes the new colour,
the others turn by the same hue). `limitTo` snaps new colours to a swatch library's nearest colour;
`recolor.randomize {map, order?, saturationBrightness?, seed?}` shuffles or varies new colours; a row with
`exclude: true` keeps its colours. `swatch.editGroup {group, colors, rename?}` rewrites a colour group in one undo
step (art linked to its global swatches follows), and `recolor.apply` with `group` recolours the art and rewrites the
group together.

```json
{"name":"run_command","arguments":{"command":"recolor.reduce","params":{"colors":2,"preserve":{"grays":true}}}}
{"name":"run_command","arguments":{"command":"recolor.apply","params":{"map":[{"from":["cmyk 0 100 100 0","cmyk 0 40 40 0"],"to":"cmyk 100 50 0 0"}],"method":"scaleTints"}}}
{"name":"run_command","arguments":{"command":"swatch.editGroup","params":{"group":"Brights","colors":["#ff0000","cmyk 0 0 100 0"]}}}
```

## Group, layer and type appearance

Groups and layers carry fills, strokes and effects of their own, as in the reference app: their fills and strokes
paint every member's geometry, and their effects apply to the members as one piece (one combined drop shadow; a
Transform or Warp moves or bends the whole group). Their Contents row (type: Characters) is a slot in the stack:
fills and strokes above it paint over the members (characters), those below under them; new items go above it.
`appearance.moveItem {"from": "contents", "to": n}` puts the row above the bottom `n` items, and an item move takes
`contents` to say where the row ends up in the same undo step; `document.node` shows the slot as
`appearance.contents_index`. The `appearance.*`, `effect.*` and `graphicStyle.apply` commands edit the selected
objects themselves (a group's own stack; layers through `ids`) or, with `target: "contents"`, the objects inside the
groups and layers. `appearance.targetContents` selects a group's members (double-clicking the Contents row).
SVG and PDF export bake a group's own fills, strokes and geometry effects into paths; its raster effects stay a
filter on the whole group.

```json
{"name":"run_command","arguments":{"command":"appearance.addFill","params":{"ids":[7]}}}
{"name":"run_command","arguments":{"command":"appearance.moveItem","params":{"ids":[7],"from":"contents","to":1}}}
{"name":"run_command","arguments":{"command":"effect.apply","params":{"ids":[2],"effect":"stylize.dropShadow"}}}
{"name":"run_command","arguments":{"command":"appearance.addStroke","params":{"ids":[7],"target":"contents"}}}
```

## Swatch libraries

Swatch libraries are read-only sets of swatches computed in code (Web Safe 216, Grays and Neutrals, Earth Tones,
Skin Tone Ramps, Pastels, Brights, Metallic Gradients, Perceptual Scales, Harmony Sets). `swatch.library.list` lists
them (`id`, `name`, `category`, `count`) and `swatch.library.get {library}` returns one's swatches and colour groups
in the `swatch.list` shape (`library` is an id or a name). `swatch.library.add {library, names?}` copies swatches
into the document as one undo step: a colour group's name brings the whole group, a swatch's name the swatch alone,
no names the whole library; swatches the document already has (same name and paint) are reported under `existing`
and not added again. `apply: "fill"|"stroke"` also applies the first one, in the same undo step (what clicking a
swatch in the library panel does). `swatch.resetDefaults {replace?}` brings back the missing default swatches.

```json
{"name":"run_command","arguments":{"command":"swatch.library.get","params":{"library":"earth-tones"}}}
{"name":"run_command","arguments":{"command":"swatch.library.add","params":{"library":"earth-tones","names":["Clay"]}}}
```
`swatch.library.save {path?, format?: "vcswatches"|"gpl"|"css", names?, name?, user?}` writes the document's swatches
as a library (`.vcswatches` keeps colour models, global, spot, gradients and colour groups; `.gpl` is 8-bit RGB;
CSS writes custom properties); without `path` it returns `{data}`, and `user: true` saves into the user library
folder of the desktop app (listed as category `user`, User Defined). `swatch.library.load {path? | data? |
dataBase64?, name?}` loads a `.vcswatches` or `.gpl` file, or another document's swatches, as a library to add from.

## Graphic style libraries

Graphic style libraries are read-only sets of graphic styles generated in code (Shadows and Glows, Outlines and
Rules, Hand-Drawn, Gradient Finishes, Shape Effects, Blends and Transparency). `graphicStyle.libraries` lists them
(`id`, `name`, `category`: `builtIn`, `user` or `loaded`, `count`) and `graphicStyle.library {library}` returns one's
styles in the `graphicStyle.list` shape (`library` is an id or a name). `graphicStyle.addFromLibrary {library, name? |
names?}` copies styles into the document's Graphic Styles as one undo step (no names: the whole library); styles the
document already has (same name and look) are reported under `existing`, a name another look has gets a number, and
the patterns the styles paint with come along. `apply: true` also applies the first one to `ids` or the selection
(`add: true` adds its appearance on top instead), in the same undo step: what clicking a style in the library panel
does. Library styles keep placed gradients in unit-box space, so each object gets them at the same place relative to
its bounds.

```json
{"name":"run_command","arguments":{"command":"graphicStyle.library","params":{"library":"hand-drawn"}}}
{"name":"run_command","arguments":{"command":"graphicStyle.addFromLibrary","params":{"library":"hand-drawn","name":"Crosshatch","apply":true}}}
```
`graphicStyle.saveLibrary {path?, names?, name?, user?}` writes the document's styles as a `.vcstyles` library (JSON:
the styles unlinked from swatches, with their opacity, blend mode, isolate and knockout, and the patterns they paint
with); without `path` it returns `{data}`, and `user: true` saves into the user library folder of the desktop app
(category `user`, User Defined). `graphicStyle.loadLibrary {path? | data? | dataBase64?, name?}` loads a `.vcstyles`
file, or another document's graphic styles, as a library to add from.

## Strokes on type

`stroke.set` without an `item` gives type its characters' stroke: weight, cap, join, miter limit and the dash
options go to every run (the object keeps no stroke of its own); an `item` still edits an object-level stroke added
with `appearance.addStroke`, which takes every stroke option (dashes, profile, opacity, blend) like a path's and paints under or over the
characters as its place relative to the Characters row says. To stroke
some characters, use `text.setRangeStyle {id, start, end, strokeOptions: {weight?, cap?, join?, miterLimit?, dash?,
dashOffset?, alignDashes?}}`. `inspect_document` reports each object's stroke as `strokeOptions` (type: its first
run's) in `stroke.set` terms. `stroke.set` on a group leaves the images and symbol instances in it alone.

## Flatten Transparency

`object.flattenTransparency {ids?, preset?, …options}` turns transparent art into opaque art that looks the same, in
one undo step. The targets split into groups of overlapping objects; groups without transparency stay as they are.
Each other group becomes one group of atomic regions: paths filled with the flat colour the art showed there (over
white; over nothing, keeping alpha, with `preserveAlpha: true`), plus one image where gradients, patterns, images,
opacity masks or raster effects reach (clipped to those regions with `clipComplexRegions`, a rectangle without).
`preset` is `high`, `medium` (the default) or `low`; option keys (`balance` 0–100, `lineArtPpi`, `gradientPpi`,
`textToOutlines`, `strokesToOutlines`, `clipComplexRegions`, `antiAlias`, `preserveAlpha`, `preserveOverprints`)
override it, at the top level or in `options`. `balance: 0` rasterizes everything; lower balances rasterize groups
that split into many regions. The result reports `{ids, vector, rasterized, options}`.

```json
{"name":"run_command","arguments":{"command":"object.flattenTransparency","params":{"ids":[12,15],"preset":"high"}}}
{"name":"run_command","arguments":{"command":"object.flattenTransparency","params":{"balance":0,"lineArtPpi":150}}}

## Pattern editing display

In pattern editing mode the tile edge (`pattern.options {showTileEdge}`) and the swatch bounds (`pattern.options
{showSwatchBounds}`: the part of the tiling the swatch repeats, dashed) are drawn in the preference
`patternTileEdgeColor`.

```json
{"name":"run_command","arguments":{"command":"pattern.options","params":{"showSwatchBounds":true}}}
{"name":"run_command","arguments":{"command":"prefs.set","params":{"key":"patternTileEdgeColor","value":"#ff4f4f"}}}
```

The Eyedropper: `appearance.copyFrom {source}` copies what the Eyedropper Options pick up and apply from `source` to the
selection (`ids`); `reverse: true` copies the selection's attributes onto `source` (Alt-click) and `append: true` adds
the source's fills and strokes on top of each target's stack (Shift+Alt-click). The options live in the preferences:
`eyedropper.setOptions {sampleSize?, pickUp?, apply?}` (or `prefs.get`/`prefs.set` with key `eyedropper`) reads and
sets them; a tree names flags under `appearance` (`transparency`, `fill` `{color, transparency, overprint}`, `stroke`
`{color, transparency, overprint, weight, cap, join, miter, dash}`), `character` and `paragraph`, and a bool for a
branch sets all of it. `paint.sampleColor {color}` puts a sampled colour (in its own model) into the active proxy.

```json
{"name":"run_command","arguments":{"command":"eyedropper.setOptions","params":{"pickUp":{"appearance":{"stroke":{"weight":false}}}}}}
{"name":"run_command","arguments":{"command":"appearance.copyFrom","params":{"source":12,"ids":[7,8]}}}
```

## Targeting layers and moving appearances

`layer.target {id}` is the Layers panel's target circle: a layer gets its visible, unlocked art selected and is itself
the target, so `appearance.*`, `effect.*`, `transparency.*` and the opacity-mask commands without `ids` act on the
layer (its opacity, its own fills and effects, an opacity mask on the whole layer); a group or object is simply
selected. `document.inspect` reports it as `target`, and any other selection change ends it.
`appearance.transfer {source, target, copy?}` is dragging a target circle onto another: the target gets the source's
fills, strokes, effects and transparency, and the source is cleared unless `copy` (Alt-drag). Dropping a circle on the
panel's trash is `appearance.clear {ids: [id]}`. Masked objects' names have a dashed underline; while an opacity mask
is edited the panel lists only an `<Opacity Mask>` entry and the document tab says `(<Opacity Mask>/Opacity Mask)`.

```json
{"name":"run_command","arguments":{"command":"layer.target","params":{"id":2}}}
{"name":"run_command","arguments":{"command":"transparency.set","params":{"opacity":50}}}
{"name":"run_command","arguments":{"command":"appearance.transfer","params":{"source":12,"target":2,"copy":true}}}
```

## Expand Appearance

`effect.expandAppearance {ids?, target?}` (Object → Expand Appearance, enabled when a selected or targeted object's appearance isn't basic)
turns appearances into plain objects in one undo step: each visible fill becomes a copy of the path painted by that
fill alone and each stroke its outline filled with its paint (a brushed stroke: its brush art), grouped in paint order
under the object's id; each piece takes its fill's or stroke's opacity and blend mode, and the group keeps the
object's transparency and opacity mask. Geometry effects are baked; raster effects become an embedded image at the
document's raster effects resolution (shadows and outer glows under the art; a blur, feather or inner glow replaces
the object with the image). Type with effects or fills and strokes of its own is outlined. A group's or layer's own
fills and strokes become objects among its members, and its members are expanded too.

```json
{"name":"run_command","arguments":{"command":"effect.expandAppearance","params":{"ids":[12]}}}
```

## Tints of global and spot colours

A colour linked to a global or spot swatch has a tint (the reference app's T slider): `paint.setFill {swatch,
tint?: 0..100}` (and `paint.setStroke`) applies the swatch at that percentage, linked, so `swatch.edit` recolours it
at its own tint. `paint.proxies` and `document.inspect` show the paint as `{type: "solid", color, swatch, tint}`
(`tint` 0..1, left out at 100 %). `swatch.new {tint?}` with the current fill (or `swatch`) saves a tint swatch,
"Name 40%": `swatch.list` reports it with `tintOf` and `tint`, applying it links to its base at that tint, and it
follows edits of its base. A spot tint prints that percentage of its plate (Separations Preview, PDF Separation
value). `edit.colors.adjustBalance {tint: -100..100}` (Global mode) shifts the tints of the selection's linked
colours and leaves the rest alone.

```json
{"name":"run_command","arguments":{"command":"paint.setFill","params":{"swatch":"Ink","tint":40}}}
{"name":"run_command","arguments":{"command":"edit.colors.adjustBalance","params":{"mode":"global","tint":-20}}}
```

## Linked gradient stops

Applying a gradient swatch (`paint.setFill {swatch}`) records it as the gradient's `swatch` (the Swatches panel
highlights it; new stops drop the link). A gradient stop can link to a global or spot swatch like a solid colour:
give it `swatch` (and `tint` %, default 100 or a tint swatch's own) instead of `color` in `paint.editGradient
{stops}` or a `gradient` paint's stops. Editing or deleting the swatch then recolours or unlinks the stop, in art and
in gradient swatches. A spot stop separates on its plate; a gradient whose stops are all tints of one spot ink (or
paper white, 0 %) exports to PDF as a Separation shading (the writer has no DeviceN, so a gradient mixing a spot
ink with other colours is written in process colours, with a warning).

```json
{"name":"run_command","arguments":{"command":"paint.editGradient","params":{"stops":[{"offset":0,"swatch":"Ink"},{"offset":1,"swatch":"Ink","tint":20}]}}}
```

## Transparency flattener presets

`flattener.presets.list` lists the built-in presets (High, Medium and Low Resolution, `builtIn: true`) and the saved
ones, each with its `options`. `flattener.presets.save {name?, newName?, preset?, …options}` creates or changes a saved
preset (a change starts from the preset's own options; `newName` renames it; built-in presets can't change);
`flattener.presets.delete {name}` deletes one. `flattener.presets.export {names?, path?}` writes them as a
`.vcflattener` JSON file (without `path` it returns `data`), and `flattener.presets.import {path? | data? |
dataBase64?, replace?}` adds a file's presets (names in use get a number unless `replace`). Saved presets live with
the preferences and work as `preset` in `object.flattenTransparency` (and anywhere else flattener options are taken).

```json
{"name":"run_command","arguments":{"command":"flattener.presets.save","params":{"name":"Press","preset":"high","balance":60}}}
{"name":"run_command","arguments":{"command":"object.flattenTransparency","params":{"preset":"Press"}}}
```

## Flattener Preview

`flattener.preview {highlight?, overprints?, preset?, …options, ids?}` reports what flattening the document (or
`ids`) would do without changing it: `counts` of transparent objects, all affected objects, patterns, outlined
strokes and type, rasterized complex regions (areas the raster/vector balance rasterizes whole), all rasterized areas
and flat-colour regions, plus `regions` (bounds, and `id` for objects) for the `highlight` asked for. `overprints:
"discard"` or `"simulate"` flatten without preserving overprints.

```json
{"name":"run_command","arguments":{"command":"flattener.preview","params":{"highlight":"allRasterized","preset":"low"}}}

## Width profiles

The Profile list holds the built-in variable-width profiles (`uniform`, `lens`, `taperStart`, `taperEnd`, `pinch`,
`teardrop`, `wave`) and profiles saved from strokes; saved ones are kept with the preferences, not in the document
(no undo step). `stroke.widthProfile.list` returns every row (`id`, `label`, `builtIn`, `points` as `[t, left,
right]`) and `current`, the selected stroke's profile (`"custom"` when it isn't listed).
`stroke.widthProfile.add {name?}` saves the selected stroke's variable width (default name "Width Profile N"),
`stroke.widthProfile.delete {name?}` removes a saved one (default: the selected stroke's; built-ins can't be deleted)
and `stroke.widthProfile.reset` removes every saved one. `stroke.set {profile}` takes a built-in id or a saved name.

```json
{"name":"run_command","arguments":{"command":"stroke.widthProfile.add","params":{"name":"Ribbon"}}}
{"name":"run_command","arguments":{"command":"stroke.set","params":{"ids":[9],"profile":"Ribbon"}}}
```

## Arrowheads

`stroke.set {startArrow, endArrow}` takes any of 40 generated heads by name (the params doc lists them; `null` for
none): arrows (`Arrow`, `Barbed`, `Concave`, `DoubleArrow`, `HalfArrowLeft`/`Right`, `Chevron`, `Feather`,
`Swallowtail`…), filled and open shapes (`Triangle`, `Circle`, `Oval`, `Target`, `Tag`, `Diamond`, `Hexagon`, `Star`,
the `…Open` rings…) and marks (`Bar`, `DoubleBar`, `DotOnBar`, `Slash`, `DoubleSlash`, `Bracket`, `Fork`, `Cross`,
`Plus`). Each head fits a box about four times its weight (stroke weight × `arrowScale`) long and wide.

## Stroke bounds and clicks

Visual bounds (Fit to Selected Art, Rasterize, exporting all art) and clicks on the canvas take
in the whole stroke as drawn: arrowheads, inside/outside alignment (an outside stroke is hit outside the path, an
inside one inside it), the width profile's width where you click, projecting caps and miter spikes up to the miter
limit. A rectangle's right-angle miters stay within half the weight of its edges.

## Expand

`object.expand {object?, fill?, stroke?, gradient?, steps?}` (Object → Expand…) turns the selection into plain art in
one undo step. `object` (on by default) outlines type, turns live shapes into paths and bakes effects; `stroke` (on)
outlines strokes into filled paths; `fill` (on) expands gradient fills: with `gradient: "objects"` (the default) into
`steps` (1–1000, default 255) solid objects (rectangles across a linear gradient, each from its band to the far end so
no seams show; concentric ellipses for a radial one, largest first; just the bands when stops are translucent), with
`gradient: "mesh"` into a gradient mesh that paints the gradient (columns or rings where its colour changes, sharp at
coincident stops), each inside a clip group shaped like the object. A freeform gradient becomes a mesh shaped like the
object either way. Expanded colours keep the colour model their stops share. With `fill: false` gradient fills stay
live. `object.expand.info` answers which options have something to expand in the selection (`{object, fill,
stroke}`), and `object.mesh.create` on a gradient-filled object colours the mesh points as the gradient paints them.

```json
{"name":"run_command","arguments":{"command":"object.expand","params":{"stroke":false,"gradient":"mesh"}}}
{"name":"run_command","arguments":{"command":"object.expand","params":{"steps":16}}}

## Lab spot colours

Colours can be CIE Lab (D50): give `{"l": 55, "a": 60, "b": 40}` (L 0–100, a and b about −128–127) wherever a
colour is taken; documents store them as `{"model": "lab", …}`. `swatch.new {color, spot: true}` or
`swatch.edit {name, mode: "lab"}` defines a spot colour in Lab (Swatch Options' Lab mode). `swatch.spotOptions
{useLab}` sets the document's Spot Colors options: with `true` (the default) Lab spot colours show and print from
their Lab values and PDF export writes their Separation spaces with a Lab alternate; with `false` art linked to them
takes their working-CMYK equivalents and PDF uses a DeviceCMYK alternate. Without `useLab` it answers the current
setting.

```json
{"name":"run_command","arguments":{"command":"swatch.new","params":{"name":"Lab Ink","color":{"l":55,"a":60,"b":40},"spot":true}}}
{"name":"run_command","arguments":{"command":"swatch.spotOptions","params":{"useLab":false}}}
```

## Attributes, URLs and image maps

The Attributes panel (`window.panel {panel: "attributes"}`, Cmd+F11) reads `attributes.info {ids?}`:
`overprintFill`, `overprintStroke`, `showCenter`, `imageMap`, `url`, `note`, `fillRule` and `reversed` (null where the
objects differ) and `recentUrls`. `attributes.set {overprintFill?, overprintStroke?, showCenter?, imageMap?, url?,
note?, ids?}` sets them in one undo step; `path.setFillRule {rule: "nonZero"|"evenOdd"}` sets the fill rule of the
paths and compound paths, and `path.reverse {reversed?}` makes subpaths run counter-clockwise (true) or clockwise
(false). SVG export wraps an object with a URL in `<a xlink:href>`, and SVG import reads `<a href>` links back.

```json
{"name":"run_command","arguments":{"command":"attributes.set","params":{"ids":[12],"url":"https://example.com","imageMap":"rectangle"}}}
{"name":"run_command","arguments":{"command":"path.setFillRule","params":{"rule":"evenOdd"}}}
```

## Registration and trim marks

Every document has the built-in `[Registration]` swatch (listed after None by `swatch.list`): a colour that prints on
every plate, process and spot. `paint.setStroke {swatch: "[Registration]"}` applies it; it can't be edited, moved,
duplicated, merged or deleted. Separations Preview shows it on each plate and PDF export writes it as
`/Separation /All`. `object.createTrimMarks {style?, allArtboards?}` draws trim marks in Registration around the
selection, or around every artboard when nothing is selected; `effect.apply {effect: "cropMarks"}` adds live crop
marks that follow the object. Both use Japanese marks (double lines at the trim and bleed edges, centre marks) when
the preference `japaneseCropMarks` is on or `style: "japanese"` is given.

```json
{"name":"run_command","arguments":{"command":"prefs.set","params":{"key":"japaneseCropMarks","value":true}}}
{"name":"run_command","arguments":{"command":"object.createTrimMarks","params":{}}}
```

## Scale Strokes & Effects

`object.scale`, `object.transform` and `object.transformEach` take `strokes?: bool` (Scale Strokes & Effects) and
`corners?: bool` (Scale Corners); without them the preferences `scaleStrokes` and `scaleCorners` (both off by
default) apply (`prefs.set {key, value}`). With strokes on, a scale by k (the square root of the transform's determinant)
multiplies stroke weights, dash lengths and dash offsets, and every distance parameter of the object's, its fills' and
its strokes' effects (drop shadow offsets and blur, offsets, radii…; `effect.list` gives each effect's `lengths`;
relative Roughen, Tweak and Zig Zag sizes stay percentages), in groups and layers too. With it off, nothing painted
changes size, and type keeps its character strokes' weight. With Scale Corners off, live corner radii keep their
size. The journal entry of a scaling command records the `strokes` and `corners` it used, so actions replay alike.

```json
{"name":"run_command","arguments":{"command":"object.scale","params":{"sx":200,"strokes":true}}}
{"name":"run_command","arguments":{"command":"object.transformEach","params":{"scaleH":50,"scaleV":50,"strokes":false}}}
```

## Use Preview Bounds

With the preference `usePreviewBounds` on (`prefs.set {key: "usePreviewBounds", value: true}`; also the Align panel
flyout's Use Preview Bounds), the Transform panel's and Control bar's X, Y, W and H, the bounding box, and
`object.align`, `object.distribute` and `object.distributeSpacing` measure visual bounds, which take in the whole
stroke (see Stroke bounds and clicks); off, they measure the paths. The three Align commands also take
`bounds: "preview"|"geometric"` for one call. `object.setBounds` then sets the visual box: with Scale Strokes &
Effects off a 100 pt wide rectangle with a 10 pt stroke set to `width: 220` gets a 210 pt path.

```json
{"name":"run_command","arguments":{"command":"object.align","params":{"horizontal":"left","bounds":"preview"}}}
```

## Width points

`stroke.widthPoint.set {id, t, left, right, index?, adjustAdjoining?}` adds a width point (side widths in points) or,
with `index`, edits or moves one; `adjustAdjoining` changes the nearest points either side in proportion. Moving a
point onto another one's `t` (or `stroke.widthPoint.copy {id, index, t}` landing there) makes a discontinuous point:
two points at the same `t` (the width before it, then after it), and the stroke's width steps there.
`document.inspect` reports a path's `strokeOptions.widthPoints` (`[t, left, right]`, factors of half the weight; null
for a uniform stroke). `stroke.widthPoint.remove {id, index | indices}` deletes points; `stroke.widthProfile.set {ids,
points}` replaces them all, as dragging several Shift-selected points with the Width tool does in one undo step.

```json
{"name":"run_command","arguments":{"command":"stroke.widthPoint.set","params":{"id":9,"t":0.5,"left":4,"right":4}}}
{"name":"run_command","arguments":{"command":"stroke.widthPoint.set","params":{"id":9,"t":0.8,"left":12,"right":12}}}
{"name":"run_command","arguments":{"command":"stroke.widthPoint.set","params":{"id":9,"index":1,"t":0.8,"left":4,"right":4}}}
```

## New art

New objects take the fill and stroke proxies (`paint.setFill` / `paint.setStroke` and the weight, whatever is
selected) on top of a template. With nothing selected, `stroke.set` / `stroke.setAdvanced` (cap, join, dashes,
arrowheads, profile…) and `graphicStyle.apply` (every fill, stroke and effect, opacity, blend mode and the link to the
style; `add: true` stacks it) set that template instead of editing art. With the Appearance panel's New Art Has Basic
Appearance off (`appearance.setNewArtBasic {on: false}`, the preference `newArtBasic`) new art takes the whole
appearance of the last selected object. `appearance.newArt` reports what the next object gets; `paint.default`
resets it.

```json
{"name":"run_command","arguments":{"command":"select.none","params":{}}}
{"name":"run_command","arguments":{"command":"stroke.set","params":{"cap":"round","dash":[6,3]}}}
{"name":"run_command","arguments":{"command":"appearance.newArt","params":{}}}
```

## Gradients on strokes

A linear or radial gradient on a stroke lies within it (placed on the page like a fill's gradient: the default), along
it (from the start of each subpath to its end) or across it (from the stroke's left edge to its right edge, left of the
path's direction; an inside or outside stroke spans the side it shows on). `paint.editGradient {stroke: true,
strokeMode: "within"|"along"|"across"}` sets it (the Gradient panel's Stroke buttons), and `document.inspect` reports it
as `strokeOptions.gradientMode`. Width profiles, dashes and arrowheads keep working: the gradient runs on through gaps
and into the heads. Outline Stroke and Expand turn such a stroke into gradient meshes clipped to its outline; SVG and PDF
write it as slices of linear gradients clipped to the outline (`vectorcraft_svg::export_with_report` and the PDF
report warn about it).

```json
{"name":"run_command","arguments":{"command":"paint.editGradient","params":{"stroke":true,"strokeMode":"along"}}}
```

## Units

Commands take and return lengths in points (the distances of `object.path.offsetPath`, `object.path.simplify`,
`object.path.splitIntoGrid` and `text.setStyle` `size`/`leading` also take a string with a unit, `"5 mm"`). The units
only change what the UI shows and how it reads typed numbers. Preferences ▸ Units has
three: General (rulers, positions and sizes, the Info panel, dialog distances, canvas measurement labels), Stroke
(weights, dashes) and Type (font size, leading, baseline shift). General is the active document's units
(`document.inspect` → `units`): `document.setUnits {units}` (Document Setup) sets it for that document, and
`prefs.set {key: "unitsGeneral", value}` sets it for the active document too (one undo step) and is the units
`file.new` starts in when it gets no `units`. Stroke and Type are the preferences `unitsStroke` and `unitsType`.
`prefs.list` marks the length preferences (`keyboardIncrement`, `cornerRadius`, `pasteOffset`, `gridlineEvery`,
`typeSizeIncrement`, `baselineShiftIncrement`) with `measure: "general"|"type"`; they are kept in points and also take
a string with a unit. Unit names: `Points`, `Picas`, `Inches`, `Millimeters`, `Centimeters`, `Pixels`, `Feet & Inches`,
`Meters`, `Yards`, `Feet` (the preferences use `points`, `picas`, … `feetInches`, `meters`, `yards`, `feet`).

```json
{"name":"run_command","arguments":{"command":"prefs.set","params":{"key":"unitsGeneral","value":"millimeters"}}}
```
