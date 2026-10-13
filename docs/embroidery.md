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
- [Redraw Image with Claude](#redraw-image-with-claude)
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
| Blanket, zigzag, motif | E-stitch prongs, an open zigzag or a repeating motif along the path (wave, scallop, diamond, cross, feather, heart, star, leaf, loop, zigzag, E-stitch, or one drawn for the design), motifs growing or shrinking along it when graded | stitch length, width, width at end, spacing, flip side |
| Redwork | the lines of a path, compound path, type or redwork group sewn as one line that ends where it starts, the way between lines retraced along them | run style (single, triple, backstitch, stem stitch), stitch length |
| Hand stitch | a run that looks sewn by hand: thick stitches (each sewn three times) with short thin ones between them, lengths varying a little | stitch length, gap, passes |
| Sculpture run | a thick-thin line: groups of triple stitches in turn with groups of single ones | triple and single stitch lengths, how many of each, passes |
| String stitch | string art (as in Paghadi work): long straight stitches spanning part of the line, overlapping; on curves they draw a curve inside it, round a circle a ring | chord length, spacing, passes |
| Satin | a column; on a closed shape, satin that turns along it | density, pull compensation (one value, or one per side), auto spacing, short stitches, auto split, preserve long stitches, tatami stitches (a tatami border), jagged edges, underlay (incl. double zigzag) |
| Tatami fill | rows of staggered stitches at the best angle, or turning along inclination lines | density, stitch length, row offset, angle, randomised needle points, gradient density, spacing profile, colour blends, texture, split lines, trapunto, underlay |
| Satin fill | satin rows across the shape | density, pull compensation |
| Contour / Spiral / Radial / Cross / Meander fill | rows following the outline (all the way in, or only so many rings: an offset fill), a spiral, rays from the centre, cross stitches on a grid (full X, half, upright + or double), or a flowing stipple line that never crosses itself | spacing, stitch length, rings (contour), cross style and cell size (cross), randomness (meander) |
| Motif fill | rows of motifs across the shape, kept inside it, each row shifted by the row offset | motif, motif size, row spacing, stitch length, row offset, angle |
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

**Motifs:** a motif run repeats one of eleven motifs along its line (wave, scallop, diamond,
cross, feather, heart, star, leaf, loop, zigzag and E-stitch; the last two make zigzag and E-stitch
fills as motif fills). With *Graded* on, the motifs
grow or shrink from *Width* at the start to *Width at end*, their repeats keeping their
proportions. Motifs, blanket prongs and their grading follow the line the way it was drawn,
whichever end sewing starts from (the closest join). A **motif fill** sews rows of motifs across a shape, half a motif inside its outline;
the row offset shifts each row along by that fraction of a motif (0.5 for a brick layout).
*Paired with* makes a paired motif fill: a second motif sewn in turn with the first along every
row (`motif_pair`), each in the same place on every row. *3D effect* sculpts a motif fill
(`warp`): **Globe out** makes a dome, the motifs largest in the middle and smaller and closer
together toward the edge; **Globe in** makes a bowl, the motifs smallest in the middle; and
**Perspective** makes the fill recede, its motifs and rows smaller toward the far side, which
*Direction* points to (90°: up). *Strength* (0 to 0.8) sets how much the motifs change; strong
effects make small stitches where motifs shrink.
**My Motifs:** *Keep Motif* (beside the motif list, or `embroidery.motif.keep {name}`) keeps one
of the design's motifs for any design. The motif lists show My Motifs after the design's own;
applying one adds it to the design. `embroidery.motif.library` lists them and
`embroidery.motif.forget {name}` takes one out; they are kept with the preferences, and
resetting the preferences keeps them.
**Embroidery › Place Motif** places one motif (10 mm long) in the middle of the artboard as a path
sewn in a single run: move, scale and reshape it like any art. The **Motif tool** (in the
Embroidery tool group) places one wherever you click, or drag across the canvas for one as long as
the drag and turned its way; pick the motif, built-in or one of the design's, in the Embroidery
panel while the tool is active.

**Motif columns:** a satin column or satin line can be sewn as motifs instead of satin (*Sew
as: Motifs*, `motif_column`): along its middle, each motif as wide as the column where it sits (a
tenth less, so it stays inside) and *Motif length* times as long, turning with the column's
rungs. On a tapered or calligraphic line the motifs grow and shrink with it (a graded motif
column). Motifs stand upright along a column drawn left to right, as on a motif run.

**Rings:** **Embroidery › Place Ring** places a 20 mm ring, 3 mm wide, in the middle of the
artboard: a **Satin Ring** (a satin border round a circle), a **Filled Ring** (a fill between two
circles) or a **Bean Ring** (a bean run round a circle). It is an ordinary circle afterwards:
move, scale and reshape it. `embroidery.placeRing` takes the `stitch`, the `diameter`, the
ring's `width` and where it goes (`at`).

**Your own motifs:** draw a motif as a path, left to right the way it should sew along a
left-to-right line, select it and choose **Embroidery › Make Motif**. An open path runs from its
first point to its last, so the motif starts and ends on the line between them; a closed shape
(or a path ending near where it starts) lies on the middle of its height and is sewn once round
from its left side, then on round to its right side. The motif keeps the drawn proportions and
belongs to the design (it is saved with it): the *Motif* list of motif runs and motif fills
offers it after the built-in motifs. Choosing it makes a run as high as the drawing's proportions
make it for its spacing, and spaces a motif fill's rows so the motifs don't overlap; Place Motif
and Motif Border take it too. Making a motif again under the same name (`embroidery.motif.make
{name}`) changes it in everything that sews it. *Delete Motif* takes it out of the design; objects
sewing it keep their copy. All motifs, built-in or drawn, sew the same way up along runs, in
motif fills and when placed: upright along a line drawn left to right (*Flip* turns them over).

**Spacing profiles and colour blends** (tatami): *Spacing profile* sets how the rows space out
across the shape (in the direction the rows stack): **Even**, **Accordion** (dense, sparse,
dense, sparse, dense: a shaded, ribbed look), **Fade out** and **Fade in** (the rows thin out to
nothing across the shape). The density changes evenly between the profile's points. Below the
presets, the profile's curve shows the rows' density across the shape (up is denser; the line is
the fill's own density; along the bottom, no rows): drag a point to reshape it, double-click to
add one (16 at most), right-click one to take it out; each change is one undo step. Agents can set
any profile (`spacing_profile: [[t, spacing], …]`, 0 for no rows). **Make Blend** (with a colour
picked in *Blend into*) turns the fill into stacked layers, one per colour, each densest across
its own band and fading to nothing at the next colour's, so the thread colours mix across the
rows; the layers together sew about as much as the fill did, and only the first keeps the
underlay. `embroidery.makeBlend` takes up to six colours. Turn the fill's angle to turn the blend.

**Outlines and borders:** **Embroidery › Add Outline** sews an outline round the selected
stitched shapes, lines and type, after them and in the top one's thread: a **Satin Outline** or
**Bean Outline** following them 1 mm out, or a **Rectangle**, **Oval**, **Rounded** or **Motif
Border** framing them. A line is outlined round the band its stitches cover (a satin line as wide
as it is sewn, a run as if 1 mm wide), a closed line or border round its outside. `embroidery.addOutline` takes the shape (`offset`, `rectangle`, `oval`, `rounded`),
how far out (`offset`, mm), the stitch (`run`, `bean`, `satin`, `motif`), its `width` and `motif`.
The outline is an ordinary path afterwards: reshape it or change its stitch type.

**Repeats and envelopes:** stitched objects inside a repeat (Object › Repeat: radial, grid or
mirror) are sewn once per copy, where each copy shows: wreaths, borders and arrays from one
motif. Selecting the source object shows every copy's stitches. Stitched objects inside an
envelope (Object › Envelope Distort) are sewn bent through it, like the art; their inclination
lines and start and stop beads don't bend with them.

**Laydown:** **Embroidery › Add Laydown** puts an open tatami (rows 2.5 mm apart, at 45°, no
underlay, travel along its outline) under the selected stitched shapes and type, 1.5 mm beyond
them, sewn first in the bottom one's thread. On towels, fleece and knits it flattens the pile so
the design sits on top instead of sinking in. Lines get no laydown. The command's `margin`,
`spacing` and `angle` change it; afterwards it is an ordinary fill.

**Hand stitch** (a run style): stitches about *Stitch length* long, each sewn forward, back and
forward again so it lies thick, joined by single stitches about *Gap* long that hardly show; each
length varies a little at random (the same way every time), so the line looks sewn by hand. It
starts and ends with a thick stitch.

**Sculpture run** (a run style): a thick-thin line. *Triple stitches* stitches *Triple stitch
length* long, each sewn forward, back and forward again, in turn with *Single stitches* stitches
*Single stitch length* long (three and one to start with); all lengths are scaled a little so
the line ends on a whole stitch.

**Cross fill styles:** *Cross style* sews each cell of a cross fill as a **full cross** (an X:
the "/" diagonals along the row and the "\" diagonals coming back), a **half cross** (the "/"
diagonals only), an **upright cross** (a + : the middle line with a bar up and down each cell,
and the middle line again coming back) or a **double cross** (an X with a + over it). *Cell size*
sets the grid.

**Jagged edges** (satin): the stitches on one side (*Side 1* is the first rail), the other or
both end short of the edge by a random amount up to *Jag depth*, for fur, grass and feathers. A
stitch always keeps at least a fifth of the column's width; the underlay isn't jagged.

**Trapunto** (tatami): travel between the parts of the fill keeps to its outline instead of
running along its rows, crossing the fill only where nothing else reaches (round a hole). Use it
with an open density (2 mm and more), where travel along the rows would show between them.

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

## Fill textures and split lines

A tatami fill sews its rows with the needle points staggered a little from row to row, which
reads as a flat, even surface. Where the needle points line up instead, they show as lines in
the fill: an embossed texture without another thread.

- **Texture** (Embroidery panel, tatami fills): a pattern tiled over the fill, with a needle point
  wherever a row crosses one of its lines (programmed splits). **Diamonds**, **Waves**,
  **Herringbone**, **Bricks**, **Circles**, **Scales**, **Hexagons**, **Chevrons**, **Zigzag**,
  **Ogee**, **Basket weave**, **Stars**, **Hearts** and **Leaves**; *Texture size* is the width of one
  pattern tile (to start with, a size that suits the stitch length), *Texture angle* turns the
  pattern against the stitch rows. Between the pattern's lines the stitches run straight to the
  next line, broken up only where the gap is longer than a long stitch, so the pattern reads
  clearly. The pattern stays put on the page while the shape changes.
- **Split lines** (drawn splits, carving stamps, flexi splits): draw paths over a tatami fill,
  select them with the fill and choose **Embroidery › Carve Split Lines**. The fill keeps its usual
  stitching and gains a needle point where each row crosses a line. A closed path presses in its
  whole outline, like a carving stamp (a star, a heart, a letter's outline); open paths draw
  lines. The paths are used up unless the command's `keep` parameter is on. When every selected
  object is a tatami fill, the front-most one is the stamp for the others.
  With the `repeat` parameter (mm), each line is copied that far apart across the whole fill:
  draw one wave across a shape for wavy lines all over it (flexi splits).
- **Contour split lines:** **Embroidery › Contour Split Lines** gives the selected tatami fills
  split lines that follow their outlines: each outline, holes too, repeated inward every 4 mm
  (`embroidery.contourSplits {spacing}`) until the shape runs out, so the fill shows rings along
  its edge.
- **Carving stamps:** **Embroidery › Carve Stamp** presses a stamp from the library into the
  selected tatami fills, in the middle of each and 60% as wide as its smaller side: star, heart,
  flower, leaf (with its veins), spiral, sun, moon, snowflake, paw print, swirl, shell,
  butterfly, clover, feather, rings and diamond, all drawn for StitchFoundry. The stamp becomes
  split lines like any carving (`embroidery.carveStamp` takes `at`, `size` and `angle`;
  `embroidery.stamps` lists the library).
- Split lines show dashed on the selected fill, move, turn and
  scale with it, and stay its own: **Make Properties Current** and styles leave them out.
  **Embroidery › Clear Split Lines** (or the button in the panel) takes them out again.
- Textures and split lines change only straight tatami rows: satin fills, turning fills (two
  inclination lines or more) and underlay ignore them, and a textured tatami shape doesn't turn
  along long thin shapes on its own.
- **Flow lines** (Florentine and liquid effects): draw one or two lines across or beside a
  tatami fill, select them with it and choose **Embroidery › Fill Along Lines**. With one line,
  every row is a copy of it moved across the fill, so the stitches follow its curve; with two,
  the rows change shape evenly from the first line to the second, so they can fan out, narrow
  or swing round. Draw the two the same way round and keep them clear of each other; rows
  crowd where the lines come together. The lines carry on straight past their ends, and a
  pair that would stretch the shape out of all proportion leaves the rows straight. Textures
  and split lines follow the rows; underlay stays straight. Flow lines show dashed on the
  selected fill, move with it, and take precedence over inclination lines;
  **Embroidery › Clear Flow Lines** (or the button in the panel) gives straight rows back.

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

## Appliqué covers

An appliqué's cover stitch is satin, blanket, an open zigzag or a triple run (or none, for raw
edges), *Cover width* wide. **Partial cover** (`partial: true`) leaves the cover out wherever an
object sewn later lies over the patch's edge: that object holds the fabric there, and the cover
under it would only add bulk.

## Click to stitch

The **Click-to-Stitch tool** (in the Embroidery tools) turns a region of a placed image into a
stitched object with one click: the region is the pixels joined to the spot clicked whose colour
is within *Tolerance* of it (a magic wand), and it becomes an object in its colour, stitched as the
tool's *Stitch as* setting says (in the Embroidery panel while the tool is chosen):

- **Fill**: tatami over the region, its holes left open (holes under 16 pixels are filled);
- **Fill without holes**: tatami over the whole region;
- **Satin**: satin across the region, turning with it;
- **Outline**: a running stitch round its edge;
- **Centre line**: running stitches along its middle, for lines and lettering drawn in the image.

The same is `embroidery.clickStitch {at, as?, tolerance?, minHole?, id?}`. Each click is one undo
step; the image stays under the stitches.

## Outlines round art

**Embroidery › Outline Around** (`embroidery.outlineAround {gap?, width?, kind?, color?}`)
draws one outline round the selected stitched art as it sews (lettering, shapes and lines
together; pieces closer than about four times the gap share one outline), its holes left out:
bubble lettering, a sticker or patch border. It is `gap` (2 mm)
from the art, a satin `width` (3 mm) wide, or a single or triple run (`kind: run|triple`), in
`color` (near black), and is sewn after everything else; it is an ordinary path to change after.

Two more kinds, after Ink/Stitch's knockdown fill (https://inkstitch.org/docs/fill-tools/):
**knockdown** (`kind: knockdown`) fills that area with light tatami (1.2 mm rows at 45°) sewn
first, under the design, to press a towel's or fleece's pile flat; **emboss** (`kind: emboss`)
fills a rectangle or circle (`shape`) reaching `extend` (5 mm) beyond the design with light
tatami, leaving the design's area out, so the design stands up from a stitched background.

## Scatter copies

**Embroidery › Scatter Copies** (`embroidery.scatter {count?, gap?, spread?, seed?, turn?}`)
strews copies of a motif over a shape: select the motif (one object or several) and a closed
shape larger than it, and the copies go inside the shape, `gap` (2 mm) apart and clear of its
edge, at random (`spread: random`, the same each time for the same `seed`) or evenly on a
triangular grid (`even`), up to `count` (20), each turned at random with `turn`. They keep the
motif's stitching and land in a group above the shape, which stays (delete it, or give it a
fill to sew behind them).

## Design library

`embroidery.library.scan {folder, query?, recursive?, sort?}` lists the machine embroidery files
in a folder and its subfolders (DST, EXP, JEF, PES, PEC, VP3, XXX, U01, PCS, TAP): each with its
size, stitches, colours, threads, date and an outline of its stitches for a thumbnail; `query`
finds names and folders, and `sort` orders them by name, date, stitches or size. What a file says
is kept while it is unchanged, so looking again is quick; a file that can't be read is listed
with why. `embroidery.library.catalogue {folder, query?, columns?, page?}` prints them: a
catalogue sheet of thumbnails in their colours, each with its name, size, stitches, colours and
format, as many pages as it takes.

## Worksheets

**Embroidery › Worksheet** (`embroidery.worksheet {kind?, customer?, notes?}`) makes a worksheet
for the design as a new A4 document, to print (File › Print) or export as PDF:

- the design drawn in its thread colours, life size when it fits (else scaled, the scale said);
- its size, stitches, colours, colour changes, trims, thread (top and bobbin) and sewing time at
  the machine's speed, and its hoop, fabric and machine;
- its colours in sewing order: a swatch, the thread's code, name and brand, stitches and length;
- a barcode of the job at the top right (`barcode: code128|code39|none`, Code 128 unless said):
  the design's name, or `barcodeText`, to scan at the machine or the counter;
- the design's colorways, each with its threads colour by colour, the one in use marked
  (`colorways: false` leaves them out);
- the notes given, else the design's own **Design notes** (set in the panel's Design section,
  or `embroidery.settings {notes}`, and kept with the design), wrapped to the page; an
  **approval sheet** (`kind: approval`) adds the customer and a line to sign.
