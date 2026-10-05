//! What the interpreter doesn't tell the import, read from the file itself: the isolate and
//! knockout flags of transparency groups, and the names of the fonts.
//!
//! hayro-interpret reports a transparency group without its flags. So each page's content is
//! walked here in the interpreter's order (page content, then form XObjects as they are drawn,
//! skipping what it skips, such as optional content that is off): the k-th form transparency
//! group it reports is the k-th group found here.

use std::collections::{HashMap, HashSet};

use hayro_interpret::CacheKey;
use hayro_syntax::Pdf;
use hayro_syntax::content::TypedIter;
use hayro_syntax::content::ops::TypedInstruction;
use hayro_syntax::object::{Array, Dict, Name, Object, ObjectIdentifier, dict_or_stream};
use hayro_syntax::page::{Page, Resources};

/// The interpreter's limit on nested form XObjects.
const MAX_DEPTH: u32 = 50;
/// The most content operators walked per page: past it the rest of the page isn't told apart.
const MAX_OPS: usize = 2_000_000;
/// The most groups read.
const MAX_GROUPS: usize = 10_000;

/// The file's optional content groups.
#[derive(Default)]
pub(crate) struct Ocgs {
    /// The groups that are off (as the interpreter decides it).
    off: HashSet<ObjectIdentifier>,
}

impl Ocgs {
    /// The groups listed in the catalog's `/OCProperties`, with their default states.
    pub fn read(pdf: &Pdf) -> Self {
        let mut o = Self::default();
        let xref = pdf.xref();
        let Some(props) = xref.get::<Dict<'_>>(xref.root_id()).and_then(|c| c.get::<Dict<'_>>(b"OCProperties")) else {
            return o;
        };
        let config = props.get::<Dict<'_>>(b"D").unwrap_or_default();
        let refs = |d: &Dict<'_>, key: &[u8]| -> Vec<ObjectIdentifier> {
            d.get::<Array<'_>>(key)
                .map(|a| a.raw_iter().filter_map(|i| i.as_obj_ref()).map(ObjectIdentifier::from).take(MAX_GROUPS).collect())
                .unwrap_or_default()
        };
        if config.get::<Name<'_>>(b"BaseState").is_some_and(|n| n.as_ref() == b"OFF") {
            o.off.extend(refs(&props, b"OCGs"));
        }
        for id in refs(&config, b"ON") {
            o.off.remove(&id);
        }
        o.off.extend(refs(&config, b"OFF"));
        o
    }
}

/// One page as the interpreter walks it.
#[derive(Default)]
pub(crate) struct Scan {
    /// Each form transparency group: isolated, knockout.
    pub groups: Vec<(bool, bool)>,
}

struct Walker<'w> {
    /// The groups that are off for the interpreter.
    off: &'w HashSet<ObjectIdentifier>,
    fonts: &'w mut HashMap<u128, String>,
    visible: Vec<bool>,
    ops: usize,
    out: Scan,
}

