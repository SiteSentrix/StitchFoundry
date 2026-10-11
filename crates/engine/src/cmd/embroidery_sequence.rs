//! Sewing order: branching (a group of objects sewn as one path), breaking a branch
//! or a many-part object apart, and sequencing the whole design.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use sf_core::attr::{Embroidery, StitchType};
use sf_core::sequence::{Item, sequence};
use vectorcraft_doc::{Node, NodeId, NodeKind};
use vectorcraft_embroidery::parts;
use vectorcraft_geom::path::{PathData, SubPath};

use super::embroidery::ids_json;
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.branch",
            "Branch",
            [],
            None,
            "{ids?} sew the selected stitched objects (two or more) as one path: they become a branch group, sewn in the order with the fewest colour changes and the shortest travel, the thread carried from one to the next as running stitches where the ones after cover it (else trimmed as usual) → {id}",
            has_selection,
            branch
        ),
        cmd!(
            "embroidery.breakApart",
            "Break Apart",
            [],
            None,
            "{ids?} separate the selection again: a branch group gives back its objects, and a stitched path or compound path made of separate parts becomes one object per part, each stitched the same way (holes stay with their part) → {ids}",
            has_selection,
            break_apart
        ),
        cmd!(
            "embroidery.autoSequence",
            "Auto Sequence",
            [],
            None,
            "{} reorder the stitched objects (within each layer or group) for the fewest colour changes, then the shortest travel, keeping objects that overlap in their stacking order → {moved}",
            has_doc,
            auto_sequence
        ),
    ]
}

fn is_branch(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { .. }) && n.embroidery.as_deref().is_some_and(|e| matches!(e.stitch, StitchType::Branch))
}

fn branch(s: &mut Session, _p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.branch";
    let roots = super::edit::selected_roots(s)?;
    let stitched = {
        let d = &s.doc()?.doc;
        let mut count = 0usize;
        for id in &roots {
            if let Some(n) = d.node(*id) {
                count += stitched_nodes_under(n);
            }
        }
        count
    };
    if stitched < 2 {
        return Err(bad(CMD, "select two or more stitched objects to sew as one path"));
    }
    let gid = s.edit("Branch", |d, sel| {
        // one selected group of stitched objects becomes the branch; else the selection is grouped
        let single_group = (roots.len() == 1).then(|| roots[0]).filter(|id| d.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Group { .. })));
        let gid = match single_group {
            Some(g) => g,
            None => super::object::group_nodes(d, &roots)?,
        };
        d.node_mut(gid).ok_or(EngineError::NoNode(gid))?.embroidery = Some(Box::new(Embroidery::new(StitchType::Branch)));
        sel.set([gid]);
        Ok(gid)
    })?;
    Ok(json!({ "id": gid.0 }))
}

/// How many stitched objects `n` is or holds (a branch group counts its members).
fn stitched_nodes_under(n: &Node) -> usize {
    if n.embroidery.is_some() && !matches!(n.kind, NodeKind::Group { .. } | NodeKind::Layer { .. }) {
        return 1;
    }
    n.children().into_iter().flatten().map(|c| stitched_nodes_under(c)).sum()
}

/// The parts of a stitched path or compound path: groups of its subpaths, an outer
/// outline with its holes each. Fewer than two → nothing to break apart.
fn node_parts(n: &Node) -> Vec<Vec<SubPath>> {
    let subs: Vec<SubPath> = match &n.kind {
        NodeKind::Path { path, .. } => path.subpaths.clone(),
        NodeKind::Compound { children, .. } => children
            .iter()
            .flat_map(|c| match &c.kind {
                NodeKind::Path { path, .. } => path.subpaths.clone(),
                _ => Vec::new(),
            })
            .collect(),
        _ => return Vec::new(),
    };
    let refs: Vec<&SubPath> = subs.iter().collect();
    let owner = parts(&refs);
    let mut groups: BTreeMap<usize, Vec<SubPath>> = BTreeMap::new();
    for (sp, o) in subs.iter().zip(owner) {
        groups.entry(o).or_default().push(sp.clone());
    }
    groups.into_values().collect()
}