- a **pattern** (`kind: pattern`) is the design over the whole page with one line of figures;
- a **placement template** (`kind: template`) is the design life size, with lines through its
  middle to the page's edges, an arrow for which way is up and a 100 mm bar to check the print
  wasn't scaled: cut it out or lay it on the garment to mark where the hoop goes;
- `placement: left-chest|right-chest|full-front|full-back|cap-front|sleeve|yoke|pocket` prints
  the usual size and place for it (typical values: garments and customers vary);
- on A4 or US Letter paper (`page: a4|letter`); a logo or anything else can be added to the sheet
  like to any document (File › Place).

The worksheet's text is in English for now.

## Multi-hooping

A design larger than the hoop is sewn in several hoopings: `embroidery.multiHoop {hoop?, width?,
height?, overlap?, dir?, format?, marks?}` lays a grid of hoopings over it that overlap by
`overlap` (10 mm), puts each object, whole, in the first hooping that holds it, and writes each
hooping as a machine file of its own (`name-1`, `name-2`…, each centred on the hoop) into `dir`.
Each hooping first sews **registration crosses** in the overlaps it shares with its neighbours,
in a colour of its own: re-hoop the fabric so the next hooping's crosses land on the ones already
sewn, then sew on. Without `dir` it only plans (which object goes where). Objects aren't split
across hoopings yet: one larger than a hooping is refused (choose a larger hoop or a wider
overlap).

