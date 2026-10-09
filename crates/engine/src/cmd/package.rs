//! File → Package: a saved document copied into a folder of its own with the files its linked
//! images show (`Links/`), the fonts its type uses (`Fonts/`, leaving out fonts whose licence
//! doesn't allow embedding) and a text report (the package's contents, then the Document Info
//! report). The packaged document links to the copies (Relink); the open document doesn't change.
//! Without a folder (the web, agents) the same files come back as a zip archive.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use vectorcraft_doc::{Document, LinkInfo};
use vectorcraft_doc::links::hash_bytes;

use serde_json::{Value, json};

use super::fileio::{create_dir, write_file};
use super::*;

const C: &str = "file.package";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "file.package",
        "Package",
        [],
        None,
        "{folder?, name?: (default: \"<document> Folder\"), copyLinks?: true, linksFolder?: true (the linked files go in Links/, else next to the document), relink?: true (the packaged document links to the copies), copyFonts?: true (the fonts its type uses go in Fonts/; fonts whose licence doesn't allow embedding are left out), report?: true (<document> Report.txt)} copy the saved document as it is now into folder/name as <document>.vectorcraft with its linked files and fonts; the open document doesn't change → {folder, files: [paths relative to folder/name], links, fonts (counts copied), missingLinks: [file names not found], skippedFonts: [{font, reason}]}; no folder → the same with {name: \"<name>.zip\", dataBase64} (a zip of the files in <name>/) instead of folder",
        has_doc,
        package
    )]
}

/// A file of the package: its path in the package folder (`/` separators) and bytes.
type Entry = (String, Vec<u8>);

/// `name`, else the first free `stem 2.ext`, `stem 3.ext`… (compared without case, as most
/// desktop file systems do); the name is taken.
fn free_name(name: &str, taken: &mut HashSet<String>) -> String {
    let p = Path::new(name);
    let stem = p.file_stem().map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned());
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let out = unique_name(&stem, |s| taken.contains(&format!("{s}{ext}").to_lowercase())) + &ext;
    taken.insert(out.to_lowercase());
    out
}

/// A font file's extension from its first bytes.
fn font_ext(bytes: &[u8]) -> &'static str {
    match bytes.get(..4) {
        Some(b"OTTO") => "otf",
        Some(b"ttcf") => "ttc",
        _ => "ttf",
    }
}

/// The options of a package.
struct Options {
    copy_links: bool,
    links_folder: bool,
    relink: bool,
    copy_fonts: bool,
    report: bool,
}

/// An asset that has been copied to the package, including rewritten linked documents.
#[derive(Clone)]
struct Copied {
    name: String,
    size: u64,
    hash: String,
}

/// The parent of a packaged document and its asset both live inside the package.
/// Use paths relative to that *document*, not the package's root: a linked document
/// in Links/ must reference its own sibling image as "image.png".
fn relative_in_package(document: &str, asset: &str) -> String {
    let parent = document.rsplit_once('/').map_or("", |(dir, _)| dir);
    let a: Vec<&str> = parent.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = asset.split('/').filter(|s| !s.is_empty()).collect();
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut out = vec![".."; a.len() - common];
    out.extend_from_slice(&b[common..]);
    out.join("/")
}

fn safe_asset_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let name = name.trim().trim_matches('.');
    if name.is_empty() { "asset".into() } else { name.into() }
}

/// Collect dependencies depth-first so nested VectorCraft files are rewritten before
/// their parents: each parent can record the final hash of the rewritten child.
/// Cycles or excessive nesting must not silently yield incomplete packages.
struct Collector<'a> {
    opts: &'a Options,
    root: Option<PathBuf>,
    entries: Vec<Entry>,
    copies: BTreeMap<String, Copied>,
    visiting: HashSet<String>,
    taken: HashSet<String>,
    link_names: HashSet<String>,
    font_files: HashSet<String>,
    fonts: usize,
    missing: Vec<String>,
    skipped: Vec<Value>,
    lines: Vec<String>,
}

