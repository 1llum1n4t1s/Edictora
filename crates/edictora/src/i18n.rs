//! 利用者に見せる文言の表示言語（9 言語: ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）と、辞書の引き方。
//!
//! 目的: 日本語以外の利用者にも、CLI の進捗・結果・エラー・差分表示を母語で届ける。
//!
//! 設計:
//! - 文言は言語ごとの JSON 辞書（`locales/<タグ>.json`、`include_str!` で埋め込む）に持ち、コードは
//!   安定した識別子のキーで参照する（[`tr!`] / [`trs!`]）。プレースホルダーは `{name}` 形式で全言語同じ集合にする。
//!   辞書に無いキーは英語、英語にも無ければキーそのものを返す（落ちない）。整合は `tests/localization.rs` が固める。
//! - 言語はプロセスで 1 つ。初回の [`current`] で [`detect`] を 1 回だけ行い、以後は使い回す。
//!   設定ファイルや引数には頼らず、失敗しない決め方にしている（フックや引数の誤りより前でも決められる）。
//!   判定順: `EDICTORA_LANGUAGE` → `EDICTORA_LANG`（旧名。互換のために読む） → `LC_ALL` → `LC_MESSAGES` → `LANG`
//!   （空・`C`・`POSIX` は未設定扱い）→ OS の表示言語（`GetUserDefaultUILanguage`）→ 判定できなければ英語。
//! - AI が読む文言（MCP のツール説明・入力スキーマ・AI に返す応答とエラー）は英語に統一し、辞書を通さない。
//!   `edictora mcp` はプロセスの言語を [`init`] で英語に固定する（応答の本体は CLI の出力を使い回すため）。
//! - 訳さないもの: 利用者のデータ（掲載文）、識別子・コマンド・パス・設定キー、OS や外部 API のエラー原文。
//! - ログ（`logging`）の定型文は作者が原因を調べるためのものなので日本語のままにする。
//!   ただし、ログに埋め込むエラー文や更新結果の文（利用者にも見せる文）は表示言語のまま記録される。
//! - ダッシュボードの画面の文言（要素を探すための文字列）はこれとは別で、`dashboard::text` が画面の言語ごとに持つ。

use std::collections::HashMap;
use std::sync::OnceLock;

/// 表示言語
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
    ChineseSimplified,
    ChineseTraditional,
    Korean,
    Spanish,
    French,
    German,
    PortugueseBrazil,
}

impl Language {
    /// 対応する全言語。辞書ファイルの一覧・整合テストはこの並びを基準にする
    pub const ALL: [Language; 9] = [
        Language::Japanese,
        Language::English,
        Language::ChineseSimplified,
        Language::ChineseTraditional,
        Language::Korean,
        Language::Spanish,
        Language::French,
        Language::German,
        Language::PortugueseBrazil,
    ];

    /// 辞書ファイル名にも使う言語タグ
    pub fn tag(self) -> &'static str {
        match self {
            Language::Japanese => "ja",
            Language::English => "en",
            Language::ChineseSimplified => "zh-CN",
            Language::ChineseTraditional => "zh-TW",
            Language::Korean => "ko",
            Language::Spanish => "es",
            Language::French => "fr",
            Language::German => "de",
            Language::PortugueseBrazil => "pt-BR",
        }
    }

    /// 辞書の JSON 原文（コンパイル時に埋め込む）
    pub fn dictionary_source(self) -> &'static str {
        match self {
            Language::Japanese => include_str!("locales/ja.json"),
            Language::English => include_str!("locales/en.json"),
            Language::ChineseSimplified => include_str!("locales/zh-CN.json"),
            Language::ChineseTraditional => include_str!("locales/zh-TW.json"),
            Language::Korean => include_str!("locales/ko.json"),
            Language::Spanish => include_str!("locales/es.json"),
            Language::French => include_str!("locales/fr.json"),
            Language::German => include_str!("locales/de.json"),
            Language::PortugueseBrazil => include_str!("locales/pt-BR.json"),
        }
    }

    fn index(self) -> usize {
        Language::ALL
            .iter()
            .position(|language| *language == self)
            .expect("every language is listed")
    }
}