## Send to machine

**Embroidery › Send to Machine** (`embroidery.send`) writes the design's machine file (in its
machine's format, as Export Machine File does) straight into the folder the machine reads: a USB
stick, the machine's own folder, or a network share (a hot folder). Add the folder once in the
Embroidery panel's *Send to Machine* section (*Add Machine Folder…*); it is remembered, the last
used first, and *Send* writes the file there under the design's name (a number is added when a
file of that name is there already; `replace: true` writes over it). The folder is checked first:
when it isn't there (the stick taken out, the share unreachable) the send stops and says so.
`embroidery.send.targets` lists the folders and whether each is there now. Machines that only take
designs over their maker's own wireless link are not reached directly; send to a folder they read.

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
(satin for narrow parts, tatami for the rest), and each one cuts what it covers out of the fills
sewn before it (Remove Underlying Stitching, with a 0.4 mm overlap so small details on top don't
stack thread), so stacked shapes don't pile up. They are sewn in the order they were drawn, with
shapes of one colour brought together where the stacking allows (Auto Sequence, within the group),
so there are fewer colour changes. Its colours become the design chart's closest threads. Images, invisible
shapes and specks under 1 mm² are left out. One undo step undoes it all. As with any automatic
digitizing, check the stitches (Check Design, the stitch player) before you sew. Claude won't copy
someone else's logo, trademark or character: the dialog says so, and an original design described
in your own words draws fine.

