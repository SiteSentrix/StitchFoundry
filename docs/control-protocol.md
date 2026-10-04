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

The Color Picker is a dialog too: `engine.execute {command: "ui.colorPicker", params: {stroke?, color?}}` opens it for the fill (or stroke) proxy; `ui.dialog.set {field: "hex", value: "00FF00"}` (or `color`, `channel`, `webOnly`, `swatches`) then `ui.dialog.confirm` applies the colour through `paint.setFill` / `paint.setStroke`.
