# StitchFoundry: embroidery digitizing

StitchFoundry is this fork's embroidery mode: machine-embroidery digitizing built into the
vector editor. Any path, compound path or type object can carry a **stitch type**. The stitches
are generated from the artwork, shown on the canvas as shaded thread, and written out as machine
files (DST, PES, JEF, …). Edit the shape and the stitches follow; give it a different stitch type
and the art stays the same.

Everything is a command (`embroidery.*`), so the same features work from the menus and panels,
the control channel, the CLI and the MCP server.

- [Quick start](#quick-start)
- [Stitch types](#stitch-types)
- [Stitch direction: automatic, then editable](#stitch-direction-automatic-then-editable)
- [The digitizing widget and brush](#the-digitizing-widget-and-brush)
- [Lettering](#lettering)
- [Auto-Digitize](#auto-digitize)
- [Design from Prompt](#design-from-prompt)
- [Design settings: hoop, fabric, threads](#design-settings-hoop-fabric-threads)
- [Stitch Sequence and the stitch player](#stitch-sequence-and-the-stitch-player)
- [Machine files](#machine-files)
- [Commands](#commands)
- [How it is built](#how-it-is-built)
- [Clean-room notes](#clean-room-notes)
- [Known limits](#known-limits)

## Quick start

1. Draw shapes, place a logo, or type text.
2. Select them and choose **Embroidery › Convert Artwork to Embroidery**. Filled areas become
   satin (if no part is thicker than 7 mm) or tatami fill, and strokes become satin lines, bean
   runs or runs, depending on their width. To pick a type yourself, use **Embroidery › Stitch Type**
   or the **Embroidery** panel.
3. Check the result on the canvas: stitches are drawn as shaded thread over a fabric-tinted
   artboard. Choose a hoop in the Embroidery panel to see the sewing field.
4. Fix the stitch direction where needed with the **Inclination tool** (below).
5. Use **File › Export › Machine Embroidery File** (or **Embroidery › Export Machine File**) and
   pick a format.

## Stitch types

| Type | What it sews | Main settings |
|---|---|---|
| Run | single run along the path | stitch length, passes |
| Bean run | triple run (each stitch three times) | stitch length, passes |
| Backstitch | hand-sewn look: each stitch goes forward two steps and back one | stitch length, passes |
| Stem stitch | rope-like outline: backstitch with slanted stitches | stitch length, width, side, passes |
| Blanket, zigzag, motif | E-stitch prongs, an open zigzag or a repeating motif along the path | stitch length, width, spacing, flip side |
| Satin | a column; on a closed shape, satin that turns along it | density, pull compensation (one value, or one per side), auto spacing, short stitches, auto split, preserve long stitches, tatami stitches (a tatami border), underlay (incl. double zigzag) |
| Tatami fill | rows of staggered stitches at the best angle, or turning along inclination lines | density, stitch length, row offset, angle, randomised needle points, gradient density, underlay |
| Satin fill | satin rows across the shape | density, pull compensation |
| Contour / Spiral / Radial / Cross / Meander fill | rows following the outline, a spiral, rays from the centre, crossed rows, or a random maze-like path | spacing, stitch length |
| Appliqué | placement run, stop, tack-down, stop, cover satin | cover width and density |
| Manual | a stitch at every anchor point | none |

Satin lines along an open path (also called steil) take a width and a **width profile**:
constant, tapered, calligraphy (a nib angle) or pressure (from a pen). A closed path painted as a
stroke with no fill is sewn the same way, round and round: a **satin border** round the shape (a
filled closed shape is satin across its area instead).

**Column A/B:** the **Column A/B tool** (Embroidery toolbar) digitizes a satin column of any
shape the way digitizers do: click a point on one side of the column, then across on the other
side, then on the first side again, and so on; Alt-click makes a smooth point. Each pair of points
sets the stitch angle there, so the stitches turn with the column (the pairs become inclination
lines you can edit later). Every click is its own step (Undo takes it back), Backspace removes the
last point, Enter or Escape ends the column. To **continue digitizing** a column later, select it
and pick the tool again: clicks add to it. An open path (a run or a satin line) continues from
its end with the Pen tool, keeping its stitch type.

**Tatami borders:** a satin column, line or border with *Tatami stitches* on is sewn in tatami rows
instead: every stitch across is split into pieces of the *Stitch length* (4 mm to start with),
shifted a quarter from row to row. Use it for borders and columns too wide for satin.

**Smart corners:** where a satin line turns sharply, rows that simply turned with it would pile up
inside the corner. *Corners* sets how the two sides meet: **Automatic** (the default) lets rows
turn round gentle corners (up to 45°), keeps each side's rows square and meets them on the miter
line at medium ones (up to 110°), and caps the tip of sharper ones; **Turn**, **Miter** and
**Capped** use one method everywhere. Tight curves never fold the inner side of the column back
over itself.

**Passes:** any run style can be sewn up to 9 times over the same needle points; 2 passes is a
backtrack, which ends where it started.

**Satin details:** *pull compensation* can differ per side (side 1 is the first rail), for a
column lying on fabric on one side and on other stitching on the other. *Preserve long stitches*
keeps a stitch longer than the machine's longest stitch whole: the frame moves in steps without a
needle drop. Satin on closed shapes (including lettering) uses the same settings as a satin column.

**Ties:** every object follows the design's tie-in and tie-off setting unless its own *Tie in* or
*Tie off* says *On* or *Off*. The design's *Tie style* (Machine settings) is **Single** (one stitch
out and back), **Double** (twice, the default) or **Triangle** (out, across and back, for fabric
that frays or stretches), *Tie length* long. **Omit last stitch** leaves out an object's last
needle point, where it would sew back over the object's edge or into the next one.

**Remove underlying stitching:** an object with this option on cuts what it covers out of every
fill sewn before it (lettering on a background fill, a patch on a jacket back), so thread doesn't
stack up where nobody sees it. The fills below still run *Overlap* mm (1 mm by default) in under
its edge, so no gap opens when the fabric pulls. A fill cut in two sews as two pieces; one covered
completely sews nothing. Moving or reshaping the object updates the cut.

Underlay is automatic by default (centre run, edge run, zigzag or tatami, chosen by width) and can
be set by hand. Choosing a **Fabric** in the Embroidery panel sets density, pull compensation and
underlay for every stitched object to suit that fabric.

## Stitch direction: automatic, then editable

Every stitched object chooses its stitch direction automatically, and the result is stored as
ordinary editable data:

- **Fills** get the straight angle that sews with the fewest breaks. Long, thin shapes and satin
  shapes get **inclination lines** instead: lines drawn across the shape that the stitches follow,
  so the direction turns smoothly from one line to the next (turning satin, turning tatami).
- To place inclination lines automatically, the two ends of the column are found and its sides
  are paired by dynamic time warping. Shapes whose pairing would run along rather than across
  them fall back to a straight angle.
- Satin needs simple columns, so shapes are cut open first: each hole is slit where the shape is
  thinnest (an "O" becomes two columns), and sharp inside corners where strokes meet are cut too
  (a "T" becomes three columns). The slits are 0.02 mm wide, too narrow to show in the stitches.

Editing:

- **Inclination tool** (Embroidery section of the toolbar): drag across a shape to add a
  line, drag a line's ends to move them, click a line and press Delete to remove it.
- The **green bead** (where sewing starts) and the **red bead** (where it stops) of the selected
  object can be dragged too. **Reset Start and Stop** in the panel restores the automatic closest
  joins.
- **Embroidery › Auto Direction** places the automatic lines again; **Embroidery › Clear
  Inclination Lines** goes back to the automatic direction.
- Inclination lines, beads and fill angles move, rotate, scale and mirror with their object.

**Reverse Stitch Direction** (Embroidery menu) swaps where the selected objects start and stop
sewing.

**Holes:** **Embroidery › Remove Holes** deletes the holes of the selected shapes (all of them, or
with the `maxArea` parameter only small ones, handy after tracing). **Fill Holes** adds an object
in each hole, sewn right after its shape, so a hole can be sewn in another thread. To add a hole,
draw a shape over a stitched object, select both and choose **Minus Front** (Window ›
Pathfinder). The hole is cut out and the object keeps its stitch settings. Pathfinder operations
carry stitch settings the same way they carry appearance.

## The digitizing widget and brush

The **Digitizing Widget** (Embroidery menu, Embroidery panel, or picking the **Embroidery Brush**
tool) is a dial in the canvas's lower-left corner:

- the **inner ring** picks what to set: Brush, Width, Density, Length or Shape;
- the **outer ring** shows that setting's values; click one to use it;
- the **centre** shows the thread colour (click to cycle; the panel has a full colour picker).

Brushes: Run, Bean, Steil, Tapered, Calligraphy, Pressure, Appliqué, Fill and Satin. Shapes:
Free (freehand), Line, Circle, Square, Rounded, Triangle, Diamond, Hexagon, Star, Heart and Arrow
(drag to size; Shift keeps proportions). With the widget open, the Embroidery panel also offers:

- **New strokes: Sewn last / Before the selection / After the selection**, which digitizes into
  the middle of the sewing order;
- **Widget values**, the widths, densities and lengths the outer ring offers (comma-separated, up to
  12 each).

Stroke ends snap to anchors and guides while Snap to Point or Smart Guides is on.

## Lettering

Type objects are stitched straight from their glyph outlines, so there are no embroidery fonts
to install, and editing the text, font or size restitches it. Small lettering becomes turning
satin columns, with counters and stroke joins cut so each piece turns cleanly. Lettering too thick
for satin becomes tatami.

## Auto-Digitize

Select a placed image and choose **Embroidery › Auto-Digitize Image** (or use the Embroidery panel
when an image is selected). The image is traced into flat colour shapes with the given number of
thread colours, the background and specks under 1 mm² are dropped, and each shape gets a stitch
type: satin for narrow parts, tatami at its best angle for the rest. The design is sewn colour by
colour, nearest shape first. It is scaled to fit the hoop if one is set (or to 100 mm if it is
larger than 200 mm). The image stays underneath as a reference. One undo step undoes it all.

## Design from Prompt

Describe a design in words and Claude, Anthropic's AI model, draws artwork for it, which
StitchFoundry then digitizes. Choose **Embroidery › Design from Prompt…** (or the button in the
Embroidery panel), describe the design ("a cartoon fox head, orange and white"), and set its size
(the longest side, in mm: 90% of the hoop's shorter side by default, else 100 mm) and the most
thread colours it may use. **Generate** sends the request, and the dialog shows how long Claude
has been drawing; **Cancel** stops waiting. If something goes wrong (a refused key, a busy
service, a drawing that grew too detailed), the dialog says so and stays open to try again.

Claude is asked for what digitizes well: filled shapes in flat colours, no part narrower than
1.5 mm stitched, no shading or tiny details, drawn back to front. The drawing arrives as one group
named after the description. Its shapes get stitch types as with Convert Artwork to Embroidery
(satin for narrow parts, tatami for the rest), sewn in the order they were drawn, and each one cuts
what it covers out of the fills sewn before it (Remove Underlying Stitching), so stacked shapes
don't pile up thread. Its colours become the design chart's closest threads. Images, invisible
shapes and specks under 1 mm² are left out. One undo step undoes it all. As with any automatic
digitizing, check the stitches (Check Design, the stitch player) before you sew. Claude won't copy
someone else's logo, trademark or character: the dialog says so, and an original design described
in your own words draws fine.

**Your own API key.** Design from Prompt uses your Anthropic API key, and each design is billed to
your Anthropic account; get a key in the [Anthropic Console](https://console.anthropic.com/settings/keys).
Paste it into the dialog. With **Remember the key on this computer** (turn it off to forget a kept
key), it is kept in a file of its own, `anthropic-api-key`, next to StitchFoundry's settings and
readable by you alone: not in the settings themselves, which the control channel and MCP agents
can read. Without a kept key, the `ANTHROPIC_API_KEY` environment variable is used.

Only your description, the size and the colour count are sent, to `api.anthropic.com` over HTTPS,
and only when you click Generate; nothing from your design leaves the computer. Certificates are
checked against the system's trusted ones, and a proxy set in the usual environment variables
(`HTTPS_PROXY`…) is used. Design from Prompt needs the desktop app: the web app can't send the
request yet. Agents don't need it: an agent that draws can hand its own SVG to
`embroidery.placeArt`, which does the rest.

## Design settings: hoop, fabric, threads

The Embroidery panel's **Design** section shows the stitch count, size, colours, trims, sewing
time and thread used, and whether the design fits the hoop. Its settings:

- **Hoop**: the sewing field, drawn on the canvas centred on the design (machine files are
  centred too); dashed red when the design doesn't fit.
- **Fabric**: presets that set density, pull compensation and underlay. **Fabric settings**
  below it shows the chosen fabric's values; **Save as New Fabric** makes a copy you can change
  (the design keeps its own fabrics, and changes apply to the design at once), and **Delete
  Fabric** removes one.
- **Machine**: the machine the design is for. The standard profiles are named by the file format
  their machines read (DST, EXP and U01 for commercial machines; PES, JEF, VP3 and XXX for home
  machines); profiles of your own (say, *Diamond deLuxe*, writing VP3) follow them. A machine sets
  the longest stitch and frame move and the speed and trim time used for time estimates, and
  **Export** writes its format unless you pick another. **Machine settings** below it change those
  values for this design, along with the shortest stitch, how long a jump must be to be trimmed,
  the colour change time and the design's tie-ins and tie-offs. **Save to Machine** keeps them in
  the machine's profile, so the next design that picks it starts with them (all of them, ties
  too); **Reset Machine** puts a changed standard profile back to how it ships. **Save as New
  Machine** makes a profile of your own from them, in the design's format; for your own profiles
  the section also has their **Name**, their file **Format** and **Delete Machine**. Profiles are
  kept with your preferences, not in the design: a design keeps the name and format of its
  machine, so it still exports in that format on a computer without the profile.
- **Thread chart**: the chart the panel's thread list picks from. Threads keep their name and
  code; PES and JEF files use the nearest colour of the machine's own palette. **Match Threads to
  Chart** (also in the Embroidery menu, for the selection) gives every stitched object the chart's
  closest thread and recolours its art: use it on a design opened from a machine file, whose
  colours name no threads.
- **Show stitches** (Hidden / Lines / TrueView), **Artwork** (Normal / Dimmed / Hidden),
  connectors, needle points, the selection's **start and stop** beads and its **inclination
  lines** (shown with any tool, not only the Inclination tool).

Thread colour follows the object's paint colour unless set in the panel. **Select › Same ›
Thread Color** and **Stitch Type** select every stitched object like the selected one.

**Check Design** (Embroidery menu, or the button in the panel's Design section) looks for spots
where thread piles up (very dense), stitches longer than 8 mm, tiny objects, lettering under about
5 mm, objects that make no stitches and a design larger than its hoop. Click a finding to select
its objects and bring the spot into view. **Check settings** under the button sets what counts as
too dense, a long stitch, small lettering and a tiny object for this design.

Density is measured as layers of thread over 2 × 2 mm squares: a tatami fill is about 1.2 layers
with its underlay, a satin column 2.5 to 3.5. Spots over 4.5 layers are flagged: four fills stacked,
satin columns crossing, or a column piling thread up inside a corner. Lettering, borders over fills
and two overlapping fills stay under it.

## Properties for new objects and styles

With nothing selected, the Embroidery panel shows **Properties for New Objects**: pick a stitch
type and set its density, underlay, pull compensation and the rest. Objects you then give that
stitch type (with the Stitch Type list or Convert Artwork to Embroidery) take them. With
an object selected, **Make Current** (also Embroidery › Make Properties Current) makes its
properties the ones new objects of its type take. **Reset to Defaults** goes back to the built-in
values. These are saved with the design, so a template carries them to new designs.

**Embroidery styles** are named sets of stitch properties. In the **Embroidery Styles** panel
(Window › Embroidery Styles), the new-style button saves the selected object's properties as a
style, and clicking a style applies it to the selection. Objects keep their thread, shape,
inclination lines and start and stop points, and follow the style: **Redefine Embroidery Style**
(panel menu) gives the style the selected object's properties and updates every object that
follows it. A "+" marks an object changed since it took its style. The Embroidery panel's
**Style** list shows and changes the selection's style; **Break Link to Embroidery Style**
leaves objects as they are but no longer following it.

## Editing single stitches

The **Stitch Edit tool** (Embroidery toolbar, or **Edit Stitches** in the panel) shows the needle
points of the selected stitched object. Click a point to pick it (Shift adds or removes), drag
across empty canvas to pick the points inside, drag picked points to move them, Alt-click a stitch
to add a needle point there, and press Delete to remove the picked ones.

The first edit **freezes** the object: its stitches are kept as they are, and changing its shape no
longer regenerates them (moving, scaling or rotating the object still carries them along). The panel
then says the stitches were edited by hand; **Regenerate Stitches** (panel or Embroidery menu) drops
the edits. **Freeze Stitches** freezes without editing.

**Split Here** (panel, with one needle point picked) splits an object's stitches in two at that
point: the object keeps the stitches before it, and a copy sewn right after it takes the rest, both
frozen. Use it to sew part of an object in another colour, or to put something between its parts.

**Machine functions:** a trim or a stop can be put after any stitch, and any stitch made a jump
(the frame moves there without sewing), with `embroidery.function.insert`; `embroidery.function.clear`
takes them out again. **Stop after** in the panel stops the machine after an object without
freezing it (to place appliqué fabric, or change thread on a single-needle machine).

The **Stitch List** panel (Window or Embroidery menu) lists every record of the design in sewing
order: number, function (stitch, jump, trim, colour change, stop), position, length and the object
that sews it. Click a row to play the design up to it and select its object; while the stitch player
runs, the list follows it.

## Stitch Sequence and the stitch player

The **Stitch Sequence** panel lists the stitched objects in sewing order, grouped by thread, with
stitch counts. Drag a row onto another to sew it just before or after that one, or use the context
menu (sew earlier, later, first, last); Shift- or Ctrl-click selects several. Each row's eye hides
or shows its object and its lock locks it; hidden objects are listed at the end, not sewn, so they
can be shown again. **Sort by Thread Color** regroups the design to sew each thread colour in one
go. The **player** redraws the design stitch by stitch, as the machine will sew it; its **travel**
buttons step back and forth by stitch, object, colour, trim or machine function, selecting the
object reached.

**Embroidery › Auto Sequence** reorders the stitched objects of each layer or group for the
fewest colour changes and then the shortest way from one object to the next, keeping objects
that overlap in their stacking order (the one on top is still sewn last).

**Embroidery › Branch** sews two or more selected objects as one path, without trims: they
become a *branch* group. The objects are sewn closest-first (overlapping ones in stacking
order), each ends where the objects still to sew cover it, and the thread runs to the next one
as small running stitches routed inside those later objects, so it is sewn over and hidden.
Where nothing covers the way, the thread is trimmed as usual. **Break Apart** turns a branch
back into separate objects; on a stitched path or compound path made of separate parts it
makes one object per part, each with the same stitching (a hole stays with its part).

## Machine files

| Format | Extension | Colours | Open | Export |
|---|---|---|---|---|
| Tajima | .dst | no (colour changes only) | yes | yes |
| Brother | .pes, .pec | PEC palette (nearest) | yes | yes |
| Janome | .jef | JEF palette (nearest) | yes | yes |
| Melco | .exp | no | yes | yes |
| Pfaff | .vp3 | RGB with names | yes | yes |
| Singer | .xxx | RGB | yes | yes |
| Barudan | .u01 | no (needle numbers) | yes | yes |

- Opening or placing a machine file turns each thread into a path of **Manual** stitches (one
  subpath per run between trims), which you can edit, move, scale and recolour.
- Long stitches and jumps are split to the format's limit. Trims are written natively where the
  format has them; DST uses the common three-jump convention.
- VP3 writes each move as one long record that ends where the next stitch starts. Programs
  differ on whether that record is a jump or a stitch; read either way, the needle goes down only
  where it would anyway.
- The format code (`crates/sf-formats`) round-trips every format and survives random and
  truncated input. Files it writes were read back by an independent open-source embroidery
  library with the same stitches, trims, colour changes, extents and colours (that library counts
  each VP3 move as a stitch, reads JEF without trims and U01 colour changes as needle numbers),
  and files that library wrote open here with the right size, colours and structure. **They have not been sewn on real machines yet**, so
  test-sew before production runs.

## Commands

Every feature is an engine command. A few of the main ones (see `document.commands` or the MCP
tool list for all parameters):

| Command | Does |
|---|---|
| `embroidery.set {type}` | give the selection a stitch type (`run`, `bean`, `blanket`, `satin`, `fill`, `satinfill`, `contour`, `spiral`, `radial`, `cross`, `meander`, `applique`, `manual`, `none`) |
| `embroidery.setParams {params}` | change stitch parameters (camelCase names, mm) |
| `embroidery.setThread {color, name?, code?, chart?}` | set the thread |
| `embroidery.convert` | stitch types from the artwork's look |
| `embroidery.autoDirection` / `addGuide` / `moveGuide` / `removeGuide` / `clearGuides` / `setBeads` | stitch direction and start/stop |
| `embroidery.draw {brush, points \| stamp, …}` | a brush stroke, as the widget draws it |
| `embroidery.column {points: [[x, y, curve?]…], id?}` | a Column A/B satin column from points on its two sides in turn; with `id`, continue that column |
| `embroidery.autoDigitize {colors, …}` | trace a selected image into stitches |
| `embroidery.placeArt {svg, size?, at?, name?, minArea?}` | place flat-colour SVG art and digitize it, as Design from Prompt does with Claude's drawing (its linked files are never read) |
| `embroidery.settings {hoop, fabric, chart, machine, …}`, `embroidery.applyFabric {name}` | design settings |
| `embroidery.machine.list`, `embroidery.machine.save {name, newName?, format?, fromDesign?, …}`, `embroidery.machine.delete {name}` | machine profiles: change the standard ones, make, rename and delete your own |
| `embroidery.sortByColor` | sew each colour in one go |
| `embroidery.autoSequence` | reorder for the fewest colour changes and the shortest travel, overlaps kept |
| `embroidery.matchThreads {chart?, all?}` | the chart's closest thread for each stitched object |
| `embroidery.makeCurrent`, `embroidery.current`, `embroidery.current.setParams {type, params}`, `embroidery.current.reset {type?}` | the properties new objects take |
| `embroidery.style.new {name?}` / `apply {name}` / `redefine {name}` / `unlink` / `rename {name, to}` / `duplicate` / `delete` / `list` | embroidery styles |
| `embroidery.fabric.save {name, from?, …}`, `embroidery.fabric.delete {name}` | fabrics made for the design |
| `embroidery.branch {ids?}`, `embroidery.breakApart {ids?}` | sew objects as one path; separate a branch or a many-part object |
| `embroidery.reverse`, `embroidery.removeHoles {maxArea?}`, `embroidery.fillHoles` | sewing direction and holes |
| `select.same.thread`, `select.same.stitchType` | select like objects |
| `embroidery.check` | sewing problems (dense spots, long stitches, tiny objects, small lettering, hoop) |
| `embroidery.stitches {id}`, `embroidery.stitches.edit {id, ops}` | an object's own stitches; move, delete and insert them |
| `embroidery.freeze`, `embroidery.regenerate` | keep stitches as they are for editing; drop the edits |
| `embroidery.stitches.split {id, at}` | split an object's stitches in two |
| `embroidery.function.insert {id, at, kind}`, `embroidery.function.clear {id, at}` | trims, stops and jumps at single stitches |
| `embroidery.info`, `embroidery.plan` | statistics; the stitches themselves |
| `embroidery.export {format}` | a machine file (`dataBase64`) |

From the control channel (`stitchfoundry --control 7979`, one JSON object per line):

```json
{"id": 1, "method": "engine.execute", "params": {"command": "shape.ellipse", "params": {"x": 50, "y": 50, "width": 200, "height": 200}}}
{"id": 2, "method": "engine.execute", "params": {"command": "embroidery.set", "params": {"type": "fill"}}}
{"id": 3, "method": "engine.execute", "params": {"command": "embroidery.export", "params": {"format": "dst"}}}
```

## How it is built

| Crate | Layer | Role |
|---|---|---|
| `sf-core` | standalone | geometry, the embroidery model, the stitch engine (runs, satin, fills, underlay, appliqué, connectors, ties, trims), inclination lines and turning, brushes, threads, hoops, fabrics; Design from Prompt's request and the reading of its reply (`ai`) |
| `sf-formats` | 1 | machine file readers and writers |
| `vectorcraft-embroidery` | 2 | the document side: converts paths and type to embroidery objects (millimetres), plans the whole document in paint order, caches unchanged objects |
| `vectorcraft-engine` | 5 | the `embroidery.*` commands and machine-file open/place |
| `vectorcraft-tools` | 4 | the Embroidery Brush and Inclination tools |
| `vectorcraft-ui-egui` | 6 | canvas overlay (TrueView mesh, hoop, beads, player), panels, widget, menus, the Design from Prompt dialog |
| `vectorcraft` (the desktop app) | 7 | sends Design from Prompt's request (ureq, rustls) and keeps the API key |

Documents store embroidery as an optional attribute on each node (`Node.embroidery`) and on
the document (`Document.embroidery`: hoop, fabric, chart). Older documents and documents without
embroidery load and save as before. Stitch order is paint order (bottom first). Positions in the
document are points; stitch settings are millimetres.

## Clean-room notes

The embroidery features are written from scratch from public descriptions of how digitizing
works and from the behaviour users expect of digitizing software. No code, assets or file
samples were taken from commercial digitizers or from open-source embroidery projects (some of
which are GPL). The machine file code was written from general format knowledge and is tested by
round trips and robustness tests in the repository.

## Known limits

- Machine files have not been sewn on real machines yet (see above).
- Machine translations of the new interface text into the 11 interface languages want a review by
  native speakers who digitize. In Russian and Ukrainian, the summary counts ("4 colors") use one
  plural form for every number.
- Auto-Digitize works best on flat-colour logos; photos and gradients need tracing by hand
  first.
- Design from Prompt works with your own Anthropic API key only (StitchFoundry accounts and AI
  credits aren't built yet), and only in the desktop app. The drawing follows the description
  loosely and can use more colours than asked (colours close enough to share a thread are merged),
  and a drawing is billed even when it comes out unusable.
- Branching orders objects closest-first rather than searching for the best tour, and only
  carries the thread across under objects of the branch sewn later.
- Appliqué is one object (placement, tack-down, cover). Splitting an appliqué where pieces overlap
  ("break up") and closing open appliqué shapes are not built yet.
