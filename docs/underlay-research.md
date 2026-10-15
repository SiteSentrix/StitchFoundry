# Underlay: research notes and plan

Status: research done 2026-10-15; plan U1–U6 proposed, not started.

Underlay is the stitching sewn before the top stitches of each object. It holds the fabric to
the stabiliser, gives the top stitches a flat, raised base so they don't sink into soft or piled
fabric, keeps satin edges crisp (no "saw-tooth"), and stops fabric showing through. Getting it
right for the column width and the fabric is most of what makes satin lettering look good or bad.
These notes come only from public documentation and articles, cited below.

## What the sources agree on

**Satin columns, by width** (Embroidery Legacy; Stitch Era *Underlay 101*; Wilcom lettering help;
Hatch; mySewnet help):

| Column width | Underlay | Notes |
|---|---|---|
| under about 1–1.5 mm | none | too narrow: any underlay pokes out |
| about 1.5–2 mm | centre run (centre walk) | one run down the middle and back |
| about 2–3.5 mm | edge run (edge walk, contour) | a run just inside each edge, 0.4–0.6 mm in; the closer, the crisper the edge, down to 0.2 mm |
| 4 mm and wider | edge run + zigzag ("German underlay") | the two edge runs alone leave the middle unsupported |
| very wide, or pile / soft fabric | add a centre run, double zigzag | all three together for the hardest cases |

Wilcom's fabric settings treat an object as *wide satin* when a 4 mm circle fits inside it,
*narrow satin* when a 2 mm circle fits, and *lettering* below that.

**Lettering, by height** (Wilcom): under 5 mm high, no underlay; 6–10 mm, a centre run; over
10 mm, an edge run; large letters (jacket backs) a second layer, sometimes a double zigzag.
Wilcom's lettering default is one edge run for the whole letter ("by shape"), which cuts
bunching, travel and stitch count.

**Stitch length:** about 2–2.5 mm for underlay runs (1.5–3.5 mm range). For small lettering, shorter:
about 1.2 mm, so the run follows tight curves and stays hidden. Longer on fleece, terry, leather and
vinyl, so short stitches don't sink in or cut the fabric. Where underlay shows at tight inside
corners, shorten its stitches rather than moving it away from the edge.

**Zigzag underlay** is a much more open satin (wider spacing), sewn to the far end and back. An
edge run and a double zigzag together give the most support and raise the satin (terry, fleece).
The sources disagree on the order. Stitch Era sews the edge walk first, then the zigzag.
Embroidery Legacy sews the zigzag first, so it doesn't pull the edge runs in.

**Fills:** an edge run plus an open tatami at 90° to the top rows, rows much wider apart than the
top. For very soft or stretchy fabrics, a double tatami (cross-hatch) with the edge run. For a
large, multi-object design, one "global" open tatami (3 mm spacing) under everything, with edge
runs under the top objects.

**By fabric:**
- Cotton and light wovens: standard density and compensation, minimal underlay.
- Knits (T-shirt, polo/piqué): more underlay (edge runs are best on knits), extra zigzag, more pull
  compensation, slightly lighter density.
