//! 設定の `category`（カテゴリ）と `privacy.policyUrl`（プライバシー ポリシー URL）の読込・正本の組み立て・対象の絞り込み。
//! Chrome を開く前の検証なので、誤りは `Config` エラー（終了コード 2）になる。

mod common;

use std::path::PathBuf;

use common::{ITEM_A, ITEM_B, config_err, test_paths, work_dir};
use edictora::cli::args::parse;
use edictora::cli::select_targets;
use edictora::config::{self, EdictoraConfig};
use edictora::listing::ExpectedItem;
use edictora::privacy::{self, PrivacyField};

struct Ctx {
    dir: PathBuf,
}

impl Ctx {
    fn new(name: &str) -> Ctx {
        let dir = work_dir(name);
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("webstore")).unwrap();
        std::fs::write(repo.join("webstore").join("desc.txt"), "■ 説明\n本文です\n").unwrap();
        Ctx { dir }
    }

    fn parse(&self, json: &str) -> edictora::Result<EdictoraConfig> {
        config::parse(
            json,
            &self.dir.join("config.json"),
            None,
            None,
            &test_paths(&self.dir),
        )
    }
}

fn item_with(extra: &str) -> String {
    format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "name": "テスト", "baseDir": "repo", {extra} }} ] }}"#
    )
}

