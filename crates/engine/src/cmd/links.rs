//! Linked images (File → Place with Link): finding their files, telling when they changed, and
//! reading them again.
//!
//! A linked image keeps its file's absolute path, the path relative to the document (written on
//! save), the file's size, modification time and hash ([`LinkInfo`]), and a low-resolution
//! preview that the document saves in place of the pixels ([`ImageBlob::proxy`]). A file is looked
//! for at its path, then at its relative path and by name in the document's folder. When a
//! document opens ([`resolve`]), the files found unchanged are read again, modified ones follow
//! Preferences → File Handling → Update Links, and missing ones show their preview.
//!
//! `links.check` reports, `links.update` reads modified files again, `links.relink` points images
//! at other files.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde_json::{Value, json};
use vectorcraft_doc::links::hash_bytes;
use vectorcraft_doc::{Document, ImageBlob, LinkInfo, NodeId, NodeKind};
use vectorcraft_geom::Affine;

use super::fileio::{self, RasterImage, absolute_path, file_stamp, read_file};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "links.check",
            "Check Links",
            [],
            None,
            "{ids?: [image ids] (default: every linked image)} where each linked file is and whether it changed since it was read → {links: [{name, path, status: ok|modified|missing, ids, found?: path (found away from its path: relative to the document, or by name in its folder), preview: true (showing the low-resolution preview, not the file)}], missing, modified}",
            has_doc,
            check
        ),
        cmd!(
            "links.update",
            "Update Links",
            [],
            None,
            "{ids?: [image ids] (default: the linked images whose file was modified or is showing its preview)} read their linked files again; each image keeps its bounds. One undo step → {updated: [ids], missing: [ids]}",
            has_doc,
            update
        ),
        cmd!(
            "links.relink",
            "Relink",
            [],
            None,
            "{ids?: [image ids] (default: the selected images; with folder and nothing selected, every missing link), path | folder} link images to the file at path, or each to the file of its link's name in folder; embedded images become linked; each keeps its bounds. One undo step → {relinked: [ids], notFound: [file names]}",
            has_doc,
            relink
        ),
    ]
}

// ---------- finding files ----------

/// What became of a linked file since it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Modified,
    Missing,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Modified => "modified",
            Status::Missing => "missing",
        }
    }
}

/// The image objects that show one linked file as one content (same path, same image).
struct Group {
    link: LinkInfo,
    key: String,
    /// The objects in the layers (images in symbols and patterns share the content, not an id).
    ids: Vec<NodeId>,
}

/// The linked images of `d` by file and content: those in the layers (only `ids` when given), then,
/// without `ids`, the ones only symbols and patterns show.
fn groups(d: &Document, ids: Option<&[NodeId]>) -> Vec<Group> {
    let mut out: Vec<Group> = vec![];
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut add = |link: &LinkInfo, key: &str, id: Option<NodeId>| {
        let i = *index.entry((link.path.clone(), key.to_string())).or_insert_with(|| {
            out.push(Group { link: link.clone(), key: key.to_string(), ids: vec![] });
            out.len() - 1
        });
        if let (Some(id), Some(g)) = (id, out.get_mut(i)) {
            g.ids.push(id);
        }
    };
    d.walk(|n| {
        if let NodeKind::Image(im) = &n.kind
            && let Some(l) = &im.link
            && ids.is_none_or(|ids| ids.contains(&n.id))
        {
            add(l, &im.key, Some(n.id));
        }
    });
    if ids.is_none() {
        d.visit_images(|_, im| {
            if let Some(l) = &im.link {
                add(l, &im.key, None);
            }
        });
    }
    out
}

/// A linked file where it was found.
struct Found {
    path: String,
    size: u64,
    modified: Option<u64>,
    /// Read when telling whether it changed needed them.
    bytes: Option<Vec<u8>>,
}

impl Found {
    /// The file's bytes (read now unless already read).
    fn read(&mut self) -> Result<Vec<u8>> {
        match self.bytes.take() {
            Some(b) => Ok(b),
            None => read_file(&self.path),
        }
    }

    /// `base` pointing at this file as read now (`bytes`).
    fn link(&self, bytes: &[u8], base: &LinkInfo) -> LinkInfo {
        LinkInfo { path: self.path.clone(), size: Some(self.size), modified: self.modified, hash: Some(hash_bytes(bytes)), ..base.clone() }
    }
}

/// The link to the file at `path` whose bytes were just read.
pub(crate) fn link_info(path: &str, bytes: &[u8]) -> LinkInfo {
    let path = absolute_path(path);
    LinkInfo {
        size: Some(bytes.len() as u64),
        modified: file_stamp(&path).and_then(|(_, m)| m),
        hash: Some(hash_bytes(bytes)),
        ..LinkInfo::new(path)
    }
}

