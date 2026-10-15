# StitchFoundry app icon

**Subject:** machine embroidery: a rectangular machine hoop with the bracket that clips onto the
embroidery arm, the machine's needle and embroidery foot, and a satin-stitched S being sewn.

**Palette:**

| Colour | Hex | Used for |
|---|---|---|
| Navy | `#1e2b4f` | the full-bleed field |
| Coral | `#f2546b` (lighter `#ff8c9b`, darker `#d63d55`, shadow `#b8344b`) | the satin S and the thread |
| Linen | `#f6f1e7` | the fabric in the hoop |
| Greys | `#e4e8ef`, `#c7cedb`, `#b3bccb`, `#8a94a8` | hoop, machine head, needle bar, foot |

**Tile:** `viewBox="0 0 512 512"`, a rounded square with `rx=112`, clipped. The macOS files add Apple's
transparent margin (tile = 824/1024 of the canvas); Windows and Linux use the tile edge to edge.

**Provenance:** original artwork made for StitchFoundry, written by hand as SVG (shapes and a stripe
pattern, no third-party material). `stitchfoundry.svg` is the canonical artwork; `stitchfoundry-small.svg`
is the same file for `hicolor/scalable`. Licence: see `LICENSE.txt`.

## Files

| File | What | Used by |
|---|---|---|
| `stitchfoundry.svg` | master vector | source for everything below |
| `stitchfoundry-small.svg` | the same artwork | `hicolor/scalable` |
| `stitchfoundry-1024.png` | full tile, 1024 px | docs, store listings |
| `stitchfoundry-macos-512.png` | tile with Apple margin, 512 px | runtime Dock icon (`apps/vectorcraft/src/main.rs`) |
| `stitchfoundry.icns` | macOS icon set, 16 to 1024 px | `cargo xtask bundle` (`CFBundleIconFile`) |
| `stitchfoundry.ico` | Windows icon, 16 to 256 px | `apps/vectorcraft/build.rs` (embedded in the `.exe`) |
| `hicolor/<size>/apps/com.stitchwright.stitchfoundry.png` | Linux theme icons, 16 to 512 px; the 256 one is also the runtime icon on Windows and Linux, the 128 one the brand mark in the app bar and About box (`crates/ui-egui/src/brand.rs`) | `packaging/linux/com.stitchwright.stitchfoundry.desktop` |
| `hicolor/scalable/apps/com.stitchwright.stitchfoundry.svg` | Linux scalable icon | as above |

## How it reaches each OS

- **macOS:** the app sets the Dock / app-switcher icon at runtime (`ViewportBuilder::with_icon`); the
  `.app` from `cargo xtask bundle` carries `stitchfoundry.icns`.
- **Windows:** `build.rs` embeds `stitchfoundry.ico` and VERSIONINFO with `winresource` (taskbar, Start
  menu, Explorer, Alt-Tab); the runtime icon covers the title bar.
- **Linux:** install `packaging/linux/com.stitchwright.stitchfoundry.desktop` to `share/applications/`
  and `hicolor/` to `share/icons/`. The app sets its Wayland app ID / X11 class to
  `com.stitchwright.stitchfoundry`, so the dock matches the window to the launcher.

## Regenerate

Edit `stitchfoundry.svg` (and copy it to `stitchfoundry-small.svg`), then run `packaging/icons.sh` (needs
`resvg`). It writes every PNG, the `.ico` (`cargo xtask ico`) and the `.icns` (`cargo xtask icns`).
