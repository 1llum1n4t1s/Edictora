//! WinHTTP を FFI で直接使う、最小限の HTTP クライアント（HTTP/TLS crate は使わない。AGENTS.md の規約）。
//!
//! 自動更新のフィード取得・パッケージのダウンロード（`update`）が使う。要求は GET だけ
//! （本文を送る要求は持たない）。守る性質:
//! - 平文（http）は 127.0.0.1 だけに限る（テスト用の偽サーバー向け）。本番の接続先は各利用側の固定値。
//! - リダイレクトは無効にする（Authorization などのヘッダーを別のホストへ運ばない・配信元を勝手に変えない）。
//!   3xx はそのままステータスとして返し、利用側が失敗として扱う。
//! - 応答の本文には上限を設ける（メモリへ受けるときは [`ResponseSink::Memory`] の `max_bytes`、
//!   ファイルへ受けるときは [`ResponseSink::File`] の `max_bytes`）。
//! - WinHTTP の同期呼び出しはブロックするので、ワーカースレッドで実行する。呼び出し側は短い間隔で
//!   中断（Ctrl+C）と全体の期限を確かめ、中断なら [`HttpError::Cancelled`]、期限切れなら [`HttpError::TimedOut`] で抜ける
//!   （何秒待ったと伝えるかは利用側が決める）。
//!   抜けた後のワーカーは、読み取りの合間に中止の合図を見て止まるか、WinHTTP 側のタイムアウトで自然に終わる
//!   （プロセス終了でも止まる）。
//! - ヘッダー（秘密を含みうる）は、エラー文・ログ・Debug 表示に出さない。そのため [`HttpRequest`] は Debug を導出しない。
//! - エラー文は「{service} への通信で…」の形にし、どの相手との通信かを利用者に示す（service は呼び出し側が渡す）。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cancel::CancelToken;

/// 接続先
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTarget {
    /// HTTPS なら true。false（平文）は 127.0.0.1 のときだけ許す
    pub secure: bool,
    /// ホスト名
    pub host: String,
    /// ポート
    pub port: u16,
    /// パス（`/` から始まる。クエリ文字列を含めてよい）
    pub path: String,
}

/// GET 要求 1 回分（フィード・パッケージの取得）。ヘッダーに秘密を含みうるので Debug は導出しない
pub struct HttpRequest {
    /// 接続先
    pub target: HttpTarget,
    /// 追加ヘッダー（`名前: 値`）。CR・LF・NUL を含めてはならない
    pub headers: Vec<String>,
    /// 名前解決・接続・送信・受信それぞれのタイムアウト（WinHTTP の操作ごと）
    pub timeout: Duration,
    /// 呼び出し全体の期限（これを過ぎたら待つのをやめる）
    pub total_timeout: Duration,
    /// エラー文に出す相手の名前（例 `更新の配信元`）
    pub service: &'static str,
}

/// 応答本文の受け先
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseSink {
    /// メモリへ受ける（`max_bytes` を超えたら失敗）
    Memory { max_bytes: usize },
    /// ファイルへ書く（ステータスが 200 のときだけ。`max_bytes` を超えたら失敗）。
    /// ファイルは新規作成または切り詰めて書く。失敗時に残った途中のファイルは呼び出し側が片付ける
    File { path: PathBuf, max_bytes: u64 },
}

/// 応答
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP ステータスコード
    pub status: u16,
    /// 本文（[`ResponseSink::Memory`] のとき。ファイルへ受けたときは空）
    pub body: Vec<u8>,
    /// 受け取った本文のバイト数（ファイルへ書いた量を含む）
    pub received_bytes: u64,
}

/// 失敗
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// 利用者による中断（Ctrl+C）
    Cancelled,
    /// 全体の期限（`total_timeout`）までに応答が返らなかった
    TimedOut,
    /// 通信の失敗（接続不可・タイムアウト・上限超え・検証エラーなど）。ヘッダーの値は含まない
    Failed(String),
}

