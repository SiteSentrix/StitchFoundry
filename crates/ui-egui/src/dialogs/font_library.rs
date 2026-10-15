//! Get Embroidery Fonts (NEW-20): a window listing the fonts of an embroidery font library (a
//! public GitHub repository of Ink/Stitch-format font folders), each downloaded with a click into
//! the Embroidery Fonts folder next to the settings and loaded for lettering. Desktop only: the
//! web demo has no `http_get` service (and no server to keep fonts on), so it doesn't offer it.
//! What the library says is untrusted (`vectorcraft_embroidery::font_library` checks it); the
//! downloads run on a thread of their own.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use serde_json::{Value, json};
use vectorcraft_embroidery::font_library::{self, LibraryFont, SOURCES};

use crate::VectorcraftApp;
use crate::theme::{self, Tokens};

/// What a thread sends back.
enum Message {
    /// The library's fonts, or why there are none.
    Listed(Result<Vec<LibraryFont>, String>),
    /// One more file of the font being downloaded.
    Progress { done: usize, total: usize },
    /// The font is in its folder (its path), or why not.
    Downloaded(Result<PathBuf, String>),
}

/// The window's state.
#[derive(Default)]
pub struct FontLibraryState {
    pub open: bool,
    /// Which of [`SOURCES`].
    source: usize,
    /// The fonts listed, for the source they were listed for.
    fonts: Option<(usize, Result<Vec<LibraryFont>, String>)>,
    search: String,
    /// The font downloading (its folder name) and how far it has got.
    downloading: Option<(String, usize, usize)>,
    message: Option<String>,
    rx: Option<Receiver<Message>>,
    /// Where fonts go instead of the Embroidery Fonts folder (tests).
    pub(crate) folder: Option<PathBuf>,
    /// Download All: the fonts still to download, and how the batch is going (fonts done, in
    /// all, failed).
    queue: Vec<LibraryFont>,
    batch: Option<(usize, usize, usize)>,
}

impl std::fmt::Debug for FontLibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontLibraryState").field("open", &self.open).field("source", &self.source).finish_non_exhaustive()
    }
}

/// Where downloaded embroidery fonts are kept: Embroidery Fonts beside the app's Fonts folder.
pub fn fonts_folder(app: &VectorcraftApp) -> Option<PathBuf> {
    app.embroidery.font_library.folder.clone().or_else(|| Some(vectorcraft_text::app_font_dir()?.parent()?.join("Embroidery Fonts")))
}

/// Whether fonts can be downloaded here (the desktop app).
pub fn available(app: &VectorcraftApp) -> bool {
    app.services.http_get.is_some() && fonts_folder(app).is_some()
}

/// `ui.fontLibrary`: opens the window (and lists the library).
pub fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    if !available(app) {
        return Err("downloading embroidery fonts works in the desktop app".into());
    }
    app.embroidery.font_library.open = true;
    if app.embroidery.font_library.fonts.is_none() {
        list(app);
    }
    Ok(json!({ "open": true }))
}

/// Starts listing the chosen library's fonts.
fn list(app: &mut VectorcraftApp) {
    let Some(get) = app.services.http_get.clone() else { return };
    let st = &mut app.embroidery.font_library;
    let Some(src) = SOURCES.get(st.source).copied() else { return };
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new().name("font-library".into()).spawn(move || {
        let r = vectorcraft_engine::guard::catch_panic(|| match get(&font_library::tree_url(src.repo), 30_000_000) {
            Ok((200, body)) => font_library::parse_tree(&String::from_utf8_lossy(&body)),
            Ok((404, _)) => Err(format!("{} isn't published yet", src.name)),
            Ok((status, _)) => Err(format!("the font library answered {status}: try again later")),
            Err(e) => Err(format!("can't reach the font library: {e}")),
        })
        .unwrap_or_else(|msg| Err(format!("internal error: {msg} (please report this bug)")));
        // Nothing waits any more once the window was closed.
        let _ = tx.send(Message::Listed(r));
    });
    match spawned {
        Ok(_) => {
            st.rx = Some(rx);
            st.fonts = None;
            st.message = None;
        }
        Err(e) => st.message = Some(format!("can't list the fonts: {e}")),
    }
}

