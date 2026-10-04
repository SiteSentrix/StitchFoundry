//! Edit → Edit Colors → Recolor Artwork: list the selection's colours and remap them.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, keep_model};
use vectorcraft_doc::{AppearanceItem, Node, NodeKind};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "recolor.colors", "Artwork Colors", [], None, "{} → {colors: [{hex, count}]} unique colours used by the selection (fills, strokes, gradient stops, text)", has_selection, colors),
        cmd!(
            "recolor.apply",
            "Recolor Artwork",
            ["Edit", "Edit Colors"],
            None,
            "{map: {\"#rrggbb\": \"#rrggbb\", …}} replace colours across the selection (gradients and text included); each new colour keeps the model of the one it replaces",
            has_selection,
            apply
        ),
    ]
}

fn visit_paints(n: &mut Node, f: &mut impl FnMut(&mut Color)) {
    let mut paint = |p: &mut Paint| match p {
        Paint::Solid { color, .. } => f(color),
        Paint::Gradient(g) => {
            for s in &mut g.gradient.stops {
                f(&mut s.color);
            }
        }
        _ => {}
    };
    for it in &mut n.appearance.items {
        match it {
            AppearanceItem::Fill(fl) => paint(&mut fl.paint),
            AppearanceItem::Stroke(st) => paint(&mut st.paint),
        }
    }
    if let NodeKind::Text(t) = &mut n.kind {
        for r in &mut t.runs {
            paint(&mut r.style.fill);
            paint(&mut r.style.stroke);
        }
    }
    if let Some(ch) = n.children_mut() {
        for c in ch.iter_mut() {
            visit_paints(std::sync::Arc::make_mut(c), f);
        }
    }
}

fn colors(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for id in &st.selection.objects {
        if let Some(n) = st.doc.node(*id) {
            let mut n = n.clone();
            visit_paints(&mut n, &mut |c| *counts.entry(c.to_hex()).or_default() += 1);
        }
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(json!({ "colors": v.iter().map(|(h, n)| json!({"hex": h, "count": n})).collect::<Vec<_>>() }))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let map: BTreeMap<String, Color> = p
        .get("map")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("recolor.apply", "missing map"))?
        .iter()
        .filter_map(|(k, v)| Some((Color::from_hex(k)?.to_hex(), color_value(v)?)))
        .collect();
    let ids = s.doc()?.selection.objects.clone();
    let mut changed = 0usize;
    s.edit("Recolor Artwork", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                visit_paints(n, &mut |c| {
                    if let Some(to) = map.get(&c.to_hex()) {
                        *c = keep_model(*c, *to);
                        changed += 1;
                    }
                });
            }
        }
        Ok(())
    })?;
    Ok(json!({ "changed": changed }))
}
