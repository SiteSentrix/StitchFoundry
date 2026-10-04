//! The Appearance panel: fill and stroke items, the active item that paint, stroke, gradient,
//! transparency and effect edits target, live effects, Clear / Reduce to Basic, and the
//! Eyedropper's appearance copy.
//!
//! **Item targeting.** Commands that edit a fill or stroke take `item?`: a paint-order index into
//! the appearance stack (`items[0]` is painted first, the bottom row of the panel). Omitted, they
//! edit the Appearance panel's active item (`appearance.setActiveItem`) when one is set for the
//! current selection and no `ids` are given, else the topmost fill or stroke; `null` always means
//! the topmost one. A targeted item lives in the selected objects' own stacks (a group's own fill,
//! not its contents'), so item edits apply to the selected objects themselves.

use serde_json::{Value, json};
use vectorcraft_color::{BlendMode, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, FillLayer, Node, NodeKind, StrokeLayer};

use super::edit::selected_roots;
use super::paint::paint_from;
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "appearance.addFill",
            "Add New Fill",
            ["Window", "Appearance"],
            None,
            "{ids?} add a fill on top of each selected object's own stack",
            has_selection,
            |s, p| add_item(s, p, true)
        ),
        cmd!(
            "appearance.addStroke",
            "Add New Stroke",
            ["Window", "Appearance"],
            None,
            "{ids?} add a stroke on top of each selected object's own stack",
            has_selection,
            |s, p| add_item(s, p, false)
        ),
        cmd!(
            "appearance.clear",
            "Clear Appearance",
            ["Window", "Appearance"],
            None,
            "{ids?} leave each object one empty fill and stroke (None; a group none of its own) and reset its opacity and blend mode",
            has_selection,
            clear_appearance
        ),
        cmd!(
            "appearance.reduceToBasic",
            "Reduce to Basic Appearance",
            ["Window", "Appearance"],
            None,
            "{ids?} keep only the topmost visible fill and stroke, without their own opacity, blend mode or effects (a group keeps no rows it lacked), and drop the object's effects",
            has_selection,
            reduce_basic
        ),
        cmd!(
            "appearance.setItem",
            "Appearance Item",
            [],
            None,
            "{index: paint-order item index, opacity?: 0..1 (values above 1 are percent), blend?: name, visible?: bool, weight?: pt (strokes), color?|none?|swatch?|gradient? (as paint.setFill), ids?} edit one fill/stroke of each selected object's own appearance stack",
            has_selection,
            set_item
        ),
        cmd!("appearance.removeItem", "Remove Item", [], None, "{index, ids?}", has_selection, remove_item),
        cmd!(
            "appearance.addEffect",
            "Add Effect",
            [],
            None,
            "same as effect.apply (an alias): {effect: id, params?: {…}, item?: appearance item index|null, ids?} → {ids, index, item}",
            has_selection,
            super::effectcmd::apply
        ),
        cmd!("appearance.duplicateItem", "Duplicate Item", ["Window", "Appearance"], None, "{index, ids?}", has_selection, duplicate_item),
        cmd!("appearance.moveItem", "Reorder Appearance Item", [], None, "{from, to, ids?} (paint-order indices)", has_selection, move_item),
        cmd!(
            "appearance.copyFrom",
            "Eyedropper",
            [],
            None,
            "{source: id, ids?} copy fill, stroke, weight, opacity and blend from `source` to ids (default: selection) and to the paint defaults",
            has_doc,
            copy_from
        ),
        cmd!(
            "appearance.setActiveItem",
            "Select Appearance Item",
            [],
            None,
            "{index: paint-order item index in the first selected object's stack | null} make that fill/stroke row the target of the paint.setFill/setStroke, stroke.set/setAdvanced, paint.editGradient/setGradientGeom, transparency.set and effect.* calls that omit `item` (a fill row brings the Fill proxy forward, a stroke row the Stroke proxy); null or any selection change clears it → {index}",
            has_selection,
            set_active_item
        ),
    ]
}

/// The Appearance panel's active row, remembered for the selection it was chosen in.
#[derive(Clone, Debug)]
pub(crate) struct ActiveItem {
    doc: u64,
    objects: Vec<NodeId>,
    index: usize,
}

impl Session {
    /// The Appearance panel's active fill/stroke: a paint-order index into the first selected
    /// object's stack, while the selection it was chosen for is unchanged.
    pub fn appearance_item(&self) -> Option<usize> {
        let a = self.active_appearance_item.as_ref()?;
        let st = self.active()?;
        if st.uid != a.doc || st.selection.objects != a.objects {
            return None;
        }
        let n = st.doc.node(*a.objects.first()?)?;
        (a.index < n.appearance.items.len()).then_some(a.index)
    }

