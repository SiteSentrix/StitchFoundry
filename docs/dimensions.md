# Dimensions

Choose **Dimension Tool** in the Modify section of the toolbar. Its control strip offers
**Length**, **Angle**, **Radius**, **Diameter**, **Options…** and **All selected edges**.

- **Length:** hover a path to highlight the segment between two anchors. Click it, move the
  pointer to place the dimension, then click again. Curved segments report their actual curve
  length, prefixed **Arc**, rather than the distance between their endpoints.
- **Two points:** Alt-click an anchor to start measuring individual points, click the second
  point, then click to place the dimension. Picked anchors remain linked to their original paths.
  Empty-space clicks also work; these fixed points have no source object to follow. Shift
  constrains a manually picked point to 45-degree increments.
- **Angle:** click two nonparallel straight segments, then click to place the angle. To choose
  three points instead, Alt-click the first arm, click the vertex, then the second arm; click
  once more to place the annotation. Angles use the smaller angle between the chosen arms.
- **Radius / Diameter:** click a circular path, then move and click to choose the direction of
  the annotation. Ellipses are rejected because they have no single radius.
- **All selected edges:** select paths or groups, then use this button or
  **Object → Dimensions → Dimension Selected Paths**. Adds up to 1,000 segment dimensions in
  one undo step. Existing dimension groups, hidden art and locked art are skipped.
- **Escape** or switching tools cancels a measurement being placed.

Double-click the tool or choose **Options…** for units, precision (0–6 decimal places), scale,
line weight/type/colour, arrows and arrow size, extension-line gaps/overshoot/visibility, font
family/style/size/colour, text position/orientation/offset. **Reset** restores defaults in the
dialog; **Cancel** leaves settings unchanged. **Save** stores tool defaults, and optionally
updates selected dimension groups. These changes are undoable; defaults persist in preferences.

Scale means **drawing : real size**, independent of zoom. At **1:10**, a 1 cm line on the page
is labelled 10 cm. At **10:1**, it is labelled 0.1 cm. Presets and custom positive ratios are
supported. Scale affects lengths, not angles or the physical size of text/arrows.

Measurements track source moves, scaling and anchor/handle edits, including undo/redo.
Deleting a source or making it degenerate removes its linked annotations in the same undo
step. Changing a path's anchor count/closedness invalidates indexed references and removes
those annotations rather than attaching them to a different edge. Recreate them after such
topology edits. A circle changed into an ellipse similarly loses its radius/diameter annotation.

Native `.vectorcraft` files retain recipes and links. SVG/PDF/bitmap output contains ordinary
paths and text, so exported dimensions look the same; generic SVG/PDF round trips do not promise
editable measurement links. Ungrouping exposes the generated paths/text as ordinary artwork.
Copied/pasted dimensions retain their measured geometry but detach their original links, so
source ids cannot accidentally bind to unrelated objects in another document.

## Commands

- `dimension.create`: fixed points or linked segment/angle/circle/anchor sources, mode, offset,
  and style options; returns the group id and label.
- `dimension.selection`: all selected path segments, one undo step.
- `dimension.update`: change existing groups' options and/or placement offset.
- `tool.select {"tool":"dimension"}` and
  `tool.setOption {"tool":"dimension","values":{"mode":"linear","style":{"units":"cm","scale":"1:10"}}}`.
- `tool.options {"tool":"dimension"}` opens the agent-accessible `dimensionOptions` dialog.

Recipes live on normal document groups (`Node.dimension`); rendering and export use their
generated children. Commands validate options and finite bounds before mutation. Source edits
refresh annotations inside `Session::edit`, sharing history and cancellation with the source.
The icon uses the project's existing original measurement artwork; no external assets are added.
