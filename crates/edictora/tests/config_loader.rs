//! C# の ConfigLoader.test.cs の移植

mod common;

use std::path::{Path, PathBuf};

use common::{ITEM_A, ITEM_B, test_paths, work_dir};
use edictora::Error;
use edictora::app_paths::AppPaths;
use edictora::config::{self, EdictoraConfig};
use edictora::listing::{DESCRIPTION_MAX_LENGTH, ListingField, build_expected};

/// テストごとの作業フォルダー（repo/webstore/listing.txt と whole.txt を置く）
struct Ctx {
    dir: PathBuf,
    paths: AppPaths,
}

impl Ctx {
    fn new(name: &str) -> Ctx {
        let dir = work_dir(name);
        let webstore = dir.join("repo").join("webstore");
        std::fs::create_dir_all(&webstore).unwrap();
        std::fs::write(
            webstore.join("listing.txt"),
            "■ 説明\n本文です\n■ カテゴリ\n",
        )
        .unwrap();
        std::fs::write(webstore.join("whole.txt"), "全体の説明\n").unwrap();
        let paths = test_paths(&dir);
        Ctx { dir, paths }
    }

    fn config_path(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    fn parse(&self, json: &str) -> edictora::Result<EdictoraConfig> {
        config::parse(json, &self.config_path(), None, None, &self.paths)
    }

    fn parse_with_profile(&self, json: &str, profile: &Path) -> edictora::Result<EdictoraConfig> {
        config::parse(
            json,
            &self.config_path(),
            profile.to_str(),
            None,
            &self.paths,
        )
    }
}

fn config_err(r: edictora::Result<EdictoraConfig>) -> String {
    match r {
        Err(Error::Config(m)) => m,
        other => panic!("Config エラーを期待しました: {other:?}"),
    }
}

#[test]
fn parse_相対パスを設定ファイルとbase_dir基準で解決する() {
    let ctx = Ctx::new("config-relative");
    let json = r#"
        {
          // コメントと末尾カンマを許す
          "profileDir": "profile",
          "items": [
            {
              "itemId": "ITEM_A",
              "name": "Replace Translator",
              "baseDir": "repo",
              "locales": {
                "ja": {
                  "description": { "file": "webstore/listing.txt", "sectionStart": "^■ 説明", "sectionEnd": "^■ カテゴリ" },
                  "homepageUrl": "https://example.com/",
                  "supportUrl": "",
                },
                "en": { "description": { "file": "webstore/whole.txt" } }
              }
            }
          ]
        }
        "#
    .replace("ITEM_A", ITEM_A);
    let config = ctx.parse(&json).unwrap();

    assert_eq!(config.profile_directory, ctx.dir.join("profile"));
    assert_eq!(config.items.len(), 1);
    let item = &config.items[0];
    assert_eq!(item.item_id, ITEM_A);
    assert_eq!(item.display_name, "Replace Translator");
    assert_eq!(item.base_directory, ctx.dir.join("repo"));

    let ja = item.locales.iter().find(|l| l.code == "ja").unwrap();
    assert_eq!(
        ja.description.as_ref().unwrap().file_path,
        ctx.dir.join("repo").join("webstore").join("listing.txt")
    );
    assert_eq!(
        ja.fields(),
        vec![
            ListingField::Description,
            ListingField::HomepageUrl,
            ListingField::SupportUrl
        ]
    );
    // 空文字は「空欄が正本」として対象に含める
    assert_eq!(ja.urls[&ListingField::SupportUrl], "");
    // 書いていない欄は対象外
    assert!(!ja.urls.contains_key(&ListingField::OfficialUrl));
    // locales は書いた順を保つ
    let codes: Vec<&str> = item.locales.iter().map(|l| l.code.as_str()).collect();
    assert_eq!(codes, vec!["ja", "en"]);
}

#[test]
fn parse_profile_dir省略時は既定のプロファイル() {
    let ctx = Ctx::new("config-default-profile");
    let config = ctx
        .parse(&format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "homepageUrl": "https://example.com/" }} }} }} ] }}"#
        ))
        .unwrap();

    // 既定は注入した保存先（テストでは作業フォルダーの下）の chrome-profile
    assert_eq!(
        config.profile_directory,
        ctx.paths.default_profile_directory()
    );
    assert_eq!(config.items[0].display_name, ITEM_A);
}

