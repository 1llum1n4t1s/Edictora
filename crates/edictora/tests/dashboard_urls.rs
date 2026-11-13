//! C# の DashboardUrls.test.cs の移植

use edictora::dashboard::urls::{
    ROOT, edit_url, is_edit_page_of, is_login_page, try_parse_publisher_id,
};
use edictora::logging::redact;

// テスト用の架空の publisherId（実在の値は使わない）
const PUBLISHER: &str = "00000000-1111-2222-3333-444444444444";
const ITEM: &str = "abcdefghijklmnopabcdefghijklmnop";

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
fn is_login_page_googleのログイン画面を判定する() {
    assert!(is_login_page(Some(
        "https://accounts.google.com/v3/signin/identifier"
    )));
    assert!(!is_login_page(Some(ROOT)));
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