fn privacy_with(policy: &str) -> String {
    item_with(&format!(r#""privacy": {{ {policy} }}"#))
}

// ───────── category ─────────

#[test]
fn categoryはキーで書けてカテゴリだけのアイテムも許す() {
    let ctx = Ctx::new("config_category_ok");
    let config = ctx
        .parse(&item_with(r#""category": "developer_tools""#))
        .unwrap();
    let item = &config.items[0];
    assert!(item.locales.is_empty());
    assert_eq!(item.category.as_deref(), Some("developer_tools"));
    let e = edictora::listing::build_expected(item).unwrap();
    assert_eq!(e.category.as_deref(), Some("developer_tools"));
    assert!(e.has_text());
}

#[test]
fn categoryを書いていなければ対象外() {
    let ctx = Ctx::new("config_category_absent");
    let config = ctx
        .parse(&item_with(
            r#""locales": { "ja": { "description": { "file": "webstore/desc.txt" } } }"#,
        ))
        .unwrap();
    assert_eq!(config.items[0].category, None);
    let e = edictora::listing::build_expected(&config.items[0]).unwrap();
    assert_eq!(e.category, None);
}

#[test]
fn categoryは全21キーを受け付ける() {
    let ctx = Ctx::new("config_category_all");
    for (key, _) in edictora::listing::category::CATEGORIES {
        let config = ctx
            .parse(&item_with(&format!(r#""category": "{key}""#)))
            .unwrap();
        assert_eq!(config.items[0].category.as_deref(), Some(key));
    }
}

#[test]
fn 未知のcategoryは有効なキーを案内して止める() {
    let ctx = Ctx::new("config_category_unknown");
    for bad in [
        "productivty",
        "Productivity",
        "CATEGORY_PRODUCTIVITY",
        "Select a category",
        "",
    ] {
        let m = config_err(ctx.parse(&item_with(&format!(r#""category": "{bad}""#))));
        assert!(m.contains("category"), "{m}");
        assert!(
            m.contains("productivity") && m.contains("functionality_and_ui"),
            "{m}"
        );
    }
}

#[test]
fn 未知のcategoryは他の欄があっても止める() {
    let ctx = Ctx::new("config_category_unknown_with_locales");
    config_err(ctx.parse(&item_with(
        r#""category": "nope", "locales": { "ja": { "description": { "file": "webstore/desc.txt" } } }"#,
    )));
}

// ───────── 対象の絞り込み ─────────

fn targets(ctx: &Ctx, json: &str, args: &[&str]) -> edictora::Result<Vec<ExpectedItem>> {
    let config = ctx.parse(json).unwrap();
    select_targets(&config, &parse(args).unwrap())
}

/// ITEM_A: カテゴリとテキスト（ja）、ITEM_B: カテゴリだけ
fn two_items() -> String {
    format!(
        r#"{{ "items": [
          {{ "itemId": "{ITEM_A}", "name": "A", "baseDir": "repo", "category": "tools",
             "locales": {{ "ja": {{ "description": {{ "file": "webstore/desc.txt" }} }} }} }},
          {{ "itemId": "{ITEM_B}", "name": "B", "baseDir": "repo", "category": "games" }}
        ] }}"#
    )
}

#[test]
fn only_textにカテゴリを含み_imagesとprivacyには含めない() {
    let ctx = Ctx::new("config_category_targets_only");
    let items = targets(&ctx, &two_items(), &["push", "--only", "text"]).unwrap();
    assert_eq!(items.len(), 2);
    // カテゴリだけのアイテムを --only images や privacy で絞ると対象が無い
    assert!(targets(&ctx, &two_items(), &["diff", "--only", "images"]).is_err());
    assert!(targets(&ctx, &two_items(), &["diff", "--only", "privacy"]).is_err());
}

#[test]
fn locale指定ではカテゴリを絞らない_カテゴリだけのアイテムは残る() {
    let ctx = Ctx::new("config_category_targets_locale");
    // ja を持つのは A だけ。B（カテゴリだけ）は ja を持たないので、全アイテム対象の指定では A だけが残る
    let items = targets(&ctx, &two_items(), &["diff", "--locale", "ja"]).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_id, ITEM_A);
    assert_eq!(items[0].category.as_deref(), Some("tools"));
}

// ───────── privacy.policyUrl ─────────

#[test]
fn policyurlだけを書いたprivacyを読み正本に載せる() {
    let ctx = Ctx::new("config_policy_ok");
    let config = ctx
        .parse(&privacy_with(
            r#""policyUrl": " https://example.com/privacy ""#,
        ))
        .unwrap();
    let p = config.items[0].privacy.as_ref().unwrap();
    assert_eq!(p.policy_url.as_deref(), Some("https://example.com/privacy"));
    let e = privacy::build_expected(&config.items[0]).unwrap().unwrap();
    assert_eq!(e.policy_url.as_deref(), Some("https://example.com/privacy"));
    assert_eq!(e.fields(), vec![PrivacyField::PolicyUrl]);
    assert!(!e.is_empty());
}

#[test]
fn policyurlの空文字は空欄が正本() {
    let ctx = Ctx::new("config_policy_empty");
    let config = ctx.parse(&privacy_with(r#""policyUrl": """#)).unwrap();
    assert_eq!(
        config.items[0]
            .privacy
            .as_ref()
            .unwrap()
            .policy_url
            .as_deref(),
        Some("")
    );
    let e = privacy::build_expected(&config.items[0]).unwrap().unwrap();
    assert_eq!(e.value_of(&PrivacyField::PolicyUrl).as_deref(), Some(""));
}

#[test]
fn policyurlのキーが無ければ対象外でprivacyは何かしら要る() {
    let ctx = Ctx::new("config_policy_absent");
    let m = config_err(ctx.parse(&privacy_with("")));
    assert!(
        m.contains("policyUrl"),
        "エラー文の欄名に policyUrl を含む: {m}"
    );
}

#[test]
fn policyurlはhttpの絶対urlだけを許す() {
    let ctx = Ctx::new("config_policy_invalid");
    for bad in [
        "example.com/privacy",
        "ftp://example.com/p",
        "/privacy",
        "javascript:alert(1)",
    ] {
        let m = config_err(ctx.parse(&privacy_with(&format!(r#""policyUrl": "{bad}""#))));
        assert!(m.contains("policyUrl") && m.contains("http"), "{m}");
    }
    // http も許す
    ctx.parse(&privacy_with(
        r#""policyUrl": "http://example.com/privacy""#,
    ))
    .unwrap();
}

#[test]
fn policyurlは2048文字まで() {
    let ctx = Ctx::new("config_policy_length");
    let base = "https://example.com/";
    let ok = format!("{base}{}", "a".repeat(2048 - base.len()));
    assert_eq!(ok.len(), 2048);
    ctx.parse(&privacy_with(&format!(r#""policyUrl": "{ok}""#)))
        .unwrap();
    let too_long = format!("{ok}a");
    let m = config_err(ctx.parse(&privacy_with(&format!(r#""policyUrl": "{too_long}""#))));
    assert!(m.contains("2,049") && m.contains("2,048"), "{m}");
}

#[test]
fn policyurlだけのアイテムはonly_privacyの対象になる() {
    let ctx = Ctx::new("config_policy_targets");
    let json = item_with(r#""privacy": { "policyUrl": "https://example.com/privacy" }"#);
    let items = targets(&ctx, &json, &["diff", "--only", "privacy"]).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].has_privacy());
    // テキストには何も無い
    assert!(targets(&ctx, &json, &["diff", "--only", "text"]).is_err());
}
