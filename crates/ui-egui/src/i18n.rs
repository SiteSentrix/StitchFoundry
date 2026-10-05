//! Interface translations. Command ids, document text and file names remain stable.
//! Untranslated labels fall back to English so coverage can grow incrementally.

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
    ("Image", "画像"),
    ("Layer", "レイヤー"),
    ("Type", "書式"),
    ("Select", "選択"),
    ("Filter", "フィルター"),
    ("Window", "ウィンドウ"),
    ("Language", "表示言語"),
    ("Save As…", "別名で保存…"),
    ("Exit", "終了"),
    ("New…", "新規…"),
    ("New", "新規"),
    ("Horizontal", "横書き"),
    ("Vertical", "縦書き"),
    ("Orientation", "組み方向"),
    ("Type Orientation", "組み方向"),
    ("Layers", "レイヤー"),
    ("History", "履歴"),
    ("Properties", "プロパティ"),
    ("Color", "カラー"),
    ("Brush Settings", "ブラシ設定"),
    ("Tools", "ツール"),
    ("Options", "オプション"),
    ("Zoom In", "ズームイン"),
    ("Zoom Out", "ズームアウト"),
    ("Fit on Screen", "画面に合わせる"),
    ("Copy", "コピー"),
    ("Cut", "切り取り"),
    ("Paste", "貼り付け"),
    ("Select All", "すべて選択"),
    ("Deselect", "選択を解除"),
    ("Export", "書き出し"),
    ("Export As…", "形式を指定して書き出し…"),
    ("Search…", "検索…"),
    ("Theme", "テーマ"),
    ("Menu", "メニュー"),
    ("File", "ファイル"),
    ("Edit", "編集"),
    ("Pages", "ページ"),
    ("View", "表示"),
    ("Help", "ヘルプ"),
    ("Preferences", "環境設定"),
    ("Preferences…", "環境設定…"),
    ("Interface language", "表示言語"),
    ("Open…", "開く…"),
    ("New blank PDF", "空白の PDF を作成"),
    ("Create PDF from file…", "ファイルから PDF を作成…"),
    ("Create PDF from images…", "画像から PDF を作成…"),
    ("Create PDF from clipboard", "クリップボードから PDF を作成"),
    ("Combine files…", "ファイルを結合…"),
    ("Save", "保存"),
    ("Save as…", "別名で保存…"),
    ("Close file", "ファイルを閉じる"),
    ("Close all", "すべて閉じる"),
    ("Revert", "保存済みの状態に戻す"),
    ("Print…", "印刷…"),
    ("Document properties…", "文書のプロパティ…"),
    ("Undo", "取り消し"),
    ("Redo", "やり直し"),
    ("Find…", "検索…"),
    ("Advanced search…", "高度な検索…"),
    ("Copy pages", "ページをコピー"),
    ("Cut pages", "ページを切り取り"),
    ("Paste pages", "ページを貼り付け"),
    ("Fit visible", "表示範囲に合わせる"),
    ("Marquee zoom", "範囲指定ズーム"),
    ("Take a snapshot", "スナップショットを作成"),
    ("Full screen mode", "全画面表示"),
    ("Read mode", "閲覧モード"),
    ("Switch light / dark theme", "明るい／暗いテーマを切り替え"),
    ("Comments panel", "コメントパネル"),
    ("Form fields panel", "フォームフィールドパネル"),
    ("Clear form", "フォームをクリア"),
    ("Find tools and commands…", "ツールとコマンドを検索…"),
    ("Zoom", "ズーム"),
    ("Actual size", "実際のサイズ"),
    ("Zoom to page level", "ページ全体を表示"),
    ("Fit to width", "幅に合わせる"),
    ("Display theme", "表示テーマ"),
    ("Side panels", "サイドパネル"),
    ("OK", "OK"),
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
