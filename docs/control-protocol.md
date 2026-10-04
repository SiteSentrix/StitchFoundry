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

Swatch editors: `engine.execute` with `ui.swatchOptions {name}` opens the `swatchOptions` dialog for a colour swatch
(fields `name`, `spot`, `global`, `mode`: `gray`/`rgb`/`hsb`/`cmyk`/`web`, `color`: `"#rrggbb"` or a colour object,
`preview`), which previews on the canvas while open; `ui.dialog.confirm` applies it with `swatch.edit` as one undo
step and `ui.dialog.cancel` rolls the preview back. Gradient swatches open the Gradient panel and pattern swatches
pattern editing instead.

Confirmations: deleting swatches from the Swatches panel opens a `confirm` dialog (fields `message`, `detail`);
`ui.dialog.confirm` runs the command it asks about (here `swatch.delete`) and `ui.dialog.cancel` drops it.

New swatches and colour groups: `ui.newSwatch {spot?, group?}` opens the `newSwatch` dialog prefilled from the active
fill or stroke (fields as in `swatchOptions` plus `group`; a gradient or pattern only takes `name`) and
`ui.newColorGroup {swatches?}` opens `newColorGroup` (fields `name`, `fromArtwork`, `toGlobal`, `includeTints`,
`swatches`); `ui.dialog.confirm` runs `swatch.new` / `swatch.newGroup`.
