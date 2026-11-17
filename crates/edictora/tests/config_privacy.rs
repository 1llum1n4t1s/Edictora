//! 設定の `privacy`（「プライバシー」タブの正本）の読込・正本の組み立て・`--only privacy` による対象の絞り込み。
//! Chrome を開く前の検証なので、誤りは `Config` エラー（終了コード 2）になる。

mod common;

use std::path::PathBuf;

use common::{ITEM_A, ITEM_B, config_err, test_paths, work_dir};
use edictora::cli::args::parse;
use edictora::cli::select_targets;
use edictora::config::{self, EdictoraConfig};
use edictora::listing::ExpectedItem;
use edictora::privacy::{self, DataKind, PrivacyField};

/// 正本ファイル（ReviewForMD の store-listing.txt と同じ形: 権限の正当性は字下げされている）
const LISTING: &str = "■ 単一の目的の説明\n閲覧中のページを Markdown にする\n\n■ データの使用に関する開示\n（略）\n\n■ 権限の正当性\n\n  activeTab:\n  タブを確認するために使用します。\n  二行目です。\n\n  storage:\n  表示言語を保存するために使用します。\n\n  ホスト権限:\n  PR ページを読み取ります。\n\n■ プライバシーポリシー URL\nhttps://example.com/privacy\n";

struct Ctx {
    dir: PathBuf,
}

impl Ctx {
    fn new(name: &str) -> Ctx {
        let dir = work_dir(name);
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("webstore")).unwrap();
        // CRLF の正本でも同じ結果になることも確かめる
        std::fs::write(
            repo.join("webstore").join("store-listing.txt"),
            LISTING.replace('\n', "\r\n"),
        )
        .unwrap();
        std::fs::write(
            repo.join("webstore").join("empty.txt"),
            "■ 見出し\n\n■ 次\n",
        )
        .unwrap();
        std::fs::write(
            repo.join("webstore").join("long.txt"),
            format!("■ 長い\n{}\n■ 次\n", "あ".repeat(1001)),
        )
        .unwrap();
        std::fs::write(
            repo.join("webstore").join("ok1000.txt"),
            format!("■ 長い\n{}\n■ 次\n", "あ".repeat(1000)),
        )
        .unwrap();
        // 説明（テキスト）も持つ別アイテム用
        std::fs::write(
            repo.join("webstore").join("desc.txt"),
            "■ 説明\n本文です\n■ カテゴリ\n",
        )
        .unwrap();
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

fn item_json(privacy: &str) -> String {
    format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "name": "テスト", "baseDir": "repo", "privacy": {privacy} }} ] }}"#
    )
}

const FILE: &str = r#""file": "webstore/store-listing.txt""#;

fn full_privacy() -> String {
    format!(
        r#"{{
        "singlePurpose": {{ {FILE}, "sectionStart": "^■ 単一の目的の説明$", "sectionEnd": "^■ データの使用に関する開示$" }},
        "permissions": {{
          "activeTab": {{ {FILE}, "sectionStart": "^  activeTab:$", "sectionEnd": "^  storage:$" }},
          "storage": {{ {FILE}, "sectionStart": "^  storage:$", "sectionEnd": "^  ホスト権限:$" }},
          "host": {{ {FILE}, "sectionStart": "^  ホスト権限:$", "sectionEnd": "^■ プライバシーポリシー URL$" }}
        }},
        "remoteCode": {{ "uses": false }},
        "dataUsage": {{ "collected": ["personallyIdentifiable", "authentication"], "certify": true }}
      }}"#
    )
}

fn expected_of(config: &EdictoraConfig) -> ExpectedItem {
    let mut e = edictora::listing::build_expected(&config.items[0]).unwrap();
    e.privacy = privacy::build_expected(&config.items[0]).unwrap();
    e
}