fn break_apart(s: &mut Session, _p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.breakApart";
    let roots = super::edit::selected_roots(s)?;
    // what each root becomes: a branch's members, or the parts of a many-part object
    let mut plans: Vec<(NodeId, Option<Vec<Vec<SubPath>>>)> = Vec::new();
    {
        let d = &s.doc()?.doc;
        for id in &roots {
            let Some(n) = d.node(*id) else { continue };
            if is_branch(n) {
                plans.push((*id, None));
            } else if n.embroidery.is_some() {
                let p = node_parts(n);
                if p.len() >= 2 {
                    plans.push((*id, Some(p)));
                }
            }
        }
    }
    if plans.is_empty() {
        return Err(bad(CMD, "select a branch, or a stitched object made of separate parts"));
    }
    let out = s.edit("Break Apart", |d, sel| {
        let mut out = Vec::new();
        for (id, parts) in &plans {
            let n = d.node(*id).cloned().ok_or(EngineError::NoNode(*id))?;
            let (par, idx, _) = d.position(*id).ok_or(EngineError::NoNode(*id))?;
            d.remove(*id)?;
            match parts {
                // a branch: its members take its place
                None => {
                    for (k, c) in n.children().into_iter().flatten().enumerate() {
                        let mut c = (**c).clone();
                        c.opacity *= n.opacity;
                        out.push(c.id);
                        d.insert(par, idx + k, c)?;
                    }
                }
                // one path per part, looking and stitched like the whole
                Some(parts) => {
                    for (k, subs) in parts.iter().enumerate() {
                        let pid = d.alloc_id();
                        let mut c = Node::path(pid, PathData::new(subs.clone()), n.appearance.clone());
                        if let NodeKind::Path { rule, .. } = &mut c.kind {
                            *rule = vectorcraft_geom::FillRule::EvenOdd;
                        }
                        c.name = n.name.clone();
                        c.opacity = n.opacity;
                        c.embroidery = n.embroidery.clone().map(|mut e| {
                            // where sewing starts and stops belonged to the whole
                            e.entry = None;
                            e.exit = None;
                            e.frozen = None;
                            e
                        });
                        out.push(pid);
                        d.insert(par, idx + k, c)?;
                    }
                }
            }
        }
        sel.set(out.iter().copied());
        Ok(out)
    })?;
    Ok(ids_json(&out))
}

fn auto_sequence(s: &mut Session, _p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.autoSequence";
    let plan = s.embroidery_plan().ok_or_else(|| bad(CMD, "no document"))?;
    // each stitched node as one item: its thread and what its objects cover
    let mut items: BTreeMap<NodeId, Item> = BTreeMap::new();
    for (k, obj) in plan.design.objects.iter().enumerate() {
        let Some(node) = plan.nodes.get(k) else { continue };
        let area = sf_core::engine::object_polygons(obj);
        let bbox = obj.bbox();
        let e = items.entry(*node).or_insert_with(|| Item { color: obj.color, area: Vec::new(), bbox });
        e.area.extend(area);
        e.bbox = e.bbox.union(bbox);
    }
    let mut moved = 0usize;
    s.edit("Auto Sequence", |d, _| {
        // siblings are reordered among themselves, parent by parent
        let mut parents: Vec<Option<NodeId>> = items.keys().map(|id| d.parent_of(*id)).collect();
        parents.sort_by_key(|p| p.map(|x| x.0));
        parents.dedup();
        for parent in parents.into_iter().flatten() {
            let Some(children) = d.node(parent).and_then(|n| n.children()).cloned() else { continue };
            let slots: Vec<usize> = children.iter().enumerate().filter(|(_, c)| items.contains_key(&c.id)).map(|(i, _)| i).collect();
            if slots.len() < 2 {
                continue;
            }
            let list: Vec<Item> = slots.iter().filter_map(|&i| items.get(&children[i].id).cloned()).collect();
            let order = sequence(&list, None);
            moved += order.iter().enumerate().filter(|(i, k)| i != *k).count();
            // the stitched objects trade places; anything else stays where it is
            let nodes: Vec<Arc<Node>> = order.iter().filter_map(|&k| slots.get(k).map(|&i| children[i].clone())).collect();
            let ch = d.children_mut(Some(parent))?;
            for (&slot, node) in slots.iter().zip(nodes) {
                ch[slot] = node;
            }
        }
        Ok(())
    })?;
    Ok(json!({ "moved": moved }))
}
