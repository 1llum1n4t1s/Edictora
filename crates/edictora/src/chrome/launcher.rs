//! Chrome を専用プロファイル + リモートデバッグで起動する（または既存に接続する）（C# の `ChromeLauncher` / `ChromeInstance`）。
//!
//! - 起動引数は最小限（自動化フラグなし）。変える場合は実画面でログインを再検証すること
//! - Chrome の標準出力・標準エラーは捨てる（Chrome のログ行が CLI の標準エラーに漏れないように）
//! - 終了は CDP の `Browser.close` で正常終了させる（Cookie を確実に書き出すため）。応答しないときだけプロセスを止める
//! - **プロファイルは絶対に削除しない**（旧 ExtWeigh の ChromeInstance は削除していたが、その経路を持たない）
//! - DevTools の確認（`/json/version`）は localhost への素の HTTP/1.1 で行う（HTTP crate は使わない）

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::cdp::CdpClient;
use crate::error::{Error, Result};
use crate::logging::{LogLevel, log};
use crate::util::{decode_text, parse_absolute_url};

/// Chrome 起動オプション
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeLaunchOptions {
    /// chrome.exe のフルパス
    pub chrome_path: PathBuf,
    /// 専用プロファイル（--user-data-dir）。ログイン状態を保持するため使い回す。
    /// ログイン情報が入る秘密情報なので、このツールは絶対に削除しない
    pub user_data_dir: PathBuf,
    /// 起動時に開く URL
    pub start_url: String,
    /// ウィンドウを作らずに起動する（--headless=new）。読み出し・比較・保存・送信の既定。
    /// ログインは利用者が画面で操作するため、false（ウィンドウ表示）で起動すること。
    ///
    /// 2026-11-13 の検証: ヘッドレスでも同じプロファイルのログイン状態をそのまま使え、
    /// 言語切替・説明欄の読み出しができた。実行中に前面のウィンドウは一度も変わらなかった。
    pub headless: bool,
}

/// 起動引数を組み立てる。
///
/// Google ログインを弾かれないよう、最小限に保つ（2026-11-13 の検証で手動ログインできた構成）。
/// - --enable-automation などの自動化フラグは付けない（navigator.webdriver が true になり、ログインを拒否されうる）
/// - 描画の間引き抑止フラグは付ける。CalculateNativeWinOcclusion を止めると、ウィンドウが裏に回っても
///   Chrome が「隠れた」と判断せず描画を続けるので、前面に出さなくてもクリックが応答する（2026-11-13 に確認）
/// - ウィンドウを表示する場合は画面内に普通に表示する（画面外配置はしない。利用者がログイン操作をするため）
/// - `headless` のときだけ --headless=new と画面サイズを付ける
pub fn build_args(options: &ChromeLaunchOptions) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", options.user_data_dir.display()),
        "--remote-debugging-port=0".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-backgrounding-occluded-windows".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-features=CalculateNativeWinOcclusion".to_string(),
    ];
    if options.headless {
        // ヘッドレスの既定の画面は 800x600 と小さいので、ダッシュボードの通常表示に近い大きさにする
        args.push("--headless=new".to_string());
        args.push("--window-size=1280,900".to_string());
    }
    args.push(options.start_url.clone());
    args
}

/// 起動済み（または既に起動していた）Chrome
pub struct ChromeInstance {
    process: Option<Child>,
    browser_ws_url: String,
    closed: bool,
}

impl ChromeInstance {
    /// ブラウザレベル CDP WebSocket エンドポイント
    pub fn browser_ws_url(&self) -> &str {
        &self.browser_ws_url
    }

    /// このツールが起動したか（false なら既に開いていた Chrome に接続しただけ）
    pub fn owned_by_tool(&self) -> bool {
        self.process.is_some()
    }