#[test]
fn 全部入りの設定を読み正本を組み立てる() {
    let ctx = Ctx::new("config_privacy_full");
    let config = ctx.parse(&item_json(&full_privacy())).unwrap();
    let item = &config.items[0];
    assert!(
        item.locales.is_empty(),
        "privacy だけを管理するアイテムを許す"
    );
    let p = item.privacy.as_ref().unwrap();
    assert!(p.single_purpose.is_some());
    let keys: Vec<&str> = p
        .permissions
        .as_ref()
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, ["activeTab", "storage", "host"], "書いた順を保つ");
    assert!(!p.remote_code.as_ref().unwrap().uses);
    let usage = p.data_usage.as_ref().unwrap();
    assert_eq!(
        usage.collected,
        [DataKind::PersonallyIdentifiable, DataKind::Authentication]
    );
    assert!(usage.certify);

    let e = privacy::build_expected(item).unwrap().unwrap();
    // 区間の本文は、前後の空行を除き共通の字下げを取り除く（CRLF の正本でも \n）
    assert_eq!(
        e.single_purpose.as_deref(),
        Some("閲覧中のページを Markdown にする")
    );
    let perms = e.permissions.as_ref().unwrap();
    assert_eq!(
        perms[0],
        (
            "activeTab".to_string(),
            "タブを確認するために使用します。\n二行目です。".to_string()
        )
    );
    assert_eq!(
        perms[1],
        (
            "storage".to_string(),
            "表示言語を保存するために使用します。".to_string()
        )
    );
    assert_eq!(
        perms[2],
        ("host".to_string(), "PR ページを読み取ります。".to_string())
    );
    assert!(!e.remote_code.as_ref().unwrap().uses);
    let usage = e.data_usage.as_ref().unwrap();
    assert_eq!(usage.collected.len(), 2);
    assert!(usage.certify);
    // 欄の一覧は表示順
    assert_eq!(e.fields()[0], PrivacyField::SinglePurpose);
}

#[test]
fn privacyを書いていないアイテムは対象外() {
    let ctx = Ctx::new("config_privacy_none");
    let json = format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "baseDir": "repo",
              "locales": {{ "ja": {{ "description": {{ {FILE}, "sectionStart": "^■ 単一の目的の説明$", "sectionEnd": "^■ データ" }} }} }} }} ] }}"#
    );
    let config = ctx.parse(&json).unwrap();
    assert!(config.items[0].privacy.is_none());
    assert!(privacy::build_expected(&config.items[0]).unwrap().is_none());
}

#[test]
fn 一部の欄だけを書ける() {
    let ctx = Ctx::new("config_privacy_partial");
    let config = ctx
        .parse(&item_json(
            r#"{ "dataUsage": { "collected": [], "certify": true } }"#,
        ))
        .unwrap();
    let e = privacy::build_expected(&config.items[0]).unwrap().unwrap();
    assert!(e.single_purpose.is_none() && e.permissions.is_none() && e.remote_code.is_none());
    assert!(e.data_usage.unwrap().collected.is_empty());
}

#[test]
fn 区間を省略するとファイル全体が本文() {
    let ctx = Ctx::new("config_privacy_whole");
    let config = ctx
        .parse(&item_json(
            r#"{ "singlePurpose": { "file": "webstore/desc.txt" } }"#,
        ))
        .unwrap();
    let e = privacy::build_expected(&config.items[0]).unwrap().unwrap();
    assert_eq!(
        e.single_purpose.as_deref(),
        Some("■ 説明\n本文です\n■ カテゴリ")
    );
}

