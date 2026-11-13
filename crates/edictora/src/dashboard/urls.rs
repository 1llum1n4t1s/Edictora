//! デベロッパー ダッシュボードの URL を組み立て・解釈する（C# の `DashboardUrls`）。
//!
//! publisherId はログイン後の URL（/webstore/devconsole/<publisherId>）から実行時に取得する。
//! コード・設定・ログには書かない（ログは `logging::redact` で伏せる）。

use crate::util::{escape_data_string, parse_absolute_url};

/// ダッシュボードのホスト
pub const HOST: &str = "chrome.google.com";

/// ダッシュボードの入口（ログイン済みなら publisherId 付きの URL へ移る）
pub const ROOT: &str = "https://chrome.google.com/webstore/devconsole/";

/// URL から publisherId を取り出す。ダッシュボードの URL でなければ None
pub fn try_parse_publisher_id(url: Option<&str>) -> Option<String> {
    let u = parse_absolute_url(url?)?;
    if u.scheme != "https" || !u.host.eq_ignore_ascii_case(HOST) {
        return None;
    }
    let segments: Vec<&str> = u.path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 3 {
        return None;
    }
    if !segments[0].eq_ignore_ascii_case("webstore")
        || !segments[1].eq_ignore_ascii_case("devconsole")
    {
        return None;
    }
    let candidate = segments[2];
    // publisherId は英数字とハイフンからなる識別子（GUID 形式）。それ以外の文字は受け付けない。
    let len = candidate.chars().count();
    if !(8..=64).contains(&len)
        || !candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    Some(candidate.to_string())
}

/// Google のログイン画面か
pub fn is_login_page(url: Option<&str>) -> bool {
    url.and_then(parse_absolute_url)
        .is_some_and(|u| u.host.eq_ignore_ascii_case("accounts.google.com"))
}

/// アイテムの編集画面（ストアの掲載情報）の URL
pub fn edit_url(publisher_id: &str, item_id: &str) -> String {
    debug_assert!(!publisher_id.trim().is_empty() && !item_id.trim().is_empty());
    format!(
        "{ROOT}{}/{}/edit",
        escape_data_string(publisher_id),
        escape_data_string(item_id)
    )
}

/// URL がそのアイテムの編集画面か（publisherId は問わない）
pub fn is_edit_page_of(url: Option<&str>, item_id: &str) -> bool {
    if try_parse_publisher_id(url).is_none() {
        return false;
    }
    let Some(u) = url.and_then(parse_absolute_url) else {
        return false;
    };
    let path = u.path.trim_end_matches('/').to_lowercase();
    path.ends_with(&format!("/{}/edit", item_id.to_lowercase()))
}