/// 中断を待ちの合間に確かめる間隔
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// 要求を送り、応答を受け取る
pub fn send(
    request: HttpRequest,
    sink: ResponseSink,
    cancel: &CancelToken,
) -> Result<HttpResponse, HttpError> {
    validate(&request).map_err(HttpError::Failed)?;
    if cancel.is_cancelled() {
        return Err(HttpError::Cancelled);
    }

    let total_timeout = request.total_timeout;
    // ワーカーが読み取りの合間に見る「やめて」の合図（中断・期限切れのとき呼び出し側が立てる）
    let abandon = Arc::new(AtomicBool::new(false));
    let worker_abandon = Arc::clone(&abandon);
    let worker_cancel = cancel.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("edictora-winhttp".to_string())
        .spawn(move || {
            let stop =
                move || worker_abandon.load(Ordering::SeqCst) || worker_cancel.is_cancelled();
            // 受け手が先に抜けていたら送れないが、それで構わない
            let _ = tx.send(sys::send_blocking(&request, &sink, &stop));
        })
        .map_err(|e| HttpError::Failed(format!("通信用のスレッドを作れませんでした: {e}")))?;

    let deadline = Instant::now() + total_timeout;
    loop {
        if cancel.is_cancelled() {
            abandon.store(true, Ordering::SeqCst);
            return Err(HttpError::Cancelled);
        }
        match rx.recv_timeout(CANCEL_POLL) {
            Ok(result) => return result.map_err(HttpError::Failed),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    abandon.store(true, Ordering::SeqCst);
                    return Err(HttpError::TimedOut);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(HttpError::Failed(
                    "通信用のスレッドが異常終了しました".to_string(),
                ));
            }
        }
    }
}

/// 送る前の検査（平文は 127.0.0.1 だけ、ヘッダーに改行を含めない）
fn validate(request: &HttpRequest) -> Result<(), String> {
    let t = &request.target;
    if !t.secure && t.host != "127.0.0.1" {
        return Err("平文の HTTP は 127.0.0.1 にしか送れません".to_string());
    }
    if t.host.is_empty() || !t.path.starts_with('/') {
        return Err("接続先の指定が正しくありません".to_string());
    }
    if t.path.chars().any(|c| c.is_control() || c == ' ') {
        return Err("接続先のパスに空白や制御文字が含まれています".to_string());
    }
    // 値は出さない（トークンを含みうるため）
    if request
        .headers
        .iter()
        .any(|h| h.contains('\r') || h.contains('\n') || h.contains('\0'))
    {
        return Err("ヘッダーに改行などの制御文字が含まれています".to_string());
    }
    if request.timeout.is_zero() || request.total_timeout.is_zero() {
        return Err("タイムアウトが 0 です".to_string());
    }
    Ok(())
}

#[cfg(windows)]
mod sys {
    //! WinHTTP の宣言と同期の送受信の本体（ワーカースレッドで呼ぶ）

    use std::ffi::c_void;
    use std::io::Write;
    use std::ptr::{null, null_mut};

    use super::{HttpRequest, HttpResponse, ResponseSink};

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
    /// ファイルへ受けるときに 1 回で読む上限（中止の合図を見る間隔にもなる）
    const FILE_CHUNK: u32 = 256 * 1024;

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
    fn failure(service: &str, api: &str) -> String {
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
        format!("{service} への通信で{hint}（{api}、WinHTTP エラー {code}）")
    }

    fn ok(result: i32, service: &str, api: &str) -> Result<(), String> {
        if result != 0 {
            Ok(())
        } else {
            Err(failure(service, api))
        }
    }

    fn handle(h: HInternet, service: &str, api: &str) -> Result<Handle, String> {
        if h.is_null() {
            Err(failure(service, api))
        } else {
            Ok(Handle(h))
        }
    }

