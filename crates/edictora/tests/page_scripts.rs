//! 埋め込んだページ内スクリプト（page-scripts/dist/edictora.js）の組み立てを確かめる。Rust 版で足したテスト。

use edictora::dashboard::page_scripts::{
    BUNDLE, GUIDE_PLACEHOLDER, collect, kinds, prelude, value,
};
use edictora::dashboard::selectors::{parse_elements, parse_strings};
use serde_json::json;

#[test]
fn 案内通知の文言を差し込む() {
    assert!(
        BUNDLE.contains(GUIDE_PLACEHOLDER),
        "生成物に差し込み位置がありません"
    );
    let p = prelude();
    assert!(!p.contains(GUIDE_PLACEHOLDER));
    assert!(p.contains("\"ガイドやヒント\""));
}

#[test]
fn 導入は1回だけで収集した要素を評価間で保つ() {
    // window.__edictora が既にあれば作り直さない（store を保つ）ガードが最外側にあること
    assert!(BUNDLE.contains("if (!host.__edictora)"));
    assert!(BUNDLE.contains("var host = window;"));
}

#[test]
fn 式は種類と位置だけを埋め込む() {
    let e = collect(kinds::TEXT_INPUT);
    assert!(
        e.trim_end()
            .ends_with("return window.__edictora.collect(\"textInput\");\n})()")
    );
    let v = value(kinds::TEXTAREA, 3);
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
