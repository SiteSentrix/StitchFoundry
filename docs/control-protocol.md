# Control protocol

`vectorcraft --control <port>` listens on `127.0.0.1:<port>` (loopback only). One JSON request per line:
`{"id": 1, "method": "ui.inspect", "params": {}}` → `{"id": 1, "ok": true, "result": {...}}` or `{"id":1,"ok":false,"error":"..."}`.

| Method | Params | |
|---|---|---|
| `engine.execute` | `{command, params}` | run any engine or UI command (see `engine.commands`) |
| `engine.commands` | | every command with label, shortcut, params doc, enablement |
| `document.inspect` | | layer tree, selection, history, paint defaults |
| `ui.inspect` | | tool, UI state, view, canvas rect, window size, perf |
| `ui.menu.list` / `ui.menu.invoke` | `{command, params}` | the full menu tree / invoke an item |
| `ui.tool.select` / `ui.tool.list` | `{tool}` | |
| `ui.pointer` | `{events:[{kind: down|drag|up|move|doubleclick, x, y, space?: "doc"|"screen", mods?}]}` | drive the active tool exactly like the mouse |
| `ui.key` / `ui.text` | `{key, shift?, alt?, cmd?}` / `{text}` | synthetic keyboard input |
| `ui.set` | `{brightness?, panel?, rulers?, outline?, grid?, smartGuides?, boundingBox?, controlBar?}` | |
| `ui.dialog.set` / `.confirm` / `.cancel` | `{field, value}` | fill and submit the open dialog |
| `ui.screenshot` | `{path?}` | capture the window (PNG). Needs a presented frame: with the screen locked or the window minimized/covered it fails after ~8 s with an explanatory error |
| `ui.render` | `{path?, scale?}` | render the artboard headlessly (PNG) |
| `ui.resize` / `ui.focus` | | |
| `app.open` / `app.save` / `app.export` / `app.quit` | `{path}` / `{path?}` / `{path?, format?, artboard?, range?, scale?, …}` | `app.open` reads every format `document.open` reads (see `document.formats`). `app.export` encodes through the engine's `document.export` (same options; the document keeps its path) and writes `path` through the host; without `path` it returns `{dataBase64, format, bytes}`, as headless mode does. `app.quit`, `file.close` and `file.closeAll` first open a `saveChanges` dialog for each modified document (they return `{"pending": "saveChanges"}`): `ui.dialog.confirm` saves, `ui.dialog.set {field: "discard", value: true}` then confirm discards, `ui.dialog.cancel` cancels the whole close or quit |
| `ui.dialog.*` on `gradientStop` | `{field: "color" \| "opacity" \| "location", value}` | double-clicking a stop on the Gradient tool's annotator opens its popover (fields `index`, `x`, `y`, `tab`); set `color` (hex), `opacity` or `location` (percentages) and `ui.dialog.confirm` to apply them to the selected stop. `gradient.selectStop {index}` picks the stop the annotator, the panels and Delete/←/→ (`ui.key`) act on |
| `engine.execute` `ui.savePdfDialog` | `{path?, preset?, range?, …document.exportPdf options}` | opens the Save PDF dialog (`file.export.pdf` with no params opens it too). Its fields are the `document.exportPdf` options: set `compatibility` with `ui.dialog.set {field: "compatibility", value: "1.5"}`, a whole section with `{field: "compression", value: {…}}`, the artboards with `__allArtboards: false` and `range`, the visible section with `__section` (General, Compression, Marks and Bleeds, Output, Advanced, Security, Summary). `ui.dialog.confirm` writes `path` (else asks for one), returns `{path, bytes, warnings}` and opens the file when `viewAfterSaving` is set |
| SVG Options (`engine.execute` of `file.export.svg`, `file.saveAs`, `file.saveCopy`) | `{}` / `{path?, svg?: {…}}` | `file.export.svg` without params, and Save As or Save a Copy to an `.svg`/`.svgz` path without SVG options, open the `svgOptions` dialog (`ui.dialog.set` its fields: `styling`, `outlineText`, `images`, `objectIds`, `decimals`, `minify`, `responsive`, `fewerTspans`, `metadata`, `preserveEditing`, `useArtboards`, `allArtboards`, `range`, `showCode`; `ui.dialog.confirm` exports or saves and remembers the choices). With `svg: {…}` (the `document.formats` SVG options) they run directly. `app.save {path?, svg?}` takes the same options; a document saved as SVG saves as SVG again with the same options |

