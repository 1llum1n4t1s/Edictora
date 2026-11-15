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
          "type": "", "expanded": "false", "checked": true }
    ]));
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].index, 2);
    assert!(elements[0].read_only);
    assert_eq!(elements[0].r#type, "url");
    assert_eq!(elements[0].expanded, None);
    assert_eq!(elements[1].expanded.as_deref(), Some("false"));
    assert_eq!(elements[1].checked, Some(true));
    assert!(parse_elements(&json!(null)).is_empty());
    assert_eq!(parse_strings(&json!(["a", 1, "b"])), vec!["a", "", "b"]);
}