#[test]
fn 設定の誤りは設定エラーになる() {
    let ctx = Ctx::new("config_privacy_errors");
    let cases: Vec<(&str, String)> = vec![
        ("空の privacy", item_json("{}")),
        (
            "未知のキー",
            item_json(r#"{ "dataUsage": { "collected": [], "certify": true }, "extra": 1 }"#),
        ),
        (
            "certify が false",
            item_json(r#"{ "dataUsage": { "collected": [], "certify": false } }"#),
        ),
        (
            "certify が無い",
            item_json(r#"{ "dataUsage": { "collected": [] } }"#),
        ),
        (
            "collected が無い",
            item_json(r#"{ "dataUsage": { "certify": true } }"#),
        ),
        (
            "未知のデータの種類",
            item_json(r#"{ "dataUsage": { "collected": ["pii"], "certify": true } }"#),
        ),
        (
            "データの種類の重複",
            item_json(r#"{ "dataUsage": { "collected": ["health", "health"], "certify": true } }"#),
        ),
        ("permissions が空", item_json(r#"{ "permissions": {} }"#)),
        (
            "権限名の前後に空白",
            item_json(&format!(
                r#"{{ "permissions": {{ " activeTab": {{ {FILE} }} }} }}"#
            )),
        ),
        (
            "権限名の重複",
            item_json(&format!(
                r#"{{ "permissions": {{ "tabs": {{ {FILE} }}, "tabs": {{ {FILE} }} }} }}"#
            )),
        ),
        (
            "権限の値が null",
            item_json(r#"{ "permissions": { "tabs": null } }"#),
        ),
        (
            "file が無い",
            item_json(r#"{ "singlePurpose": { "sectionStart": "^a$", "sectionEnd": "^b$" } }"#),
        ),
        (
            "sectionStart だけ",
            item_json(&format!(
                r#"{{ "singlePurpose": {{ {FILE}, "sectionStart": "^a$" }} }}"#
            )),
        ),
        (
            "正規表現が不正",
            item_json(&format!(
                r#"{{ "singlePurpose": {{ {FILE}, "sectionStart": "(", "sectionEnd": "^b$" }} }}"#
            )),
        ),
        (
            "先読みは使えない",
            item_json(&format!(
                r#"{{ "singlePurpose": {{ {FILE}, "sectionStart": "^(?=a)", "sectionEnd": "^b$" }} }}"#
            )),
        ),
        (
            "ファイルが無い",
            item_json(r#"{ "singlePurpose": { "file": "webstore/none.txt" } }"#),
        ),
        (
            "remoteCode に uses が無い",
            item_json(r#"{ "remoteCode": {} }"#),
        ),
        (
            "uses true なのに justification が無い",
            item_json(r#"{ "remoteCode": { "uses": true } }"#),
        ),
        (
            "uses false なのに justification がある",
            item_json(&format!(
                r#"{{ "remoteCode": {{ "uses": false, "justification": {{ {FILE} }} }} }}"#
            )),
        ),
    ];
    for (name, json) in cases {
        let message = config_err(ctx.parse(&json));
        assert!(!message.is_empty(), "{name}");
    }
}

#[test]
fn 誤りの文言に場所が入る() {
    let ctx = Ctx::new("config_privacy_places");
    let m = config_err(ctx.parse(&item_json(
        r#"{ "dataUsage": { "collected": [], "certify": false } }"#,
    )));
    assert!(
        m.contains("privacy.dataUsage") && m.contains("certify"),
        "{m}"
    );
    assert!(m.contains(ITEM_A), "{m}");
    let m = config_err(ctx.parse(&item_json(
        r#"{ "dataUsage": { "collected": ["pii"], "certify": true } }"#,
    )));
    assert!(
        m.contains("pii") && m.contains("personallyIdentifiable"),
        "{m}"
    );
}

#[test]
fn 画面に出す前の正本の検証() {
    let ctx = Ctx::new("config_privacy_expected");
    // 空の本文
    let config = ctx
        .parse(&item_json(
            r#"{ "singlePurpose": { "file": "webstore/empty.txt", "sectionStart": "^■ 見出し$", "sectionEnd": "^■ 次$" } }"#,
        ))
        .unwrap();
    let m = config_err(privacy::build_expected(&config.items[0]));
    assert!(m.contains("singlePurpose"), "{m}");

    // 1,000 文字ちょうどは通り、1,001 文字は超過
    let ok = ctx
        .parse(&item_json(
            r#"{ "singlePurpose": { "file": "webstore/ok1000.txt", "sectionStart": "^■ 長い$", "sectionEnd": "^■ 次$" } }"#,
        ))
        .unwrap();
    assert!(privacy::build_expected(&ok.items[0]).is_ok());
    let long = ctx
        .parse(&item_json(
            r#"{ "singlePurpose": { "file": "webstore/long.txt", "sectionStart": "^■ 長い$", "sectionEnd": "^■ 次$" } }"#,
        ))
        .unwrap();
    let m = config_err(privacy::build_expected(&long.items[0]));
    assert!(m.contains("1,001") && m.contains("1,000"), "{m}");

    // 区間の見出しが無い
    let missing = ctx
        .parse(&item_json(
            r#"{ "singlePurpose": { "file": "webstore/store-listing.txt", "sectionStart": "^■ 無い見出し$", "sectionEnd": "^■ 次$" } }"#,
        ))
        .unwrap();
    config_err(privacy::build_expected(&missing.items[0]));
}

// ───────── 対象の絞り込み ─────────

fn targets(ctx: &Ctx, json: &str, args: &[&str]) -> edictora::Result<Vec<ExpectedItem>> {
    let config = ctx.parse(json).unwrap();
    select_targets(&config, &parse(args).unwrap())
}

/// ITEM_A: テキスト（ja）と privacy の両方、ITEM_B: テキストだけ
fn two_items() -> String {
    format!(
        r#"{{ "items": [
          {{ "itemId": "{ITEM_A}", "name": "A", "baseDir": "repo",
             "locales": {{ "ja": {{ "description": {{ "file": "webstore/desc.txt" }} }} }},
             "privacy": {{ "singlePurpose": {{ "file": "webstore/desc.txt" }} }} }},
          {{ "itemId": "{ITEM_B}", "name": "B", "baseDir": "repo",
             "locales": {{ "ja": {{ "description": {{ "file": "webstore/desc.txt" }} }} }} }}
        ] }}"#
    )
}

#[test]
fn 既定はテキストと画像とプライバシーのすべて() {
    let ctx = Ctx::new("config_privacy_targets_all");
    let items = targets(&ctx, &two_items(), &["diff"]).unwrap();
    assert_eq!(items.len(), 2);
    assert!(!items[0].locales.is_empty() && items[0].has_privacy());
    assert!(!items[1].locales.is_empty() && !items[1].has_privacy());
}

#[test]
fn only_privacyはプライバシーだけ() {
    let ctx = Ctx::new("config_privacy_targets_only");
    let items = targets(&ctx, &two_items(), &["diff", "--only", "privacy"]).unwrap();
    // privacy を書いていない B は対象外（全アイテムが対象のときは黙って除く）
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_id, ITEM_A);
    assert!(items[0].locales.is_empty(), "テキストは対象外");
    assert!(items[0].images.is_none());
    assert!(items[0].has_privacy());

    // アイテムを明示したのに対象が無ければ誤り
    let m = config_err(targets(
        &ctx,
        &two_items(),
        &["diff", "--only", "privacy", "--item", ITEM_B],
    ));
    assert!(m.contains(ITEM_B), "{m}");
}

#[test]
fn only_textではプライバシーを対象にしない() {
    let ctx = Ctx::new("config_privacy_targets_text");
    let items = targets(&ctx, &two_items(), &["push", "--only", "text"]).unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| !i.has_privacy()));
}

#[test]
fn only_imagesではプライバシーを対象にしない() {
    let ctx = Ctx::new("config_privacy_targets_images");
    // 画像を書いていないので対象が無く、設定エラー
    config_err(targets(&ctx, &two_items(), &["diff", "--only", "images"]));
}

#[test]
fn locale指定ではプライバシーを絞らない() {
    let ctx = Ctx::new("config_privacy_targets_locale");
    let items = targets(&ctx, &two_items(), &["diff", "--locale", "ja"]).unwrap();
    assert!(
        items[0].has_privacy(),
        "全言語共通なので --locale では絞らない"
    );
    // build_expected 側の説明文は ja だけ
    assert_eq!(items[0].locales.len(), 1);
}

#[test]
fn 正本の組み立ては正本のファイルの変更に追随する() {
    let ctx = Ctx::new("config_privacy_rebuild");
    let config = ctx.parse(&item_json(&full_privacy())).unwrap();
    let a = expected_of(&config);
    let b = expected_of(&config);
    assert_eq!(a.privacy, b.privacy);
}