    /// 自分で起動した Chrome を正常終了させる。既に開いていた Chrome は閉じない（利用者の操作中かもしれないため）。
    /// 中断（Ctrl+C）の後にも呼ばれるので、中断の合図は見ない
    pub fn close(&mut self, client: Option<&CdpClient>) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        let Some(process) = self.process.as_mut() else {
            return;
        };
        let exited = matches!(process.try_wait(), Ok(Some(_)));
        if !exited && let Some(c) = client {
            // Browser.close は応答前に接続が切れることがある。終了待ちで判断する
            let _ = c.send("Browser.close", None, None, Duration::from_secs(10));
        }
        if !wait_for_exit(process, Duration::from_secs(20)) {
            // 正常終了しなかったときだけ止める。プロファイルには触れない
            log(
                "Chrome が時間内に終了しなかったため、プロセスを停止します",
                LogLevel::Warning,
            );
            kill_tree(process);
        }
    }
}

/// 専用プロファイルの Chrome に接続する。
/// そのプロファイルの Chrome がデバッグポート付きで既に開いていれば接続し、なければ新しく起動する。
///
/// 同じプロファイルの Chrome が開いていると、新しい chrome.exe は既存のウィンドウに処理を渡してすぐ終了する。
/// そのため先に既存への接続を試す。デバッグポートなしで開かれていた場合は接続できないので、
/// 「閉じてから再実行」を案内するエラーにする。
pub fn launch_or_attach(
    options: &ChromeLaunchOptions,
    cancel: &CancelToken,
) -> Result<ChromeInstance> {
    std::fs::create_dir_all(&options.user_data_dir)?;
    let port_file = options.user_data_dir.join("DevToolsActivePort");

    // 1) 既に開いている Chrome（前回の login で開いたままのウィンドウなど）に接続を試す
    if port_file.is_file()
        && let Some((port, ws_path)) = try_read_port_file(&port_file)
        && let Some(ws) = probe(port, &ws_path)
    {
        log(
            &format!("既に開いている Chrome に接続します (port={port})"),
            LogLevel::Info,
        );
        return Ok(ChromeInstance {
            process: None,
            browser_ws_url: ws,
            closed: false,
        });
    }

    // 2) 新しく起動する。前回のポートファイルが残っていても読まないよう、起動時刻より新しいものだけを採る
    let launched_at = SystemTime::now() - Duration::from_secs(1);
    let mut process = Command::new(&options.chrome_path)
        .args(build_args(options))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            Error::Io(format!(
                "Chrome の起動に失敗しました: {}（{e}）",
                options.chrome_path.display()
            ))
        })?;

    match wait_for_devtools(&port_file, launched_at, &mut process, cancel) {
        Ok(ws) => {
            log(
                &format!("Chrome 起動完了 (pid={})", process.id()),
                LogLevel::Debug,
            );
            Ok(ChromeInstance {
                process: Some(process),
                browser_ws_url: ws,
                closed: false,
            })
        }
        Err(e) => {
            // 起動に失敗したプロセスだけを止める。プロファイルは削除しない
            if !matches!(process.try_wait(), Ok(Some(_))) {
                kill_tree(&mut process);
            }
            Err(e)
        }
    }
}

/// DevToolsActivePort（1 行目: ポート、2 行目: ws パス）を読む
fn try_read_port_file(port_file: &Path) -> Option<(u16, String)> {
    // Chrome 書き込み中の読み取り競合は None（呼び出し側で再試行する）
    let bytes = std::fs::read(port_file).ok()?;
    let text = decode_text(&bytes);
    let mut lines = text.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.to_string();
    (port > 0).then_some((port, path))
}

