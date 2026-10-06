//! Interface translations. Command ids, document text and file names remain stable.
//! Untranslated labels fall back to English so coverage can grow incrementally. Every entry is a
//! label VectorCraft's menus show (tested), so labels from other apps don't creep in.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ja,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::En, Self::Ja];

    pub fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }

    pub fn tr(self, text: &str) -> &str {
        if self == Self::Ja
            && let Some((_, japanese)) = JAPANESE.iter().find(|(english, _)| *english == text)
        {
            return japanese;
        }
        text
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
    use super::*;

    #[test]
    fn translations_are_unique_and_preserve_unknown_text() {
        for (i, (en, ja)) in JAPANESE.iter().enumerate() {
            assert!(!ja.is_empty());
            assert!(JAPANESE.iter().take(i).all(|(other, _)| en != other));
            assert_eq!(Language::En.tr(en), *en);
        }
        assert_eq!(Language::Ja.tr("File"), "ファイル");
        assert_eq!(Language::Ja.tr("日本語の文書.pdf"), "日本語の文書.pdf");
        assert_eq!(Language::parse("xx"), None);
    }

    #[test]
    fn every_translated_label_is_a_menu_label() {
        fn walk(items: &[crate::menus::Item], out: &mut Vec<&'static str>) {
            for i in items {
                match i {
                    crate::menus::Item::Cmd(l, ..) | crate::menus::Item::Todo(l, _) | crate::menus::Item::Header(l) => out.push(l),
                    crate::menus::Item::Sub(l, children) => {
                        out.push(l);
                        walk(children, out);
                    }
                    crate::menus::Item::Sep => {}
                }
            }
        }
        let mut labels = vec![];
        for (title, items) in crate::menus::menu_tree() {
            labels.push(title);
            walk(&items, &mut labels);
        }
        for (en, _) in JAPANESE {
            assert!(labels.contains(en), "`{en}` isn't a VectorCraft menu label");
        }
    }

    #[test]
    fn language_command_validates_and_persists_without_a_document() {
        let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        crate::menus::run_ui_command(&mut app, "app.language", &serde_json::json!({"lang": "ja"})).unwrap().unwrap();
        assert_eq!(app.ui.language, Language::Ja);
        assert_eq!(crate::menus::checked(&app, "app.language", &serde_json::json!({"lang": "ja"})), Some(true));
        assert!(crate::menus::run_ui_command(&mut app, "app.language", &serde_json::json!({"lang": "xx"})).unwrap().is_err());
        assert_eq!(app.ui.language, Language::Ja);
        let saved = serde_json::to_string(&app.ui).unwrap();
        let restored: crate::state::UiState = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.language, Language::Ja);
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
}
