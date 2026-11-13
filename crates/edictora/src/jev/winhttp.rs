//! WinHTTP を FFI で直接使う、最小限の POST（HTTP/TLS crate は使わない。AGENTS.md の規約）。
//!
//! - 接続先は [`HttpTarget`] で渡す。平文（http）は 127.0.0.1 だけに限る（テスト用の偽サーバー向け）。
//!   本番の接続先は `jev::client` の固定値で、CLI・設定・環境変数からは変えられない。
//! - リダイレクトは無効にする（Authorization ヘッダーを別のホストへ運ばないため）。3xx はそのまま失敗として返す。
//! - 応答の本文は [`MAX_RESPONSE_BYTES`] までに制限する。
//! - WinHTTP の同期呼び出しはブロックするので、ワーカースレッドで実行する。呼び出し側は短い間隔で
//!   中断（Ctrl+C）と全体の期限を確かめ、中断なら `Cancelled`、期限切れなら [`Error::Jev`] で抜ける。
//!   抜けた後のワーカーは WinHTTP 側のタイムアウト（同じ長さ）で自然に終わる（プロセス終了でも止まる）。
//! - ヘッダー（API トークンを含む）は、エラー文・ログ・Debug 表示に出さない。そのため [`HttpPost`] は Debug を導出しない。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cancel::CancelToken;
use crate::error::{Error, Result};

/// 応答本文の上限（Jev の応答は小さい。異常に大きければ途中で止める）
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// 接続先
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTarget {
    /// HTTPS なら true。false（平文）は 127.0.0.1 のときだけ許す
    pub secure: bool,
    /// ホスト名
    pub host: String,
    /// ポート
    pub port: u16,
    /// パス（`/` から始まる）
    pub path: String,
}

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

/// 応答
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP ステータスコード
    pub status: u16,
    /// 本文
    pub body: Vec<u8>,
}

/// 中断を待ちの合間に確かめる間隔
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// POST を送り、応答を受け取る。失敗は [`Error::Jev`]、中断は `Cancelled`
pub fn post(request: HttpPost, cancel: &CancelToken) -> Result<HttpResponse> {
    validate(&request)?;
    cancel.check()?;

    let timeout = request.timeout;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("edictora-winhttp".to_string())
        .spawn(move || {
            // 受け手が先に抜けていたら送れないが、それで構わない
            let _ = tx.send(sys::post_blocking(&request));
        })
        .map_err(|e| Error::Jev(format!("通信用のスレッドを作れませんでした: {e}")))?;

    // WinHTTP 側のタイムアウトは操作ごとなので、全体の期限は別に持つ（少し余裕を足す）
    let deadline = Instant::now() + timeout + Duration::from_secs(2);
    loop {
        cancel.check()?;
        match rx.recv_timeout(CANCEL_POLL) {
            Ok(result) => return result.map_err(Error::Jev),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    return Err(Error::Jev(format!(
                        "Jev の応答を {} 秒待ちましたが返りませんでした（タイムアウト）",
                        timeout.as_secs()
                    )));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Error::Jev("通信用のスレッドが異常終了しました".to_string()));
            }
        }
    }
}

/// 送る前の検査（平文は 127.0.0.1 だけ、ヘッダーに改行を含めない）
fn validate(request: &HttpPost) -> Result<()> {
    let t = &request.target;
    if !t.secure && t.host != "127.0.0.1" {
        return Err(Error::Jev(
            "平文の HTTP は 127.0.0.1 にしか送れません".to_string(),
        ));
    }
    if t.host.is_empty() || !t.path.starts_with('/') {
        return Err(Error::Jev("接続先の指定が正しくありません".to_string()));
    }
    // 値は出さない（トークンを含むため）
    if request
        .headers
        .iter()
        .any(|h| h.contains('\r') || h.contains('\n') || h.contains('\0'))
    {
        return Err(Error::Jev(
            "ヘッダーに改行などの制御文字が含まれています".to_string(),
        ));
    }
    if request.timeout.is_zero() {
        return Err(Error::Jev("タイムアウトが 0 です".to_string()));
    }
    Ok(())
}