/// ポートが本当にこのプロファイルの Chrome の DevTools か確かめ、ブラウザの WebSocket URL を返す（違えば None）。
///
/// `expected_path` は DevToolsActivePort の 2 行目（/devtools/browser/<id>）。
/// 古いポートファイルの番号を別の Chrome が使っている場合に、誤って接続しないよう照合する。
pub fn probe(port: u16, expected_path: &str) -> Option<String> {
    let body = http_get_localhost(port, "/json/version", Duration::from_secs(3))?;
    let json: Value = serde_json::from_str(&body).ok()?;
    let url = json.get("webSocketDebuggerUrl")?.as_str()?;
    let parsed = parse_absolute_url(url)?;
    let ok = parsed.scheme == "ws"
        && (parsed.host == "127.0.0.1" || parsed.host == "localhost")
        && parsed.path == expected_path.trim();
    ok.then(|| url.to_string())
}

/// 127.0.0.1 へ素の HTTP/1.1 GET を送り、200 の本文を返す（応答なし・別のプログラムなら None）
fn http_get_localhost(port: u16, path: &str, timeout: Duration) -> Option<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;

    let deadline = Instant::now() + timeout;
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        if Instant::now() > deadline || raw.len() > 1024 * 1024 {
            break;
        }
        // 本文の長さが分かっていて読み終えたら終わる（Connection: close を守らない相手への備え）
        if let Some((head, body)) = split_http(&raw)
            && let Some(len) = content_length(&head)
            && body.len() >= len
        {
            break;
        }
    }

    let (head, body) = split_http(&raw)?;
    let status_ok = head
        .lines()
        .next()
        .is_some_and(|l| l.split_whitespace().nth(1) == Some("200"));
    if !status_ok {
        return None;
    }
    let chunked = head.lines().any(|l| {
        l.to_ascii_lowercase().starts_with("transfer-encoding:")
            && l.to_ascii_lowercase().contains("chunked")
    });
    let body = if chunked {
        decode_chunked(&body)?
    } else {
        body
    };
    let body = match content_length(&head) {
        Some(len) if !chunked && body.len() > len => body[..len].to_vec(),
        _ => body,
    };
    String::from_utf8(body).ok()
}

fn split_http(raw: &[u8]) -> Option<(String, Vec<u8>)> {
    let pos = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some((
        String::from_utf8_lossy(&raw[..pos]).into_owned(),
        raw[pos + 4..].to_vec(),
    ))
}

fn content_length(head: &str) -> Option<usize> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| v.trim().parse().ok())
            .flatten()
    })
}

fn decode_chunked(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = body.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&body[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if body.len() < size {
            return None;
        }
        out.extend_from_slice(&body[..size]);
        body = body.get(size + 2..)?;
    }
}

/// 起動後に書かれた DevToolsActivePort を待ち、接続先を確かめる
fn wait_for_devtools(
    port_file: &Path,
    launched_at: SystemTime,
    process: &mut Child,
    cancel: &CancelToken,
) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        cancel.check()?;
        if matches!(process.try_wait(), Ok(Some(_))) {
            return Err(Error::Io(
                "Chrome が起動直後に終了しました。同じプロファイルの Chrome がデバッグポートなしで開いている可能性があります。そのウィンドウを閉じてから再実行してください。"
                    .to_string(),
            ));
        }
        let fresh = std::fs::metadata(port_file)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t >= launched_at);
        if fresh
            && let Some((port, ws_path)) = try_read_port_file(port_file)
            && let Some(ws) = probe(port, &ws_path)
        {
            return Ok(ws);
        }
        cancel.sleep(Duration::from_millis(250))?;
    }
    Err(Error::Timeout(
        "Chrome の DevTools が 30 秒以内に応答しませんでした（Chrome の起動失敗）".to_string(),
    ))
}

/// プロセスの終了を待つ（時間内に終わったら true）
fn wait_for_exit(process: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match process.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return true,
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// プロセスを子プロセスごと止める（C# の `Kill(entireProcessTree: true)`）。プロファイルには触れない
fn kill_tree(process: &mut Child) {
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let status = Command::new(system_root.join("System32").join("taskkill.exe"))
            .args(["/PID", &process.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|s| s.success()) {
            let _ = process.wait();
            return;
        }
    }
    let _ = process.kill();
    let _ = process.wait();
}