/// Starts downloading `font` from source `source` into the fonts folder.
fn download(app: &mut VectorcraftApp, font: LibraryFont) {
    let (Some(get), Some(root)) = (app.services.http_get.clone(), fonts_folder(app)) else { return };
    let st = &mut app.embroidery.font_library;
    let Some(src) = SOURCES.get(st.source).copied() else { return };
    let (tx, rx) = channel();
    let folder = font.folder.clone();
    let total = font.files.len();
    let spawned = std::thread::Builder::new().name("font-download".into()).spawn(move || {
        let r = vectorcraft_engine::guard::catch_panic(|| {
            // into a folder of its own beside the final one, moved there when complete
            let dir = root.join(&font.folder);
            let part = root.join(format!("{}.part", font.folder));
            let _ = std::fs::remove_dir_all(&part); // a download left from before, if any
            std::fs::create_dir_all(&part).map_err(|e| format!("can't make {}: {e}", part.display()))?;
            for (k, (path, size)) in font.files.iter().enumerate() {
                let parts = font.local_parts(path).ok_or_else(|| format!("a file of the font has an unsafe name: {path}"))?;
                let (status, body) = get(&font_library::raw_url(src.repo, path), (*size).max(1) + 1024)?;
                if status != 200 {
                    return Err(format!("the font library answered {status} for {path}"));
                }
                let mut to = part.clone();
                for p in &parts {
                    to.push(p);
                }
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&to, &body).map_err(|e| format!("can't write {}: {e}", to.display()))?;
                let _ = tx.send(Message::Progress { done: k + 1, total });
            }
            let _ = std::fs::remove_dir_all(&dir); // an older copy of the same font
            std::fs::rename(&part, &dir).map_err(|e| format!("can't finish {}: {e}", dir.display()))?;
            Ok(dir)
        })
        .unwrap_or_else(|msg| Err(format!("internal error: {msg} (please report this bug)")));
        let _ = tx.send(Message::Downloaded(r));
    });
    match spawned {
        Ok(_) => {
            st.rx = Some(rx);
            st.downloading = Some((folder, 0, total));
            st.message = None;
        }
        Err(e) => st.message = Some(format!("can't download the font: {e}")),
    }
}

/// Each frame: take what the threads sent.
fn poll(app: &mut VectorcraftApp, ctx: Option<&egui::Context>) {
    loop {
        let Some(rx) = app.embroidery.font_library.rx.as_ref() else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(TryRecvError::Empty) => {
                if let Some(ctx) = ctx {
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                }
                return;
            }
            Err(TryRecvError::Disconnected) => {
                let st = &mut app.embroidery.font_library;
                st.rx = None;
                if st.downloading.take().is_some() {
                    st.message = Some("the download stopped".into());
                }
                return;
            }
        };
        match msg {
            Message::Listed(r) => {
                let st = &mut app.embroidery.font_library;
                st.fonts = Some((st.source, r));
                st.rx = None;
            }
            Message::Progress { done, total } => {
                if let Some(d) = app.embroidery.font_library.downloading.as_mut() {
                    (d.1, d.2) = (done, total);
                }
            }
            Message::Downloaded(r) => {
                app.embroidery.font_library.rx = None;
                let ok = r.is_ok();
                let name = app.embroidery.font_library.downloading.take().map(|d| d.0).unwrap_or_default();
                let loaded = r.and_then(|dir| app.run("embroidery.font.load", json!({ "path": dir.to_string_lossy() })));
                app.embroidery.font_library.message = Some(match loaded {
                    Ok(f) => {
                        if let Some(n) = f["name"].as_str() {
                            app.embroidery.lettering_font = n.to_string();
                        }
                        format!("{} {}", f["name"].as_str().unwrap_or(&name), tl!("is ready for lettering"))
                    }
                    Err(e) => e,
                });
                // Download All: on to the next font
                let st = &mut app.embroidery.font_library;
                if let Some((done, total, failed)) = st.batch.as_mut() {
                    *done += 1;
                    if !ok {
                        *failed += 1;
                    }
                    let (done, total, failed) = (*done, *total, *failed);
                    if st.queue.is_empty() {
                        st.batch = None;
                        st.message = Some(if failed == 0 {
                            format!("{done} {}", tl!("fonts downloaded, ready for lettering"))
                        } else {
                            format!("{} {done}, {} {failed}", tl!("Fonts downloaded:"), tl!("failed:"))
                        });
                    } else {
                        let next = st.queue.remove(0);
                        st.message = Some(format!("{} {} / {total}", tl!("Font"), done + 1));
                        download(app, next);
                    }
                }
            }
        }
    }
}