    /// Re-point the active item after an edit of `ids` changed its stack (`None` drops it). Edits
    /// of other objects leave it alone.
    fn remap_appearance_item(&mut self, ids: &[NodeId], f: impl FnOnce(usize) -> Option<usize>) {
        if let Some(a) = &mut self.active_appearance_item
            && a.objects.first().is_some_and(|o| ids.contains(o))
        {
            match f(a.index) {
                Some(i) => a.index = i,
                None => self.active_appearance_item = None,
            }
        }
    }
}

/// Which fill or stroke of an appearance stack an edit changes (the `item` param).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemTarget {
    /// The topmost fill or stroke (for effects: the object-level effects).
    Top,
    /// Item `index`. `explicit`: named by the `item` param, so it must exist and be of the edited
    /// kind; otherwise it is the panel's active item, which applies only where it fits.
    Item { index: usize, explicit: bool },
}

/// Parse the `item` param of `cmd` (see the module docs).
pub(crate) fn item_target(s: &Session, p: &Value, cmd: &str) -> Result<ItemTarget> {
    match p.get("item") {
        Some(Value::Null) => Ok(ItemTarget::Top),
        Some(v) => v
            .as_u64()
            .map(|i| ItemTarget::Item { index: i as usize, explicit: true })
            .ok_or_else(|| bad(cmd, "`item` must be an appearance item index (paint order) or null")),
        None if p.get("ids").is_some() || p.get("id").is_some() => Ok(ItemTarget::Top),
        None => Ok(s.appearance_item().map_or(ItemTarget::Top, |index| ItemTarget::Item { index, explicit: false })),
    }
}

impl ItemTarget {
    /// Objects a fill/stroke edit changes: the selected objects' own stacks when an item is
    /// targeted, else the painted leaves ([`paint_targets`]).
    pub(crate) fn targets(self, s: &Session, p: &Value) -> Result<Vec<NodeId>> {
        match self {
            ItemTarget::Top => paint_targets(s, p),
            ItemTarget::Item { .. } => appearance_targets(s, p),
        }
    }

    /// The target of a fill (`fill`) or stroke edit: the panel's active item stands only for edits
    /// of its own kind, so a fill row leaves stroke edits on the topmost stroke of the painted
    /// leaves (as without an active item) and vice versa. Explicit items are kept (and checked).
    pub(crate) fn of_kind(self, s: &Session, fill: bool) -> Self {
        let ItemTarget::Item { index, explicit: false } = self else { return self };
        let fits = s.active().and_then(|st| st.doc.node(*st.selection.objects.first()?)?.appearance.items.get(index).map(|it| it.is_fill() == fill));
        if fits == Some(true) { self } else { ItemTarget::Top }
    }

    /// The item of `ap` a fill (`fill`) or stroke edit changes; `None` = the topmost one.
    pub(crate) fn resolve(self, ap: &Appearance, fill: bool, cmd: &str) -> Result<Option<usize>> {
        match self {
            ItemTarget::Top => Ok(None),
            ItemTarget::Item { index, explicit } => match ap.item_of_kind(Some(index), fill) {
                None if explicit => Err(bad(cmd, format!("appearance item {index} is not a {}", if fill { "fill" } else { "stroke" }))),
                found => Ok(found),
            },
        }
    }

    /// The item whose own effects an effect edit changes; `None` = the object-level effects.
    pub(crate) fn effects_item(self, ap: &Appearance, cmd: &str) -> Result<Option<usize>> {
        match self {
            ItemTarget::Top => Ok(None),
            ItemTarget::Item { index, .. } if index < ap.items.len() => Ok(Some(index)),
            ItemTarget::Item { index, explicit: true } => Err(bad(cmd, format!("no appearance item {index}"))),
            ItemTarget::Item { explicit: false, .. } => Ok(None),
        }
    }
}

/// Objects whose own appearance stack a command edits: `ids`/`id`, else the selected objects
/// (a selected compound-path member stands for its compound). Layers are skipped.
pub(crate) fn appearance_targets(s: &Session, p: &Value) -> Result<Vec<NodeId>> {
    if p.get("ids").is_none() && p.get("id").is_none() {
        return selected_roots(s);
    }
    let d = &s.doc()?.doc;
    Ok(targets(s, p)?.into_iter().filter(|id| d.node(*id).is_some_and(|n| !n.is_layer())).collect())
}

