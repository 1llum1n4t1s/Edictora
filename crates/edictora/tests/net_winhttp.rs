//! 共用の通信層（`net::winhttp`）の契約のテスト。自動更新が使う GET を、127.0.0.1 の偽 HTTP サーバー
//! （平文）だけを相手に確かめる。本物の配信元には繋がない。
//!
//! 取得・検証・適用までの流れは `update_download.rs` が確かめている（HTTP エラー・リダイレクトを追わない・
//! ファイルへ受ける上限・事前の中断など）。ここでは、それが前提にしている通信層そのものの安全策
//! （全体の期限・通信中の中断・接続失敗・平文の制限・ヘッダーの検査・メモリへ受ける上限・秘密の非表示）を固める。
// テスト名に GET などの英大文字を含めるため
#![allow(non_snake_case)]
#![cfg(windows)]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use edictora::cancel::CancelToken;
use edictora::net::winhttp::{HttpError, HttpRequest, HttpTarget, ResponseSink, send};

/// テスト用の架空の秘密（ヘッダーの値。エラー文に出ないことを確かめる）
const SECRET: &str = "secret-header-value-DO-NOT-LEAK-1234";

/// エラー文に出す相手の名前
const SERVICE: &str = "テスト相手";

/// 偽サーバーが受け取った要求（要求行とヘッダー）
#[derive(Debug)]
struct Received {
    request_line: String,
    headers: Vec<String>,
}

impl Received {
    fn header(&self, name: &str) -> Option<String> {
        let prefix = format!("{}:", name.to_lowercase());
        self.headers
            .iter()
            .find(|h| h.to_lowercase().starts_with(&prefix))
            .map(|h| h[prefix.len()..].trim().to_string())
    }
}

fn read_request(stream: &mut TcpStream) -> Received {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end().to_string();
        if line.is_empty() {
            break;
        }
        headers.push(line);
    }
    Received {
        request_line: request_line.trim_end().to_string(),
        headers,
    }
}

/// 1 回だけ応答する偽サーバー。`response` が None なら応答せずに待ち続ける（タイムアウト・中断の確認用）
fn serve_once(response: Option<Vec<u8>>) -> (u16, mpsc::Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let received = read_request(&mut stream);
        let _ = tx.send(received);
        match response {
            Some(r) => {
                let _ = stream.write_all(&r);
                let _ = stream.flush();
            }
            None => thread::sleep(Duration::from_secs(10)),
        }
    });
    (port, rx)
}

fn http_response(status: &str, body: &[u8]) -> Vec<u8> {
    let mut r = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    r
}

/// GET 要求を作る（`op` は操作ごとのタイムアウト、`total` は全体の期限）
fn request(
    host: &str,
    port: u16,
    headers: Vec<String>,
    op: Duration,
    total: Duration,
) -> HttpRequest {
    HttpRequest {
        target: HttpTarget {
            secure: false,
            host: host.to_string(),
            port,
            path: "/x?channel=win".to_string(),
        },
        headers,
        timeout: op,
        total_timeout: total,
        service: SERVICE,
    }
}

fn secret_header() -> Vec<String> {
    vec![format!("X-Secret: {SECRET}")]
}

fn memory(max_bytes: usize) -> ResponseSink {
    ResponseSink::Memory { max_bytes }
}

fn assert_no_secret(e: &HttpError) {
    let text = format!("{e:?}");
    assert!(!text.contains(SECRET), "ヘッダーの値が出ている: {text}");
}

#[test]
fn GETで追加ヘッダーを送りステータスと本文を返す() {
    let (port, rx) = serve_once(Some(http_response("200 OK", b"hello")));
    let r = send(
        request(
            "127.0.0.1",
            port,
            secret_header(),
            Duration::from_secs(10),
            Duration::from_secs(12),
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"hello");
    assert_eq!(r.received_bytes, 5);

    let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(got.request_line, "GET /x?channel=win HTTP/1.1");
    assert_eq!(got.header("x-secret").as_deref(), Some(SECRET));
}

#[test]
fn メモリへ受ける応答は上限を超えたら失敗する() {
    let (port, _rx) = serve_once(Some(http_response("200 OK", &[b'a'; 100])));
    let e = send(
        request(
            "127.0.0.1",
            port,
            vec![],
            Duration::from_secs(10),
            Duration::from_secs(12),
        ),
        memory(10),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        matches!(&e, HttpError::Failed(m) if m.contains(SERVICE) && m.contains("大きすぎ")),
        "{e:?}"
    );
}

#[test]
fn 応答が無ければ期限までに抜け_ヘッダーの値を出さない() {
    // 操作ごとのタイムアウト（1 秒）が先に効く。どちらの失敗でも、全体の期限（3 秒）までには抜ける
    let (port, _rx) = serve_once(None);
    let started = Instant::now();
    let e = send(
        request(
            "127.0.0.1",
            port,
            secret_header(),
            Duration::from_secs(1),
            Duration::from_secs(3),
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        matches!(e, HttpError::Failed(_) | HttpError::TimedOut),
        "{e:?}"
    );
    assert_no_secret(&e);
    assert!(started.elapsed() < Duration::from_secs(6));

    // 操作ごとのタイムアウトが長くても、全体の期限（1 秒）で TimedOut にして抜ける
    let (port, _rx) = serve_once(None);
    let started = Instant::now();
    let e = send(
        request(
            "127.0.0.1",
            port,
            secret_header(),
            Duration::from_secs(30),
            Duration::from_secs(1),
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(e, HttpError::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn 通信中の中断の合図で待ちを抜けてCancelledを返す() {
    let (port, _rx) = serve_once(None);
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        trigger.cancel();
    });
    let started = Instant::now();
    let e = send(
        request(
            "127.0.0.1",
            port,
            vec![],
            Duration::from_secs(30),
            Duration::from_secs(30),
        ),
        memory(1024),
        &cancel,
    )
    .unwrap_err();
    assert_eq!(e, HttpError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn 接続できなければ相手の名前入りの失敗にし_ヘッダーの値を出さない() {
    // 使われていないポートを得てすぐ閉じる
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let e = send(
        request(
            "127.0.0.1",
            port,
            secret_header(),
            Duration::from_secs(5),
            Duration::from_secs(7),
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        matches!(&e, HttpError::Failed(m) if m.contains(SERVICE) && m.contains("WinHTTP")),
        "{e:?}"
    );
    assert_no_secret(&e);
}

#[test]
fn 平文は127_0_0_1以外へ送らず_ヘッダーの改行も拒否する() {
    let one_sec = Duration::from_secs(1);
    let e = send(
        request(
            "example.com",
            80,
            vec!["A: b".to_string()],
            one_sec,
            one_sec,
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        matches!(&e, HttpError::Failed(m) if m.contains("127.0.0.1")),
        "{e:?}"
    );

    // 改行入りのヘッダーは送らない（別のヘッダーを紛れ込ませる注入の防止）。値はエラー文に出さない
    let e = send(
        request(
            "127.0.0.1",
            80,
            vec![format!("A: {SECRET}\r\nEvil: x")],
            one_sec,
            one_sec,
        ),
        memory(1024),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(&e, HttpError::Failed(_)), "{e:?}");
    assert_no_secret(&e);
}
