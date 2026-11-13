//! DevTools の確認（`/json/version` と DevToolsActivePort の照合）を、127.0.0.1 の偽 HTTP サーバーで確かめる。
//! Rust 版で足したテスト（C# 版は HttpClient を使っていた部分。Chrome は起動しない）。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

use edictora::chrome::probe;

/// 1 回だけ応答する偽サーバー。受け取った要求の先頭行も返す
fn serve_once(status: &str, body: &str, chunked: bool) -> (u16, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status = status.to_string();
    let body = body.replace("{PORT}", &port.to_string());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
        let response = if chunked {
            let half = body.len() / 2;
            format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                half,
                &body[..half],
                body.len() - half,
                &body[half..]
            )
        } else {
            format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        };
        stream.write_all(response.as_bytes()).unwrap();
        request.lines().next().unwrap_or_default().to_string()
    });
    (port, handle)
}

const VERSION: &str = r#"{ "Browser": "Chrome/150", "webSocketDebuggerUrl": "ws://127.0.0.1:{PORT}/devtools/browser/abc-123" }"#;

#[test]
fn パスが一致すればブラウザのwebsocket_urlを返す() {
    let (port, server) = serve_once("200 OK", VERSION, false);
    let ws = probe(port, "/devtools/browser/abc-123\n");
    assert_eq!(
        ws,
        Some(format!("ws://127.0.0.1:{port}/devtools/browser/abc-123"))
    );
    assert_eq!(server.join().unwrap(), "GET /json/version HTTP/1.1");
}

#[test]
fn チャンク形式の応答も読める() {
    let (port, server) = serve_once("200 OK", VERSION, true);
    assert!(probe(port, "/devtools/browser/abc-123").is_some());
    server.join().unwrap();
}

#[test]
fn パスが違えば別のchromeとみなして接続しない() {
    let (port, server) = serve_once("200 OK", VERSION, false);
    assert_eq!(probe(port, "/devtools/browser/other"), None);
    server.join().unwrap();
}

#[test]
fn localhost以外やws以外の接続先は使わない() {
    let remote = r#"{ "webSocketDebuggerUrl": "ws://192.0.2.1:9222/devtools/browser/abc-123" }"#;
    let (port, server) = serve_once("200 OK", remote, false);
    assert_eq!(probe(port, "/devtools/browser/abc-123"), None);
    server.join().unwrap();

    let tls = r#"{ "webSocketDebuggerUrl": "wss://127.0.0.1:9222/devtools/browser/abc-123" }"#;
    let (port, server) = serve_once("200 OK", tls, false);
    assert_eq!(probe(port, "/devtools/browser/abc-123"), None);
    server.join().unwrap();
}

#[test]
fn 成功以外の応答や応答なしはnone() {
    let (port, server) = serve_once("404 Not Found", VERSION, false);
    assert_eq!(probe(port, "/devtools/browser/abc-123"), None);
    server.join().unwrap();

    // 誰も待ち受けていないポート
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    assert_eq!(probe(port, "/devtools/browser/abc-123"), None);
}
