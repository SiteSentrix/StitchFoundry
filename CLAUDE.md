# VectorCraft — instructions for agents

VectorCraft is a clean-room, open-source, Rust-native vector illustration app targeting Adobe Illustrator parity (and beyond). It runs natively on macOS, Windows and Linux, and on the web via WASM. Siblings: `../photocraft` (Photoshop-class) and `../printcraft` (Acrobat-class); same conventions.

Formerly **DrawCraft** (renamed 2026-10-01): old `.drawcraft` files and `"format": "drawcraft"` headers still open (`vectorcraft_format::LEGACY_EXTENSION`), and preferences migrate from the old config folder. Keep those paths working; use the new name everywhere else.

## Start every session here
1. Read `plan/STATUS.md` (current milestone, next task), then the task in `plan/execution-plan.md` §3 and the relevant `plan/architecture.md` section. Behaviour reference: `plan/illustrator/*.md`.
2. Follow the autonomous operation protocol (`plan/execution-plan.md` §7): orient → plan → implement + test → verify → record → commit. Don't stop to ask unless §7 lists the decision as the user's.

`plan/` is gitignored (local only).

## Non-negotiables
- **Clean-room.** Never read, disassemble or copy anything inside the Illustrator bundle (names/listings only). Never copy Adobe icons, artwork, presets or wording beyond feature names. Behaviour comes from public docs and black-box observation of the running app with synthetic documents only (screenshots by window id, stored under `plan/illustrator/screenshots/`, never committed). Never copy GPL/AGPL code (Inkscape, lib2geom…).
- **Assets: no Adobe iconography or images — ever (absolute rule, from the project owner).**
  - Never add, copy, trace, redraw-from, embed or ship any icon, image, artwork, cursor, preset, swatch/brush/symbol/pattern/style library, ICC profile or screenshot from Adobe products or from any other source whose licence doesn't allow it.
  - Every image, icon, font or other asset must be one of:
    - original work created for VectorCraft by a contributor (who licenses it MIT OR Apache-2.0);
    - OSI open source;
    - public domain / CC0;
    - Creative Commons with redistribution allowed.
  - **Every asset file must have a row in [`ASSETS.md`](ASSETS.md)** (path, author, source URL, licence, notes). Put licence texts next to the assets (e.g. `assets/fonts/OFL-*.txt`) and summarize them in `NOTICE`.
  - `cargo xtask assets` (run by `cargo xtask ci`) fails on any unattributed asset.
  - Code and original assets are MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). The ArtCraft logos in `docs/brand/` are trademarks, not open source, under `docs/brand/LICENSE-brand.txt`: never modify them or use them outside VectorCraft.
  - Prefer art generated in code for defaults (swatches, brushes, symbols, patterns, cursors). If the provenance of an asset is unclear, don't add it.
  - Reference screenshots of Illustrator stay local under the gitignored `plan/` and are never committed, published or used as assets.
- **Everything is a command.** User-visible behaviour = a command in `crates/engine/src/cmd/*` (id, label, menu path, shortcut, params doc, `enabled`, `run`) + tests. Tools emit commands (Begin/Preview/Commit). UI-only commands live in `crates/ui-egui/src/menus.rs` (`UI_COMMANDS`). The control channel and MCP reach all of them.
- **Layering** is enforced by `cargo xtask layers`. Nothing below L6 depends on egui/eframe/winit/rfd.
- **The UI is thin**: panels read engine state and act through `app.run(id, params)`. Colours come from `theme::Tokens`.
- **Rust only** (no handwritten JS/TS). **Never break wasm** (`cargo xtask wasm`).
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy -D warnings, tests, layers, wasm). One task id per commit (`M2.1: pen tool`).

## Running and looking at the app
- `cargo run --release -p vectorcraft -- --control 7979 [file.svg|file.vectorcraft]` (sibling apps' agents use the same default port: if the log says it failed to bind, pick another port — otherwise your requests reach a different app).
- Drive it: JSON lines on `127.0.0.1:7979`, e.g. `{"id":1,"method":"engine.execute","params":{"command":"shape.rectangle","params":{"x":10,"y":10,"width":100,"height":50}}}` then `{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}`. Methods: `crates/ui-egui/src/control.rs`.
- **For UI work, look at the result** (take `ui.screenshot`, read the PNG) and compare with `plan/illustrator/02-ui-ux.md` / `10-observed-ui.md`. `ui.screenshot` needs a presented frame: if it errors (screen locked), check the art with `ui.render` / `vectorcraft-cli run … --export x.png`, and cover panels with a headless egui frame test (see `panels/transparency.rs` tests).
- **Performance:** `vectorcraft-cli perf` checks the budgets (render/pan/zoom, hit test, save/open, SVG, Pathfinder) on a synthetic 50k-path document; `vectorcraft-cli bench FILE` times one file. Run them before and after renderer, format or geometry changes, on an idle machine (the report warns when the load average makes timings noise).
- MCP: `vectorcraft-cli mcp` (see `docs/mcp.md`).
- Shell gotcha: `mv`/`cp` are aliased interactive here — use `/bin/mv -f` / `/bin/cp -f`.
- Parallel agents: separate `CARGO_TARGET_DIR` per agent; edit only the crates you own; write manifests atomically. Each target dir grows to ~30 GB: delete yours when you finish (a full disk fails links with `errno=28`).

## Roadmap
`ROADMAP.md` (committed) tracks status, milestones and time-to-parity estimates. Update it whenever a milestone task lands.
