//! Interface translations. Command ids, document text and file names remain stable.
//! Untranslated labels fall back to English so coverage can grow incrementally. Every entry is a
//! label VectorCraft's menus show (tested), so labels from other apps don't creep in.

mod czech;
use czech::CZECH;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ja,
    Cs,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::En, Self::Ja, Self::Cs];

    pub fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ja => "日本語",
            Self::Cs => "Čeština",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            "cs" => Some(Self::Cs),
            _ => None,
        }
    }

    pub fn tr(self, text: &str) -> &str {
        let table = match self {
            Self::En => return text,
            Self::Ja => JAPANESE,
            Self::Cs => CZECH,
        };
        table.iter().find(|(english, _)| *english == text).map_or(text, |(_, translated)| translated)
    }
}

const JAPANESE: &[(&str, &str)] = &[
    ("Object", "オブジェクト"),
    ("Effect", "効果"),
    ("Settings…", "環境設定…"),
    ("Type", "書式"),
    ("Select", "選択"),
    ("Window", "ウィンドウ"),
    ("Language", "表示言語"),
    ("Save As…", "別名で保存…"),
    ("New…", "新規…"),
    ("Horizontal", "横書き"),
    ("Vertical", "縦書き"),
    ("Type Orientation", "組み方向"),
    ("Layers", "レイヤー"),
    ("History", "履歴"),
    ("Properties", "プロパティ"),
    ("Color", "カラー"),
    ("Tools", "ツール"),
    ("Zoom In", "ズームイン"),
    ("Zoom Out", "ズームアウト"),
    ("Copy", "コピー"),
    ("Cut", "切り取り"),
    ("Paste", "貼り付け"),
    ("Deselect", "選択を解除"),
    ("Export", "書き出し"),
    ("Export As…", "形式を指定して書き出し…"),
    ("File", "ファイル"),
    ("Edit", "編集"),
    ("View", "表示"),
    ("Help", "ヘルプ"),
    ("Preferences…", "環境設定…"),
    ("Open…", "開く…"),
    ("Save", "保存"),
    ("Revert", "保存済みの状態に戻す"),
    ("Print…", "印刷…"),
    ("Undo", "取り消し"),
    ("Redo", "やり直し"),
];

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::menus::Item;

    const TABLES: [(Language, &[(&str, &str)]); 2] = [(Language::Ja, JAPANESE), (Language::Cs, CZECH)];

    /// Menu labels Czech shows as they are: the product name, language names, a format name, the
    /// built-in workspace names and the perspective grid presets (names, shown untranslated
    /// wherever else they appear).
    const KEEP_AS_IS: &[&str] = &[
        "VectorCraft",
        "English",
        "日本語",
        "Čeština",
        "OpenType",
        "Essentials",
        "Essentials Classic",
        "Automation",
        "Layout",
        "Painting",
        "Printing and Proofing",
        "Tracing",
        "Typography",
        "Web",
        "[1P-Normal View]",
        "[1P-Low View]",
        "[1P-High View]",
        "[2P-Normal View]",
        "[2P-Low View]",
        "[2P-High View]",
        "[3P-Normal View]",
        "[3P-Low View]",
    ];

    /// Every menu title, submenu, item and header label with its command and params (`""`/null
    /// for the ones that run nothing).
    fn menu_labels() -> Vec<(&'static str, &'static str, Value)> {
        fn walk(items: &[Item], out: &mut Vec<(&'static str, &'static str, Value)>) {
            for i in items {
                match i {
                    Item::Cmd(l, id, p) => out.push((l, id, p.clone())),
                    Item::Todo(l, _) | Item::Header(l) => out.push((l, "", Value::Null)),
                    Item::Sub(l, children) => {
                        out.push((l, "", Value::Null));
                        walk(children, out);
                    }
                    Item::Sep => {}
                }
            }
        }
        let mut labels = vec![];
        for (title, items) in crate::menus::menu_tree() {
            labels.push((title, "", Value::Null));
            walk(&items, &mut labels);
        }
        labels
    }

    /// The labels toggling items switch to ("Show Guides" once guides are hidden…), from a
    /// document with every toggle flipped.
    fn toggled_labels() -> Vec<String> {
        let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("object.envelope.makeWithWarp", json!({})).unwrap();
        let toggles = [
            "view.outline",
            "view.edges",
            "view.cornerWidget",
            "view.textThreads",
            "type.hiddenCharacters",
            "view.gradientAnnotator",
            "view.artboards",
            "view.rulers",
            "view.boundingBox",
            "view.transparencyGrid",
            "view.guides",
            "view.grid",
            "view.guides.lock",
            "view.slices.hide",
            "view.printTiling",
            "perspective.grid.show",
            "perspective.grid.rulers",
            "perspective.grid.lock",
            "object.envelope.editContents",
        ];
        let mut labels = vec![];
        for id in toggles {
            let before = crate::menus::dynamic_label(&app, id, "");
            app.run(id, json!({})).unwrap();
            let after = crate::menus::dynamic_label(&app, id, "");
            assert_ne!(before, after, "{id} didn't toggle its label");
            labels.extend([before, after]);
        }
        labels
    }

    #[test]
    fn translations_are_unique_and_preserve_unknown_text() {
        for (language, table) in TABLES {
            for (i, (en, translated)) in table.iter().enumerate() {
                assert!(!translated.is_empty());
                assert!(table.iter().take(i).all(|(other, _)| en != other), "`{en}` twice");
                assert_eq!(en.ends_with('…'), translated.ends_with('…'), "`{en}` and `{translated}` differ in their ellipsis");
                assert_eq!(Language::En.tr(en), *en);
                assert_eq!(language.tr(en), *translated);
            }
            assert_eq!(language.tr("日本語の文書.pdf"), "日本語の文書.pdf");
        }
        assert_eq!(Language::Ja.tr("File"), "ファイル");
        assert_eq!(Language::Cs.tr("File"), "Soubor");
        assert_eq!(Language::parse("xx"), None);
        for language in Language::ALL {
            let code = serde_json::to_value(language).unwrap();
            assert_eq!(Language::parse(code.as_str().unwrap()), Some(language));
        }
        assert_eq!(serde_json::to_value(Language::Cs).unwrap(), json!("cs"));
        assert_eq!(Language::Cs.name(), "Čeština");
    }

    #[test]
    fn every_translated_label_is_a_menu_label() {
        let mut labels: Vec<String> = menu_labels().into_iter().map(|(l, ..)| l.to_string()).collect();
        // Object › Plug-ins' header leaves the menu while any test has a filter plug-in installed.
        labels.push(crate::dialogs::plugin::NO_FILTERS.into());
        labels.extend(toggled_labels());
        for (language, table) in TABLES {
            for (en, _) in table {
                assert!(labels.iter().any(|l| l == en), "{language:?}: `{en}` isn't a VectorCraft menu label");
            }
        }
    }

    #[test]
    fn czech_translates_every_menu_label() {
        let translated = |l: &str| CZECH.iter().any(|(en, _)| *en == l);
        let labels = menu_labels();
        // Font names and sizes, and installed plug-ins' own names, are not interface text.
        let interface = |(label, id, p): &(&str, &str, Value)| {
            let plugin = *id == "plugin.dialog" || p.get("effect").and_then(Value::as_str).is_some_and(|e| e.starts_with("plugin."));
            *id != "text.setStyle" && !plugin && !KEEP_AS_IS.contains(label)
        };
        let mut missing: Vec<String> = labels.iter().filter(|l| interface(l)).map(|(l, ..)| l.to_string()).collect();
        missing.extend(toggled_labels());
        missing.retain(|l| !translated(l));
        missing.dedup();
        assert!(missing.is_empty(), "untranslated Czech menu labels: {missing:?}");
        for keep in KEEP_AS_IS {
            assert!(labels.iter().any(|(l, ..)| l == keep), "`{keep}` isn't a menu label");
            assert!(!translated(keep), "`{keep}` is both kept and translated");
        }
    }

    #[test]
    fn language_command_validates_and_persists_without_a_document() {
        let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        crate::menus::run_ui_command(&mut app, "app.language", &json!({"lang": "ja"})).unwrap().unwrap();
        assert_eq!(app.ui.language, Language::Ja);
        assert_eq!(crate::menus::checked(&app, "app.language", &json!({"lang": "ja"})), Some(true));
        assert!(crate::menus::run_ui_command(&mut app, "app.language", &json!({"lang": "xx"})).unwrap().is_err());
        assert_eq!(app.ui.language, Language::Ja);
        let saved = serde_json::to_string(&app.ui).unwrap();
        let restored: crate::state::UiState = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.language, Language::Ja);
        assert_eq!(crate::menus::run_ui_command(&mut app, "app.language", &json!({"lang": "cs"})).unwrap(), Ok(json!("cs")));
        assert_eq!(app.ui.language, Language::Cs);
        assert_eq!(crate::menus::checked(&app, "app.language", &json!({"lang": "cs"})), Some(true));
        assert_eq!(crate::menus::checked(&app, "app.language", &json!({"lang": "ja"})), Some(false));
        let restored: crate::state::UiState = serde_json::from_str(&serde_json::to_string(&app.ui).unwrap()).unwrap();
        assert_eq!(restored.language, Language::Cs);
    }

    #[test]
    fn the_language_menu_offers_every_language() {
        let items: Vec<_> = menu_labels().into_iter().filter(|(_, id, _)| *id == "app.language").collect();
        assert_eq!(items.len(), Language::ALL.len());
        for language in Language::ALL {
            let code = serde_json::to_value(language).unwrap();
            assert!(items.iter().any(|(l, _, p)| *l == language.name() && p["lang"] == code), "{language:?} isn't in the Language menu");
        }
    }

    #[test]
    fn japanese_glyphs_are_available_without_system_fonts() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.fonts_mut(|fonts| {
            let families: Vec<_> = fonts.definitions().families.keys().cloned().collect();
            for family in families {
                let font = egui::FontId::new(13.0, family);
                for ch in "日本語ファイル編集".chars() {
                    assert!(fonts.has_glyph(&font, ch), "missing {ch} in {font:?}");
                }
            }
        });
    }

    /// Czech letters (and the punctuation Czech text uses) come from each family's own first font,
    /// not from a fallback further down the stack. (`has_glyph` can't tell: it counts characters of
    /// the face that draws missing glyphs, the first one, as missing.)
    #[test]
    fn czech_glyphs_are_available_without_system_fonts() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.fonts_mut(|fonts| {
            let families: Vec<_> = fonts.definitions().families.iter().map(|(f, stack)| (f.clone(), stack.first().cloned())).collect();
            for (family, first) in families {
                let first = first.unwrap();
                let mut font = fonts.fonts.font(&family);
                let chars = font.characters();
                for ch in "áčďéěíňóřšťúůýžÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ„“‚‘…–".chars() {
                    assert!(chars.get(&ch).is_some_and(|fonts| fonts.contains(&first)), "{first} ({family:?}) has no {ch}");
                }
            }
        });
    }
}