    /// 同期で送受信する（ブロックする）。`stop` が true を返したら読み取りの合間に止める
    pub fn send_blocking(
        request: &HttpRequest,
        sink: &ResponseSink,
        stop: &dyn Fn() -> bool,
    ) -> Result<HttpResponse, String> {
        let service = request.service;
        let target = &request.target;
        let timeout_ms = i32::try_from(request.timeout.as_millis()).unwrap_or(i32::MAX);

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
                service,
                "WinHttpOpen",
            )?;
            ok(
                WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms),
                service,
                "WinHttpSetTimeouts",
            )?;

            let host = wide(&target.host);
            let connect = handle(
                WinHttpConnect(session.0, host.as_ptr(), target.port, 0),
                service,
                "WinHttpConnect",
            )?;

            let verb = wide("GET");
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
                service,
                "WinHttpOpenRequest",
            )?;

            // リダイレクトを追わない（ヘッダーを別ホストへ運ばない・配信元を勝手に変えない）
            let disable = WINHTTP_DISABLE_REDIRECTS;
            ok(
                WinHttpSetOption(
                    req.0,
                    WINHTTP_OPTION_DISABLE_FEATURE,
                    (&disable as *const u32).cast(),
                    std::mem::size_of::<u32>() as u32,
                ),
                service,
                "WinHttpSetOption",
            )?;

            let headers = wide(&request.headers.join("\r\n"));
            let (headers_ptr, headers_len) = if request.headers.is_empty() {
                (null(), 0)
            } else {
                (headers.as_ptr(), NUL_TERMINATED)
            };
            // GET なので送る本文は無い
            ok(
                WinHttpSendRequest(req.0, headers_ptr, headers_len, null(), 0, 0, 0),
                service,
                "WinHttpSendRequest",
            )?;
            ok(
                WinHttpReceiveResponse(req.0, null_mut()),
                service,
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
                service,
                "WinHttpQueryHeaders",
            )?;
            let status = u16::try_from(status).unwrap_or(0);

            match sink {
                ResponseSink::Memory { max_bytes } => {
                    let body = read_to_memory(req.0, service, *max_bytes, stop)?;
                    Ok(HttpResponse {
                        status,
                        received_bytes: body.len() as u64,
                        body,
                    })
                }
                ResponseSink::File { path, max_bytes } => {
                    // 200 以外の本文（エラーページなど）はファイルにしない
                    if status != 200 {
                        return Ok(HttpResponse {
                            status,
                            body: Vec::new(),
                            received_bytes: 0,
                        });
                    }
                    let mut file = std::fs::File::create(path).map_err(|e| {
                        format!(
                            "ダウンロード先のファイルを作れませんでした（{}）: {e}",
                            path.display()
                        )
                    })?;
                    let written = read_to_file(req.0, service, &mut file, *max_bytes, stop)?;
                    file.flush().and_then(|_| file.sync_all()).map_err(|e| {
                        format!("ダウンロードしたファイルを書き終えられませんでした: {e}")
                    })?;
                    Ok(HttpResponse {
                        status,
                        body: Vec::new(),
                        received_bytes: written,
                    })
                }
            }
        }
    }

    /// 本文をメモリへ読む（上限つき）
    ///
    /// # Safety
    /// `req` は有効な要求ハンドル
    unsafe fn read_to_memory(
        req: HInternet,
        service: &str,
        max_bytes: usize,
        stop: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, String> {
        let mut body: Vec<u8> = Vec::new();
        loop {
            if stop() {
                return Err(format!("{service} からの受信を中止しました"));
            }
            let mut available: u32 = 0;
            // SAFETY: 呼び出し側が有効なハンドルを渡す。available はこの関数内の変数
            ok(
                unsafe { WinHttpQueryDataAvailable(req, &mut available) },
                service,
                "WinHttpQueryDataAvailable",
            )?;
            if available == 0 {
                break;
            }
            if body.len() + available as usize > max_bytes {
                return Err(format!(
                    "{service} の応答が大きすぎます（{max_bytes} バイトを超えました）"
                ));
            }
            let start = body.len();
            body.resize(start + available as usize, 0);
            let mut read: u32 = 0;
            // SAFETY: body[start..] は available バイトの書き込み可能な領域
            ok(
                unsafe {
                    WinHttpReadData(req, body[start..].as_mut_ptr().cast(), available, &mut read)
                },
                service,
                "WinHttpReadData",
            )?;
            body.truncate(start + read as usize);
            if read == 0 {
                break;
            }
        }
        Ok(body)
    }

    /// 本文をファイルへ書く（上限つき）。書いたバイト数を返す
    ///
    /// # Safety
    /// `req` は有効な要求ハンドル
    unsafe fn read_to_file(
        req: HInternet,
        service: &str,
        file: &mut std::fs::File,
        max_bytes: u64,
        stop: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        let mut buffer = vec![0u8; FILE_CHUNK as usize];
        let mut written: u64 = 0;
        loop {
            if stop() {
                return Err(format!("{service} からのダウンロードを中止しました"));
            }
            let mut available: u32 = 0;
            // SAFETY: 呼び出し側が有効なハンドルを渡す
            ok(
                unsafe { WinHttpQueryDataAvailable(req, &mut available) },
                service,
                "WinHttpQueryDataAvailable",
            )?;
            if available == 0 {
                break;
            }
            let to_read = available.min(FILE_CHUNK);
            if written + u64::from(to_read) > max_bytes {
                return Err(format!(
                    "{service} から受け取るデータが宣言より大きくなりました（{max_bytes} バイトを超えました）"
                ));
            }
            let mut read: u32 = 0;
            // SAFETY: buffer は FILE_CHUNK バイト以上の書き込み可能な領域で、to_read はそれ以下
            ok(
                unsafe { WinHttpReadData(req, buffer.as_mut_ptr().cast(), to_read, &mut read) },
                service,
                "WinHttpReadData",
            )?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read as usize])
                .map_err(|e| format!("ダウンロードしたデータを書き込めませんでした: {e}"))?;
            written += u64::from(read);
        }
        Ok(written)
    }
}

#[cfg(not(windows))]
mod sys {
    use super::{HttpRequest, HttpResponse, ResponseSink};

    /// Windows 以外は対象外（配布物は Windows 向けだけ）
    pub fn send_blocking(
        request: &HttpRequest,
        _sink: &ResponseSink,
        _stop: &dyn Fn() -> bool,
    ) -> Result<HttpResponse, String> {
        Err(format!(
            "{} への通信は Windows でだけ使えます",
            request.service
        ))
    }
}