**Your own API key.** Design from Prompt uses your Anthropic API key, and each design is billed to
your Anthropic account; get a key in the [Anthropic Console](https://console.anthropic.com/settings/keys).
Paste it into the dialog. With **Remember the key on this computer**, once Anthropic accepts the
key it is kept in a file of its own, `anthropic-api-key`, next to StitchFoundry's settings and
readable by you alone: not in the settings themselves, which the control channel and MCP agents
can read. Turn the option off and Generate to forget a kept key. Without a kept key, the `ANTHROPIC_API_KEY` environment variable is used.

Only your description, the size and the colour count are sent (and, to redraw an image, the
image: see below), to `api.anthropic.com` over HTTPS, and only when you click Generate; nothing
else from your design leaves the computer. Certificates are checked against the system's trusted
ones, and a proxy set in the usual environment variables (`HTTPS_PROXY`…) is used. Design from
Prompt needs the desktop app: the web app can't send the request yet. Agents don't need it: an
agent that draws can hand its own SVG to `embroidery.placeArt`, which does the rest.

## Redraw Image with Claude

Select a placed logo or image and choose **Embroidery › Redraw Image with Claude…** (or **Redraw
with Claude…** in the Embroidery panel, next to Auto-Digitize). Claude redraws it as clean
flat-colour art in the same dialog as Design from Prompt, with the same API key: it keeps the
image's composition, shapes, lettering and colours, leaves out its background (unless it is part
of the design, like a badge shape), merges shades into at most the thread colours asked for, and
thickens or drops what is too fine to stitch. Write anything to change in the box ("leave out the
tagline", "make the letters bolder"); it is optional. Only redraw artwork you own or have
permission to use.

The drawing lands on the image: its frame is the image's place, sized to the size asked about the
image's centre (by default the image's own size, brought into the hoop, as Auto-Digitize does). It
is then digitized as Design from Prompt's drawings are: stitch types, underlying fills cut, the
chart's threads and same-coloured shapes together, in one undo step. The image stays underneath to
compare with. Unlike Auto-Digitize, which traces the pixels, Claude draws the shapes anew, so
blurry, small or photographed logos come out clean; check the lettering and the details against
the image before you sew.

The image is sent as a PNG (a JPEG when that would be too large), no larger than 1568 pixels on its
longest side, which is as much as the model looks at.

## Design settings: hoop, fabric, threads

The Embroidery panel's **Design** section shows the stitch count, size, colours, trims, sewing
time and thread used, and whether the design fits the hoop. Its settings:

- **Hoop**: the sewing field, drawn on the canvas centred on the design (machine files are
  centred too); dashed red when the design doesn't fit.