Swatch editors: `engine.execute` with `ui.swatchOptions {name}` opens the `swatchOptions` dialog for a colour swatch
(fields `name`, `spot`, `global`, `mode`: `gray`/`rgb`/`hsb`/`lab`/`cmyk`/`web`, `color`: `"#rrggbb"` or a colour object,
`preview`), which previews on the canvas while open; `ui.dialog.confirm` applies it with `swatch.edit` as one undo
step and `ui.dialog.cancel` rolls the preview back. For a gradient swatch the dialog has `name` only (it shows the
gradient) and OK renames it; pattern swatches open pattern editing instead. Dropping a gradient (or colour) on a
swatch of its kind in the Swatches panel with Alt held replaces it (`swatch.edit {name, paint}`).

Confirmations: deleting swatches from the Swatches panel opens a `confirm` dialog (fields `message`, `detail`);
`ui.dialog.confirm` runs the command it asks about (here `swatch.delete`) and `ui.dialog.cancel` drops it.

New swatches and colour groups: `ui.newSwatch {spot?, group?}` opens the `newSwatch` dialog prefilled from the active
fill or stroke (fields as in `swatchOptions` plus `group`; a gradient or pattern only takes `name`) and
`ui.newColorGroup {swatches?}` opens `newColorGroup` (fields `name`, `fromArtwork`, `toGlobal`, `includeTints`,
`swatches`); `ui.dialog.confirm` runs `swatch.new` / `swatch.newGroup`.

The Color Picker is a dialog too: `engine.execute {command: "ui.colorPicker", params: {stroke?, color?}}` opens it for the fill (or stroke) proxy; `ui.dialog.set {field: "hex", value: "00FF00"}` (or `color`, `channel`, `webOnly`, `swatches`) then `ui.dialog.confirm` applies the colour through `paint.setFill` / `paint.setStroke`.

Graphic styles: `ui.graphicStyleOptions {name}` opens the `graphicStyleOptions` dialog (field `name`) for a style and
`ui.dialog.confirm` renames it with `graphicStyle.rename` (a name another style has returns an error and keeps the
dialog open); without `name` it names a new style made from the selection (`graphicStyle.new`).
`ui.mergeGraphicStyles {names}` opens the same dialog to name the style `ui.dialog.confirm` merges from them
(`graphicStyle.merge`). Deleting styles from the panel asks first with a `confirm` dialog (`graphicStyle.delete`).

Gradient panel: double-clicking a stop on the panel's slider opens the same `gradientStop` popover, with a `screen`
field (`[x, y]`, screen points) in place of `x`/`y`. The panel's stop eyedropper selects the Eyedropper tool with the
tool option `stop` (the tool to return to; `tool.setOption {key: "stop", value: "gradient"}`): its next click on art
samples the colour there into the selected stop (`paint.sampleColor {color, stop}`) and switches back. Dragging a
swatch, a Fill/Stroke proxy or the panel's gradient thumbnail onto art runs `paint.setFill` / `paint.setStroke` (the
active proxy) with the paint's params and the object's `ids`; a colour dropped on the panel's ramp adds or recolours
a stop.

Freeform gradients: double-clicking a point with the Gradient tool opens the same `gradientStop` popover on the selected
point (`paint.freeform.selectPoint`); its fields are `color` (hex), `opacity` and `spread` (percentages), applied by
`ui.dialog.confirm` through `paint.freeform.setPoint`. `ui.key` Escape ends the line Lines mode is drawing.

Tool options: double-clicking a tool button runs `tool.options {tool}`. For `gradient` it opens the Gradient panel;
for `eyedropper` it opens Eyedropper Options, an `eyedropperOptions` dialog (fields `sampleSize` 1/3/5, `pickUp` and
`apply`, the attribute trees of `eyedropper.setOptions`) whose `ui.dialog.confirm` runs `eyedropper.setOptions` (what
`appearance.copyFrom` copies). Gradient tool handles snap to
anchors, edges and smart guides; Shift constrains them to 45° steps from the `constrainAngle` preference.

