//! The embroidery font libraries fonts can be downloaded from (NEW-20): public GitHub
//! repositories of Ink/Stitch-format fonts, each a folder with its `font.json`, its glyphs' art,
//! a preview and its own licence. This module only reads a repository's file list and says what
//! to fetch and where to keep it; the app does the downloading (desktop only: the web demo has no
//! server to keep fonts on and doesn't offer it). Everything a repository says is untrusted:
//! names and paths are checked, and counts and sizes capped, before anything is written.

use serde_json::Value;

/// A font library: a public GitHub repository of Ink/Stitch-format font folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    /// What the library is called in the app.
    pub name: &'static str,
    /// `owner/repository` on GitHub.
    pub repo: &'static str,
    /// Where it is described, for its licences.
    pub page: &'static str,
}

/// The libraries the app offers: the project's own (once it is published) and Ink/Stitch's,
/// whose fonts are free to download, each under its own licence (most the SIL Open Font
/// License).
pub const SOURCES: &[Source] = &[
    Source { name: "StitchFoundry fonts", repo: "Stitchwright/stitchfoundry-fonts", page: "https://github.com/Stitchwright/stitchfoundry-fonts" },
    Source { name: "Ink/Stitch font library", repo: "inkstitch/embroidery-fonts", page: "https://github.com/inkstitch/embroidery-fonts" },
];

/// Most entries a repository's file list may have.
pub const MAX_ENTRIES: usize = 50_000;
/// Most files one font may have.
pub const MAX_FILES: usize = 400;
/// Most bytes one file may have.
pub const MAX_FILE_BYTES: u64 = 40_000_000;
/// Most bytes one font may have in all.
pub const MAX_FONT_BYTES: u64 = 120_000_000;

/// One downloadable font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFont {
    /// Its folder's name (`abecedaire_small`), the name it is kept under.
    pub folder: String,
    /// Its folder's path in the repository (`src/abecedaire_small`).
    pub dir: String,
    /// Its files: (path in the repository, bytes).
    pub files: Vec<(String, u64)>,
    /// Its preview image's path, if it has one.
    pub preview: Option<String>,
}

impl LibraryFont {
    /// Bytes to download.
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.1).sum()
    }

    /// The name shown: the folder's words, each capitalised (`Abecedaire Small`).
    pub fn title(&self) -> String {
        self.folder
            .split(['_', '-', ' '])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Where file `path` (one of [`Self::files`]) is kept, relative to the font's own folder, as
    /// path components; `None` when it isn't inside the font's folder or a component isn't safe.
    pub fn local_parts(&self, path: &str) -> Option<Vec<String>> {
        let rest = path.strip_prefix(&self.dir)?.strip_prefix('/')?;
        let parts: Vec<String> = rest.split('/').map(str::to_string).collect();
        parts.iter().all(|p| safe_name(p)).then_some(parts)
    }
}

/// The file list of a repository's default branch (the GitHub API, one request).
pub fn tree_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/git/trees/HEAD?recursive=1")
}

/// A file of a repository's default branch.
pub fn raw_url(repo: &str, path: &str) -> String {
    let encoded: Vec<String> = path.split('/').map(percent_encode).collect();
    format!("https://raw.githubusercontent.com/{repo}/HEAD/{}", encoded.join("/"))
}

fn percent_encode(part: &str) -> String {
    part.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Whether `name` is a safe file or folder name to write: letters, digits, spaces, `-`, `_`, `.`,
/// `(`, `)` and `+`, not `.` or `..`, not starting with a dot, at most 120 characters.
pub fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 120 && !name.starts_with('.') && name.chars().all(|c| c.is_alphanumeric() || " -_.()+".contains(c))
}

