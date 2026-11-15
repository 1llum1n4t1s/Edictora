//! ダッシュボードに載っている画像の原本を取得してハッシュにする。
//!
//! ダッシュボードの `<img>` は縮小版の URL（googleusercontent.com）を指す。末尾の `=s0` が「原寸の原本」を返す指定で、
//! 2026-11-15 にバックアップした原本のバイト列と一致することを確かめてある（SearchClock など 15 件）。
//! 取得した原本は `png::pixel_digest` で画素のダイジェストにする（バイト列そのものは比べない。理由は `images` モジュールの冒頭）。
//! 通信は `net::winhttp`（HTTP/TLS crate は使わない）。守ること:
//! - 接続先は https の `*.googleusercontent.com` だけ（ページから読んだ URL が別のホストを指していても取りに行かない）
//! - Cookie・認証ヘッダーは送らない（画像 URL は推測できない長い ID で守られた公開 URL。ログイン情報を外へ出さない）
//! - 上限 16 MB、リダイレクトなし（`net::winhttp` の既定）
//! - 取得できなければ「比較不能」（`Error::Dashboard`）で、一致扱いにも常に置換にもしない

use std::time::Duration;

use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::images::png;
use crate::net::winhttp::{self, HttpError, HttpRequest, HttpTarget, ResponseSink};
use crate::util::parse_absolute_url;
use crate::{tr, trs};

/// 画像 1 枚の最大サイズ（原本）
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// 画像の原本の画素ダイジェストを得る境目（テストでは偽物に差し替える）
pub trait ImageFetcher {
    /// `src`（ページの `<img>` の URL）の原本の画素ダイジェスト（大文字の 16 進 SHA-256）。
    /// 取得・デコードできないときは `Error::Dashboard`（比較不能）、中断は `Error::Cancelled`
    fn pixel_digest_of(&self, src: &str, cancel: &CancelToken) -> Result<String>;
}

/// 本物の取得（WinHTTP）
pub struct WinHttpImageFetcher;

impl ImageFetcher for WinHttpImageFetcher {
    fn pixel_digest_of(&self, src: &str, cancel: &CancelToken) -> Result<String> {
        let target = original_target(src)?;
        let request = HttpRequest {
            target,
            headers: Vec::new(),
            timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(90),
            service: trs!("images.image_server"),
        };
        let response = winhttp::send(
            request,
            ResponseSink::Memory {
                max_bytes: MAX_IMAGE_BYTES,
            },
            cancel,
        )
        .map_err(|e| match e {
            HttpError::Cancelled => Error::Cancelled,
            HttpError::TimedOut => Error::Dashboard(trs!("images.fetch_timed_out").to_string()),
            HttpError::Failed(m) => Error::Dashboard(tr!("images.fetch_failed", detail = m)),
        })?;
        if response.status != 200 {
            return Err(Error::Dashboard(tr!(
                "images.fetch_status",
                status = response.status
            )));
        }
        png::pixel_digest(&response.body).map_err(|e| {
            Error::Dashboard(tr!(
                "images.the_fetched_image_cannot_be_decoded",
                detail = e
            ))
        })
    }
}

/// ページの画像 URL から、原本を取りに行く接続先を作る（https の googleusercontent.com だけ）
pub fn original_target(src: &str) -> Result<HttpTarget> {
    let refuse = || {
        Error::Dashboard(tr!(
            "images.refusing_to_fetch_the_image_url",
            src = abbreviate(src)
        ))
    };
    let text = src.trim();
    // クエリ・フラグメント・利用者情報・ポートを持つ URL は、想定外の形なので取りに行かない
    if text.contains(['?', '#']) {
        return Err(refuse());
    }
    let url = parse_absolute_url(text).ok_or_else(refuse)?;
    let authority = text
        .strip_prefix("https://")
        .map(|r| r.split('/').next().unwrap_or(""))
        .ok_or_else(refuse)?;
    let host_ok =
        url.host == "googleusercontent.com" || url.host.ends_with(".googleusercontent.com");
    if url.scheme != "https" || !host_ok || !authority.eq_ignore_ascii_case(&url.host) {
        return Err(refuse());
    }
    // 末尾のサイズ指定（`=s160-...` など）を外して `=s0`（原寸）にする
    let mut path = url.path.clone();
    let last_slash = path.rfind('/').unwrap_or(0);
    if let Some(eq) = path.rfind('=')
        && eq > last_slash
    {
        path.truncate(eq);
    }
    path.push_str("=s0");
    Ok(HttpTarget {
        secure: true,
        host: url.host,
        port: 443,
        path,
    })
}

/// エラー文に出す URL（長いので先頭だけ）
fn abbreviate(src: &str) -> String {
    let s: String = src.chars().take(60).collect();
    if src.chars().count() > 60 {
        format!("{s}...")
    } else {
        s
    }
}
