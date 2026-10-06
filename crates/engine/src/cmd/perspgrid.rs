//! View → Perspective Grid → Define Grid: `perspective.grid.get` reads the grid (the model, the
//! Define Grid fields and the station point) and `perspective.grid.define` sets it from those
//! fields. The model and the camera maths live in `vectorcraft_tools::distort::perspective`.

use serde_json::{Value, json};
use vectorcraft_tools::distort::perspective::{GridDefinition, PerspectiveGrid};

use super::distortcmds::{grid_of, store_grid};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "perspective.grid.get",
            "Perspective Grid",
            [],
            None,
            "{} → {grid: the model (as perspective.grid.set takes it), define: the Define Grid fields (as perspective.grid.define takes them), station: {x, y, distance} (the viewer: centre of vision on the horizon, distance to the picture plane, pt), defined: false while the document uses the default grid}",
            has_doc,
            grid_get
        ),
        cmd!(
            "perspective.grid.define",
            "Define Perspective Grid",
            [],
            None,
            "{name?: preset name (\"\" = custom), kind?: 1|2|3, units?: unit name (points, pixels, inches, centimeters, millimeters…), scale?: [artboard, real world], gridline?: gridline every, angle?: viewing angle (0–90°), distance?: viewing distance, horizonHeight?: above the ground level, thirdVp?: [x right, y up] from the centre of vision (3-point), leftColor?, rightColor?, groundColor?: \"#rrggbb\", opacity?: 0–100} lengths are real-world lengths in `units` at `scale`; missing fields keep the grid's (see perspective.grid.get define). Only what changes changes: a new type, angle or distance moves the vanishing points around the station point. One undo step → the new define fields",
            has_doc,
            grid_define
        ),
    ]
}

/// What `perspective.grid.get` reports for `g`.
pub(crate) fn grid_info(doc: &vectorcraft_doc::Document, g: &PerspectiveGrid) -> Value {
    let st = g.station();
    json!({
        "grid": g.definition_json(),
        "define": g.definition(),
        "station": {"x": st.x, "y": g.horizon, "distance": st.distance},
        "defined": PerspectiveGrid::from_doc(doc).is_some(),
    })
}

fn grid_get(s: &mut Session, _: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    Ok(grid_info(doc, &grid_of(doc)))
}

fn grid_define(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "perspective.grid.define";
    let old = grid_of(&s.doc()?.doc);
    let def: GridDefinition = old.definition().merged(p).map_err(|e| bad(C, e))?;
    let mut g = old.with_definition(&def).map_err(|e| bad(C, e))?;
    // A changed grid is no longer the preset it came from (unless it was given a name).
    if def.name == old.name && !def.same(&old.definition()) {
        g.name.clear();
    }
    if g != old {
        s.edit("Define Perspective Grid", |d, _| {
            store_grid(d, &g);
            Ok(())
        })?;
    }
    Ok(json!(g.definition()))
}