Effect dialogs: `engine.execute {command: "effect.dialog", params: {effect, index?, item?}}` opens the `effect` dialog
(fields: the effect's parameters, `preview`). With `index` it edits that applied effect of `item` (null: the object's
effects) prefilled with its values, and `ui.dialog.confirm` runs `effect.setParams`; otherwise confirm runs
`effect.apply`. Choosing an effect that the list already has returns `{"pending": "effectExists"}` and opens the
`effectExists` question: `ui.dialog.confirm` opens the applied effect's dialog, `ui.dialog.set {field: "discard",
value: true}` then confirm opens a fresh one that adds another, `ui.dialog.cancel` drops it.

Overprint Black: the Edit → Edit Colors → Overprint Black… menu item opens a `command` parameter dialog for
`edit.colors.overprintBlack` (fields `remove`, `percentage`, `fill`, `stroke`, `includeCmyBlacks`,
`includeSpotBlacks`); `ui.dialog.set` then `ui.dialog.confirm` runs it on the selection.

Color Guide: `color.harmony {color, rule, steps?, variation?, amount?}` returns the harmony group (base first) and
its variation grid; the panel draws the same grid. `ui.colorGuideOptions` opens the `colorGuideOptions` dialog
(fields `steps` 1–20, `amount` 0–100) and `ui.dialog.confirm` sets the panel's options, which `ui.inspect` reports
as `ui.color_guide` (`variation`, `steps`, `amount`). Save Colors as Swatches runs `swatch.new {colors: [...]}`
(one swatch per colour, one undo step).

Color Guide Limit to Library: `ui.colorGuideLimit {library}` (a swatch library id or name, `"document"` for the
document's swatches, `""` for none) limits the panel's colours; `ui.inspect` reports it as `ui.color_guide_limit`, and
`color.harmony {..., limitTo}` answers the limited guide (every colour snaps to the library's nearest colour, ΔE 2000).
Edit or Apply Colors runs `ui.recolorDialog {colors, library?}` with the harmony colours: on the selected art it
recolours as usual; with nothing selected the colours themselves are the rows and `ui.dialog.confirm` saves them as a
new colour group (`swatch.newGroup`, named by the `groupName` field).

Color Themes panel (`window.panel {panel: "colorThemes"}`): its Create tab makes a theme on a harmony wheel and its
My Themes tab lists the saved themes; every action runs a command (`colorTheme.save`, `colorTheme.delete`,
`colorTheme.addToSwatches`, `swatch.newGroup` for the theme being made), and `colorTheme.list` reads the library.

Edit Colors dialogs: `ui.colorBalanceDialog` opens Adjust Colors (`colorBalance`: fields `mode` `gray`/`rgb`/`cmyk`/
`global`, the channels `r` `g` `b` / `c` `m` `y` `k` / `gray` / `tint` (global mode) from −100 to 100, `convert`, `fill`, `stroke`,
`preview`) and `ui.saturateDialog` opens Saturate (`saturate`: `intensity` −100..100, `preview`). Both preview on the
canvas while open; `ui.dialog.confirm` keeps the result as one undo step (`edit.colors.adjustBalance` /
`edit.colors.saturate`) and `ui.dialog.cancel` rolls it back. Global mode answers with an error until tints of global
and spot colours exist; the dialog then stays open.

Recolor Artwork: `ui.recolorDialog {colors?, library?, group?}` opens the `recolor` dialog on the selected art
(`colors`: a count for an n-colour job, or colours to assign as the new colours; `library`: Limit to Library, a library
or `"document"`, `""` for the first library; `group`: a colour group, Edit or Apply Color Group). Fields: `rows` (`[{from: [keys], to: key, exclude?}]`
as `recolor.reduce` returns them), `colors` (null for Auto), `method`, `preserveWhite`, `preserveBlack`,
`preserveGrays`, `limitTo`, `groupName`, `tab` (`assign` or `edit`), `rule` (harmony id), `linked`, `preview`. Setting
`colors`, a preserve flag or `limitTo` reduces the rows again (also on `ui.dialog.confirm`). The dialog previews
`recolor.apply` on the canvas; confirm keeps it as one undo step (with a group, the group is rewritten in the same
step; with no art selected only the group changes). In the Swatches panel, double-clicking a colour group's folder,
or the options button while a group is selected, opens it.

Library panel: `engine.execute {command: "window.swatchLibrary", params: {library}}` opens the read-only library
panel on a swatch library (`ui.inspect` shows it as `library_panel: {kind, id}`; `library: null` closes it).
Clicking a swatch there runs `swatch.library.add {library, names: [name], apply}` (the active proxy, or the other
one with Alt), so one undo step adds and applies it; Shift/Cmd-clicks select swatches and colour groups for
Add to Swatches.
`window.swatchLibrary.other {path?}` loads a library file (or another document's swatches) and opens it there;
opening a `.vcswatches` or `.gpl` file with `app.open` does the same. `ui.saveSwatchLibrary {names?}` opens the
`saveSwatchLibrary` dialog (fields `name`, `format`: `vcswatches`/`gpl`/`css`, `user`: save to the user library
folder, `selectedOnly` with `names`); `ui.dialog.confirm` runs `swatch.library.save` (to a file it asks for a path).

Graphic style libraries open in the same panel: `window.graphicStyleLibrary {library}` (`library_panel: {kind:
"graphicStyles", id}`; `library: null` closes it). Clicking a style there runs `graphicStyle.addFromLibrary {library,
name, apply: true, add}` (`add` with Alt), adding and applying it in one undo step; Shift/Cmd-clicks select styles
for Add to Graphic Styles. `window.graphicStyleLibrary.other {path?}` loads a `.vcstyles` file (or another
document's graphic styles) and opens it there; opening a `.vcstyles` file with `app.open` does the same. The User
Defined libraries are `window.userGraphicStyleLibrary1`…`10` in Window → Graphic Style Libraries.
`ui.saveGraphicStyleLibrary {names?}` opens the `saveGraphicStyleLibrary` dialog (fields `name`, `user`: save to the
user library folder, `selectedOnly` with `names`); `ui.dialog.confirm` runs `graphicStyle.saveLibrary` (to a file it
asks for a path).

Tile Edge Color: Object → Pattern → Tile Edge Color… (`ui.tileEdgeColor`) opens the `tileEdgeColor` dialog (field
`color`: `#rrggbb` or a preset name such as "Light Blue"); `ui.dialog.confirm` sets the preference
`patternTileEdgeColor` (`prefs.set`), the colour pattern editing mode draws the tile edge and the swatch bounds in.