/// 表示言語を上書きする環境変数。MCP クライアントの登録や CI で言語を固定するのに使う
pub const LANGUAGE_ENV: &str = "EDICTORA_LANGUAGE";
/// 旧名の環境変数（ja / en だけだった頃の名前）。互換のために読む。両方あれば [`LANGUAGE_ENV`] が優先
pub const LEGACY_LANGUAGE_ENV: &str = "EDICTORA_LANG";

static CURRENT: OnceLock<Language> = OnceLock::new();

/// 現在の表示言語。初回の呼び出しで判定し、以後は同じ値を返す
pub fn current() -> Language {
    *CURRENT.get_or_init(detect)
}

/// 表示言語を決め打ちする（プロセスで最初の 1 回だけ有効。`current` が先に呼ばれていれば無視する）。
/// `edictora mcp` が AI 向けに英語へ固定するのに使う
pub fn init(language: Language) {
    let _ = CURRENT.set(language);
}

/// 表示言語を決める。失敗しない（フックの処理より前に呼んでも終了コードに影響しない）
pub fn detect() -> Language {
    // 明示の上書きと POSIX のロケール変数は、空・C・POSIX 以外なら最初に見つかった値で決める
    for name in [
        LANGUAGE_ENV,
        LEGACY_LANGUAGE_ENV,
        "LC_ALL",
        "LC_MESSAGES",
        "LANG",
    ] {
        if let Some(language) = std::env::var(name).ok().as_deref().and_then(parse) {
            return language;
        }
    }
    os_language().unwrap_or(Language::English)
}

/// `ja`、`ja_JP.UTF-8`、`zh-Hant-TW`、`pt_BR`、`Japanese_Japan.932` などを対応言語に照合する。
/// 照合規則: `zh-Hans`・`zh-CN`・`zh-SG` は zh-CN、`zh-Hant`・`zh-TW`・`zh-HK`・`zh-MO` は zh-TW、
/// `pt`（`pt-PT` を含む）は pt-BR、それ以外は主言語で照合する。未対応の言語は英語。
/// 空文字・`C`・`POSIX`（`C.UTF-8` を含む）は判定材料にならないため None を返す
pub fn parse(value: &str) -> Option<Language> {
    let value = value.trim();
    let base = value
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if base.is_empty() || base == "c" || base == "posix" {
        return None;
    }
    let mut subtags = base.split(['_', '-']);
    let primary = subtags.next().unwrap_or_default();
    let rest: Vec<&str> = subtags.collect();
    Some(match primary {
        "ja" | "japanese" => Language::Japanese,
        "zh" => chinese(&rest),
        // Windows の英語名ロケール（`Chinese (Traditional)_Taiwan.950` など）
        name if name.starts_with("chinese") => {
            if ["traditional", "taiwan", "hong kong", "macao", "macau"]
                .iter()
                .any(|marker| base.contains(marker))
            {
                Language::ChineseTraditional
            } else {
                Language::ChineseSimplified
            }
        }
        "ko" | "korean" => Language::Korean,
        "es" | "spanish" => Language::Spanish,
        "fr" | "french" => Language::French,
        "de" | "german" => Language::German,
        "pt" | "portuguese" => Language::PortugueseBrazil,
        _ => Language::English,
    })
}

/// `zh` の続きのタグから簡体字・繁体字を決める。文字体系（Hans/Hant）を地域より優先し、
/// どちらも無い `zh` だけの指定は簡体字とする
fn chinese(rest: &[&str]) -> Language {
    if rest.contains(&"hant") {
        return Language::ChineseTraditional;
    }
    if rest.contains(&"hans") {
        return Language::ChineseSimplified;
    }
    if rest.iter().any(|tag| ["tw", "hk", "mo"].contains(tag)) {
        return Language::ChineseTraditional;
    }
    Language::ChineseSimplified
}

