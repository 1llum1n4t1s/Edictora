//! `--jev` 用の POST（共用の [`crate::net::winhttp`] を薄く包む）。
//!
//! 通信の本体（WinHTTP の FFI・リダイレクト無効・タイムアウト・Ctrl+C で抜ける・平文は 127.0.0.1 だけ）は
//! `net::winhttp` にあり、自動更新と共用する。ここは Jev 向けの形（[`HttpPost`]・応答の上限・
//! 失敗を [`Error::Jev`] にする）だけを受け持つ。
//!
//! - 接続先は [`HttpTarget`] で渡す。本番の接続先は `jev::client` の固定値で、CLI・設定・環境変数からは変えられない。
//! - 応答の本文は [`MAX_RESPONSE_BYTES`] までに制限する。
//! - 呼び出し全体の期限は、操作ごとのタイムアウトに 2 秒の余裕を足したもの。
//! - ヘッダー（API トークンを含む）は、エラー文・ログ・Debug 表示に出さない。そのため [`HttpPost`] は Debug を導出しない。

use std::time::Duration;

use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::net::winhttp::{self as net, HttpError, HttpRequest, Method, ResponseSink};

pub use crate::net::winhttp::{HttpResponse, HttpTarget};

/// 応答本文の上限（Jev の応答は小さい。異常に大きければ途中で止める）
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// エラー文に出す相手の名前
const SERVICE: &str = "Jev";

/// POST 1 回分。ヘッダーに秘密を含むので Debug は導出しない
pub struct HttpPost {
    /// 接続先
    pub target: HttpTarget,
    /// 追加ヘッダー（`名前: 値`）。CR・LF を含めてはならない
    pub headers: Vec<String>,
    /// 本文
    pub body: Vec<u8>,
    /// 名前解決・接続・送信・受信それぞれのタイムアウト。呼び出し全体の期限にも使う
    pub timeout: Duration,
}

/// POST を送り、応答を受け取る。失敗は [`Error::Jev`]、中断は `Cancelled`
pub fn post(request: HttpPost, cancel: &CancelToken) -> Result<HttpResponse> {
    let timeout = request.timeout;
    let result = net::send(
        HttpRequest {
            method: Method::Post,
            target: request.target,
            headers: request.headers,
            body: request.body,
            timeout,
            // WinHTTP 側のタイムアウトは操作ごとなので、全体の期限は別に持つ（少し余裕を足す）。
            // 0 は net 側の検証で弾く（「タイムアウトが 0 です」）
            total_timeout: if timeout.is_zero() {
                Duration::ZERO
            } else {
                timeout + Duration::from_secs(2)
            },
            service: SERVICE,
        },
        ResponseSink::Memory {
            max_bytes: MAX_RESPONSE_BYTES,
        },
        cancel,
    );
    match result {
        Ok(r) => Ok(r),
        Err(HttpError::Cancelled) => Err(Error::Cancelled),
        // 利用者に伝えるのは操作ごとのタイムアウト（全体の期限はそれに 2 秒の余裕を足したもの）
        Err(HttpError::TimedOut) => Err(Error::Jev(format!(
            "Jev の応答を {} 秒待ちましたが返りませんでした（タイムアウト）",
            timeout.as_secs()
        ))),
        Err(HttpError::Failed(m)) => Err(Error::Jev(m)),
    }
}