impl<'a> Collector<'a> {
    fn new(opts: &'a Options, root: Option<PathBuf>, doc_name: &str) -> Self {
        let mut taken = HashSet::new();
        taken.insert(doc_name.to_lowercase());
        Self {
            opts, root, entries: Vec::new(), copies: BTreeMap::new(), visiting: HashSet::new(),
            taken, link_names: HashSet::new(), font_files: HashSet::new(), fonts: 0,
            missing: Vec::new(), skipped: Vec::new(), lines: Vec::new(),
        }
    }

    fn collect(&mut self, doc: &mut Document, source: &str, destination: &str, depth: usize) -> Result<()> {
        if depth > vectorcraft_doc::placed_document::MAX_DEPTH {
            return Err(bad(C, "placed-document links exceed the supported nesting depth (8)"));
        }
        if !self.visiting.insert(source.to_string()) {
            return Err(bad(C, format!("circular placed-document link involving {source}")));
        }
        // A file's saved path is only meaningful inside its own document.
        // Different linked documents can reuse the same stale absolute path yet
        // resolve it to distinct local assets. Keep a mapping per document.
        let mut local = BTreeMap::<String, Copied>::new();
        if self.opts.copy_links {
            let mut placed = HashSet::new();
            doc.visit_placed(|_, p| { placed.insert(p.link.path.clone()); });
            self.lines.push(format!("LINKED FILES IN {source}"));
            for file in super::links::linked_files(doc, Some(source)) {
                let found_path = file.found_path.as_deref().unwrap_or(&file.path);
                if self.visiting.contains(found_path) {
                    return Err(bad(C, format!("circular placed-document link involving {}", file.path)));
                }
                if let Some(existing) = self.copies.get(found_path) {
                    local.insert(file.path.clone(), existing.clone());
                    continue;
                }
                let Some(mut bytes) = file.bytes else {
                    self.lines.push(format!("{}: not found ({}), not copied", file.name, file.path));
                    self.missing.push(if depth == 0 { file.name.clone() } else { format!("{} ({source})", file.name) });
                    continue;
                };
                if self.copies.len() >= 10_000 {
                    return Err(bad(C, "more than 10000 linked files: package dependencies in smaller parts"));
                }
                let dir = if self.opts.links_folder { "Links/" } else { "" };
                let name = safe_asset_name(&file.name);
                let unique = free_name(&name, if self.opts.links_folder { &mut self.link_names } else { &mut self.taken });
                let destination_file = format!("{dir}{unique}");
                if placed.contains(&file.path) {
                    let saved = vectorcraft_format::load_file(&bytes)
                        .map_err(|e| bad(C, format!("linked VectorCraft document {} cannot be packaged: {e}", file.path)))?;
                    let mut nested = saved.doc;
                    self.collect(&mut nested, found_path, &destination_file, depth + 1)?;
                    if self.opts.relink {
                        // Preserve embedded ICC profiles when rewriting a linked native file.
                        let mut options = vectorcraft_format::SaveOptions::for_doc(&nested);
                        options.profiles = saved.profiles;
                        bytes = vectorcraft_format::save_with(&nested, &options)
                            .map_err(|e| bad(C, format!("could not rewrite packaged document {}: {e}", file.path)))?;
                    }
                }
                let copy = Copied { name: destination_file.clone(), size: bytes.len() as u64, hash: hash_bytes(&bytes) };
                self.copies.insert(found_path.to_string(), copy.clone());
                local.insert(file.path.clone(), copy);
                self.lines.push(format!("{} → {destination_file}", file.path));
                self.entries.push((destination_file, bytes));
            }
            self.lines.push(String::new());
        }
        if self.opts.copy_fonts {
            let db = vectorcraft_text::FontDb::global();
            self.lines.push(format!("FONTS IN {source}"));
            for (family, style) in super::fonts::used_fonts(doc) {
                let font = format!("{family} {style}");
                let reason = match db.face(&family, &style) {
                    Some(f) if f.family.eq_ignore_ascii_case(&family) && f.embeddable() => {
                        let data = f.file_data();
                        let file = f.path().and_then(Path::file_name).map_or_else(
                            || {
                                format!("{}-{}.{}", f.family, f.style, font_ext(data))
                                    .replace(|c: char| !(c.is_alphanumeric() || "-_.".contains(c)), "")
                            },
                            |n| n.to_string_lossy().into_owned(),
                        );
                        if self.font_files.insert(file.to_lowercase()) {
                            self.entries.push((format!("Fonts/{file}"), data.to_vec()));
                            self.fonts += 1;
                        }
                        self.lines.push(format!("{font} → Fonts/{file}"));
                        continue;
                    }
                    Some(f) if f.family.eq_ignore_ascii_case(&family) => "its licence doesn't allow embedding",
                    _ => "not available on this computer",
                };
                self.lines.push(format!("{font}: {reason}, not copied"));
                self.skipped.push(json!({ "font": font, "reason": reason }));
            }
            self.lines.push(String::new());
        }
        if self.opts.relink {
            // The child files have already been rewritten; the hash and size of each
            // target must reflect the *packaged* bytes rather than the original.
            doc.update_links(|link: &mut LinkInfo| {
                if let Some(copy) = local.get(&link.path) {
                    let relative = relative_in_package(destination, &copy.name);
                    link.path = self.root.as_ref().map_or_else(|| copy.name.clone(), |r| r.join(&copy.name).to_string_lossy().into_owned());
                    link.relative = Some(relative);
                    link.size = Some(copy.size);
                    link.hash = Some(copy.hash.clone());
                    link.modified = None;
                }
            });
        }
        self.visiting.remove(source);
        Ok(())
    }
}