/// One undo step (`label`) running `f` on each of `ids` with the item a fill (`fill`) or stroke
/// edit aimed at `item` changes there (`None`: the topmost one; text then edits its characters).
pub(crate) fn edit_items(
    s: &mut Session,
    ids: &[NodeId],
    item: ItemTarget,
    cmd: &str,
    label: &str,
    fill: bool,
    mut f: impl FnMut(&mut Node, Option<usize>) -> Result<()>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    s.edit(label, |d, _| {
        for id in ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let index = item.resolve(&n.appearance, fill, cmd)?;
            f(n, index)?;
        }
        Ok(())
    })
}

/// Whether an edit aimed at `item` changes a stroke: the `stroke` param, else the kind of the
/// targeted item (on the first target object), else `default`.
pub(crate) fn edits_stroke(s: &Session, p: &Value, item: ItemTarget, default: bool) -> Result<bool> {
    if let Some(b) = p.get("stroke").and_then(Value::as_bool) {
        return Ok(b);
    }
    let ItemTarget::Item { index, .. } = item else { return Ok(default) };
    let ids = appearance_targets(s, p)?;
    let d = &s.doc()?.doc;
    Ok(ids.first().and_then(|id| d.node(*id)?.appearance.items.get(index)).map_or(default, |it| !it.is_fill()))
}

pub(crate) fn index_param(p: &Value, key: &str, cmd: &str) -> Result<usize> {
    p.get(key).and_then(Value::as_u64).map(|i| i as usize).ok_or_else(|| bad(cmd, format!("missing integer `{key}`")))
}

fn set_active_item(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "appearance.setActiveItem";
    let index = match p.get("index") {
        Some(Value::Null) => {
            s.active_appearance_item = None;
            return Ok(json!({ "index": Value::Null }));
        }
        Some(_) => index_param(p, "index", C)?,
        None => return Err(bad(C, "missing `index` (an item index or null)")),
    };
    let st = s.doc()?;
    let objects = st.selection.objects.clone();
    let first = objects.first().and_then(|id| st.doc.node(*id)).ok_or_else(|| EngineError::Other("nothing selected".into()))?;
    let fill = first.appearance.items.get(index).ok_or_else(|| bad(C, format!("no appearance item {index}")))?.is_fill();
    let doc = st.uid;
    s.fill_active = fill;
    s.active_appearance_item = Some(ActiveItem { doc, objects, index });
    Ok(json!({ "index": index }))
}

fn add_item(s: &mut Session, p: &Value, fill: bool) -> Result<Value> {
    let ids = appearance_targets(s, p)?;
    s.edit(if fill { "Add New Fill" } else { "Add New Stroke" }, |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                let item = if fill {
                    AppearanceItem::Fill(FillLayer::new(n.appearance.fill_paint()))
                } else {
                    AppearanceItem::Stroke(StrokeLayer::new(n.appearance.stroke_paint(), n.appearance.stroke_width().max(1.0)))
                };
                n.appearance.items.push(item);
            }
        }
        Ok(())
    })?;
    ok()
}

fn clear_appearance(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = appearance_targets(s, p)?;
    s.edit("Clear Appearance", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                // No fill, no stroke, and the object's Opacity row back to Default. A group keeps no
                // fill or stroke rows of its own.
                n.appearance = if is_group(n) { Appearance::default() } else { Appearance::basic(Paint::None, Paint::None, 1.0) };
                n.opacity = 1.0;
                n.blend = BlendMode::Normal;
            }
        }
        Ok(())
    })?;
    s.active_appearance_item = None;
    ok()
}

fn is_group(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { .. })
}

fn reduce_basic(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = appearance_targets(s, p)?;
    s.edit("Reduce to Basic Appearance", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                // The topmost visible fill and stroke stay, without their own transparency or effects.
                let top = |fill: bool| n.appearance.items.iter().rev().find(|i| i.visible() && i.is_fill() == fill).cloned();
                let (fill, stroke) = (top(true), top(false));
                // Other objects always keep a fill row (None when there was none); a group only
                // keeps the rows it had.
                let fill =
                    (fill.is_some() || !is_group(n)).then(|| AppearanceItem::Fill(FillLayer::new(fill.map_or(Paint::None, |f| f.paint().clone()))));
                n.appearance = Appearance { items: fill.into_iter().collect(), effects: vec![] };
                if let Some(AppearanceItem::Stroke(mut st)) = stroke {
                    st.effects.clear();
                    st.opacity = 1.0;
                    st.blend = BlendMode::Normal;
                    n.appearance.items.push(AppearanceItem::Stroke(st));
                }
            }
        }
        Ok(())
    })?;
    s.active_appearance_item = None;
    ok()
}

