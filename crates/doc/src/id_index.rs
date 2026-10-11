//! A cache of every node's index path, so a lookup by id checks one path instead of walking the
//! tree.
//!
//! The tree is edited in many places through public fields, so the cache can't be kept in step
//! with it. It is checked instead: a cached path is used only while the node at that path still
//! has the id (a check as deep as the tree). A path that no longer checks out rebuilds the cache,
//! and an id the cache doesn't have (deleted, or added since) is looked up by walking, as before.
//! An id that appears more than once (a damaged file) is never cached, so lookups keep returning
//! its first place in tree order.
//!
//! The one case a cached lookup can answer differently from a walk: a second node with an
//! existing id put in the tree *after* the cache was built, earlier in tree order than the first.
//! The lookup then returns the original node (also one with that id) instead of the new one. The
//! app never makes such a node (paste and duplicate give new ids, undo restores whole trees);
//! duplicated ids come from damaged files, which are cached afresh when opened.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{Node, NodeId};

/// Each id's index path; `None` for an id that appears more than once.
pub(crate) type Paths = HashMap<NodeId, Option<Vec<usize>>>;

/// Ids found by walking because the cache didn't have them, before the cache is rebuilt: each
/// rebuild costs a few walks, so rebuilding only after a few keeps a lookup's cost within a small
/// factor of a walk whatever the edit pattern (say, add an object, look it up, again and again).
const MISSES_PER_REBUILD: u32 = 4;

/// The cache, shared by clones of a document (each checks every path it uses against its own tree).
#[derive(Default)]
pub struct IdIndex(RwLock<Option<Arc<Paths>>>, std::sync::atomic::AtomicU32);

impl IdIndex {
    /// The current cache, if one was built.
    pub(crate) fn get(&self) -> Option<Arc<Paths>> {
        self.0.read().ok().and_then(|g| g.clone())
    }

    /// Rebuild the cache from `layers` (one walk) and return it.
    pub(crate) fn rebuild(&self, layers: &[Arc<Node>]) -> Arc<Paths> {
        fn walk(nodes: &[Arc<Node>], path: &mut Vec<usize>, out: &mut Paths) {
            for (i, n) in nodes.iter().enumerate() {
                path.push(i);
                out.entry(n.id).and_modify(|e| *e = None).or_insert_with(|| Some(path.clone()));
                if let Some(ch) = n.children() {
                    walk(ch, path, out);
                }
                path.pop();
            }
        }
        let mut out = Paths::new();
        walk(layers, &mut Vec::new(), &mut out);
        let out = Arc::new(out);
        if let Ok(mut g) = self.0.write() {
            *g = Some(out.clone());
        }
        self.1.store(0, std::sync::atomic::Ordering::Relaxed);
        out
    }

    /// Note an id found by walking that the cache didn't have; true when it's time to rebuild.
    pub(crate) fn missed(&self) -> bool {
        self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 >= MISSES_PER_REBUILD
    }
}

impl Clone for IdIndex {
    fn clone(&self) -> Self {
        IdIndex(RwLock::new(self.get()), Default::default())
    }
}

/// A cache, not document content: two documents are equal whatever their caches hold.
impl PartialEq for IdIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for IdIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdIndex")
    }
}
