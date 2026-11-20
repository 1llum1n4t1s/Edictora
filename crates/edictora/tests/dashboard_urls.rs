//! C# の DashboardUrls.test.cs の移植

mod common;

use common::ITEM_A as ITEM;
use edictora::dashboard::urls::{
    ROOT, edit_url, is_edit_page_of, is_privacy_page_of, privacy_url, try_parse_publisher_id,
};
use edictora::logging::{redact, redact_account, redact_diagnostics};

// テスト用の架空の publisherId（実在の値は使わない）
const PUBLISHER: &str = "00000000-1111-2222-3333-444444444444";

#[test]
fn try_parse_publisher_id_ダッシュボードのurlから取り出す() {
    let a = try_parse_publisher_id(Some(&format!(
        "https://chrome.google.com/webstore/devconsole/{PUBLISHER}"
    )));
    assert_eq!(a.as_deref(), Some(PUBLISHER));
    let b = try_parse_publisher_id(Some(&format!(
        "https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit?hl=ja"
    )));
    assert_eq!(b.as_deref(), Some(PUBLISHER));
}

#[test]
fn try_parse_publisher_id_入口やほかのサイトは対象外() {
    assert!(try_parse_publisher_id(Some(ROOT)).is_none());
    assert!(try_parse_publisher_id(Some("https://accounts.google.com/signin")).is_none());
    assert!(
        try_parse_publisher_id(Some(&format!(
            "https://evil.example/webstore/devconsole/{PUBLISHER}"
        )))
        .is_none()
    );
    assert!(
        try_parse_publisher_id(Some(&format!(
            "http://chrome.google.com/webstore/devconsole/{PUBLISHER}"
        )))
        .is_none()
    );
    assert!(try_parse_publisher_id(None).is_none());
}

#[test]
fn edit_url_編集画面のurlを作る() {
    let url = edit_url(PUBLISHER, ITEM);

    assert_eq!(
        url,
        format!("https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit")
    );
    assert!(is_edit_page_of(Some(&url), ITEM));
    assert!(!is_edit_page_of(
        Some(&format!(
            "https://chrome.google.com/webstore/devconsole/{PUBLISHER}"
        )),
        ITEM
    ));
}

#[test]
fn redact_ログに出す文字列からpublisher_idを伏せる() {
    let text =
        format!("移動先: https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit");

    let redacted = redact(&text);

    assert!(!redacted.contains(PUBLISHER));
    assert!(redacted.contains(&format!("devconsole/<publisher>/{ITEM}/edit")));
    // 2 回通しても変わらない
    assert_eq!(redacted, redact(&redacted));
}

// 実画面のアカウント ボタンの名前と同じ形（氏名・アドレスは架空の値）
#[test]
fn redact_account_アカウントボタンの氏名とメールアドレスを伏せる() {
    let ja = r#"button "Google アカウント: Taro Example (taro.example@example.com), Google メンバーシップ" {expanded=false}"#;
    let en = r#"button "Google Account: 山田 花子 (hanako+cws@mail.example.co.jp)" @1,2,3,4"#;

    let ja_r = redact_account(ja);
    let en_r = redact_account(en);

    assert_eq!(
        ja_r,
        r#"button "Google アカウント: <name> (<email>), Google メンバーシップ" {expanded=false}"#
    );
    assert_eq!(
        en_r,
        r#"button "Google Account: <name> (<email>)" @1,2,3,4"#
    );
    // 2 回通しても変わらない
    assert_eq!(ja_r, redact_account(&ja_r));
}

#[test]
fn redact_account_本文中のメールアドレスも伏せ_ほかの文字列は変えない() {
    let text = r#"textbox "説明" value="お問い合わせ: support@example.com まで。Google アカウントの設定は不要です""#;

    assert_eq!(
        redact_account(text),
        r#"textbox "説明" value="お問い合わせ: <email> まで。Google アカウントの設定は不要です""#
    );
    // @ を含んでもアドレスの形でなければ変えない
    let plain = r#"text "@x,y,w,h の矩形" link "user@localhost""#;
    assert_eq!(redact_account(plain), plain);
}

#[test]
fn redact_diagnostics_publisher_idと氏名とメールアドレスをまとめて伏せる() {
    let text = format!(
        "url: https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit\n\
         option \"投稿者\" {{selected=true data-value=\"{PUBLISHER}\"}}\n\
         button \"Google アカウント: Taro Example (taro@example.com)\""
    );

    let redacted = redact_diagnostics(&text, PUBLISHER);

    assert!(!redacted.contains(PUBLISHER));
    assert!(!redacted.contains("Taro Example"));
    assert!(!redacted.contains("taro@example.com"));
    assert!(redacted.contains(&format!("devconsole/<publisher>/{ITEM}/edit")));
    assert!(redacted.contains("data-value=\"<publisher>\""));
    assert!(redacted.contains("Google アカウント: <name> (<email>)"));
    // publisherId が空でも、アカウントの伏せ字は行う
    assert!(!redact_diagnostics(&text, "").contains("taro@example.com"));
}

#[test]
fn privacy_url_プライバシー_タブのurlを作る() {
    let url = privacy_url(PUBLISHER, ITEM);
    assert_eq!(
        url,
        format!("https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit/privacy")
    );
    assert!(is_privacy_page_of(Some(&url), ITEM));
    // 末尾のスラッシュ・大文字小文字の違いは同じ画面
    assert!(is_privacy_page_of(
        Some(&format!("{url}/")),
        &ITEM.to_uppercase()
    ));
    // 掲載情報の編集画面・別のアイテム・別のタブは対象外
    assert!(!is_privacy_page_of(Some(&edit_url(PUBLISHER, ITEM)), ITEM));
    assert!(!is_privacy_page_of(
        Some(&url),
        "ponmlkjihgfedcbaponmlkjihgfedcba"
    ));
    assert!(!is_privacy_page_of(
        Some(&format!(
            "https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit/listing"
        )),
        ITEM
    ));
    assert!(!is_privacy_page_of(
        Some("https://accounts.google.com/signin"),
        ITEM
    ));
    assert!(!is_privacy_page_of(None, ITEM));
    // 「プライバシー」タブは編集画面の URL ではない（open_item の確認に取り違えない）
    assert!(!is_edit_page_of(Some(&url), ITEM));
}
