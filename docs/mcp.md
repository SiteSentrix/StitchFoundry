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
| `export` | `{path, format?, scale?, selection?}` | `svg`, `pdf`, `png`, `jpg`, `webp` or `vectorcraft`. When `format` is omitted, it comes from the path's extension. `selection: true` exports the selected objects cropped to their bounds; `outlineText: true` writes SVG text as paths. Live effects are kept. |
| `add_text` | `{text, x?, y?, width?, height?, path?, mode?, pathEffect?, size?, font?, color?}` | Point type at (x, y); area type with `width`/`height`; or `path` + `mode` (`area`/`onPath`) to flow text in or along a path, with `pathEffect` (`rainbow`, `skew`, `3dRibbon`, `stairStep`, `gravity`). |
| `apply_effect` | `{effect?, params?, ids?}` | Appends a live effect. Without `effect`, returns the effect catalogue with parameters and defaults. |
| `pathfinder` | `{operation, ids?}` | `unite`, `minusFront`, `intersect`, `exclude`, `divide`, `trim`, `merge`, `crop`, `outline`, `minusBack`. For a live version, apply the `pathfinder.*` effect to a group. |
| `transform` | `{ids?, dx?, dy?, rotate?, scale?, scaleX?, scaleY?, reflect?, shear?, origin?, copy?}` | Runs move, rotate, scale, reflect, shear in that order. With `copy`, the first step duplicates. |
| `create_graph` | `{type?, x, y, width, height, csv? \| series?, categories?, rows?}` | The nine Illustrator graph types. Edit later with `graph.setData` / `graph.setType` via `run_command`. |
| `text_wrap` | `{ids?, offset?, invert?, release?}` | Area type below the objects (same layer) flows around them. |
| `undo` / `redo` | `{}` | |

Errors (unknown tool, bad arguments, a disabled or failing command) come back as a normal result with
`isError: true` and a message. The model can read the message and retry.

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
```

Drive a tool like a mouse:

```json
{"name":"pointer_gesture","arguments":{"tool":"ellipse","events":[
  {"kind":"down","x":100,"y":100},{"kind":"drag","x":150,"y":140},{"kind":"up","x":200,"y":180}]}}
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
applies to the `--cmd` just before it. `run` also accepts the host commands `file.open`, `file.save`, `file.export`
and `tool.select`. `run --in`, `convert` and `info` read every format `document.open` reads
(`vectorcraft-cli --help` lists them).