- **Fabric**: presets that set density, pull compensation and underlay, and say what to hoop
  under it: a **Stabiliser** (tear-away for stable wovens, caps and towels; cut-away or a no-show
  mesh for knits, which stretch; wash-away for sheer work) and a water-soluble topping for pile
  fabrics, printed on worksheets too. **Fabric settings**
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
- **Start at** and **End at**: where sewing starts (the first object entered at its point
  closest to the design's centre, a corner or the middle of a side) and where the frame goes when
  the design is done, after a trim: the centre, say, to take the hoop off with the needle clear,
  or a corner to line up the next hooping (`embroidery.settings {start, end}`).
- **Basting first**: a basting run, long running stitches (6 mm) sewn before everything else in
  the first thread and without ties, holding the fabric to the stabiliser, to pick out after.
  *Basting round* the design (*Basting margin* out from it, 5 mm) or the hoop (that far in from
  its edge, the hoop centred on the design); it is written into the machine file
  (`embroidery.settings {basting: true | {hoop?, margin?, length?} | false}`).
- **Estimate cost**: what sewing the design costs, shown under its figures (*Cost a piece*) and on
  worksheets. *Cost settings*: a price per 1000 stitches, the machine's time an hour (shared by
  the pieces its *Machine heads* sew at once), thread per 1000 m (top and bobbin) and a set-up
  charge an order, in a currency; `embroidery.cost {quantity}` prices an order, the set-up spread
  over its pieces (`embroidery.settings {cost: {perThousand?, perHour?, threadPerKm?, setup?,
  heads?, currency?}}`).
- **Colorway**: other thread schemes for the same design, a shirt in navy and one in white.
  *New Colorway* copies the threads in use; each of the design's colours (in the order it first
  uses them) then gets a thread of the colorway's own, from the colour buttons below. The
  colorway picked is what the stitches show, machine files carry and worksheets print (named
  there); *The design's own* goes back to the threads set on the objects, and the art keeps its
  colours throughout (`embroidery.colorway.list|add|activate|setThread|rename|remove`).
- **Thread chart**: the chart the panel's thread list picks from. Threads keep their name and
  code; PES and JEF files use the nearest colour of the machine's own palette. **Match Threads to
  Chart** (also in the Embroidery menu, for the selection) gives every stitched object the chart's
  closest thread and recolours its art: use it on a design opened from a machine file, whose
  colours name no threads.
- **Show stitches** (Hidden / Lines / TrueView), **Artwork** (Normal / Dimmed / Hidden),
  connectors, needle points, the selection's **start and stop** beads and its **inclination
  lines** (shown with any tool, not only the Inclination tool).
- **Repeats across** and **Repeats down** (up to 10 each) show the whole design repeated in a
  grid on the canvas, **Gap between repeats** apart, to preview a repeated layout (a border, an
  all-over pattern); only the design itself is sewn (`ui.embroideryRepeats`).

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
freezing it (to place appliqué fabric, or change thread on a single-needle machine). **Own colour
block** gives an object a colour block of its own: a colour change before it even when the object
before is in the same colour (split colour blocks: say, a dark red sewn as two blocks so a
multi-needle machine can take it from two needles, or so it stops there; `ownBlock` in
`embroidery.setParams`).

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

**Embroidery › Add Buttonholes** places a straight, rounded or keyhole buttonhole (15 mm slit,
4 mm wide) in the middle of the artboard: satin columns bordering a slit that is cut open after
sewing (0.6 mm is left open for the cut). Each buttonhole is a group of ordinary stitched paths,
sewn in order: a run round it to hold the fabric, one side, the far end (a bartack, a satin end
rounded round the slit, or a keyhole eye), the other side and a bartack at the near end. Move,
turn and resize it like any art. `embroidery.addButtonholes` places a string of them too
(`count`, evenly spaced from `at` to `to`), with the slit `length`, the `width` and an `angle`. The Embroidery panel's **Buttonholes** section adds one or a row
of them in the middle of the artboard: style, length, width, how many, how far apart (middle to
middle) and an angle, the row running square to the slits.

**Embroidery › Automatic Knife** cuts the selected stitched shapes round their holes and where
they branch (at sharp inside corners, like the arms of a T) into simpler pieces, each its own
object with the same stitching and its own automatic direction, a single angle or turning;
the pieces overlap by 0.3 mm along the cuts (`overlap`) and become a branch group, sewn as one
path. Edit each piece's direction like any object's; Break Apart separates the group.

