//! ローカライズの整合テスト。
//!
//! 守りたい契約:
//! - 9 言語の辞書（`src/locales/<タグ>.json`）が、同じキー集合・同じプレースホルダー集合を持つ
//! - コード（`src/**/*.rs` の `tr!` / `trs!`）が参照するキーはすべて辞書にあり、渡す名前付き引数も辞書の
//!   プレースホルダーと一致する
//! - 辞書に未使用のキーが無い（消し忘れ・綴りの誤りを検出する）
//! - 値が空でない、改行・書式の崩れ（波括弧の不一致）が無い

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use edictora::i18n::{self, Language};

fn dictionary_of(language: Language) -> BTreeMap<String, String> {
    serde_json::from_str(language.dictionary_source())
        .unwrap_or_else(|e| panic!("{} の辞書が JSON として壊れています: {e}", language.tag()))
}

/// `{name}` 形式のプレースホルダー名の集合（名前は小文字・数字・アンダースコア）
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + 1..start + len];
        if !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            found.insert(name.to_owned());
        }
        rest = &rest[start + len + 1..];
    }
    found
}

#[test]
fn 全言語の辞書は同じキーと同じプレースホルダーを持つ() {
    let english = dictionary_of(Language::English);
    assert!(!english.is_empty());
    for language in Language::ALL {
        let dictionary = dictionary_of(language);
        let tag = language.tag();
        let missing: Vec<_> = english
            .keys()
            .filter(|k| !dictionary.contains_key(*k))
            .collect();
        let extra: Vec<_> = dictionary
            .keys()
            .filter(|k| !english.contains_key(*k))
            .collect();
        assert!(
            missing.is_empty(),
            "{tag}: 英語にあって無いキー: {missing:?}"
        );
        assert!(extra.is_empty(), "{tag}: 英語に無い余分なキー: {extra:?}");
        for (key, value) in &dictionary {
            assert!(!value.trim().is_empty(), "{tag}: {key} が空です");
            assert_eq!(
                placeholders(value),
                placeholders(&english[key]),
                "{tag}: {key} のプレースホルダーが英語と違います: {value}"
            );
            // 書式の崩れ（開いていない・閉じていない波括弧）がない
            assert_eq!(
                value.matches('{').count(),
                value.matches('}').count(),
                "{tag}: {key} の波括弧が対応していません: {value}"
            );
        }
    }
}

#[test]
fn 日本語と英語以外は英語のコピーではない() {
    // 訳し忘れ（英語の値のまま）の検出。製品名・コマンド名だけのような短い値や、記号・数字だけの値は除く。
    // 日本語の辞書は「既存の文言をそのまま移したもの」なので、英語と同じ値は無い前提で確かめる
    let english = dictionary_of(Language::English);
    for language in Language::ALL {
        if language == Language::English {
            continue;
        }
        let dictionary = dictionary_of(language);
        let same: Vec<_> = dictionary
            .iter()
            .filter(|(k, v)| {
                *v == &english[*k] && v.chars().filter(|c| c.is_alphabetic()).count() > 24
            })
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(
            same.is_empty(),
            "{}: 英語のままのキー（訳し忘れ）: {same:?}",
            language.tag()
        );
    }
}

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("ディレクトリを読めません") {
        let path = entry.expect("エントリ").path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// `tr!(` / `trs!(` の呼び出しから (キー, 名前付き引数の名前の集合) を集める。
/// 括弧の対応を数えて呼び出しの終わりを決める（引数の中の文字列・入れ子の呼び出しは飛ばす）
fn used_keys() -> BTreeMap<String, Vec<(String, BTreeSet<String>)>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&root, &mut files);
    let start = regex::Regex::new(r#"\btrs?!\(\s*"([^"]+)""#).unwrap();
    let mut used: BTreeMap<String, Vec<(String, BTreeSet<String>)>> = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for m in start.captures_iter(&text) {
            let key = m[1].to_owned();
            let whole = m.get(0).unwrap();
            // マクロ定義・説明の中の例（`tr!("key", id = id)` のコメント）も同じ形なので、そのまま数える
            let mut depth = 1;
            let mut names = BTreeSet::new();
            let bytes = text.as_bytes();
            let mut i = whole.end();
            let mut token_start = i;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'"' => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'"' {
                            if bytes[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b',' if depth == 1 => token_start = i + 1,
                    b'=' if depth == 1
                        && bytes.get(i + 1) != Some(&b'=')
                        && !matches!(
                            bytes.get(i.wrapping_sub(1)),
                            Some(b'!' | b'<' | b'>' | b'=')
                        ) =>
                    {
                        let name = text[token_start..i].trim();
                        if !name.is_empty()
                            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        {
                            names.insert(name.to_owned());
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let place = format!("{}", file.strip_prefix(&root).unwrap_or(&file).display());
            used.entry(key).or_default().push((place, names));
        }
    }
    used
}

#[test]
fn コードが参照するキーは辞書にあり_引数はプレースホルダーと一致する() {
    let english = dictionary_of(Language::English);
    let used = used_keys();
    assert!(
        used.len() > 100,
        "参照の検出に失敗しています: {}",
        used.len()
    );
    for (key, uses) in &used {
        let Some(value) = english.get(key) else {
            panic!(
                "辞書に無いキー: {key}（{:?}）",
                uses.iter().map(|u| &u.0).collect::<Vec<_>>()
            );
        };
        let expected = placeholders(value);
        for (place, names) in uses {
            // i18n.rs の説明・テストの例は引数の形が自由なので、辞書の存在だけ確かめる
            if place == "i18n.rs" {
                continue;
            }
            assert_eq!(
                names, &expected,
                "{place}: {key} に渡す名前付き引数が辞書のプレースホルダーと違います"
            );
        }
    }
}

#[test]
fn 辞書に未使用のキーが無い() {
    let used = used_keys();
    for language in Language::ALL {
        let unused: Vec<_> = dictionary_of(language)
            .into_keys()
            .filter(|k| !used.contains_key(k))
            .collect();
        assert!(
            unused.is_empty(),
            "{}: 未使用のキー: {unused:?}",
            language.tag()
        );
    }
}

#[test]
fn 辞書に無いキーは英語_英語にも無ければキーそのもの() {
    assert_eq!(i18n::tr_in(Language::German, "no.such.key"), "no.such.key");
    // どの言語でも既存のキーは引ける（辞書が壊れて空になっていない）
    for language in Language::ALL {
        assert_ne!(
            i18n::tr_in(language, "cli.off"),
            "cli.off",
            "{}",
            language.tag()
        );
    }
}