fn set_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = index_param(p, "index", "appearance.setItem")?;
    let paint = paint_from(s, p)?;
    let ids = appearance_targets(s, p)?;
    let blend = str_param(p, "blend").and_then(BlendMode::parse);
    s.edit("Appearance", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let Some(item) = n.appearance.items.get_mut(idx) else { continue };
            let (pp, op, bl, vis) = match item {
                AppearanceItem::Fill(f) => (&mut f.paint, &mut f.opacity, &mut f.blend, &mut f.visible),
                AppearanceItem::Stroke(st) => {
                    if let Some(w) = p.get("weight").and_then(Value::as_f64) {
                        st.width = w.max(0.0);
                    }
                    (&mut st.paint, &mut st.opacity, &mut st.blend, &mut st.visible)
                }
            };
            if let Some(pa) = &paint {
                *pp = pa.clone();
            }
            if let Some(o) = p.get("opacity").and_then(Value::as_f64) {
                *op = if o > 1.0 { o / 100.0 } else { o }.clamp(0.0, 1.0) as f32;
            }
            if let Some(b) = blend {
                *bl = b;
            }
            if let Some(v) = p.get("visible").and_then(Value::as_bool) {
                *vis = v;
            }
        }
        Ok(())
    })?;
    ok()
}

fn remove_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = index_param(p, "index", "appearance.removeItem")?;
    let ids = appearance_targets(s, p)?;
    s.edit("Remove Item", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id)
                && idx < n.appearance.items.len()
            {
                n.appearance.items.remove(idx);
            }
        }
        Ok(())
    })?;
    s.remap_appearance_item(&ids, |a| match a.cmp(&idx) {
        std::cmp::Ordering::Less => Some(a),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(a - 1),
    });
    ok()
}

fn duplicate_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = index_param(p, "index", "appearance.duplicateItem")?;
    let ids = appearance_targets(s, p)?;
    s.edit("Duplicate Item", |d, _| {
        let mut any = false;
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            if let Some(item) = n.appearance.items.get(idx).cloned() {
                n.appearance.items.insert(idx + 1, item);
                any = true;
            }
        }
        if any { Ok(()) } else { Err(bad("appearance.duplicateItem", format!("no item at index {idx}"))) }
    })?;
    s.remap_appearance_item(&ids, |a| Some(if a > idx { a + 1 } else { a }));
    ok()
}

fn move_item(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "appearance.moveItem";
    let from = index_param(p, "from", C)?;
    let to = index_param(p, "to", C)?;
    let ids = appearance_targets(s, p)?;
    let mut landed = to;
    s.edit("Reorder Appearance", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let items = &mut n.appearance.items;
            if from >= items.len() {
                return Err(bad(C, format!("no item at index {from}")));
            }
            let it = items.remove(from);
            landed = to.min(items.len());
            items.insert(landed, it);
        }
        Ok(())
    })?;
    // The moved row stays the active one; the rows it passed shift by one.
    s.remap_appearance_item(&ids, |a| {
        Some(if a == from {
            landed
        } else if from < a && a <= landed {
            a - 1
        } else if landed <= a && a < from {
            a + 1
        } else {
            a
        })
    });
    ok()
}

fn copy_from(s: &mut Session, p: &Value) -> Result<Value> {
    let src_id = id_param(p, "source").ok_or_else(|| bad("appearance.copyFrom", "missing `source` id"))?;
    let src = s.doc()?.doc.node(src_id).cloned().ok_or(EngineError::NoNode(src_id))?;
    let appearance = match &src.kind {
        NodeKind::Text(t) => match t.runs.first() {
            Some(r) => Appearance::basic(r.style.fill.clone(), r.style.stroke.clone(), r.style.stroke_width),
            None => src.appearance.clone(),
        },
        _ => src.appearance.clone(),
    };
    s.paint.fill = appearance.fill_paint();
    s.paint.stroke = appearance.stroke_paint();
    if appearance.stroke().is_some() {
        s.paint.stroke_width = appearance.stroke_width();
    }
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let mut targets = leaf_targets(s, &ids)?;
    targets.retain(|id| *id != src_id);
    if targets.is_empty() {
        return Ok(json!({ "ids": [] }));
    }
    let (opacity, blend) = (src.opacity, src.blend);
    s.edit("Eyedropper", |d, _| {
        for id in &targets {
            let Some(n) = d.node_mut(*id) else { continue };
            n.opacity = opacity;
            n.blend = blend;
            if let NodeKind::Text(t) = &mut n.kind {
                for r in &mut t.runs {
                    r.style.fill = appearance.fill_paint();
                    r.style.stroke = appearance.stroke_paint();
                    r.style.stroke_width = appearance.stroke_width();
                }
                continue;
            }
            n.appearance = appearance.clone();
        }
        Ok(())
    })?;
    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