Window title bar: on Windows and Linux the window has no OS decorations and the app bar is the title bar. Its
caption buttons (Minimize, Maximize/Restore, Close) sit at the bar's right end, 46 pt each; they are window chrome,
not commands, so drive them with `ui.click` if needed. Close runs `app.quit` (the same `saveChanges` questions for
modified documents), and empty bar space and the 5 pt window edges move and resize the window. macOS and the web
build are unchanged.

Fill/Stroke chips and panel shortcuts: the Control bar's and Properties' Fill and Stroke chips bring their proxy
forward (`paint.toggleActive {fill}`) and open a popover with the Swatches panel (Shift-click: the Color panel's mixer);
a swatch clicked there runs `paint.setFill` / `paint.setStroke`. Panel keys (Color F6, Color Guide Shift+F3,
Appearance Shift+F6, Graphic Styles Shift+F5, Stroke Cmd+F10, Gradient Cmd+F9, Transparency Cmd+Shift+F10) run
`window.panel {panel}` and can be pressed with `ui.key`; `ui.menu.list` shows them on the Window menu's items.

Flatten Transparency: `ui.flattenTransparencyDialog` opens the `flattenTransparency` dialog for the selection
(fields `preset`: a preset name, setting it loads that preset's options; the option keys of
`object.flattenTransparency`: `balance` 0–100, `lineArtPpi` and `gradientPpi` 1–2400, `textToOutlines`,
`strokesToOutlines`, `clipComplexRegions`, `antiAlias`, `preserveAlpha`, `preserveOverprints`; `preview`, off at first).
With `preview` on it flattens on the canvas while open; `ui.dialog.confirm` keeps the result as one undo step and
`ui.dialog.cancel` rolls it back. Out-of-range values answer with an error and the dialog stays open.

Flattener presets: `ui.flattenerPresetsDialog {selected?}` opens Edit → Transparency Flattener Presets (dialog
`flattenerPresets`). Fields: `selected` (a preset name), then the selected preset's `name` and option keys. On a saved
preset, `ui.dialog.set` of an option saves it and a new `name` renames it (both through `flattener.presets.save`);
built-in presets don't change. New, Delete, Import… and Export… are `flattener.presets.save {preset}`,
`flattener.presets.delete`, `flattener.presets.import` and `flattener.presets.export`; `ui.dialog.confirm` closes.
Opening a `.vcflattener` file with `app.open` imports its presets. In the Flatten Transparency dialog, Save Preset…
keeps the dialog's options as a saved preset.

Flattener Preview: `window.panel {panel: "flattenerPreview"}` shows the panel; `ui.flattenerPreview {highlight?,
overprints?, preset?, options?, showOptions?}` sets it (a preset loads its options, `options` adjust them), shows it
and refreshes its snapshot of the document, answering as `flattener.preview` does. `ui.inspect` reports the settings
as `ui.flattener_preview`. The panel keeps its snapshot until Refresh (it notes when the document or the settings
changed since); its preview greys the art and paints the highlighted objects or areas red.

Expand: Object → Expand… (`ui.expandDialog`, or `object.expand` without params from the menus, palette and shortcuts)
opens the `expand` dialog for the selection. Fields: `object`, `fill`, `stroke` (all on; `__object`, `__fill`,
`__stroke` say which the selection has something for, as `object.expand.info` does, and the others are disabled),
`gradient` (`objects` or `mesh`) and `steps` (1–1000, default 255). `ui.dialog.confirm` runs `object.expand` with them as
one undo step.

Attributes panel: `window.panel {panel: "attributes"}` (Cmd+F11) shows it; its controls run `attributes.set`,
`path.reverse {reversed}` and `path.setFillRule {rule}`, and its Browser button runs the UI command
`attributes.openUrl {url?}`, which opens the URL (default: the selection's) in the web browser and answers `{url}`.

Spot Colors: the Swatches panel menu's Spot Colors… (`ui.spotColors`) opens the `spotColors` dialog (field `useLab`:
true shows and separates spot colours defined in Lab from their Lab values, false from their working-CMYK
equivalents); `ui.dialog.confirm` runs `swatch.spotOptions {useLab}` as one undo step.

Scale options: `ui.menuDialog {command}` opens the dialog a command's menu item opens (`object.scale` → `scale`,
`object.transformEach` → `transformEach`, also Move, Rotate, Reflect, Shear and the Path dialogs). The Scale dialog
(`scale`) and Object → Transform → Transform Each… (dialog `transformEach`: `scaleH`,
`scaleV` %, `moveH`, `moveV` pt, `rotate` °, `reflectX`, `reflectY`, `random`, `reference` 0–8, `copy`, `preview`)
have the fields `corners` (Scale Corners) and `strokes` (Scale Strokes & Effects). They start from the preferences
`scaleCorners` and `scaleStrokes`, and `ui.dialog.confirm` keeps them as the preferences (`prefs.set`) before running
`object.scale` / `object.transformEach` with them. The Transform panel's checkboxes and flyout item and the Properties
panel's checkbox set the same preferences through `prefs.set`.

