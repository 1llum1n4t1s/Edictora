//! ページ内で実行するスクリプトの式を組み立てる（C# の `PageScripts`）。
//!
//! スクリプト本体は page-scripts/src/edictora.ts（TypeScript）にあり、Bun でまとめた
//! page-scripts/dist/edictora.js を `include_str!` で埋め込む（cargo build と配布 exe に Bun は要らない）。
//! 生成物が最新かは page-scripts で `bun run check` を実行して確かめる。
//!
//! - 本体は `window.__edictora` に API を 1 回だけ導入する（収集した要素の配列を評価間で保つ）
//! - スクリプトには利用者の入力値を埋め込まない。埋め込むのは種類名（固定の文字列）と位置（整数）だけ
//! - 常設の案内通知の文言（`__EDICTORA_GUIDE__`）は、ここで画面の言語の文言セットの値を JSON にして差し込む
//!   （無い言語では `null`。空文字にすると、すべてのダイアログが「案内通知」として除外されてしまうため）
//! - 導入部は画面の言語ごとに作る。ページ側は最初に評価した導入部で `window.__edictora` を作り、
//!   以降は作り直さない。そのため画面の言語は、導入部を含む式を評価する前（`open_item` / `reload` の直後）に判定する

use std::sync::OnceLock;

use crate::dashboard::text::{DashboardLang, DashboardText};

/// Bun で生成したページ内スクリプト（導入部）
pub const BUNDLE: &str = include_str!("../../../../page-scripts/dist/edictora.js");

/// 案内通知の文言を差し込む位置
pub const GUIDE_PLACEHOLDER: &str = "__EDICTORA_GUIDE__";

/// 画面の言語を判定するための式（`<html lang>` を読むだけ）。
///
/// 導入部を含まない固定の式で、利用者の入力値も埋め込まない。値の解釈は `selectors::dashboard_lang` で行う
pub const DOCUMENT_LANG: &str = "document.documentElement ? document.documentElement.lang : ''";

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

/// 文言を差し込んだ導入部（画面の言語ごとに 1 回だけ作る）
pub fn prelude(t: &DashboardText) -> &'static str {
    static JA: OnceLock<String> = OnceLock::new();
    static EN: OnceLock<String> = OnceLock::new();
    let cell = match t.lang {
        DashboardLang::Ja => &JA,
        DashboardLang::En => &EN,
    };
    cell.get_or_init(|| {
        // None は JSON の null になる（ページ側は null なら案内通知を除外しない）
        let guide = serde_json::to_string(&t.guide_notice_marker).expect("文字列の JSON 化");
        BUNDLE.replace(GUIDE_PLACEHOLDER, &guide)
    })
}

/// `window.__edictora.<call>` を評価する式を作る
fn call(t: &DashboardText, call: &str) -> String {
    format!(
        "(() => {{\n{}\nreturn window.__edictora.{call};\n}})()",
        prelude(t)
    )
}

fn quote(kind: &str) -> String {
    serde_json::to_string(kind).expect("文字列の JSON 化")
}

pub fn collect(t: &DashboardText, kind: &str) -> String {
    call(t, &format!("collect({})", quote(kind)))
}

pub fn scroll_to(t: &DashboardText, kind: &str, index: i32) -> String {
    call(t, &format!("scrollTo({}, {index})", quote(kind)))
}

pub fn point(t: &DashboardText, kind: &str, index: i32) -> String {
    call(t, &format!("point({}, {index})", quote(kind)))
}

pub fn focus_select_all(t: &DashboardText, kind: &str, index: i32) -> String {
    call(t, &format!("focusSelectAll({}, {index})", quote(kind)))
}

pub fn value(t: &DashboardText, kind: &str, index: i32) -> String {
    call(t, &format!("value({}, {index})", quote(kind)))
}

pub fn messages(t: &DashboardText) -> String {
    call(t, "messages()")
}

pub fn dialogs(t: &DashboardText) -> String {
    call(t, "dialogs()")
}

pub fn body_text(t: &DashboardText) -> String {
    call(t, "bodyText()")
}
