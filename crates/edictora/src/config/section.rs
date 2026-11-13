//! 正本ファイルから説明文を取り出す（C# の `SectionExtractor`）。
//!
//! 2026-11-13 の検証で使った puppeteer スクリプト（push.mjs）の切り出しと、完全に同じ結果を返すことが要件。
//! push.mjs の処理:
//! ```js
//! const lines = text.replace(/\r\n/g, "\n").split("\n");
//! const s = lines.findIndex((l) => start.test(l));
//! const e = lines.findIndex((l, i) => i > s && end.test(l));
//! return lines.slice(s + 1, e).join("\n").replace(/^\n+/, "").replace(/\s+$/, "");
//! ```
//! 差異と注意点:
//! - JS 版は見出しが見つからないと黙って誤った範囲を切り出す（slice に -1 が入る）。ここではエラーで止める。
//! - 単独の CR も LF に寄せる（改行は \n に正規化する方針。CRLF だけのファイルでは JS 版と同じ結果）。
//! - 先頭は「空行（\n）」だけを落とし、行頭の空白は残す。末尾は JS の \s 相当の空白をすべて落とす。
//!   Rust の `char::is_whitespace` は JS の \s と範囲が少し違う（U+0085 / U+FEFF）ため、JS の定義を自前で持つ。
//! - 正規表現は 1 行ずつに当てる。
//! - ファイル全体を使う場合も、同じ改行正規化と前後の除去を行う。
//!
//! **正規表現の方言**: Rust の `regex` crate を使う。C# 版（.NET の Regex）と違い、
//! 先読み・後読み（`(?=…)` `(?!…)` `(?<=…)` `(?<!…)`）と後方参照（`\1`）は使えない（設定読込時にエラーになる）。
//! その代わり、どんなパターンでも線形時間で終わる（C# 版にあった 2 秒の打ち切りは要らない）。

use regex::Regex;

use crate::config::TextSource;
use crate::error::{Error, Result};
use crate::util::read_text_file;

/// ファイルを読み、`source` の指定どおりに説明文を取り出す
pub fn extract_source(source: &TextSource) -> Result<String> {
    let text = read_text_file(&source.file_path).map_err(|e| {
        Error::Config(format!(
            "正本ファイルを読めません: {}（{e}）",
            source.file_path.display()
        ))
    })?;
    extract(
        &text,
        source.section_start.as_deref(),
        source.section_end.as_deref(),
        &source.file_path.display().to_string(),
    )
}

/// 文字列から説明文を取り出す。`section_start` と `section_end` が
/// どちらも None ならファイル全体を、両方あれば見出し行の次から終端見出しの手前までを返す。
pub fn extract(
    text: &str,
    section_start: Option<&str>,
    section_end: Option<&str>,
    source_name: &str,
) -> Result<String> {
    let normalized = normalize_new_lines(text);
    let (start, end) = match (section_start, section_end) {
        (None, None) => return Ok(trim_like_spike(&normalized).to_string()),
        (Some(s), Some(e)) => (s, e),
        _ => {
            return Err(Error::Config(format!(
                "sectionStart と sectionEnd は両方指定してください: {source_name}"
            )));
        }
    };

    let start_re = create_regex(start, "sectionStart")?;
    let end_re = create_regex(end, "sectionEnd")?;
    let lines: Vec<&str> = normalized.split('\n').collect();

    let s = lines
        .iter()
        .position(|l| start_re.is_match(l))
        .ok_or_else(|| {
            Error::Config(format!(
                "開始見出し（{start}）が見つかりません: {source_name}"
            ))
        })?;
    let e = lines
        .iter()
        .enumerate()
        .skip(s + 1)
        .find(|(_, l)| end_re.is_match(l))
        .map(|(i, _)| i)
        .ok_or_else(|| {
            Error::Config(format!(
                "終端見出し（{end}）が開始見出しより後に見つかりません: {source_name}"
            ))
        })?;

    Ok(trim_like_spike(&lines[s + 1..e].join("\n")).to_string())
}

/// 正規表現の構文を検証する（設定読込時の早期検出用）
pub fn validate_pattern(pattern: &str, name: &str) -> Result<()> {
    create_regex(pattern, name).map(|_| ())
}

/// CRLF と単独 CR を LF に寄せる
pub fn normalize_new_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 先頭の連続した改行と、末尾の空白（JS の \s 相当）を落とす
pub fn trim_like_spike(text: &str) -> &str {
    text.trim_start_matches('\n')
        .trim_end_matches(is_js_whitespace)
}

/// JavaScript の正規表現 \s に含まれる文字か（ECMAScript の WhiteSpace + LineTerminator）
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c as u32,
        0x09 | 0x0A
            | 0x0B
            | 0x0C
            | 0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
            | 0x2000..=0x200A
    )
}

fn create_regex(pattern: &str, name: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|e| {
        Error::Config(format!(
            "{name} の正規表現が正しくありません: {pattern}（{e}）。先読み・後読み・後方参照は使えません"
        ))
    })
}