impl Walker<'_> {
    fn is_visible(&self) -> bool {
        self.visible.last().copied().unwrap_or(true)
    }

    fn begin(&mut self, on: bool) {
        let v = self.is_visible() && on;
        self.visible.push(v);
    }

    /// Enter optional content `d` (object `id`): a group, or a membership dictionary whose policy
    /// decides from its groups.
    fn begin_oc(&mut self, d: &Dict<'_>, id: Option<ObjectIdentifier>) {
        let membership = id.is_none() || d.get::<Name<'_>>(b"Type").is_some_and(|t| t.as_ref() == b"OCMD");
        let on = match id {
            Some(id) if !membership => !self.off.contains(&id),
            _ => {
                let ids: Vec<ObjectIdentifier> = match d.get::<Array<'_>>(b"OCGs") {
                    Some(a) => a.raw_iter().filter_map(|i| i.as_obj_ref()).map(ObjectIdentifier::from).collect(),
                    None => d.get_ref(b"OCGs").map(ObjectIdentifier::from).into_iter().collect(),
                };
                let on = |i: &ObjectIdentifier| !self.off.contains(i);
                ids.is_empty()
                    || match d.get::<Name<'_>>(b"P").as_deref() {
                        Some(b"AllOn") => ids.iter().all(on),
                        Some(b"AnyOff") => !ids.iter().all(on),
                        Some(b"AllOff") => !ids.iter().any(on),
                        _ => ids.iter().any(on),
                    }
            }
        };
        self.begin(on);
    }

    fn note_fonts(&mut self, res: &Resources<'_>) {
        for (key, _) in res.fonts.entries() {
            if let Some(f) = res.fonts.get::<Dict<'_>>(key.as_ref())
                && let Some(name) = f.get::<Name<'_>>(b"BaseFont")
            {
                self.fonts.entry(f.cache_key()).or_insert_with(|| String::from_utf8_lossy(name.as_ref()).into_owned());
            }
        }
    }

    fn walk(&mut self, mut iter: TypedIter<'_>, res: &Resources<'_>, depth: u32) {
        self.note_fonts(res);
        while let Some(op) = iter.next() {
            self.ops += 1;
            if self.ops > MAX_OPS {
                return;
            }
            match op {
                TypedInstruction::BeginMarkedContentWithProperties(bdc) => {
                    let oc = match bdc.1 {
                        Object::Name(n) => {
                            res.properties.get_ref(n.as_ref()).map(|r| (res.properties.get::<Dict<'_>>(n.as_ref()).unwrap_or_default(), r))
                        }
                        o => dict_or_stream(o)
                            .and_then(|(props, _)| props.get_ref(b"OC").map(|r| (props.get::<Dict<'_>>(b"OC").unwrap_or_default(), r))),
                    };
                    match oc {
                        Some((d, r)) => self.begin_oc(&d, Some(ObjectIdentifier::from(r))),
                        None => self.begin(true),
                    }
                }
                TypedInstruction::BeginMarkedContent(_) => self.begin(true),
                TypedInstruction::EndMarkedContent(_) => {
                    self.visible.pop();
                }
                TypedInstruction::XObject(x) => {
                    if !self.is_visible() || depth >= MAX_DEPTH {
                        continue;
                    }
                    let Some(s) = res.get_x_object(x.0) else { continue };
                    let d = s.dict();
                    if d.get::<Name<'_>>(b"Subtype").as_deref() != Some(b"Form") || d.get::<[f32; 4]>(b"BBox").is_none() {
                        continue;
                    }
                    let Ok(data) = s.decoded() else { continue };
                    let oc = d.get::<Dict<'_>>(b"OC");
                    if let Some(oc) = &oc {
                        self.begin_oc(oc, d.get_ref(b"OC").map(ObjectIdentifier::from));
                    }
                    if self.is_visible() {
                        if let Some(g) = d.get::<Dict<'_>>(b"Group")
                            && self.out.groups.len() < MAX_GROUPS
                        {
                            self.out.groups.push((g.get::<bool>(b"I").unwrap_or(false), g.get::<bool>(b"K").unwrap_or(false)));
                        }
                        let inner = Resources::from_parent(d.get::<Dict<'_>>(b"Resources").unwrap_or_default(), res.clone());
                        self.walk(TypedIter::new(data.as_ref()), &inner, depth + 1);
                    }
                    if oc.is_some() {
                        self.visible.pop();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Walk `page` as the interpreter will. Fonts found are noted in `fonts` (their cache key → base
/// font name).
pub(crate) fn scan_page(page: &Page<'_>, ocgs: &Ocgs, fonts: &mut HashMap<u128, String>) -> Scan {
    let mut w = Walker { off: &ocgs.off, fonts, visible: vec![], ops: 0, out: Scan::default() };
    w.walk(page.typed_operations(), page.resources(), 0);
    w.out
}