/// The fonts in a repository's file list (the GitHub API's recursive tree, as JSON): every folder
/// holding a `font.json`, with every file under it. Fonts with unsafe names, too many files or
/// too many bytes are left out; the fonts come sorted by name.
pub fn parse_tree(json: &str) -> Result<Vec<LibraryFont>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("the font list isn't readable: {e}"))?;
    if let Some(msg) = v.get("message").and_then(Value::as_str) {
        return Err(format!("the font library answered: {msg}"));
    }
    let entries = v.get("tree").and_then(Value::as_array).ok_or("the font list has no files")?;
    if entries.len() > MAX_ENTRIES {
        return Err(format!("the font library lists more than {MAX_ENTRIES} files"));
    }
    let files: Vec<(&str, u64)> = entries
        .iter()
        .filter(|e| e.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|e| Some((e.get("path")?.as_str()?, e.get("size").and_then(Value::as_u64).unwrap_or(0))))
        .collect();
    let mut fonts: Vec<LibraryFont> = Vec::new();
    for (path, _) in files.iter().filter(|(p, _)| *p == "font.json" || p.ends_with("/font.json")) {
        let dir = path.strip_suffix("font.json").unwrap_or_default().trim_end_matches('/').to_string();
        let folder = dir.rsplit('/').next().unwrap_or_default().to_string();
        if dir.is_empty() || !safe_name(&folder) {
            continue;
        }
        let prefix = format!("{dir}/");
        let mine: Vec<(String, u64)> = files.iter().filter(|(p, _)| p.starts_with(&prefix)).map(|(p, s)| ((*p).to_string(), *s)).collect();
        let font = LibraryFont {
            preview: mine.iter().map(|f| &f.0).find(|p| p.strip_prefix(&prefix).is_some_and(|r| r.eq_ignore_ascii_case("preview.png"))).cloned(),
            folder,
            dir,
            files: mine,
        };
        let fits = font.files.len() <= MAX_FILES
            && font.files.iter().all(|(p, s)| *s <= MAX_FILE_BYTES && font.local_parts(p).is_some())
            && font.size() <= MAX_FONT_BYTES;
        // a font folder inside another font's folder isn't a font of its own
        if fits && !fonts.iter().any(|f| font.dir.starts_with(&format!("{}/", f.dir)) || f.dir.starts_with(&prefix)) {
            fonts.push(font);
        }
    }
    fonts.sort_by_key(LibraryFont::title);
    Ok(fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_lists_its_font_folders_and_their_files() {
        let tree = r#"{"sha": "x", "tree": [
            {"path": "README.md", "type": "blob", "size": 10},
            {"path": "src", "type": "tree"},
            {"path": "src/abecedaire_small/font.json", "type": "blob", "size": 900},
            {"path": "src/abecedaire_small/ltr.svg", "type": "blob", "size": 50000},
            {"path": "src/abecedaire_small/LICENSE", "type": "blob", "size": 4000},
            {"path": "src/abecedaire_small/preview.png", "type": "blob", "size": 20000},
            {"path": "src/garamond/font.json", "type": "blob", "size": 800},
            {"path": "src/garamond/ltr/latin.svg", "type": "blob", "size": 70000},
            {"path": "src/bad..name/font.json", "type": "blob", "size": 1},
            {"path": "src/sneaky/font.json", "type": "blob", "size": 1},
            {"path": "src/sneaky/../../etc/passwd", "type": "blob", "size": 1},
            {"path": "src/huge/font.json", "type": "blob", "size": 99999999999}
        ], "truncated": false}"#;
        let fonts = parse_tree(tree).unwrap();
        let names: Vec<String> = fonts.iter().map(LibraryFont::title).collect();
        assert_eq!(names, vec!["Abecedaire Small", "Bad..name", "Garamond"]);
        let abc = &fonts[0];
        assert_eq!(abc.files.len(), 4);
        assert_eq!(abc.size(), 74_900);
        assert_eq!(abc.preview.as_deref(), Some("src/abecedaire_small/preview.png"));
        assert_eq!(fonts[2].local_parts("src/garamond/ltr/latin.svg"), Some(vec!["ltr".to_string(), "latin.svg".to_string()]));
        // paths that would leave the font's folder, and other fonts' files, are refused
        assert_eq!(abc.local_parts("src/garamond/font.json"), None);
        assert_eq!(abc.local_parts("src/abecedaire_small/../x"), None);
        assert!(parse_tree(r#"{"message": "Not Found"}"#).unwrap_err().contains("Not Found"));
        assert!(parse_tree("<html>").is_err());
    }

    #[test]
    fn names_and_urls_are_safe() {
        for good in ["font.json", "ltr", "My Font (2)", "a-b_c.svg"] {
            assert!(safe_name(good), "{good}");
        }
        for bad in ["", ".", "..", ".hidden", "a/b", "a\\b", "c:", "x\u{0}"] {
            assert!(!safe_name(bad), "{bad:?}");
        }
        assert_eq!(
            raw_url("inkstitch/embroidery-fonts", "src/a b/font.json"),
            "https://raw.githubusercontent.com/inkstitch/embroidery-fonts/HEAD/src/a%20b/font.json"
        );
        assert_eq!(tree_url("inkstitch/embroidery-fonts"), "https://api.github.com/repos/inkstitch/embroidery-fonts/git/trees/HEAD?recursive=1");
    }
}