- Fleece: heavy underlay, wider spacing, more pull compensation.
- Denim and twill: less underlay, lighter density, longer stitches, little compensation.
- Caps: strong edge underlay, firmer density; centre-out order, underlay first.
- Towels / terry:
  - A zigzag underlay even under thin lettering columns, to mat the loops down.
  - An edge run plus zigzag on columns over 3 mm.
  - A crossed underlay under fills.
  - Columns and fills 10–20 % denser, more pull compensation.
  - A knockdown (a light fill in the towel's colour, reaching about ⅛″ past the design) for long
    pile.
  - Water-soluble topping.
  - Bold designs: fine detail disappears into the loops.

Everything above is a rule of thumb. Every source says to test-stitch on the real fabric.

## What StitchFoundry does now

- **Satin, automatic underlay** (`sf-core` `satin.rs` `underlay_passes`):
  - under 1 mm, none;
  - under 2.2 mm, a centre run;
  - under 4.5 mm, an edge run;
  - 4.5 mm and wider, an edge run then a zigzag.
- **Settings:** inset 0.4 mm, run stitch length 2.5 mm, zigzag spacing 2 mm. This matches the
  sources except the zigzag starts at 4.5 mm instead of 4 mm, and the stitch length doesn't shorten
  for narrow columns.
- **Fills:** an edge run and an open tatami at 90° (2.5 mm rows). This matches.
- **Fabric presets** (`embroidery.applyFabric`): nine fabrics, each with satin/fill density, pull and
  push compensation, satin underlay recipe, fill underlay, stabiliser and topping advice. Towel /
  terry puts an edge run + zigzag under every satin. They reach satin lettering ("Grace" 25 mm:
  1,594 → 2,133 stitches).
- **Make a Design never asks about the fabric:** every design gets the woven defaults. A word made
  for a towel gets only the light edge run, which sinks into the loops.
- **Knockdown** exists as an outline kind (`embroidery.outline {kind: knockdown}`), but nothing
  offers it for a towel.

## Plan

- **U1 · Fabric step in Make a Design:** "What are you sewing on?", with plain choices
  (cotton/quilting cotton, T-shirt, polo, sweatshirt/fleece, towel, denim/canvas, cap,
  leather/vinyl).
  - It applies the matching preset to what it makes, and is remembered as My Machine's usual
    fabric.
  - The steps list and printed worksheet say which stabiliser and whether to use topping.
- **U2 · Satin widths as the sources give them:** the zigzag joins the edge run from 4 mm (not
  4.5); the centre run from 1.5 mm, nothing under 1.5 mm.
- **U3 · Shorter underlay stitches on narrow columns:** the centre run and edge runs shorten
  towards 1.2–1.5 mm on columns under about 2.5 mm, so they follow curves and stay hidden. Pile
  fabrics keep longer ones.
- **U4 · Towel recipe as the sources describe it:**
  - zigzag under thin lettering too;
  - edge run + zigzag over 3 mm;
  - a crossed (double) tatami under fills;
  - a knockdown under the design, offered (on by default) when Towel is chosen;
  - topping in the notes.
- **U5 · Fleece and stretchy knits:** a double tatami (cross-hatch) under fills, as Wilcom suggests
  for very soft fabrics.
- **U6 · See it:** "Show underlay" in Watch It Sew / the stitch player draws underlay in a lighter
  shade, so its presence and shape can be checked before sewing.

Each lands with tests (stitch counts, underlay passes by width) and a sew-out check list. Real
sew-outs on the Designer Diamond deLuxe decide the final numbers.

## Sources

- Embroidery Legacy, *Understanding underlay*: https://embroiderylegacy.com/embroidery-digitizing-underlay-digitizing/
- Embroidery Legacy, *Digitizing tips for towels & terry*: https://embroiderylegacy.com/embroidery-digitizing-towels-terry-cloth/
- Stitch Era (ColDesi), *Underlay 101*: https://support.coldesi.com/Portals/8/Documents/Liberty%20Docs/Underlay%20101.pdf
- Wilcom, *Setting lettering underlay*: https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Lettering/lettering_advanced/lettering_advanced-22.htm
- Wilcom, *Automatic underlays*: https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/stabilizing/stabilizing-6.htm
- Wilcom, *About auto fabrics*: https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Digitizing/properties/properties-9.htm
- Wilcom, *Tatami underlay settings*: https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/stabilizing/stabilizing-11.htm
- Hatch, *How to choose the right underlay*: https://hatchembroidery.com/resources/blog/how-to-choose-the-right-underlay-for-your-machine-embroidery-designs
- Ink/Stitch, *Satin column* (centre-walk, contour, zigzag underlay): https://inkstitch.org/docs/stitches/satin-column/
- mySewnet help, *Satin column*: https://softwarehelp.mysewnet.com/MSW/140/Embroidery/Topics/Satin_Column.htm
- ASI, *Keep your small letters crisp*: https://members.asicentral.com/news/how-to/may-2018/keep-your-small-letters-crisp/
