//! The batched lookups ([`Document::index_paths`], `node_at`, `node_mut_at`, `ancestry_at`) agree
//! with the one-id ones on nested trees with missing and duplicated ids.
use super::*;
use vectorcraft_geom::shapes;

/// Layers with paths and groups nested `depth` deep; every 7th id is reused (a damaged file), so
/// first-in-tree-order matters.
fn tree(seed: u64) -> (Document, Vec<NodeId>) {
    let mut d = Document::new(612.0, 792.0);
    let mut r = seed | 1;
    let mut next = |n: u64| {
        r ^= r << 13;
        r ^= r >> 7;
        r ^= r << 17;
        r % n
    };
    let mut ids = vec![];
    let leaf = |d: &mut Document, ids: &mut Vec<NodeId>| {
        let id = if ids.len() % 7 == 6 { ids[ids.len() / 2] } else { d.alloc_id() };
        ids.push(id);
        Arc::new(Node::path(id, shapes::rectangle(Rect::new(0.0, 0.0, 5.0, 5.0)), Appearance::default_art()))
    };
    for l in 0..3 {
        let lid = d.alloc_id();
        let mut layer = Node::layer(lid, &format!("L{l}"), LayerColor::Preset(0));
        ids.push(lid);
        for _ in 0..(5 + next(10)) {
            let child = if next(3) == 0 {
                let mut inner: Vec<Arc<Node>> = (0..(1 + next(4))).map(|_| leaf(&mut d, &mut ids)).collect();
                for _ in 0..next(3) {
                    let gid = d.alloc_id();
                    ids.push(gid);
                    inner = vec![Arc::new(Node::group(gid, inner))];
                }
                let gid = d.alloc_id();
                ids.push(gid);
                Arc::new(Node::group(gid, inner))
            } else {
                leaf(&mut d, &mut ids)
            };
            if let Some(ch) = layer.children_mut() {
                ch.push(child);
            }
        }
        d.layers.push(Arc::new(layer));
    }
    ids.push(NodeId(999_999)); // not in the tree
    (d, ids)
}

#[test]
fn batched_lookups_match_one_at_a_time() {
    for seed in 1..40 {
        let (d, ids) = tree(seed);
        let paths = d.index_paths(ids.iter().copied());
        for id in &ids {
            assert_eq!(paths.get(id).cloned(), d.index_path(*id), "seed {seed} id {id:?}");
            let p = paths.get(id);
            assert_eq!(p.and_then(|p| d.node_at(p)).map(|n| n as *const Node), d.node(*id).map(|n| n as *const Node), "seed {seed} node {id:?}");
            assert_eq!(p.and_then(|p| d.ancestry_at(p)), d.ancestry(*id), "seed {seed} ancestry {id:?}");
        }
        // A subset, and nothing.
        let some: Vec<NodeId> = ids.iter().copied().step_by(3).collect();
        let sub = d.index_paths(some.iter().copied());
        assert!(some.iter().all(|id| sub.get(id).cloned() == d.index_path(*id)));
        assert!(d.index_paths(std::iter::empty()).is_empty());
        // Mutable access reaches the same node as node_mut, copy-on-write.
        let mut a = d.clone();
        let mut b = d.clone();
        for id in ids.iter().step_by(5) {
            if let Some(n) = a.node_mut(*id) {
                n.opacity = 0.25;
            }
            if let Some(n) = d.index_path(*id).and_then(|p| b.node_mut_at(&p)) {
                n.opacity = 0.25;
            }
        }
        assert_eq!(serde_json::to_string(&a).ok(), serde_json::to_string(&b).ok(), "seed {seed}");
    }
}
