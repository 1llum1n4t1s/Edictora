//! C# の SectionExtractor.test.cs の移植（実フィクスチャで ja=1492 文字・en=3172 文字、CRLF でも同じ結果）

mod common;

use common::fixture;
use edictora::Error;
use edictora::config::TextSource;
use edictora::config::section::{extract, extract_source, is_js_whitespace, validate_pattern};
use edictora::util::utf16_len;
use sha2::{Digest, Sha256};

// 期待値は 2026-11-13 の検証スクリプト（push.mjs と同じ切り出し関数）を Bun で実行して得たもの。
const JA_SHA256: &str = "034207b5b6dce2d8fde506c2ef269998b88b267ceca0d69dff5151f3d2608839";
const EN_SHA256: &str = "5e9f8dfa9191fa32d6d7beff9d65a50fa97873b72b69bb19a4af9d2cc1905145";

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn source(name: &str, start: &str, end: &str) -> TextSource {
    TextSource {
        file_path: fixture(name),
        section_start: Some(start.to_string()),
        section_end: Some(end.to_string()),
    }
}

fn config_err(r: edictora::Result<String>) -> String {
    match r {
        Err(Error::Config(m)) => m,
        other => panic!("Config エラーを期待しました: {other:?}"),
    }
}

#[test]
fn extract_replace_translatorの日本語説明が検証済みの切り出しと一致する() {
    let text = extract_source(&source("store-listing.txt", "^■ 説明", "^■ カテゴリ")).unwrap();

    assert_eq!(utf16_len(&text), 1492);
    assert_eq!(sha256(&text), JA_SHA256);
    assert!(
        text.starts_with("お好みのクラウド LLM を使って、表示中の Web ページをその場で置換翻訳す")
    );
}

#[test]
fn extract_replace_translatorの英語説明が検証済みの切り出しと一致する() {
    let text = extract_source(&source(
        "store-listing.en.txt",
        "^■ Description",
        "^■ Category",
    ))
    .unwrap();

    assert_eq!(utf16_len(&text), 3172);
    assert_eq!(sha256(&text), EN_SHA256);
    assert!(text.starts_with("Translate the web page you are viewing i"));
}

#[test]
fn extract_crlfのファイルでも同じ結果になる() {
    let lf = std::fs::read_to_string(fixture("store-listing.txt"))
        .unwrap()
        .replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");

    let text = extract(&crlf, Some("^■ 説明"), Some("^■ カテゴリ"), "正本").unwrap();

    assert_eq!(sha256(&text), JA_SHA256);
    assert!(!text.contains('\r'));
}

#[test]
fn extract_見出しの次から終端見出しの手前までを前後の空行を除いて返す() {
    let src = "■ 概要\n短い\n■ 説明\n\n\n  一行目\n二行目  \n\n■ カテゴリ\n仕事効率化\n";

    let text = extract(src, Some("^■ 説明"), Some("^■ カテゴリ"), "正本").unwrap();

    // 先頭は空行だけを落とし行頭の空白は残す。末尾の空白・空行は落とす。
    assert_eq!(text, "  一行目\n二行目");
}

#[test]
fn extract_末尾の全角空白もjsの空白として落とす() {
    // 全角空白（U+3000）と NBSP（U+00A0）は JS の \s に含まれる
    let src = "■ A\n本文\u{3000}\u{00A0}\n■ B";
    let text = extract(src, Some("^■ A"), Some("^■ B"), "正本").unwrap();
    assert_eq!(text, "本文");
}

#[test]
fn extract_終端見出しは開始見出しより後ろだけを探す() {
    let text = extract(
        "■ B\n前\n■ A\n本文\n■ B\n後",
        Some("^■ A"),
        Some("^■ B"),
        "正本",
    )
    .unwrap();
    assert_eq!(text, "本文");
}

#[test]
fn extract_開始見出しが無ければ例外() {
    let m = config_err(extract(
        "本文",
        Some("^■ 説明"),
        Some("^■ カテゴリ"),
        "正本",
    ));
    assert!(m.contains("開始見出し"));
}

#[test]
fn extract_終端見出しが無ければ例外() {
    let m = config_err(extract(
        "■ 説明\n本文",
        Some("^■ 説明"),
        Some("^■ カテゴリ"),
        "正本",
    ));
    assert!(m.contains("終端見出し"));
}

#[test]
fn extract_区間指定なしはファイル全体を同じ規則で整える() {
    let text = extract("\r\n\r\n本文1\r\n本文2\r\n\r\n", None, None, "正本").unwrap();
    assert_eq!(text, "本文1\n本文2");
}

#[test]
fn extract_区間指定が片方だけなら例外() {
    config_err(extract("本文", Some("^■ 説明"), None, "正本"));
}

#[test]
fn validate_pattern_壊れた正規表現は例外() {
    assert!(matches!(
        validate_pattern("([", "sectionStart"),
        Err(Error::Config(_))
    ));
}

#[test]
fn is_js_whitespace_jsの空白定義に合わせる() {
    assert!(is_js_whitespace('\u{3000}'));
    assert!(is_js_whitespace('\u{FEFF}'));
    assert!(is_js_whitespace('\u{2005}'));
    // .NET / Rust では空白だが JS の \s ではない
    assert!(!is_js_whitespace('\u{0085}'));
    assert!(!is_js_whitespace('a'));
}

// ───────── Rust 版で足したもの ─────────

#[test]
fn validate_pattern_先読みや後方参照はrustのregexでは使えない() {
    // README / DESIGN.md に書いた制約を確かめる（.NET では使えた構文）
    assert!(matches!(
        validate_pattern("^■ 説明(?=$)", "sectionStart"),
        Err(Error::Config(_))
    ));
    assert!(matches!(
        validate_pattern(r"^(■)\1", "sectionStart"),
        Err(Error::Config(_))
    ));
}
