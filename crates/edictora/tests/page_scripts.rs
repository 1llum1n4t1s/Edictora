//! 埋め込んだページ内スクリプト（page-scripts/dist/edictora.js）の組み立てを確かめる。Rust 版で足したテスト。

use edictora::dashboard::page_scripts::{
    BUNDLE, DOCUMENT_LANG, GUIDE_PLACEHOLDER, collect, kinds, prelude, value,
};
use edictora::dashboard::selectors::{
    ELEMENT_GONE_MARKER, OFFSCREEN_COVER, parse_elements, parse_strings,
};
use edictora::dashboard::text::{EN, JA};
use serde_json::json;

#[test]
fn 案内通知の文言を差し込む() {
    assert!(
        BUNDLE.contains(GUIDE_PLACEHOLDER),
        "生成物に差し込み位置がありません"
    );
    let p = prelude(&JA);
    assert!(!p.contains(GUIDE_PLACEHOLDER));
    assert!(p.contains("\"ガイドやヒント\""));
}

#[test]
fn 案内通知の語が無い言語ではnullを差し込む() {
    // 空文字だと includes("") が常に真になり、すべてのダイアログが除外されるので null にする
    let p = prelude(&EN);
    assert!(!p.contains(GUIDE_PLACEHOLDER));
    assert!(p.contains("const GUIDE = null;"));
    assert!(!p.contains("ガイドやヒント"));
    // ページ側は null を「除外しない」として扱う
    assert!(BUNDLE.contains("GUIDE !== null && text(d).includes(GUIDE)"));
    // 言語ごとに別の導入部
    assert_ne!(prelude(&JA), prelude(&EN));
}

#[test]
fn 言語の判定式は導入部を含まない固定の式() {
    assert!(!DOCUMENT_LANG.contains("__edictora"));
    assert!(DOCUMENT_LANG.contains("document.documentElement.lang"));
}

#[test]
fn ページ内スクリプトの固定の印がrust側と一致する() {
    // 利用者向けの文は Rust 側で作るので、ページ側は固定の印だけを返す（日本語の文を持たない）
    assert!(BUNDLE.contains(&format!("\"{ELEMENT_GONE_MARKER}\" + kind")));
    assert!(BUNDLE.contains(&format!("\"{OFFSCREEN_COVER}\"")));
    assert!(!BUNDLE.contains("要素が画面から消えました"));
}

#[test]
fn 導入は1回だけで収集した要素を評価間で保つ() {
    // window.__edictora が既にあれば作り直さない（store を保つ）ガードが最外側にあること
    assert!(BUNDLE.contains("if (!host.__edictora)"));
    assert!(BUNDLE.contains("var host = window;"));
}

#[test]
fn 式は種類と位置だけを埋め込む() {
    let e = collect(&JA, kinds::TEXT_INPUT);
    assert!(
        e.trim_end()
            .ends_with("return window.__edictora.collect(\"textInput\");\n})()")
    );
    let v = value(&JA, kinds::TEXTAREA, 3);
    assert!(v.contains("return window.__edictora.value(\"textarea\", 3);"));
}