#[cfg(windows)]
mod sys {
    //! WinHTTP の宣言と同期 POST の本体（ワーカースレッドで呼ぶ）

    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    use super::{HttpPost, HttpResponse, MAX_RESPONSE_BYTES};

    type HInternet = *mut c_void;

    const WINHTTP_ACCESS_TYPE_NO_PROXY: u32 = 1;
    const WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY: u32 = 4;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_OPTION_DISABLE_FEATURE: u32 = 63;
    const WINHTTP_DISABLE_REDIRECTS: u32 = 0x0000_0002;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    /// ヘッダー文字列が NUL 終端であることを示す長さ（-1L）
    const NUL_TERMINATED: u32 = u32::MAX;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access_type: u32,
            proxy: *const u16,
            proxy_bypass: *const u16,
            flags: u32,
        ) -> HInternet;
        fn WinHttpSetTimeouts(
            handle: HInternet,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
        fn WinHttpConnect(
            session: HInternet,
            server: *const u16,
            port: u16,
            reserved: u32,
        ) -> HInternet;
        fn WinHttpOpenRequest(
            connect: HInternet,
            verb: *const u16,
            object_name: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept_types: *const *const u16,
            flags: u32,
        ) -> HInternet;
        fn WinHttpSetOption(
            handle: HInternet,
            option: u32,
            buffer: *const c_void,
            length: u32,
        ) -> i32;
        fn WinHttpSendRequest(
            request: HInternet,
            headers: *const u16,
            headers_length: u32,
            optional: *const c_void,
            optional_length: u32,
            total_length: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(request: HInternet, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(
            request: HInternet,
            info_level: u32,
            name: *const u16,
            buffer: *mut c_void,
            buffer_length: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn WinHttpQueryDataAvailable(request: HInternet, available: *mut u32) -> i32;
        fn WinHttpReadData(
            request: HInternet,
            buffer: *mut c_void,
            to_read: u32,
            read: *mut u32,
        ) -> i32;
        fn WinHttpCloseHandle(handle: HInternet) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLastError() -> u32;
    }

    /// WinHTTP のハンドル（drop で閉じる）
    struct Handle(HInternet);

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: WinHttpOpen 系が返した有効なハンドルを 1 回だけ閉じる
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 失敗した API 名と GetLastError を、利用者に分かる文にする（ヘッダーや本文は含めない）
    fn failure(api: &str) -> String {
        // SAFETY: 引数なしの読み取り
        let code = unsafe { GetLastError() };
        let hint = match code {
            12002 => "タイムアウトしました",
            12007 => "サーバー名を解決できません",
            12017 => "通信が取り消されました",
            12029 => "サーバーに接続できません",
            12030 => "接続が切れました",
            12152 => "サーバーの応答が正しくありません",
            12175 => "TLS（証明書）の検証に失敗しました",
            _ => "通信に失敗しました",
        };
        format!("Jev への通信で{hint}（{api}、WinHTTP エラー {code}）")
    }

    fn ok(result: i32, api: &str) -> Result<(), String> {
        if result != 0 {
            Ok(())
        } else {
            Err(failure(api))
        }
    }

    fn handle(h: HInternet, api: &str) -> Result<Handle, String> {
        if h.is_null() {
            Err(failure(api))
        } else {
            Ok(Handle(h))
        }
    }

    /// 同期で POST する（ブロックする）
    pub fn post_blocking(request: &HttpPost) -> Result<HttpResponse, String> {
        let target = &request.target;
        let timeout_ms = i32::try_from(request.timeout.as_millis()).unwrap_or(i32::MAX);
        let body_len = u32::try_from(request.body.len())
            .map_err(|_| "Jev へ送る本文が大きすぎます".to_string())?;

        let agent = wide(concat!("Edictora/", env!("CARGO_PKG_VERSION")));
        // 127.0.0.1（テスト）はプロキシを通さない。本番は OS のプロキシ設定に従う
        let access = if target.secure {
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
        } else {
            WINHTTP_ACCESS_TYPE_NO_PROXY
        };

        // SAFETY: 以下の WinHTTP 呼び出しには、NUL 終端の UTF-16 文字列と、この関数内で生きている
        // バッファ・ハンドルだけを渡す。ハンドルは Handle の drop で逆順に閉じる
        unsafe {
            let session = handle(
                WinHttpOpen(agent.as_ptr(), access, null(), null(), 0),
                "WinHttpOpen",
            )?;
            ok(
                WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms),
                "WinHttpSetTimeouts",
            )?;

            let host = wide(&target.host);
            let connect = handle(
                WinHttpConnect(session.0, host.as_ptr(), target.port, 0),
                "WinHttpConnect",
            )?;

            let verb = wide("POST");
            let path = wide(&target.path);
            let flags = if target.secure {
                WINHTTP_FLAG_SECURE
            } else {
                0
            };
            let req = handle(
                WinHttpOpenRequest(
                    connect.0,
                    verb.as_ptr(),
                    path.as_ptr(),
                    null(),
                    null(),
                    null(),
                    flags,
                ),
                "WinHttpOpenRequest",
            )?;

            // リダイレクトを追わない（Authorization を別ホストへ運ばない）
            let disable = WINHTTP_DISABLE_REDIRECTS;
            ok(
                WinHttpSetOption(
                    req.0,
                    WINHTTP_OPTION_DISABLE_FEATURE,
                    (&disable as *const u32).cast(),
                    std::mem::size_of::<u32>() as u32,
                ),
                "WinHttpSetOption",
            )?;

            let headers = wide(&request.headers.join("\r\n"));
            ok(
                WinHttpSendRequest(
                    req.0,
                    headers.as_ptr(),
                    NUL_TERMINATED,
                    request.body.as_ptr().cast(),
                    body_len,
                    body_len,
                    0,
                ),
                "WinHttpSendRequest",
            )?;
            ok(
                WinHttpReceiveResponse(req.0, null_mut()),
                "WinHttpReceiveResponse",
            )?;

            let mut status: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            ok(
                WinHttpQueryHeaders(
                    req.0,
                    WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                    null(),
                    (&mut status as *mut u32).cast(),
                    &mut size,
                    null_mut(),
                ),
                "WinHttpQueryHeaders",
            )?;

            let mut body: Vec<u8> = Vec::new();
            loop {
                let mut available: u32 = 0;
                ok(
                    WinHttpQueryDataAvailable(req.0, &mut available),
                    "WinHttpQueryDataAvailable",
                )?;
                if available == 0 {
                    break;
                }
                if body.len() + available as usize > MAX_RESPONSE_BYTES {
                    return Err(format!(
                        "Jev の応答が大きすぎます（{} バイトを超えました）",
                        MAX_RESPONSE_BYTES
                    ));
                }
                let start = body.len();
                body.resize(start + available as usize, 0);
                let mut read: u32 = 0;
                ok(
                    WinHttpReadData(
                        req.0,
                        body[start..].as_mut_ptr().cast(),
                        available,
                        &mut read,
                    ),
                    "WinHttpReadData",
                )?;
                body.truncate(start + read as usize);
                if read == 0 {
                    break;
                }
            }

            Ok(HttpResponse {
                status: u16::try_from(status).unwrap_or(0),
                body,
            })
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use super::{HttpPost, HttpResponse};

    /// Windows 以外は対象外（配布物は Windows 向けだけ）
    pub fn post_blocking(_request: &HttpPost) -> Result<HttpResponse, String> {
        Err("Jev への通信は Windows でだけ使えます".to_string())
    }
}