**Embroidery › Redwork** sews the selected line art (paths, compound paths and type) the way
hand redwork is sewn: as one line that starts and ends at the same point, so there is one tie-in
and one tie-off. Every line is sewn once in the run style (*Single*, *Triple*, *Backstitch* or
*Stem stitch*); to get from one line to the next the needle retraces lines already sewn as a
single run on top, choosing the shortest ways, so no travel shows off the lines. Lines that
cross, touch, or end within 0.3 mm of another line meet there; parts that don't touch are
joined by travel or a trim. The art becomes a *redwork* group in the first thread given to its lines (else the first
line's stroke colour), and
its own stitching (a fill, say) is left out while it is in the group; the panel sets the run
style and stitch length. **Break Apart** gives the art back as it was. A single path, compound
path or type can be redwork on its own too (stitch type *Redwork*): its subpaths are sewn as
one line.

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
| Pfaff (older machines; experimental) | .pcs | RGB, 16 at most | yes | yes |
| Happy | .tap | no (DST records) | yes | yes |

- Opening or placing a machine file turns each thread into a path of **Manual** stitches (one
  subpath per run between trims), which you can edit, move, scale and recolour.
- PCS is **experimental**: its layout comes from public descriptions of the format, and no
  machine has sewn one yet. It holds 16 colours at most, positions in 1/6 mm, and no trim (a
  trim is written as a move to where the needle is). TAP is DST's records without the header.
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

## Ink/Stitch designs

An SVG made with Ink/Stitch, the embroidery extension for Inkscape, opens stitched the way its
Ink/Stitch settings say (they are kept with the art and saved with it): dashed strokes run, with
their stitch length, bean stitch and repeats; satin columns sew between their two rails, their
rungs becoming inclination lines; other strokes become satin lines as wide as the stroke; and
filled shapes fill, with their fill method (contour, meander and circular fills as contour,
meander and spiral fills, the others tatami), angle, row spacing, stitch length, stagger and
underlay. Art without settings of its own is sewn with the defaults, as Ink/Stitch would. A fill
with a stroke sews the fill only. Commands (trims, stops) and the newer stitch types
(ripple, tartan, gradient fills…) come in as their nearest StitchFoundry stitching. The mapping
was written from Ink/Stitch's public documentation, not its code.

## Embroidery fonts

Lettering can use embroidery fonts in Ink/Stitch's font format: a folder with `font.json` and
its glyphs' art (`ltr.svg`, or SVGs in an `ltr` folder), each glyph a layer named
`GlyphLayer-<glyph>` digitized with Ink/Stitch settings. StitchFoundry ships none: load font
folders you have (`embroidery.font.load {path}`; Ink/Stitch's own fonts are free to download
from its font library, each under its own licence, most of them the SIL Open Font License), and
set text with them (`embroidery.lettering {text, font, size, at}`): each glyph's art sewn the way
its settings say, placed with the font's advances and kerning, lines its leading apart, ligatures
first, in the font's letter case, characters it lacks shown as its default glyph. `size` is the
capitals' height in mm; outside the sizes the font says it sews well at, the lettering comes with
a warning. Font folders loaded are remembered: later sessions load them again the first time
lettering needs a font (*Forget Font* in the panel, or `embroidery.font.forget {name}`, unloads one
and forgets its folder; a folder that has gone is skipped with a warning). The lettering itself is stitched art, a
group per glyph, saved with the design. The lettering's group remembers what it says, in what font,
how big and where: `embroidery.lettering.edit {text?, font?, size?, leading?}` sets it again where
it is now, moved, turned and scaled as it is (scaling it changes its size), as long as its font is
loaded; the glyphs' own stitching can be changed like any art's, until it is set again.
**Embroidery › Break Apart** turns it into its glyphs, no longer lettering. In the Embroidery panel, **Embroidery Lettering**
does the same: *Load Embroidery Font…* picks a font folder, then choose the font, type the text,
set its size (the sizes the font sews well at are shown, marked ▲ as a warning while the size typed is outside them) and *Place Lettering* puts it in the
middle of the artboard. With a lettering selected, the section shows what it says, in what font and
how big: change any of them and *Update Lettering* sets it again where it is. *Baseline* bends the
lettering round a circle (*Radius*): **Arch** over its top, **Valley** along its bottom, glyphs
upright to it (`arc`: the radius in mm, negative along the bottom). **Vertical** sets the letters
one under another, each centred, extra lines side by side (`vertical`). *Place Along Selected
Line* sets the text along any line or shape you have drawn, from its start, each letter upright to
it (`path`: its id, or true for the selected one); the line is kept with the lettering, which
can then be moved or scaled as a whole, and `path: null` sets it straight again. *Spiral* winds it in round
the circle, the radius shrinking that many mm a turn (negative: winding out; `spiral`), for text
that runs on round and round; lettering placed from the panel
is centred on the artboard (`align`: left, centre or right on `at`).

One letter at a time: with a lettering selected, the section's *Letter* rows move a letter along
its line or down from it, scale it or turn it about the middle of its baseline (*Reset Letter* puts
it back); so does `embroidery.lettering.letter {index, dx?, dy?, scale?, angle?}`, or, with no
`index`, the letter selected inside the lettering. The changes are kept when the lettering is set
again (a letter is known by its place in the text) and move, scale and turn with it; a letter's
stitching can be changed like any art's until it is set again.

How lettering is sewn (*Sew order*, *Join* and *Trims* in the section; `order`, `lines`, `join`
and `trim` on `embroidery.lettering` and `.edit`):

- **Sew order**: the letters left to right, right to left, or **centre out**: the middle letter
  first, then the letters back to the first, then on from the middle to the last, so a cap's
  front panel is held where it matters most. `lines: up` sews the lines from the last up.
- **Join**: *Closest* starts each letter where it is closest to where the last stopped; *Along
  the baseline* starts each letter at its baseline's start and stops it at the end (start and stop
  points on its first and last pieces), so the thread runs on along the baseline, hidden in a
  towel's pile.