#[test]
fn 収集結果の変換() {
    let elements = parse_elements(&json!([
        { "index": 2, "tag": "input", "text": "", "label": "公式 URL", "disabled": false, "readOnly": true,
          "type": "url", "expanded": null, "checked": null },
        { "index": 5, "tag": "div", "text": "言語", "label": "", "disabled": true, "readOnly": false,
          "type": "", "expanded": "false", "checked": true },
        // 画像の欄（element_selectors から移した assertion）
        { "index": 2, "tag": "input", "type": "file", "text": "全言語向けアセット",
          "accept": ".png,.jpg,.jpeg",
          "images": [{ "alt": "スクリーンショット 1", "src": "https://lh3.googleusercontent.com/x=w160" }],
          "width": 0, "height": 48 },
        // カテゴリの選択肢（同上）
        { "index": 4, "tag": "li", "text": "機能と UI", "label": "カテゴリ",
          "disabled": false, "readOnly": false, "type": "", "expanded": null, "checked": null,
          "dataValue": "CATEGORY_FUNCTIONALITY_AND_UI", "selected": "true", "group": 1,
          "width": 0, "height": 0 },
        { "index": 5, "tag": "li", "text": "x", "label": "カテゴリ",
          "disabled": false, "readOnly": false, "type": "", "expanded": null, "checked": null,
          "dataValue": "CATEGORY_TOOLS", "selected": "false", "group": 1,
          "width": 10, "height": 20 },
        { "index": 6, "tag": "li", "text": "y", "label": "",
          "disabled": false, "readOnly": false, "type": "", "expanded": null, "checked": null }
    ]));
    assert_eq!(elements.len(), 6);
    assert_eq!(elements[2].accept, ".png,.jpg,.jpeg");
    assert_eq!(elements[2].images.len(), 1);
    assert_eq!(elements[2].images[0].alt, "スクリーンショット 1");
    assert_eq!((elements[2].width, elements[2].height), (0, 48));
    assert_eq!(elements[3].data_value, "CATEGORY_FUNCTIONALITY_AND_UI");
    assert_eq!(elements[3].selected, Some(true));
    assert_eq!(elements[3].group, 1);
    assert_eq!(elements[4].selected, Some(false));
    assert_eq!((elements[4].width, elements[4].height), (10, 20));
    // 属性が無い要素（他の種類）は group が -1、selected が None
    assert_eq!(elements[5].selected, None);
    assert_eq!(elements[5].group, -1);
    assert_eq!(elements[5].data_value, "");
    assert_eq!(elements[0].index, 2);
    assert!(elements[0].read_only);
    assert_eq!(elements[0].r#type, "url");
    assert_eq!(elements[0].expanded, None);
    assert_eq!(elements[1].expanded.as_deref(), Some("false"));
    assert_eq!(elements[1].checked, Some(true));
    assert!(parse_elements(&json!(null)).is_empty());
    assert_eq!(parse_strings(&json!(["a", 1, "b"])), vec!["a", "", "b"]);
}

#[test]
fn 画像用の式は種類と位置だけを埋め込む() {
    use edictora::dashboard::page_scripts::{collect_slot, element};

    let e = collect(&JA, kinds::FILE_INPUT);
    assert!(e.contains("return window.__edictora.collect(\"fileInput\");"));
    let s = collect_slot(&JA, kinds::IMAGE_BUTTON, 2);
    assert!(s.contains("return window.__edictora.collectSlot(\"imageButton\", 2);"));
    let el = element(&JA, kinds::FILE_INPUT, 4);
    assert!(el.contains("return window.__edictora.element(\"fileInput\", 4);"));
    // ファイルのパスは式に入らない（set_file_input_files が CDP の引数で渡す）
    assert!(!el.contains(".png"));
}

#[test]
fn 画像の収集関数を持ち_srcが空のimgは数えない() {
    assert!(BUNDLE.contains("collectSlot"));
    assert!(BUNDLE.contains("input[type=file]"));
    // 削除したスロットに残る src の空の img を 1 枚と数えない（2026-11-15 の実画面）
    assert!(BUNDLE.contains("realImages"));
    assert!(BUNDLE.contains("img"));
    // 画像の操作（削除・アップロード・並べ替え）をページ内スクリプトから直接行わない
    assert!(!BUNDLE.contains(".click()"));
    assert!(!BUNDLE.contains("DataTransfer"));
}

#[test]
fn チェックボックスの収集はダイアログの外だけで_クリックはページ側で行わない() {
    // 「プライバシー」タブのデータ使用・宣誓のチェックボックス（ダイアログの外）
    assert!(BUNDLE.contains("checkbox: () =>"));
    assert!(BUNDLE.contains("input[type=checkbox], [role=checkbox]"));
    let e = collect(&JA, kinds::CHECKBOX);
    assert!(e.contains("return window.__edictora.collect(\"checkbox\");"));
    // 押すのは CDP の実際のマウス入力（ページ内スクリプトから .click() しない）
    assert!(!BUNDLE.contains(".click()"));
}

#[test]
fn 一覧の選択肢の収集は閉じた一覧の要素も含み情報を返すだけ() {
    use edictora::dashboard::page_scripts::kinds;
    assert_eq!(kinds::LISTBOX_OPTION, "listboxOption");
    let e = collect(&JA, kinds::LISTBOX_OPTION);
    assert!(e.contains("collect(\"listboxOption\")"));
    // ページ側: 種類の定義・選択肢の情報（data-value・aria-selected・一覧の通し番号）を持ち、可視性で絞らない
    assert!(BUNDLE.contains("listboxOption"));
    assert!(BUNDLE.contains("data-value"));
    assert!(BUNDLE.contains("aria-selected"));
    assert!(BUNDLE.contains("dataValue"));
    // どの選択肢を選ぶかの判断をページ側に持たない（カテゴリの文言・キーを埋め込まない）
    assert!(!BUNDLE.contains("カテゴリ"));
    assert!(!BUNDLE.contains("CATEGORY_"));
}
