//! Edit → Edit Colors → Recolor Artwork: list the selection's colours and remap them.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, keep_model};
use vectorcraft_doc::swatches::node_colors;
use vectorcraft_doc::{AppearanceItem, NodeKind};

use super::colorcmds::{Recolor, Scope};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "recolor.colors",
            "Artwork Colors",
            [],
            None,
            "{} → {colors: [{hex, count}]} unique colours used by the selection (fills, strokes, gradient stops, text, mesh points and the tiles of pattern fills and strokes), most used first",
            has_selection,
            colors
        ),
        cmd!(
            "recolor.apply",
            "Recolor Artwork",
            ["Edit", "Edit Colors"],
            None,
            "{map: {\"#rrggbb\": \"#rrggbb\", …}, includeImages?: true (pixels of those colours in embedded images too; the image becomes a recoloured copy), includePatterns?: true (a pattern fill or stroke gets a recoloured copy as a new pattern swatch; the original pattern stays)} replace colours across the selection (gradients, text and meshes included; each new colour keeps the model of the one it replaces) as one undo step → {changed}",
            has_selection,
            apply
        ),
    ]
}

fn colors(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let d = &st.doc;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut count = |c: &Color, _: Option<&str>| *counts.entry(c.to_hex()).or_default() += 1;
    // The patterns used as fills or strokes: their tiles count once each.
    let mut patterns: Vec<&str> = vec![];
    for n in st.selection.objects.iter().filter_map(|id| d.node(*id)) {
        node_colors(n, &mut count);
        n.walk(&mut |m| {
            let runs = match &m.kind {
                NodeKind::Text(t) => t.runs.as_slice(),
                _ => &[],
            };
            let items = m.appearance.items.iter().map(|it| match it {
                AppearanceItem::Fill(l) => &l.paint,
                AppearanceItem::Stroke(l) => &l.paint,
            });
            for p in items.chain(runs.iter().flat_map(|r| [&r.style.fill, &r.style.stroke])) {
                if let Paint::Pattern { pattern, .. } = p
                    && !patterns.contains(&pattern.as_str())
                {
                    patterns.push(pattern);
                }
            }
        });
    }
    for def in patterns.iter().filter_map(|p| d.pattern(p)) {
        def.art.iter().for_each(|n| node_colors(n, &mut count));
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
    let f = |c: Color| map.get(&c.to_hex()).map_or(c, |to| keep_model(c, *to));
    let scope = Scope { fill: true, stroke: true, ..Scope::of(p) };
    let changed = s.edit("Recolor Artwork", |d, _| Ok(Recolor::new(scope, &f).run(d, &ids)))?;
    Ok(json!({ "changed": changed }))
}
