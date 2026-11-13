//! 版（SemVer 2.0）の解釈と比較。
//!
//! Velopack の SDK は `semver` crate の `Version::parse` と `>` で版を比べる（vendored の manager.rs の
//! `check_for_updates`）。crate を増やさないため、同じ規則（`major.minor.patch[-pre][+build]`、
//! 数値の先頭 0 は不可、前後の空白は不可）をここで実装する。
//!
//! 比較は SemVer 2.0 の優先順位どおり（major → minor → patch → プレリリース。プレリリース付きは無しより低い）。
//! ビルドメタデータ（`+` 以降）は SemVer の規定どおり優先順位に含めない（`semver` crate は最後の決め手に
//! 使うが、Velopack の配布物の版にビルドメタデータは付かないので結果は変わらない）。

use std::cmp::Ordering;
use std::fmt;

/// プレリリースの識別子 1 つ
#[derive(Debug, Clone, PartialEq, Eq)]
enum PreId {
    /// 数字だけ（数値として比べる）
    Numeric(u64),
    /// 英数字と `-`（ASCII 順で比べる）
    Alpha(String),
}

impl PartialOrd for PreId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PreId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreId::Numeric(a), PreId::Numeric(b)) => a.cmp(b),
            // 数字だけの識別子は、英字を含む識別子より低い
            (PreId::Numeric(_), PreId::Alpha(_)) => Ordering::Less,
            (PreId::Alpha(_), PreId::Numeric(_)) => Ordering::Greater,
            (PreId::Alpha(a), PreId::Alpha(b)) => a.cmp(b),
        }
    }
}

/// 版
#[derive(Debug, Clone)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pre: Vec<PreId>,
    /// 元の文字列（表示用。ビルドメタデータも含む）
    text: String,
}

impl Version {
    /// 文字列を解釈する（SemVer 2.0 として正しくなければ None）
    pub fn parse(text: &str) -> Option<Version> {
        if text.is_empty() || text.trim() != text {
            return None;
        }
        // ビルドメタデータ（+ 以降）を分ける。識別子の形だけ確かめる
        let (main_pre, build) = match text.split_once('+') {
            Some((a, b)) => (a, Some(b)),
            None => (text, None),
        };
        if let Some(build) = build
            && !build.split('.').all(is_valid_identifier)
        {
            return None;
        }
        let (main, pre) = match main_pre.split_once('-') {
            Some((a, b)) => (a, Some(b)),
            None => (main_pre, None),
        };
        let mut parts = main.split('.');
        let major = parse_number(parts.next()?)?;
        let minor = parse_number(parts.next()?)?;
        let patch = parse_number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(p) => {
                let mut ids = Vec::new();
                for id in p.split('.') {
                    if !is_valid_identifier(id) {
                        return None;
                    }
                    if id.bytes().all(|b| b.is_ascii_digit()) {
                        ids.push(PreId::Numeric(parse_number(id)?));
                    } else {
                        ids.push(PreId::Alpha(id.to_string()));
                    }
                }
                ids
            }
        };
        Some(Version {
            major,
            minor,
            patch,
            pre,
            text: text.to_string(),
        })
    }

    /// プレリリースか
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

/// 数値部分（先頭 0 は「0」そのものだけ許す）
fn parse_number(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if s.len() > 1 && s.starts_with('0') {
        return None;
    }
    s.parse().ok()
}

/// 識別子（英数字と `-`、空は不可）
fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // プレリリース無しの方が高い
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                // 識別子を順に比べ、すべて等しければ識別子の多い方が高い（Vec の辞書順と同じ）
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}