/// The file at `path` when there is one.
fn found_at(path: String) -> Option<Found> {
    let (size, modified) = file_stamp(&path)?;
    Some(Found { path, size, modified, bytes: None })
}

/// Where the file of `link` is: at its path, else at its relative path or by name in the
/// document's folder `dir`.
fn locate(link: &LinkInfo, dir: Option<&Path>) -> Option<Found> {
    let mut paths = vec![link.path.clone()];
    if let Some(dir) = dir {
        let near = link.relative.iter().map(|r| dir.join(r)).chain([dir.join(link.name())]);
        paths.extend(near.map(|p| p.to_string_lossy().into_owned()));
    }
    paths.into_iter().find_map(found_at)
}

/// Where the file of group `g` is and whether it still has the content shown. Its bytes are read
/// only when its size or modification time changed.
fn probe(g: &Group, dir: Option<&Path>) -> (Status, Option<Found>) {
    let Some(mut f) = locate(&g.link, dir) else { return (Status::Missing, None) };
    let l = &g.link;
    if l.size == Some(f.size) && l.modified.is_some() && l.modified == f.modified {
        return (Status::Ok, Some(f));
    }
    let Ok(bytes) = f.read() else { return (Status::Missing, None) };
    let same = match &l.hash {
        Some(h) => *h == hash_bytes(&bytes),
        // Linked before links kept a hash: the image the file decodes to.
        None => fileio::raster_image(&bytes).is_ok_and(|img| img.key == g.key),
    };
    f.bytes = Some(bytes);
    (if same { Status::Ok } else { Status::Modified }, Some(f))
}

/// The folder of the document saved at `doc_path`.
fn folder(doc_path: Option<&str>) -> Option<&Path> {
    doc_path.and_then(|p| Path::new(p).parent())
}

/// Is image `key` of `d` showing its preview (its file not read)?
fn previewing(d: &Document, key: &str) -> bool {
    d.images.get(key).is_none_or(ImageBlob::is_proxy)
}

// ---------- reading files into the document ----------

/// Keep `full` (a file's pixels) as image `key` of `d`, unless pixels are there already; a
/// preview there is kept, and a linked image gets one.
pub(crate) fn store_image(d: &mut Document, key: &str, full: ImageBlob, linked: bool) {
    let blob = match d.images.get(key) {
        Some(b) if !b.is_proxy() => b.clone(),
        old => ImageBlob { proxy: old.and_then(|b| b.proxy.clone()), ..full },
    };
    d.images.insert(key.to_string(), if linked { blob.with_proxy() } else { blob });
}

/// The file read for a link: the link to it and its image.
type FileRead = (LinkInfo, RasterImage);

/// The file at `f` read for a link like `base` (its image decoded).
fn read_image(f: &mut Found, base: &LinkInfo) -> Result<FileRead> {
    let bytes = f.read()?;
    Ok((f.link(&bytes, base), fileio::raster_image(&bytes)?))
}

/// Image objects `ids` show the file read for `link` (`img`), each keeping its bounds.
fn install(d: &mut Document, ids: &[NodeId], (link, img): FileRead) {
    store_image(d, &img.key, img.blob, true);
    for id in ids {
        if let Some(n) = d.node_mut(*id)
            && let NodeKind::Image(im) = &mut n.kind
        {
            if (im.width, im.height) != (img.width, img.height) {
                im.xf *= Affine::scale_non_uniform(im.width as f64 / img.width as f64, im.height as f64 / img.height as f64);
            }
            (im.key, im.width, im.height, im.link) = (img.key.clone(), img.width, img.height, Some(link.clone()));
        }
    }
}

/// Groups as `{name, path, ids}` rows (those with objects in the layers).
fn rows<'a>(gs: impl Iterator<Item = &'a Group>) -> Vec<Value> {
    gs.filter(|g| !g.ids.is_empty()).map(|g| json!({ "name": g.link.name(), "path": g.link.path, "ids": ids_json(&g.ids) })).collect()
}

fn ids_json(ids: &[NodeId]) -> Vec<u64> {
    ids.iter().map(|id| id.0).collect()
}

/// What [`resolve`] found when a document opened.
#[derive(Default)]
pub struct Resolved {
    /// `{name, path, ids}` of files that weren't found: their images show the preview.
    pub missing: Vec<Value>,
    /// Modified files left as they were (Update Links isn't Automatically).
    pub modified: Vec<Value>,
    /// Modified files read again (Update Links: Automatically).
    pub updated: Vec<Value>,
}