- **Trims**: where the jumps need them (the machine's settings), or after each letter, each word
  or each line.

These follow the public descriptions of lettering sequencing and joins, for example
https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Lettering/lettering_advanced/lettering_advanced-15.htm
and https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Lettering/lettering_advanced/lettering_advanced-16.htm.

## Warped lettering

Lettering takes the envelopes and warps any art does (**Object › Envelope Distort › Make with
Warp**: arc, arch, bulge, flag, wave, fish, rise, fisheye, inflate, squeeze, twist): its stitches
bend with the letters, and it stays lettering, so it can be set again with new text inside the
envelope (`embroidery.lettering.edit`). Moderate bends sew best; strong ones stretch satin
columns wide or crowd them.

## Lettering stitches and small text

*Letters sewn* (`stitch`) sews lettering as the font digitizes it, all **satin** (its filled
parts sewn as satin across them) or all **fill** (each letter one tatami fill, the shape of what
it sews: for large, wide letters). *Small text* (`small: auto|on|off`; auto: capitals under
6 mm) sews small lettering cleanly: satin over a centre run only, half the pull compensation and
short stitches round curves; fills with a light edge-run underlay and shorter stitches; runs in
short stitches. Both stay with the lettering when it is set again.

## Appliqué lettering

*Appliqué letters* in the lettering section (`applique: "#rrggbb"` on `embroidery.lettering` and
`embroidery.lettering.edit`) makes each letter a patch of fabric that colour: one shape, the
outline of what the letter sews (its fills and satin columns together), sewn as appliqué: a
placement line and a stop to lay the fabric, a tack-down and a stop to trim it, then the cover.
The appliqué settings can be changed per letter like any appliqué; the lettering stays lettering,
and turning it off again brings back the font's own stitching. Bold, wide fonts suit it best.

## 3D puff

**Embroidery › 3D Puff** (`embroidery.puff {ids?, on?, cap?}`) sews the selected objects' satin
over foam: dense (0.16 mm), with no underlay, auto split or short stitches (they would flatten
or cut the foam inside the column), the sides pushed out 0.35 mm (the foam pushes the thread out
rather than pulling it in), and a cap across each end of a column, its stitches reaching 0.5 mm
(`cap`) past the end so the foam is cut and covered there too. Chosen again on satin that is all
puff already, it turns it back to plain satin. Lettering takes it too (`puff: true` on
`embroidery.lettering` and `.edit`, *3D puff* in the lettering section): its glyphs' satin is puff,
and stays so when the lettering is set again. A filled shape made puff is sewn as satin across
it over the foam (made plain again, it fills again); other pieces are left as they are.

**Embroidery › Sew Puff Last** (`embroidery.puff.last`) orders a design the way puff is sewn:
the flat parts first, then the puff. In each layer (and plain group), the objects that are all
puff move after the others, in the order they were in, and the last piece sewn before the first
of them gets a stop, so the machine pauses for the foam to be laid over what is already sewn.

To sew it: give the puff its own thread colour so the machine stops before it, lay the foam
(2–4 mm, in the thread's colour) over the area, sew, then tear the foam away and heat it to shrink
the fibres left. Columns 3 to 7 mm wide puff best: narrower ones don't hold the foam, wider ones
let it bulge out. These follow public descriptions of the practice:
https://wilcom.com/resources/blog/qa-from-3d-puff-embroidery-webinar and
https://hatchembroidery.com/resources/blog/how-to-digitize-for-3d-puffy-foam-machine-embroidery.

## Making embroidery fonts

**Embroidery › Embroidery Fonts › New Embroidery Font** starts the font creator: a new document
with one artboard per glyph (A–Z, a–z, 0–9 and `.,!?'-&`; `embroidery.fontDoc.new {name, chars,
size, descent}` takes others, ligatures such as "fi" too), each named after its glyph and as wide
as its advance, with guides at the baseline and the capitals' height (25 mm unless you say). Digitize
each glyph on its artboard with the usual tools; resize an artboard to change how far the next glyph
starts. `embroidery.fontDoc.setProps` sets the font's name, line spacing, space width, letter case,
the sizes it sews well at, description and licence; `embroidery.fontDoc.kern {pair, value}` sets a
pair's kerning; `embroidery.fontDoc.addGlyphs` adds glyphs; `embroidery.fontDoc.info` lists the
glyphs and which still need art. **Save Embroidery Font…** writes the font folder (font.json and
glyphs.stitchfoundry, the glyphs in StitchFoundry's own format with their stitching) and loads it
for lettering at once; the folder is remembered like any font folder.

A font can also start from type: `embroidery.fontDoc.fromType {family, style, chars, size}` makes
a font document from an installed TrueType or OpenType family, each glyph its outline at the
family's own advance, digitized as Convert Artwork would (satin where the strokes are thin enough,
fill where they're wide), ready to refine and save. Check the type's licence allows it.

Designs on keys (a keyboard collection): select a design in a font document and *Assign* it to a
character in the Font Creator section (`embroidery.fontDoc.assign {glyph}`): it moves onto that
glyph's artboard (one is made if the font hasn't the glyph), its bottom on the baseline, the
artboard widened to hold it. Save the font and type with the designs like any lettering.

Any loaded font, Ink/Stitch's or one made here, opens as a font document to refine
(`embroidery.font.edit {name}`): its glyphs on their artboards at their advances, its properties
and kerning; saving it replaces the loaded font of that name (rename it with
`embroidery.fontDoc.setProps` to keep both). `embroidery.fontDoc.merge {font, chars}` merges
fonts: another font's glyphs this one hasn't yet come in below the others, scaled so their
capitals are as tall.

The Embroidery panel's **Font Creator** section does all this: *New Embroidery Font*, *New Font
from Type* (pick the family) or *Edit Font* (a loaded one) to start; in a font document, the font's
name, line spacing, space, letter case and the sizes it sews well at, its glyphs (those still to
digitize dim; click one to go to its artboard), glyphs to add, kerning pairs, merging another
font's glyphs, *Try in a New Document* and *Save Embroidery Font…*.

## Monograms

The Embroidery panel's **Monogram** section sets one to three initials in the lettering font:
**Classic** makes the middle one of three larger, each letter centred on the monogram's middle; **In
a line** sets them one size on one baseline. A border goes round them: a satin circle, diamond or
square, or a circle of scallops (`embroidery.monogram {letters, font, size, style, border, at}`).
Each letter is lettering of its own, set again like any (`embroidery.lettering.edit`), and the
whole monogram is one group.

More monogram choices: the **stacked** style sets the letters one above the other; borders
also come as an **oval**, **octagon**, **heart** or **shield**, and in up to four **rings** 3 mm
apart (`rings`); an **ornament** either side of the letters, mirrored (hearts, stars, dots as
satin, swirls as a triple run; `ornament`), is drawn in code and goes inside the border.

**Templates**: *Template* picks a look from the gallery (Classic circle, Double ring, Wedding
heart, Royal shield, Diamond trio, Scalloped, Starry oval, Square block, Stacked crest) or one of
your own; *Save as Template* keeps the look now set (style, border, rings, ornament, size) under a
name, kept with your preferences for any design, and *Delete Template* removes one of yours
(`embroidery.monogram.templates`, `.saveTemplate`, `.deleteTemplate`; `embroidery.monogram
{template}`, anything given with it winning).

## Team names

One design, many names (a team's shirts): set the name in an embroidery font, select it, and in the
Embroidery panel's **Team Names** section type the names, one a line (a line break in a name sets
two lines), or *Import Names…* from a CSV table (its first column; `embroidery.teamNames.read
{csv | path, column, header}` reads any column, commas or semicolons, quoted fields). *Export One
File per Name…* writes a machine file for each into a folder: the design with the lettering set
to that name, named after it (`embroidery.teamNames.export {names, dir, format, prefix}`). The
design itself is left as it is. *Lay Out Together* puts every name in one design, in rows and
columns a gap apart, each still lettering, for a large hoop or patches cut apart after
(`embroidery.teamNames.layout {names, columns, gap}`); *Names Worksheet* lists each name with
its size, stitches and sewing time, and the totals for the order
(`embroidery.teamNames.worksheet {names, page}`).

## Commands

Every feature is an engine command. A few of the main ones (see `document.commands` or the MCP
tool list for all parameters):

| Command | Does |
|---|---|
| `embroidery.set {type}` | give the selection a stitch type (`run`, `bean`, `blanket`, `satin`, `fill`, `satinfill`, `contour`, `spiral`, `radial`, `cross`, `meander`, `applique`, `manual`, `redwork`, `none`) |
| `embroidery.setParams {params}` | change stitch parameters (camelCase names, mm) |
| `embroidery.setThread {color, name?, code?, chart?}` | set the thread |
| `embroidery.convert` | stitch types from the artwork's look |
| `embroidery.autoDirection` / `addGuide` / `moveGuide` / `removeGuide` / `clearGuides` / `setBeads` | stitch direction and start/stop |
| `embroidery.setParams {params: {texture: {pattern, size?, angle?} \| null}}` | a tatami fill's texture (programmed splits): `diamonds`, `waves`, `herringbone`, `bricks`, `circles`, `scales` |
| `embroidery.carveSplits {ids?, repeat?, keep?}`, `embroidery.setSplits {id, splits}`, `embroidery.clearSplits {ids?}` | split lines in tatami fills: carved from other art (a stamp's outline, lines, lines repeated across the fill), set from points, cleared |
| `embroidery.contourSplits {ids?, spacing?}` | split lines following tatami fills' outlines inward |
| `embroidery.carveStamp {stamp, ids?, at?, size?, angle?}`, `embroidery.stamps` | press a carving stamp from the library into tatami fills; list the library |
| `embroidery.fillAlongLines {ids?, keep?}`, `embroidery.setFlowLines {id, lines}`, `embroidery.clearFlowLines {ids?}` | flow lines in tatami fills: rows copying one line or flowing from one to another, taken from other art, set from points, cleared |
| `embroidery.placeMotif {motif, size?, at?, angle?}` | one motif (built-in or the design's) as a path sewn in a single run |
| `embroidery.motif.make {name?, ids?}`, `embroidery.motif.apply {name, ids?}`, `embroidery.motif.list`, `embroidery.motif.delete {name}` | the design's own motifs: made from the selected path, sewn by the selected runs and fills (paths not stitched yet become motif runs or fills), listed with how many objects sew each, removed |
| `embroidery.addLaydown {ids?, margin?, spacing?, angle?}` | an open fill sewn first under the selected shapes and type (pile fabrics) |
| `embroidery.makeBlend {colors, ids?, spacing?}` | a tatami fill as stacked layers blending 2 to 6 thread colours across its rows |
| `embroidery.addOutline {ids?, shape?, offset?, stitch?, width?, motif?}` | a stitched outline or border round the selected shapes, lines and type |
| `embroidery.placeRing {stitch?, diameter?, width?, at?}` | a satin, filled or bean ring |
| `embroidery.draw {brush, points \| stamp, …}` | a brush stroke, as the widget draws it |
| `embroidery.column {points: [[x, y, curve?]…], id?}` | a Column A/B satin column from points on its two sides in turn; with `id`, continue that column |
| `embroidery.autoDigitize {colors, …}` | trace a selected image into stitches |
| `embroidery.placeArt {svg, size?, at?, frame?, name?, minArea?}` | place flat-colour SVG art and digitize it, as Design from Prompt does with Claude's drawing (its linked files are never read); `frame` fits the SVG's page into a rectangle, as Redraw Image does over the image |
| `embroidery.settings {hoop, fabric, chart, machine, …}`, `embroidery.applyFabric {name}` | design settings |
| `embroidery.machine.list`, `embroidery.machine.save {name, newName?, format?, fromDesign?, …}`, `embroidery.machine.delete {name}` | machine profiles: change the standard ones, make, rename and delete your own |
| `embroidery.sortByColor` | sew each colour in one go |
| `embroidery.autoSequence {ids?}` | reorder for the fewest colour changes and the shortest travel, overlaps kept; with `ids`, only inside those groups |
| `embroidery.matchThreads {chart?, all?}` | the chart's closest thread for each stitched object |
| `embroidery.makeCurrent`, `embroidery.current`, `embroidery.current.setParams {type, params}`, `embroidery.current.reset {type?}` | the properties new objects take |
| `embroidery.style.new {name?}` / `apply {name}` / `redefine {name}` / `unlink` / `rename {name, to}` / `duplicate` / `delete` / `list` | embroidery styles |
| `embroidery.fabric.save {name, from?, …}`, `embroidery.fabric.delete {name}` | fabrics made for the design |
| `embroidery.branch {ids?}`, `embroidery.breakApart {ids?}` | sew objects as one path; separate a branch, a redwork group or a many-part object |
| `embroidery.addButtonholes {style?, length?, width?, angle?, at?, to?, count?}` | straight, rounded or keyhole buttonholes: one, or a string evenly spaced from `at` to `to` |
| `embroidery.knife {ids?, overlap?}` | cut stitched shapes round their holes and at their branches into pieces that each turn their own way, a branch group |
| `embroidery.redwork {ids?, kind?}` | sew line art as redwork: one line ending where it starts, the way between lines retraced along them (`kind`: `run`, `bean`, `backstitch`, `stemstitch`) |
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
- Design from Prompt and Redraw Image work with your own Anthropic API key only (StitchFoundry
  accounts and AI credits aren't built yet), and only in the desktop app. The drawing follows the
  description or the image loosely and can use more colours than asked (colours close enough to
  share a thread are merged), and a drawing is billed even when it comes out unusable.
- Branching orders objects closest-first rather than searching for the best tour, and only
  carries the thread across under objects of the branch sewn later.
- Appliqué is one object (placement, tack-down, cover). Splitting an appliqué where pieces overlap
  ("break up") and closing open appliqué shapes are not built yet.