fn package(s: &mut Session, p: &Value) -> Result<Value> {
    let o = Options {
        copy_links: bool_or(p, "copyLinks", true),
        links_folder: bool_or(p, "linksFolder", true),
        relink: bool_or(p, "relink", true),
        copy_fonts: bool_or(p, "copyFonts", true),
        report: bool_or(p, "report", true),
    };
    let doc_path = s.doc()?.path.clone().ok_or_else(|| bad(C, "save the document first: Package collects a saved document"))?;
    let stem = Path::new(&doc_path).file_stem().map_or_else(|| "Untitled".into(), |s| s.to_string_lossy().into_owned());
    let name = str_param(p, "name").map_or_else(|| format!("{stem} Folder"), str::to_string);
    if name.trim().is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(bad(C, format!("`{name}` can't name a folder")));
    }
    let folder = str_param(p, "folder");
    let root = folder.map(|f| Path::new(f).join(&name));
    let info = if o.report { Some(super::docinfo::report(s, false)?) } else { None };
    let st = s.doc()?;
    let mut doc = (*st.doc).clone();
    let doc_name = format!("{stem}.{}", vectorcraft_format::EXTENSION);
    // Prepare original links for the destination *before* rebasing copied links.
    // Recalculating afterward would overwrite the portable relative paths.
    if let Some(root) = &root
        && let Some(d) = super::links::with_relative_paths(&doc, &root.join(&doc_name).to_string_lossy())
    {
        doc = d;
    }
    let mut collector = Collector::new(&o, root.clone(), &doc_name);
    collector.collect(&mut doc, &doc_path, &doc_name, 0)?;
    let mut entries = std::mem::take(&mut collector.entries);
    entries.insert(0, (doc_name.clone(), vectorcraft_format::save_file(&doc)));
    let mut taken = std::mem::take(&mut collector.taken);
    let mut lines = std::mem::take(&mut collector.lines);
    let missing = std::mem::take(&mut collector.missing);
    let skipped = std::mem::take(&mut collector.skipped);
    let fonts = collector.fonts;
    let links = collector.copies.len();

    // The report.
    if let Some(info) = info {
        let report_name = free_name(&format!("{stem} Report.txt"), &mut taken);
        let mut text = format!("Package: {doc_name}\nFrom: {doc_path}\n\n");
        lines.iter().for_each(|l| text.push_str(&format!("{l}\n")));
        text.push_str(&info);
        entries.push((report_name, text.into_bytes()));
    }

    let files: Vec<&str> = entries.iter().map(|(p, _)| p.as_str()).collect();
    let mut out = json!({ "files": files, "links": links, "fonts": fonts, "missingLinks": missing, "skippedFonts": skipped });
    match root {
        Some(root) => {
            for (rel, bytes) in &entries {
                let path = root.join(rel);
                if let Some(dir) = path.parent() {
                    create_dir(&dir.to_string_lossy())?;
                }
                write_file(&path.to_string_lossy(), bytes)?;
            }
            out["folder"] = json!(root.to_string_lossy());
        }
        None => {
            let named: Vec<(String, &[u8])> = entries.iter().map(|(rel, b)| (format!("{name}/{rel}"), b.as_slice())).collect();
            let zip = zip(&named)?;
            out["name"] = json!(format!("{name}.zip"));
            out["bytes"] = json!(zip.len());
            out["dataBase64"] = json!(vectorcraft_format::base64_encode(&zip));
        }
    }
    Ok(out)
}