impl Resolved {
    /// The `document.open` result fields.
    pub fn to_json(&self) -> Value {
        json!({ "missingLinks": self.missing, "modifiedLinks": self.modified, "updatedLinks": self.updated })
    }
}

/// The linked files of document `d`, just read from `doc_path`: those found unchanged are read
/// again (their images had the preview), modified ones are read again when `update`, links found
/// away from their path take the new path. Nothing here is an undo step.
pub fn resolve(d: &mut Document, doc_path: Option<&str>, update: bool) -> Resolved {
    let mut out = Resolved::default();
    for g in groups(d, None) {
        let row = rows(std::iter::once(&g));
        match probe(&g, folder(doc_path)) {
            (Status::Missing, _) => out.missing.extend(row),
            (Status::Modified, Some(mut f)) if update => match read_image(&mut f, &g.link) {
                Ok(read) => {
                    install(d, &g.ids, read);
                    out.updated.extend(row);
                }
                Err(_) => out.missing.extend(row),
            },
            (Status::Modified, _) => out.modified.extend(row),
            (Status::Ok, Some(mut f)) => {
                let preview = previewing(d, &g.key);
                // Read when the images show the preview, or when the file's details changed.
                let bytes = if preview || f.bytes.is_some() { f.read().ok() } else { None };
                if preview {
                    let Some(img) = bytes.as_deref().and_then(|b| fileio::raster_image(b).ok()) else {
                        out.missing.extend(row);
                        continue;
                    };
                    store_image(d, &g.key, img.blob, true);
                } else if let Some(b) = d.images.get(&g.key).filter(|b| b.proxy.is_none()) {
                    // Linked before links kept a preview.
                    let b = b.clone().with_proxy();
                    d.images.insert(g.key.clone(), b);
                }
                // Found away from its path, or read again: the link follows.
                let link = match &bytes {
                    Some(b) => f.link(b, &g.link),
                    None => LinkInfo { path: f.path.clone(), ..g.link.clone() },
                };
                if link != g.link {
                    set_links(d, &g.ids, &link);
                }
            }
            (Status::Ok, None) => {}
        }
    }
    out
}

/// Image objects `ids` link to `link`.
fn set_links(d: &mut Document, ids: &[NodeId], link: &LinkInfo) {
    for id in ids {
        if let Some(n) = d.node_mut(*id)
            && let NodeKind::Image(im) = &mut n.kind
        {
            im.link = Some(link.clone());
        }
    }
}

// ---------- relative paths ----------

/// `target` relative to folder `base` with `/` separators; `None` when they share no root (another
/// drive, or either is relative).
fn relative_path(target: &Path, base: &Path) -> Option<String> {
    let (t, b): (Vec<Component>, Vec<Component>) = (target.components().collect(), base.components().collect());
    if !matches!(t.first(), Some(Component::Prefix(_) | Component::RootDir)) {
        return None;
    }
    // Windows paths compare without case.
    let same = |x: &Component, y: &Component| if cfg!(windows) { x.as_os_str().eq_ignore_ascii_case(y.as_os_str()) } else { x == y };
    let common = t.iter().zip(&b).take_while(|(x, y)| same(x, y)).count();
    if common == 0 {
        return None;
    }
    let ups = std::iter::repeat_n(std::borrow::Cow::Borrowed(".."), b.len().saturating_sub(common));
    let rest = t.iter().skip(common).map(|c| c.as_os_str().to_string_lossy());
    Some(ups.chain(rest).collect::<Vec<std::borrow::Cow<str>>>().join("/"))
}

/// `d` as saved to `dest`: the links' paths relative to its folder written in; `None` when they
/// are already.
pub fn with_relative_paths(d: &Document, dest: &str) -> Option<Document> {
    let dest = absolute_path(dest);
    let dir = Path::new(&dest).parent()?;
    let mut changes = vec![];
    d.walk(|n| {
        if let NodeKind::Image(im) = &n.kind
            && let Some(l) = &im.link
        {
            let r = relative_path(Path::new(&l.path), dir);
            if r != l.relative {
                changes.push((n.id, r));
            }
        }
    });
    if changes.is_empty() {
        return None;
    }
    let mut d = d.clone();
    for (id, r) in changes {
        if let Some(n) = d.node_mut(id)
            && let NodeKind::Image(im) = &mut n.kind
            && let Some(l) = &mut im.link
        {
            l.relative = r;
        }
    }
    Some(d)
}

// ---------- commands ----------