Use Preview Bounds: the Align panel flyout's Use Preview Bounds item shows and toggles the `usePreviewBounds`
preference (`prefs.set`); while it is on, the bounding box and the Transform panel, Properties and Control bar
fields measure visual bounds.

Width Point Edit: double-clicking a width point with the Width tool, or `ui.widthPointEdit {id, index}`, opens the
`widthPoint` dialog (fields `id`, `index`, `t`, `side1` and `side2`: the left and right widths in points, `linked`,
`adjustAdjoining`). `ui.dialog.confirm` runs `stroke.widthPoint.set` with them; `ui.dialog.set {field: "discard",
value: true}` then confirm (the Delete button) removes the point with `stroke.widthPoint.remove`.

Units: dialog distance fields (Move's `dx`/`dy`, shape sizes, Offset Path's `offset`, Split Into Grid's `gutter`,
Artboard Options' sizes, effects' distances, `transformEach`'s `moveH`/`moveV`, New Document's `width`/`height`, the
length preferences in `preferences`) show in the General unit (the active document's units, or `unitsGeneral` without a
document). Through `ui.dialog.set` a number is in points and a string may carry its unit (`"10 mm"`; a bare number in a
string is points). The `preferences` dialog's `unitsGeneral` starts as the active document's units, and OK sets them.
`newDocument` has `units` (a unit label; print presets start in `unitsGeneral`, screen presets in Pixels).

Document Setup: File → Document Setup… (`file.documentSetup`, Cmd+Alt+P) opens the `documentSetup` dialog. Its fields
are what `document.setup` reports (`units`, `bleed` [top, bottom, left, right] in pt, `outlineImages`,
`highlightSubstitutedFonts`, `highlightSubstitutedGlyphs`, `gridSize`, `gridColors`, `simulatePaper`, `flattenerPreset`,
`discardWhiteOverprint`, `language`, `quotes`, `typographersQuotes`, `superscript`, `subscript`, `smallCapsSize`,
`exportText`, `backgroundContents`) plus `tab` (`General`, `Transparency` or `Type`) and `bleedLinked`.
`ui.dialog.confirm` runs `document.setup` with them as one undo step; a bad value answers with an error and the dialog
stays open.