/// Windows の LANGID を対応言語へ変換する。未対応の言語は英語
pub fn language_from_langid(id: u16) -> Language {
    // 下位 10 ビットが主言語、上位 6 ビットが副言語。中国語は副言語で簡体字・繁体字を分ける
    match id & 0x3ff {
        0x11 => Language::Japanese,
        0x12 => Language::Korean,
        0x0a => Language::Spanish,
        0x0c => Language::French,
        0x07 => Language::German,
        // pt-PT（0x0816）も pt-BR として扱う
        0x16 => Language::PortugueseBrazil,
        0x04 => match id >> 10 {
            // 台湾 0x01・香港 0x03・マカオ 0x05・zh-Hant（0x7c04 → 0x1f）は繁体字
            0x01 | 0x03 | 0x05 | 0x1f => Language::ChineseTraditional,
            _ => Language::ChineseSimplified,
        },
        _ => Language::English,
    }
}

#[cfg(windows)]
fn os_language() -> Option<Language> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: 引数なしで LANGID を返すだけの API
    Some(language_from_langid(unsafe { GetUserDefaultUILanguage() }))
}

#[cfg(not(windows))]
fn os_language() -> Option<Language> {
    // Windows 専用のツール。ほかの OS では判定しない（ビルドを通すためだけの分岐）
    None
}

type Dictionary = HashMap<String, String>;

/// 言語ごとの辞書。初回参照時に JSON を読み、以後は使い回す（壊れた辞書は空として扱い、英語へ落とす。
/// 壊れていないことは整合テストが確かめる）
fn dictionary(language: Language) -> &'static Dictionary {
    static DICTIONARIES: [OnceLock<Dictionary>; 9] = [const { OnceLock::new() }; 9];
    DICTIONARIES[language.index()]
        .get_or_init(|| serde_json::from_str(language.dictionary_source()).unwrap_or_default())
}