fn check(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids");
    let (mut missing, mut modified) = (0, 0);
    let mut links = vec![];
    for g in groups(&st.doc, ids.as_deref()).iter().filter(|g| !g.ids.is_empty()) {
        let (status, found) = probe(g, folder(st.path.as_deref()));
        missing += usize::from(status == Status::Missing);
        modified += usize::from(status == Status::Modified);
        let mut row = json!({ "name": g.link.name(), "path": g.link.path, "status": status.as_str(), "ids": ids_json(&g.ids) });
        if let Some(f) = found.filter(|f| f.path != g.link.path) {
            row["found"] = json!(f.path);
        }
        if previewing(&st.doc, &g.key) {
            row["preview"] = json!(true);
        }
        links.push(row);
    }
    Ok(json!({ "links": links, "missing": missing, "modified": modified }))
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids");
    let (mut reads, mut missing) = (vec![], vec![]);
    for g in groups(&st.doc, ids.as_deref()).into_iter().filter(|g| !g.ids.is_empty()) {
        match probe(&g, folder(st.path.as_deref())) {
            (Status::Missing, _) | (_, None) => missing.extend(ids_json(&g.ids)),
            // Unchanged and showing the file: nothing to read unless asked for by id.
            (Status::Ok, Some(_)) if ids.is_none() && !previewing(&st.doc, &g.key) => {}
            // Unreadable as an image counts as missing.
            (_, Some(mut f)) => match read_image(&mut f, &g.link) {
                Ok(read) => reads.push((g.ids, read)),
                Err(_) => missing.extend(ids_json(&g.ids)),
            },
        }
    }
    let updated: Vec<u64> = reads.iter().flat_map(|(ids, _)| ids_json(ids)).collect();
    if !reads.is_empty() {
        s.edit("Update Links", |d, _| {
            reads.into_iter().for_each(|(ids, read)| install(d, &ids, read));
            Ok(())
        })?;
    }
    Ok(json!({ "updated": updated, "missing": missing }))
}

fn relink(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "links.relink";
    let st = s.doc()?;
    let folder_param = str_param(p, "folder");
    let image = |id: &NodeId| st.doc.node(*id).and_then(|n| if let NodeKind::Image(im) = &n.kind { Some(im) } else { None });
    let ids: Vec<NodeId> = match ids_param(p, "ids") {
        Some(ids) => ids,
        None => {
            let selected: Vec<NodeId> = st.selection.objects.iter().copied().filter(|id| image(id).is_some()).collect();
            match folder_param {
                Some(_) if selected.is_empty() => groups(&st.doc, None)
                    .into_iter()
                    .filter(|g| probe(g, folder(st.path.as_deref())).0 == Status::Missing)
                    .flat_map(|g| g.ids)
                    .collect(),
                _ => selected,
            }
        }
    };
    if let Some(id) = ids.iter().find(|id| image(id).is_none()) {
        return Err(bad(C, format!("object {} is not an image", id.0)));
    }
    if ids.is_empty() {
        return Err(bad(C, "select the images to relink, or pass ids"));
    }
    // (ids, the file) per file to read.
    let mut files: Vec<(Vec<NodeId>, String)> = vec![];
    let mut not_found = vec![];
    match (str_param(p, "path"), folder_param) {
        (Some(path), _) => files.push((ids, absolute_path(path))),
        (None, Some(dir)) => {
            for id in ids {
                let Some(link) = image(&id).and_then(|im| im.link.as_ref()) else { continue };
                let path = absolute_path(&Path::new(dir).join(link.name()).to_string_lossy());
                match files.iter_mut().find(|(_, p)| *p == path) {
                    Some((ids, _)) => ids.push(id),
                    None if file_stamp(&path).is_some() => files.push((vec![id], path)),
                    None => not_found.push(link.name().to_string()),
                }
            }
        }
        (None, None) => return Err(bad(C, "give path (a file) or folder")),
    }
    let mut reads = vec![];
    for (ids, path) in files {
        let bytes = read_file(&path)?;
        let img = fileio::raster_image(&bytes).map_err(|e| bad(C, format!("{path}: {e}")))?;
        reads.push((ids, (link_info(&path, &bytes), img)));
    }
    let relinked: Vec<u64> = reads.iter().flat_map(|(ids, _)| ids_json(ids)).collect();
    if !reads.is_empty() {
        s.edit("Relink", |d, _| {
            reads.into_iter().for_each(|(ids, read)| install(d, &ids, read));
            Ok(())
        })?;
    }
    not_found.sort();
    not_found.dedup();
    Ok(json!({ "relinked": relinked, "notFound": not_found }))
}
