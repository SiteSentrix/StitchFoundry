# macOS DMG window

What Finder shows when the DMG opens: a 660 × 400 pt background with the app icon and the
`Applications` link side by side. `package.sh` copies these files into the image; nothing here is
generated at build time, so the DMG still builds with `hdiutil makehybrid` (no mounted device, no
Finder scripting on CI).

| File | What |
|---|---|
| `background.svg` | Source of the background: the app icon's figure on the VectorCraft colour field (`#e8573f`), Ink and Paper, Inter and JetBrains Mono. |
| `background.tiff` | The background at 1x (660 × 400 px, 72 dpi) and 2x (1320 × 800 px, 144 dpi) in one HiDPI TIFF. Goes to `.background/background.tiff`. |
| `DS_Store` | Finder's view settings for the volume: window size, icon size 128, VectorCraft.app at (326, 205), `Applications` at (574, 205), and the background. Goes to `.DS_Store`. |

## Rules

- **The volume name has no version** (`VectorCraft`, not `VectorCraft <version>`). `.DS_Store` points at the
  background through an alias that includes the volume name, so a versioned name loses the
  background. The DMG file name still carries the version.
- **Finder draws the icon labels in black in light and dark mode** when a window has a background,
  so the area under both icons stays light (Paper).
- **Nothing goes inside the icon boxes:** artwork keeps 10 pt clear of each 128 pt icon box and of
  the label strip under it.

## Regenerate

1. Edit `background.svg` and render it with [resvg](https://github.com/linebender/resvg), with
   Inter and JetBrains Mono available (`--use-fonts-dir` pointing at a craft-fonts checkout):

   ```sh
   resvg --use-fonts-dir ../craft-fonts -w 660  background.svg bg.png
   resvg --use-fonts-dir ../craft-fonts -w 1320 background.svg bg@2x.png
   sips -s dpiWidth 144 -s dpiHeight 144 bg@2x.png
   tiffutil -cathidpicheck bg.png bg@2x.png -out background.tiff
   ```

2. `DS_Store` only changes if the window size, icon positions or background file name change.
   Recreate it once on a Mac with [create-dmg](https://github.com/create-dmg/create-dmg) and copy it
   out of the mounted volume:

   ```sh
   create-dmg --volname "VectorCraft" --background background.tiff --window-size 660 432 \
     --icon-size 128 --icon "VectorCraft.app" 326 205 --hide-extension "VectorCraft.app" \
     --app-drop-link 574 205 seed.dmg <folder with VectorCraft.app>
   hdiutil attach seed.dmg -mountpoint /tmp/seed && cp "/tmp/seed/.DS_Store" DS_Store
   ```

   (660 × 432 includes Finder's 32 pt title bar; the content area is 660 × 400.)