/// The window, while open.
pub(super) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.embroidery.font_library.open {
        return;
    }
    poll(app, Some(ctx));
    let mut open = true;
    let mut start: Option<LibraryFont> = None;
    let mut all: Vec<LibraryFont> = Vec::new();
    let mut relist = false;
    let root = fonts_folder(app);
    egui::Window::new(tl!("Get Embroidery Fonts")).open(&mut open).collapsible(false).default_size([520.0, 560.0]).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        let st = &mut app.embroidery.font_library;
        ui.horizontal(|ui| {
            ui.label(tl!("Library"));
            for (i, src) in SOURCES.iter().enumerate() {
                if ui.selectable_label(st.source == i, src.name).clicked() && st.source != i {
                    st.source = i;
                    relist = true;
                }
            }
        });
        if let Some(src) = SOURCES.get(st.source) {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(tl!("Each font comes with its own licence, in its folder.")).size(12.0).color(t.text_dim));
                ui.hyperlink_to(egui::RichText::new(tl!("About the library")).size(12.0), src.page);
            });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(tl!("Search"));
            ui.add(egui::TextEdit::singleline(&mut st.search).desired_width(240.0));
        });
        if let Some(m) = &st.message {
            ui.label(egui::RichText::new(m.as_str()).color(t.accent));
        }
        if let Some((name, done, total)) = &st.downloading {
            ui.label(format!("{} {name}: {done} / {total}", tl!("Downloading")));
        }
        let mut stop = false;
        if let Some((done, total, _)) = st.batch {
            ui.horizontal(|ui| {
                ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_width(260.0).text(format!("{done} / {total}")));
                stop = ui.button(tl!("Stop")).clicked();
            });
        }
        if stop {
            // the font on its way finishes; the rest are left
            st.queue.clear();
        }
        ui.separator();
        let busy = st.downloading.is_some();
        match &st.fonts {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tl!("Listing the fonts…"));
                });
            }
            Some((_, Err(e))) => {
                ui.label(egui::RichText::new(e.as_str()).color(t.error));
                if ui.button(tl!("Try Again")).clicked() {
                    relist = true;
                }
            }
            Some((_, Ok(fonts))) => {
                let q = st.search.trim().to_lowercase();
                let shown: Vec<&LibraryFont> = fonts.iter().filter(|f| q.is_empty() || f.title().to_lowercase().contains(&q)).collect();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", shown.len(), tl!("fonts"))).color(t.text_dim));
                    let missing: Vec<&LibraryFont> =
                        fonts.iter().filter(|f| !root.as_ref().is_some_and(|r| r.join(&f.folder).join("font.json").exists())).collect();
                    let mb = missing.iter().map(|f| f.size()).sum::<u64>() as f64 / 1e6;
                    let label = format!("{} ({}, {mb:.0} MB)", tl!("Download All"), missing.len());
                    if !missing.is_empty()
                        && ui
                            .add_enabled(!busy, egui::Button::new(label))
                            .on_hover_text(tl!("Every font not downloaded yet, one after another"))
                            .clicked()
                    {
                        all = missing.into_iter().cloned().collect();
                    }
                });
                egui::ScrollArea::vertical().max_height(380.0).show_rows(ui, 26.0, shown.len(), |ui, range| {
                    for f in shown.get(range).unwrap_or_default() {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(f.title()).font(theme::semibold(13.0)));
                            ui.label(egui::RichText::new(format!("{:.1} MB", f.size() as f64 / 1e6)).size(11.0).color(t.text_dim));
                            if let (Some(p), Some(src)) = (&f.preview, SOURCES.get(st.source)) {
                                ui.hyperlink_to(egui::RichText::new(tl!("Preview")).size(11.0), font_library::raw_url(src.repo, p));
                            }
                            let have = root.as_ref().is_some_and(|r| r.join(&f.folder).join("font.json").exists());
                            let label = if have { tl!("Download Again") } else { tl!("Download") };
                            if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                                start = Some((*f).clone());
                            }
                            if have {
                                ui.label(egui::RichText::new("✓").color(t.accent));
                            }
                        });
                    }
                });
            }
        }
        if let Some(r) = &root {
            ui.separator();
            ui.label(egui::RichText::new(format!("{} {}", tl!("Fonts are kept in"), r.display())).size(11.0).color(t.text_dim));
        }
    });
    if relist {
        list(app);
    }
    if let Some(f) = start {
        download(app, f);
    }
    if !all.is_empty() {
        let st = &mut app.embroidery.font_library;
        st.batch = Some((0, all.len(), 0));
        let first = all.remove(0);
        st.queue = all;
        download(app, first);
    }
    if !open {
        app.embroidery.font_library.open = false;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::Services;

    const FONT_JSON: &str = r#"{"name": "Library Test", "font_license": "MIT", "leading": 60, "size": 10, "letter_case": "upper",
      "horiz_adv_x_space": 20, "units_per_em": 60, "horiz_adv_x": {"A": 40}}"#;
    const FONT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100">
      <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-A" id="la"><path id="a1" d="M 2 70 H 25 V 80 H 2 Z" style="fill:#000000"/></g></svg>"##;

    /// A font library served by a fake `http_get`: its file list and a font's files.
    fn app_with_library(folder: PathBuf) -> (VectorcraftApp, Arc<Mutex<Vec<String>>>) {
        let tree = json!({"tree": [
            {"path": "src/library_test/font.json", "type": "blob", "size": FONT_JSON.len()},
            {"path": "src/library_test/ltr.svg", "type": "blob", "size": FONT_SVG.len()},
            {"path": "src/library_test/LICENSE", "type": "blob", "size": 3},
            {"path": "src/other_font/font.json", "type": "blob", "size": 10},
        ]})
        .to_string();
        let mut files: HashMap<String, Vec<u8>> = HashMap::new();
        let repo = SOURCES[1].repo;
        files.insert(font_library::tree_url(repo), tree.into_bytes());
        files.insert(font_library::raw_url(repo, "src/library_test/font.json"), FONT_JSON.into());
        files.insert(font_library::raw_url(repo, "src/library_test/ltr.svg"), FONT_SVG.into());
        files.insert(font_library::raw_url(repo, "src/library_test/LICENSE"), b"MIT".to_vec());
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = asked.clone();
        let services = Services {
            http_get: Some(Arc::new(move |url: &str, limit: u64| {
                log.lock().unwrap().push(url.to_string());
                match files.get(url) {
                    Some(b) if (b.len() as u64) <= limit => Ok((200, b.clone())),
                    Some(_) => Err("too big".into()),
                    None => Ok((404, Vec::new())),
                }
            })),
            ..Default::default()
        };
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), services);
        app.embroidery.font_library.folder = Some(folder);
        (app, asked)
    }

    /// Polls as frames do until `done` says so (5 s at most).
    fn wait(app: &mut VectorcraftApp, done: impl Fn(&VectorcraftApp) -> bool) {
        for _ in 0..500 {
            poll(app, None);
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out");
    }

    /// NEW-20: the library's fonts are listed, a click downloads one into its folder and loads it
    /// for lettering; the project's own library, not published yet, says so.
    #[test]
    fn a_font_is_listed_downloaded_and_loaded() {
        let dir = std::env::temp_dir().join(format!("sf-font-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mut app, asked) = app_with_library(dir.clone());
        // the project's own library first: not there yet
        app.run("ui.fontLibrary", json!({})).unwrap();
        wait(&mut app, |a| a.embroidery.font_library.fonts.is_some());
        let Some((_, Err(e))) = &app.embroidery.font_library.fonts else { panic!("listed") };
        assert!(e.contains("isn't published yet"), "{e}");
        // Ink/Stitch's
        app.embroidery.font_library.source = 1;
        list(&mut app);
        wait(&mut app, |a| a.embroidery.font_library.fonts.is_some());
        let Some((_, Ok(fonts))) = &app.embroidery.font_library.fonts else { panic!("listed") };
        let names: Vec<String> = fonts.iter().map(LibraryFont::title).collect();
        assert_eq!(names, vec!["Library Test", "Other Font"]);
        let font = fonts[0].clone();
        download(&mut app, font);
        wait(&mut app, |a| a.embroidery.font_library.downloading.is_none());
        let msg = app.embroidery.font_library.message.clone().unwrap_or_default();
        assert!(msg.contains("Library Test"), "{msg}");
        assert!(dir.join("library_test").join("font.json").is_file() && dir.join("library_test").join("LICENSE").is_file());
        assert!(!dir.join("library_test.part").exists(), "the partial folder is gone");
        assert!(app.session.embroidery_fonts.iter().any(|f| f.info.name == "Library Test"), "loaded for lettering");
        assert_eq!(app.embroidery.lettering_font, "Library Test");
        // only the library's own addresses were asked for
        assert!(
            asked.lock().unwrap().iter().all(|u| u.starts_with("https://api.github.com/") || u.starts_with("https://raw.githubusercontent.com/"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Download All fetches every font not downloaded yet, one after another.
    #[test]
    fn download_all_fetches_every_font_in_turn() {
        let dir = std::env::temp_dir().join(format!("sf-font-library-all-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mut app, _) = app_with_library(dir.clone());
        app.embroidery.font_library.source = 1;
        app.run("ui.fontLibrary", json!({})).unwrap();
        wait(&mut app, |a| a.embroidery.font_library.fonts.is_some());
        let Some((_, Ok(fonts))) = &app.embroidery.font_library.fonts else { panic!("listed") };
        let mut all = fonts.clone();
        let st = &mut app.embroidery.font_library;
        st.batch = Some((0, all.len(), 0));
        let first = all.remove(0);
        st.queue = all;
        download(&mut app, first);
        wait(&mut app, |a| a.embroidery.font_library.batch.is_none());
        // the second font's files aren't all served: one done, one failed
        let msg = app.embroidery.font_library.message.clone().unwrap_or_default();
        assert!(msg.contains('1'), "{msg}");
        assert!(dir.join("library_test").join("font.json").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Without the download service (the web demo) the window doesn't open.
    #[test]
    fn the_web_demo_offers_no_downloads() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Services::default());
        assert!(!available(&app));
        assert!(app.run("ui.fontLibrary", json!({})).is_err());
    }
}