#[test]
fn parse_profile指定は設定ファイルより優先する() {
    let ctx = Ctx::new("config-profile-override");
    let other = ctx.dir.join("other-profile");
    let config = ctx
        .parse_with_profile(r#"{ "profileDir": "profile", "items": [] }"#, &other)
        .unwrap();
    assert_eq!(config.profile_directory, other);
}

#[test]
fn parse_アイテムidの形式が違えば例外() {
    let ctx = Ctx::new("config-bad-id");
    let m = config_err(ctx.parse(
        r#"{ "items": [ { "itemId": "short", "locales": { "ja": { "homepageUrl": "https://example.com/" } } } ] }"#,
    ));
    assert!(m.contains("itemId"));
}

#[test]
fn parse_アイテムidは小文字に揃え重複を検出する() {
    let ctx = Ctx::new("config-dup-id");
    let upper = ITEM_A.to_uppercase();
    let m = config_err(ctx.parse(&format!(
        r#"{{ "items": [
              {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "homepageUrl": "https://example.com/" }} }} }},
              {{ "itemId": "{upper}", "locales": {{ "ja": {{ "homepageUrl": "https://example.com/" }} }} }}
            ] }}"#
    )));
    assert!(m.contains("重複"));
}

#[test]
fn parse_未知のキーは綴り間違いとして例外() {
    let ctx = Ctx::new("config-unknown-key");
    let m = config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "homepageURL": "https://example.com/" }} }} }} ] }}"#
    )));
    assert!(m.contains("形式"));
}

#[test]
fn parse_欄が1つも無いロケールは例外() {
    let ctx = Ctx::new("config-empty-locale");
    config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ }} }} }} ] }}"#
    )));
}

#[test]
fn parse_urlがhttpでなければ例外() {
    let ctx = Ctx::new("config-mailto");
    let m = config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "supportUrl": "mailto:a@example.com" }} }} }} ] }}"#
    )));
    assert!(m.contains("supportUrl"));
}

#[test]
fn parse_正本ファイルが無ければ例外() {
    let ctx = Ctx::new("config-missing-source");
    let m = config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "description": {{ "file": "missing.txt" }} }} }} }} ] }}"#
    )));
    assert!(m.contains("正本ファイルがありません"));
}

#[test]
fn parse_区間の片側だけ指定は例外() {
    let ctx = Ctx::new("config-half-section");
    config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "baseDir": "repo",
              "locales": {{ "ja": {{ "description": {{ "file": "webstore/listing.txt", "sectionStart": "^■ 説明" }} }} }} }} ] }}"#
    )));
}

#[test]
fn parse_ロケールコードの区切りを正規化し重複を検出する() {
    let ctx = Ctx::new("config-locale-normalize");
    let config = ctx
        .parse(&format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "pt_BR": {{ "homepageUrl": "https://example.com/" }} }} }} ] }}"#
        ))
        .unwrap();
    assert_eq!(config.items[0].locales[0].code, "pt-BR");

    config_err(ctx.parse(&format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "pt_BR": {{ "homepageUrl": "https://a.example/" }}, "pt-BR": {{ "homepageUrl": "https://b.example/" }} }} }} ] }}"#
    )));
}

#[test]
fn load_設定ファイルが無ければrequire_fileで分岐する() {
    let ctx = Ctx::new("config-load-missing");
    let missing = ctx.dir.join("none.json");
    let missing = missing.to_str().unwrap();

    config_err(config::load(Some(missing), None, None, true, &ctx.paths));
    let empty = config::load(Some(missing), None, None, false, &ctx.paths).unwrap();
    assert!(empty.items.is_empty());
}

#[test]
fn build_正本の値を取り出しロケールで絞り込める() {
    let ctx = Ctx::new("config-build");
    let config = ctx
        .parse(&format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM_B}", "baseDir": "repo", "locales": {{
              "ja": {{ "description": {{ "file": "webstore/listing.txt", "sectionStart": "^■ 説明", "sectionEnd": "^■ カテゴリ" }} }},
              "en": {{ "description": {{ "file": "webstore/whole.txt" }}, "officialUrl": "https://example.com/en" }}
            }} }} ] }}"#
        ))
        .unwrap();

    let expected = build_expected(&config.items[0]).unwrap();
    let value = |code: &str| {
        expected
            .locales
            .iter()
            .find(|l| l.code == code)
            .unwrap()
            .values[&ListingField::Description]
            .clone()
    };
    assert_eq!(value("ja"), "本文です");
    assert_eq!(value("en"), "全体の説明");

    let only_en = expected.filter_locales(&["en".to_string()]).unwrap();
    assert_eq!(only_en.locales.len(), 1);
    assert_eq!(only_en.locales[0].code, "en");
    assert!(matches!(
        expected.filter_locales(&["fr".to_string()]),
        Err(Error::Config(_))
    ));
}

#[test]
fn build_説明が上限を超えたら例外() {
    let ctx = Ctx::new("config-too-long");
    std::fs::write(
        ctx.dir.join("long.txt"),
        "あ".repeat(DESCRIPTION_MAX_LENGTH + 1),
    )
    .unwrap();
    let config = ctx
        .parse(&format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "locales": {{ "ja": {{ "description": {{ "file": "long.txt" }} }} }} }} ] }}"#
        ))
        .unwrap();
    assert!(matches!(
        build_expected(&config.items[0]),
        Err(Error::Config(_))
    ));
}
