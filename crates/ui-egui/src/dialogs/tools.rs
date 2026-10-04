//! Dialogs a tool click opens: shape sizes, transform values, graph size and data, flare options
//! and artboard options.

use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::state::Dialog;

/// A click with a shape tool opens its size dialog.
pub fn open_tool_dialog(app: &mut VectorcraftApp, kind: &str, p: Value) {
    let x = p.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = p.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    let d = match kind {
        "rectangle" | "ellipse" => Dialog::new(kind, json!({"x": x, "y": y, "width": "100 pt", "height": "100 pt"})),
        "roundedRectangle" => Dialog::new(kind, json!({"x": x, "y": y, "width": "100 pt", "height": "100 pt", "radius": "12 pt"})),
        "polygon" => Dialog::new(kind, json!({"x": x, "y": y, "radius": "50 pt", "sides": 6})),
        "star" => Dialog::new(kind, json!({"x": x, "y": y, "radius1": "50 pt", "radius2": "25 pt", "points": 5})),
        "lineSegment" => Dialog::new(kind, json!({"x": x, "y": y, "length": "100 pt", "angle": 0})),
        // Graph tool click: the graph's size.
        "graph" => Dialog::new(
            "command",
            json!({"__command": "graph.create", "__label": "Graph", "type": p.get("type").cloned().unwrap_or(json!("column")), "x": x, "y": y, "width": 200, "height": 150}),
        ),
        // After drawing a graph: Graph Data (CSV: header row of series, then category, values…).
        "graphData" => match app.session.execute("graph.setData", &json!({})) {
            Ok(v) => Dialog::new("command", json!({"__command": "graph.setData", "__label": "Graph Data", "csv": v["csv"]})),
            Err(_) => return,
        },
        // Flare Tool Options (Center / Halo / Rays / Rings), applied through the generic command dialog.
        "flare" => Dialog::new(
            "command",
            json!({"__command": "shape.flare", "__label": "Flare Tool Options", "cx": x, "cy": y, "diameter": 100, "opacity": 50, "brightness": 30,
                "growth": 20, "fuzziness": 50, "rays": 15, "longest": 300, "rayFuzziness": 100, "pathLength": 300, "rings": 10, "largest": 50, "direction": 45}),
        ),
        "rotate" | "reflect" | "scale" | "shear" | "artboardOptions" => {
            let mut base = match kind {
                "rotate" => json!({"angle": 0}),
                "reflect" => json!({"axis": "vertical"}),
                "scale" => json!({"sx": 100, "sy": 100, "uniform": true}),
                "shear" => json!({"angle": 0, "axis": "horizontal"}),
                _ => json!({}),
            };
            if let (Some(b), Some(o)) = (base.as_object_mut(), p.as_object()) {
                for (k, v) in o {
                    b.insert(k.clone(), v.clone());
                }
            }
            Dialog::new(kind, base)
        }
        _ => return,
    };
    app.ui.dialog = Some(d);
}