// ---------- zip archives ----------

/// The MS-DOS time and date a zip entry records: now (UTC), else 1980-01-01.
fn dos_time() -> (u16, u16) {
    let Some(t) = vectorcraft_doc::metadata::now_unix() else { return (0, 0x21) };
    let [y, mo, d, h, mi, s] = vectorcraft_doc::metadata::civil(t);
    let date = (((y - 1980).clamp(0, 127) << 9) | (mo << 5) | d) as u16;
    (((h << 11) | (mi << 5) | (s / 2)) as u16, date)
}

/// A zip archive of `files` ((path with `/` separators, bytes)), each deflated, or stored when
/// that isn't smaller.
pub(crate) fn zip(files: &[(String, &[u8])]) -> Result<Vec<u8>> {
    let too_big = || bad(C, "too large for a zip archive: package to a folder");
    let io = |e: std::io::Error| EngineError::Other(e.to_string());
    let (time, date) = dos_time();
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, bytes) in files {
        let mut crc = flate2::Crc::new();
        crc.update(bytes);
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(bytes).map_err(io)?;
        let deflated = enc.finish().map_err(io)?;
        let (method, data): (u16, &[u8]) = if deflated.len() < bytes.len() { (8, &deflated) } else { (0, bytes) };
        let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
        let (packed, size) = (u32::try_from(data.len()).map_err(|_| too_big())?, u32::try_from(bytes.len()).map_err(|_| too_big())?);
        let name_len = u16::try_from(name.len()).map_err(|_| too_big())?;
        // Version 2.0, UTF-8 names, method, time, date, CRC-32, sizes, name length, no extra field.
        let common = [&20u16.to_le_bytes()[..], &0x0800u16.to_le_bytes(), &method.to_le_bytes(), &time.to_le_bytes(), &date.to_le_bytes()].concat();
        let sums = [crc.sum().to_le_bytes(), packed.to_le_bytes(), size.to_le_bytes()].concat();
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&common);
        out.extend_from_slice(&sums);
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&common);
        central.extend_from_slice(&sums);
        central.extend_from_slice(&name_len.to_le_bytes());
        // No extra field or comment, disk 0, no attributes.
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(files.len()).map_err(|_| too_big())?;
    let (start, len) = (u32::try_from(out.len()).map_err(|_| too_big())?, u32::try_from(central.len()).map_err(|_| too_big())?);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}
