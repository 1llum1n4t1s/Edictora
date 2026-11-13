//! ページ内で実行するスクリプトの式を組み立てる（C# の `PageScripts`）。
//!
//! スクリプト本体は page-scripts/src/edictora.ts（TypeScript）にあり、Bun でまとめた
//! page-scripts/dist/edictora.js を `include_str!` で埋め込む（cargo build と配布 exe に Bun は要らない）。
//! 生成物が最新かは page-scripts で `bun run check` を実行して確かめる。
//!
//! - 本体は `window.__edictora` に API を 1 回だけ導入する（収集した要素の配列を評価間で保つ）
//! - スクリプトには利用者の入力値を埋め込まない。埋め込むのは種類名（固定の文字列）と位置（整数）だけ
//! - 常設の案内通知の文言（`__EDICTORA_GUIDE__`）は、ここで Rust 側の定数を JSON 文字列にして差し込む

use std::sync::OnceLock;

use crate::dashboard::text::GUIDE_NOTICE_MARKER;

/// Bun で生成したページ内スクリプト（導入部）
pub const BUNDLE: &str = include_str!("../../../../page-scripts/dist/edictora.js");

/// 案内通知の文言を差し込む位置
pub const GUIDE_PLACEHOLDER: &str = "__EDICTORA_GUIDE__";

/// 要素の種類（JS 側の収集関数名）
pub mod kinds {
    pub const COMBOBOX: &str = "combobox";
    pub const OPTION: &str = "option";
    pub const BUTTON: &str = "button";
    pub const TEXTAREA: &str = "textarea";
    pub const TEXT_INPUT: &str = "textInput";
    pub const DIALOG_BUTTON: &str = "dialogButton";
    pub const DIALOG_CHECKBOX: &str = "dialogCheckbox";
}

/// 文言を差し込んだ導入部
pub fn prelude() -> &'static str {
    static PRELUDE: OnceLock<String> = OnceLock::new();
    PRELUDE.get_or_init(|| {
        let guide = serde_json::to_string(GUIDE_NOTICE_MARKER).expect("文字列の JSON 化");
        BUNDLE.replace(GUIDE_PLACEHOLDER, &guide)
    })
}

/// `window.__edictora.<call>` を評価する式を作る
fn call(call: &str) -> String {
    format!(
        "(() => {{\n{}\nreturn window.__edictora.{call};\n}})()",
        prelude()
    )
}

fn quote(kind: &str) -> String {
    serde_json::to_string(kind).expect("文字列の JSON 化")
}

pub fn collect(kind: &str) -> String {
    call(&format!("collect({})", quote(kind)))
}

pub fn scroll_to(kind: &str, index: i32) -> String {
    call(&format!("scrollTo({}, {index})", quote(kind)))
}

pub fn point(kind: &str, index: i32) -> String {
    call(&format!("point({}, {index})", quote(kind)))
}

pub fn focus_select_all(kind: &str, index: i32) -> String {
    call(&format!("focusSelectAll({}, {index})", quote(kind)))
}

pub fn value(kind: &str, index: i32) -> String {
    call(&format!("value({}, {index})", quote(kind)))
}

pub fn messages() -> String {
    call("messages()")
}

pub fn dialogs() -> String {
    call("dialogs()")
}

pub fn body_text() -> String {
    call("bodyText()")
}
