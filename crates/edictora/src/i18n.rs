//! 利用者に見せる文言の表示言語（日本語 / 英語）。
//!
//! 目的: 日本語を読まない利用者にも、CLI の進捗・結果・エラー、差分表示、MCP のツール説明を英語で届ける。
//!
//! 設計:
//! - 言語はプロセスで 1 つ。`main` / 更新用 exe の入口で [`init`] を 1 回だけ呼ぶ（[`detect`] の結果を渡す）。
//!   フックや引数の誤り（終了コード 2）より前でも決められるよう、設定ファイルや引数には頼らず、
//!   失敗しない決め方（環境変数 `EDICTORA_LANG` → OS の表示言語）にしている。
//! - [`init`] が呼ばれていない（ライブラリのテストなど）ときは日本語。既存のテストが日本語の文言を
//!   そのまま確かめているため、テストの出力は日本語のまま変わらない。
//! - 文言は呼び出し箇所に日本語と英語を並べて書く（[`tr!`] / [`trs!`]）。キーの一覧表を別に持つと、
//!   片方だけ直す・引数がずれるといった食い違いが起きやすいため。
//! - ログ（`logging`）の定型文は作者が原因を調べるためのものなので日本語のままにする。
//!   ただし、ログに埋め込むエラー文や更新結果の文（利用者にも見せる文）は表示言語のまま記録される。
//! - ダッシュボードの画面の文言（要素を探すための文字列）はこれとは別で、`dashboard::text` が画面の言語ごとに持つ。

use std::sync::OnceLock;

/// 表示言語
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 日本語（既定。作者の環境と既存のテスト）
    Ja,
    /// 英語（日本語以外の利用者）
    En,
}

/// 表示言語を上書きする環境変数（`ja` / `en`）。MCP クライアントの登録や CI で言語を固定するのに使う
pub const LANG_ENV: &str = "EDICTORA_LANG";

static LANG: OnceLock<Lang> = OnceLock::new();

/// 現在の表示言語。[`init`] の前は日本語
pub fn lang() -> Lang {
    LANG.get().copied().unwrap_or(Lang::Ja)
}

/// 表示言語を決める（プロセスで最初の 1 回だけ有効。2 回目以降は無視する）
pub fn init(lang: Lang) {
    let _ = LANG.set(lang);
}

/// 環境変数の値を解釈する（`ja` / `en`、大文字小文字と前後の空白は問わない。`ja-JP` のような地域付きも受ける）。
/// 解釈できない値は None（OS の表示言語へ回す）
pub fn parse_lang(value: &str) -> Option<Lang> {
    let v = value.trim().to_ascii_lowercase();
    let primary = v.split(['-', '_']).next().unwrap_or("");
    match primary {
        "ja" => Some(Lang::Ja),
        "en" => Some(Lang::En),
        _ => None,
    }
}

/// Windows の LANGID から言語を決める（主言語が日本語なら日本語、それ以外は英語）
pub fn from_langid(langid: u16) -> Lang {
    // PRIMARYLANGID は下位 10 ビット。LANG_JAPANESE = 0x11
    if langid & 0x3ff == 0x11 {
        Lang::Ja
    } else {
        Lang::En
    }
}

/// 表示言語を決める: 環境変数 `EDICTORA_LANG` → OS の表示言語（`GetUserDefaultUILanguage`）。
/// 失敗しない（フックの処理より前に呼んでも終了コードに影響しない）
pub fn detect() -> Lang {
    if let Some(l) = std::env::var(LANG_ENV).ok().as_deref().and_then(parse_lang) {
        return l;
    }
    from_langid(os_ui_langid())
}

#[cfg(windows)]
fn os_ui_langid() -> u16 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: 引数なしで LANGID を返すだけの API
    unsafe { GetUserDefaultUILanguage() }
}

#[cfg(not(windows))]
fn os_ui_langid() -> u16 {
    // Windows 専用のツール。ほかの OS では英語にする（ビルドを通すためだけの分岐）
    0x09
}

/// 日本語と英語の書式文字列を並べ、現在の表示言語のほうで `format!` する（`String` を返す）。
///
/// 例: `tr!("{n} 件を保存しました", "Saved {n} item(s)")`、`tr!("{} 件", "{} item(s)", count)`。
/// 両方の書式文字列は同じ引数を使うこと（片方だけが使う位置引数は `format!` のエラーになる）。
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Ja => ::std::format!($ja),
            $crate::i18n::Lang::En => ::std::format!($en),
        }
    };
    ($ja:literal, $en:literal, $($arg:tt)+) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Ja => ::std::format!($ja, $($arg)+),
            $crate::i18n::Lang::En => ::std::format!($en, $($arg)+),
        }
    };
}

/// 日本語と英語の固定文字列から、現在の表示言語のほうを選ぶ（`&'static str` を返す。書式なし）
#[macro_export]
macro_rules! trs {
    ($ja:expr, $en:expr $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Ja => $ja,
            $crate::i18n::Lang::En => $en,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lang_accepts_codes_and_regions() {
        assert_eq!(parse_lang("ja"), Some(Lang::Ja));
        assert_eq!(parse_lang(" EN "), Some(Lang::En));
        assert_eq!(parse_lang("ja-JP"), Some(Lang::Ja));
        assert_eq!(parse_lang("en_US"), Some(Lang::En));
        assert_eq!(parse_lang("fr"), None);
        assert_eq!(parse_lang(""), None);
    }

    #[test]
    fn from_langid_uses_primary_language() {
        assert_eq!(from_langid(0x0411), Lang::Ja); // ja-JP
        assert_eq!(from_langid(0x0409), Lang::En); // en-US
        assert_eq!(from_langid(0x040c), Lang::En); // fr-FR は英語へ
    }

    #[test]
    fn default_is_japanese_and_macros_capture_identifiers() {
        // テストでは init を呼ばないので日本語
        assert_eq!(lang(), Lang::Ja);
        let n = 3;
        assert_eq!(tr!("{n} 件", "{n} item(s)"), "3 件");
        assert_eq!(tr!("{} 件", "{} item(s)", n + 1), "4 件");
        assert_eq!(trs!("はい", "yes"), "はい");
    }
}