/// 指定した言語でキーの文言を返す。辞書に無ければ英語、英語にも無ければキーそのもの
pub fn tr_in(language: Language, key: &'static str) -> &'static str {
    dictionary(language)
        .get(key)
        .or_else(|| dictionary(Language::English).get(key))
        .map_or(key, String::as_str)
}

/// 現在の表示言語でキーの文言を返す
pub fn tr(key: &'static str) -> &'static str {
    tr_in(current(), key)
}

/// `{name}` 形式のプレースホルダーを値に置き換える。置換後の文字列の中身は再度は置換しない
pub fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(len) => {
                let name = &after[..len];
                match values.iter().find(|(n, _)| *n == name) {
                    Some((_, value)) => out.push_str(value),
                    // 値の無い名前（または波括弧そのもの）はそのまま残す
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[len + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// 指定した言語でキーの文言を返し、プレースホルダーを置き換える
pub fn tr_with_in(language: Language, key: &'static str, values: &[(&str, String)]) -> String {
    fill(tr_in(language, key), values)
}

/// 現在の表示言語でキーの文言を返し、プレースホルダーを置き換える
pub fn tr_with(key: &'static str, values: &[(&str, String)]) -> String {
    fill(tr(key), values)
}

/// 辞書のキーから、現在の表示言語の文言（`String`）を作る。値は `名前 = 式`（`Display`）で渡す。
///
/// 例: `tr!("cli.item_is_not_in_the_config_file", id = id)`。辞書側は `"Item {id} is not in the config file"`。
/// 値が無ければキーだけでよい（`trs!` と違い `String` を返す）。
#[macro_export]
macro_rules! tr {
    ($key:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::tr_with(
            $key,
            &[$((::std::stringify!($name), ::std::string::ToString::to_string(&$value))),*],
        )
    };
}

/// 辞書のキーから、現在の表示言語の固定文字列（`&'static str`。書式なし）を返す
#[macro_export]
macro_rules! trs {
    ($key:literal $(,)?) => {
        $crate::i18n::tr($key)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_locale_identifiers() {
        for value in [
            "ja",
            "ja_JP.UTF-8",
            "ja-JP",
            "JA_jp",
            "Japanese_Japan.932",
            "ja_JP@calendar=japanese",
        ] {
            assert_eq!(parse(value), Some(Language::Japanese), "{value}");
        }
        for value in ["en_US.UTF-8", "en", "en-GB", "it_IT", "ru"] {
            assert_eq!(parse(value), Some(Language::English), "{value}");
        }
        for value in ["", "  ", "C", "C.UTF-8", "POSIX"] {
            assert_eq!(parse(value), None, "{value:?}");
        }
    }

    #[test]
    fn matches_chinese_scripts_and_regions() {
        for value in [
            "zh",
            "zh-CN",
            "zh_CN.UTF-8",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-SG",
            "zh-Hans-SG",
            "zh-Hans-HK",
            "Chinese_China.936",
        ] {
            assert_eq!(parse(value), Some(Language::ChineseSimplified), "{value}");
        }
        for value in [
            "zh-TW",
            "zh_TW.UTF-8",
            "zh-Hant",
            "zh-Hant-TW",
            "zh-HK",
            "zh-MO",
            "zh-Hant-CN",
            "Chinese (Traditional)_Taiwan.950",
        ] {
            assert_eq!(parse(value), Some(Language::ChineseTraditional), "{value}");
        }
    }

    #[test]
    fn matches_other_primary_languages() {
        for (value, expected) in [
            ("ko", Language::Korean),
            ("ko_KR.UTF-8", Language::Korean),
            ("Korean_Korea.949", Language::Korean),
            ("es", Language::Spanish),
            ("es-MX", Language::Spanish),
            ("es_ES@euro", Language::Spanish),
            ("fr_FR@euro", Language::French),
            ("fr-CA", Language::French),
            ("de_DE", Language::German),
            ("de-AT", Language::German),
            ("pt", Language::PortugueseBrazil),
            ("pt-BR", Language::PortugueseBrazil),
            ("pt_PT.UTF-8", Language::PortugueseBrazil),
            ("Portuguese_Brazil.1252", Language::PortugueseBrazil),
        ] {
            assert_eq!(parse(value), Some(expected), "{value}");
        }
    }

    #[test]
    fn maps_windows_language_ids() {
        for (id, expected) in [
            (0x0411, Language::Japanese),
            (0x0409, Language::English),
            (0x0804, Language::ChineseSimplified),
            (0x1004, Language::ChineseSimplified),
            (0x0004, Language::ChineseSimplified),
            (0x0404, Language::ChineseTraditional),
            (0x0c04, Language::ChineseTraditional),
            (0x1404, Language::ChineseTraditional),
            (0x7c04, Language::ChineseTraditional),
            (0x0412, Language::Korean),
            (0x0c0a, Language::Spanish),
            (0x040c, Language::French),
            (0x0407, Language::German),
            (0x0416, Language::PortugueseBrazil),
            (0x0816, Language::PortugueseBrazil),
            (0x0410, Language::English),
        ] {
            assert_eq!(language_from_langid(id), expected, "{id:#06x}");
        }
    }

    #[test]
    fn tags_are_unique_and_round_trip() {
        let tags: std::collections::BTreeSet<_> = Language::ALL.iter().map(|l| l.tag()).collect();
        assert_eq!(tags.len(), Language::ALL.len());
        for language in Language::ALL {
            assert_eq!(parse(language.tag()), Some(language));
        }
    }

    #[test]
    fn missing_keys_return_the_key_and_fill_replaces_names() {
        assert_eq!(tr_in(Language::French, "no.such.key"), "no.such.key");
        assert_eq!(
            fill(
                "a {x} b {x} {y} {z}",
                &[("x", "1".into()), ("y", "{x}".into())]
            ),
            "a 1 b 1 {x} {z}"
        );
    }

    #[test]
    fn macros_fill_named_values() {
        // 言語は環境（.cargo/config.toml の EDICTORA_LANGUAGE）で決まる。
        // どの言語でも値が埋め込まれ、波括弧が残らないことだけを確かめる
        let text = tr!("cli.item_is_not_in_the_config_file", id = "abc");
        assert!(text.contains("abc") && !text.contains('{'), "{text}");
        assert!(!trs!("cli.off").is_empty());
    }
}
