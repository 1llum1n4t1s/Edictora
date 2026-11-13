//! WinHTTP 経由の Jev クライアントのテスト。127.0.0.1 の偽 HTTP サーバー（平文）だけを相手にし、
//! 本物の API は呼ばない。トークンもテスト用の架空の値を使う（secrets.json は読まない）。
// テスト名に Jev・URL などの英大文字を含めるため
#![allow(non_snake_case)]
#![cfg(windows)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::jev::client::{JevClient, JevToken};
use edictora::jev::winhttp::{self, HttpPost, HttpTarget};
use edictora::jev::{Candidate, ChoiceRequest, ElementChooser, JevPick};

/// テスト用の架空のトークン（エラー文に出ないことを確かめる）
const TOKEN: &str = "test-token-DO-NOT-LEAK-1234";

/// 偽サーバーが受け取ったリクエスト
#[derive(Debug)]
struct Received {
    request_line: String,
    headers: Vec<String>,
    body: String,
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
    let received = Received {
        request_line: request_line.trim_end().to_string(),
        headers,
        body: String::new(),
    };
    let len: usize = received
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).unwrap();
    Received {
        body: String::from_utf8(body).unwrap(),
        ..received
    }
}

/// 1 回だけ応答する偽サーバー。`response` が None なら応答せずに待ち続ける（タイムアウト・中断の確認用）
fn serve_once(response: Option<String>) -> (u16, mpsc::Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let received = read_request(&mut stream);
        let _ = tx.send(received);
        match response {
            Some(r) => {
                let _ = stream.write_all(r.as_bytes());
                let _ = stream.flush();
            }
            None => thread::sleep(Duration::from_secs(10)),
        }
    });
    (port, rx)
}

fn http_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn client(port: u16, timeout: Duration) -> JevClient {
    JevClient::with_loopback_endpoint(
        JevToken::parse(Some(TOKEN)).unwrap(),
        "jev-latest",
        port,
        timeout,
    )
}

fn request() -> ChoiceRequest {
    ChoiceRequest {
        purpose: "言語の選択肢（ロケール en）".to_string(),
        candidates: vec![
            Candidate {
                key: "c0".to_string(),
                role: "option".to_string(),
                tag: "div".to_string(),
                r#type: None,
                labels: vec![],
                text: Some("日本語 – ja".to_string()),
            },
            Candidate {
                key: "c1".to_string(),
                role: "option".to_string(),
                tag: "div".to_string(),
                r#type: None,
                labels: vec![],
                text: Some("英語 – en （デフォルト）".to_string()),
            },
        ],
    }
}

fn assert_no_token(e: &Error) {
    let text = format!("{e} {e:?}");
    assert!(!text.contains(TOKEN), "トークンが出ている: {text}");
}

#[test]
fn 成功応答を答えにし_認証ヘッダーと本文を送る() {
    let body = r#"{"model":"jev-2026","answers":{"target":{"type":"choice","choice":"c1","confidence":0.96,"probabilities":{"c0":0.02,"c1":0.96,"none":0.02}}},"usage":{"input_tokens":120,"output_tokens":1}}"#;
    let (port, rx) = serve_once(Some(http_response("200 OK", body)));
    let answer = client(port, Duration::from_secs(10))
        .choose(&request(), &CancelToken::new())
        .unwrap();
    assert_eq!(answer.pick, JevPick::Candidate(1));
    assert_eq!(answer.confidence, 0.96);

    let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(got.request_line, "POST /v1/systemone HTTP/1.1");
    assert_eq!(
        got.header("authorization").as_deref(),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert!(
        got.header("content-type")
            .unwrap()
            .starts_with("application/json")
    );
    let sent: serde_json::Value = serde_json::from_str(&got.body).unwrap();
    assert_eq!(sent["model"], "jev-latest");
    assert_eq!(
        sent["state"]["candidates"][1]["text"],
        "英語 – en （デフォルト）"
    );
    assert_eq!(sent["questions"]["target"]["type"], "choice");
    // 本文にトークンは入らない
    assert!(!got.body.contains(TOKEN));
}

#[test]
fn 失敗応答はdetailを添えたJevエラーにしトークンを出さない() {
    let (port, _rx) = serve_once(Some(http_response(
        "400 Bad Request",
        r#"{"detail":"unknown model"}"#,
    )));
    let e = client(port, Duration::from_secs(10))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    match &e {
        Error::Jev(m) => {
            assert!(m.contains("HTTP 400"), "{m}");
            assert!(m.contains("unknown model"), "{m}");
        }
        other => panic!("{other:?}"),
    }
    assert_no_token(&e);

    let (port, _rx) = serve_once(Some(http_response(
        "422 Unprocessable Entity",
        r#"{"detail":[{"loc":["body","questions"],"msg":"field required","type":"missing"}]}"#,
    )));
    let e = client(port, Duration::from_secs(10))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    assert!(matches!(&e, Error::Jev(m) if m.contains("HTTP 422") && m.contains("field required")));
    assert_no_token(&e);

    let (port, _rx) = serve_once(Some(http_response(
        "401 Unauthorized",
        r#"{"detail":"bad token"}"#,
    )));
    let e = client(port, Duration::from_secs(10))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    assert!(matches!(&e, Error::Jev(m) if m.contains("EDICTORA_JEV_API_TOKEN")));
    assert_no_token(&e);
}

#[test]
fn 応答の形が違えばJevエラー() {
    // JSON でない
    let (port, _rx) = serve_once(Some(http_response("200 OK", "<html>oops</html>")));
    assert!(matches!(
        client(port, Duration::from_secs(10)).choose(&request(), &CancelToken::new()),
        Err(Error::Jev(_))
    ));
    // 送っていないキー
    let (port, _rx) = serve_once(Some(http_response(
        "200 OK",
        r#"{"answers":{"target":{"type":"choice","choice":"c7","confidence":0.9,"probabilities":{}}}}"#,
    )));
    assert!(matches!(
        client(port, Duration::from_secs(10)).choose(&request(), &CancelToken::new()),
        Err(Error::Jev(m)) if m.contains("送っていない")
    ));
}

#[test]
fn リダイレクトは追わずにエラーにする() {
    let (port, _rx) = serve_once(Some(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:1/v1/systemone\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string(),
    ));
    let e = client(port, Duration::from_secs(10))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    assert!(
        matches!(&e, Error::Jev(m) if m.contains("HTTP 307")),
        "{e:?}"
    );
}

#[test]
fn 応答が無ければタイムアウトでJevエラー() {
    let (port, _rx) = serve_once(None);
    let started = Instant::now();
    let e = client(port, Duration::from_secs(1))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    assert!(matches!(&e, Error::Jev(_)), "{e:?}");
    assert_no_token(&e);
    // 全体の期限（タイムアウト + 2 秒の余裕）までには抜ける
    assert!(started.elapsed() < Duration::from_secs(6));
}

#[test]
fn 中断の合図で待ちを抜けてCancelledを返す() {
    let (port, _rx) = serve_once(None);
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        trigger.cancel();
    });
    let started = Instant::now();
    let r = client(port, Duration::from_secs(30)).choose(&request(), &cancel);
    assert_eq!(r.unwrap_err(), Error::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn 接続できなければJevエラー() {
    // 使われていないポートを得てすぐ閉じる
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let e = client(port, Duration::from_secs(5))
        .choose(&request(), &CancelToken::new())
        .unwrap_err();
    assert!(
        matches!(&e, Error::Jev(m) if m.contains("WinHTTP")),
        "{e:?}"
    );
    assert_no_token(&e);
}

#[test]
fn 平文は127_0_0_1以外へ送らず_ヘッダーの改行も拒否する() {
    let post = |host: &str, header: &str| HttpPost {
        target: HttpTarget {
            secure: false,
            host: host.to_string(),
            port: 80,
            path: "/x".to_string(),
        },
        headers: vec![header.to_string()],
        body: vec![],
        timeout: Duration::from_secs(1),
    };
    assert!(matches!(
        winhttp::post(post("example.com", "A: b"), &CancelToken::new()),
        Err(Error::Jev(m)) if m.contains("127.0.0.1")
    ));
    assert!(matches!(
        winhttp::post(post("127.0.0.1", "A: b\r\nEvil: x"), &CancelToken::new()),
        Err(Error::Jev(_))
    ));
}

#[test]
fn トークンは空や改行入りを拒否しDebugでも値を出さない() {
    assert!(matches!(JevToken::parse(None), Err(Error::Usage(_))));
    assert!(matches!(JevToken::parse(Some("  ")), Err(Error::Usage(_))));
    let e = JevToken::parse(Some("abc\r\nX: y")).unwrap_err();
    assert!(!e.message().contains("abc"));
    let t = JevToken::parse(Some(TOKEN)).unwrap();
    assert!(!format!("{t:?}").contains(TOKEN));
    let c = client(1, Duration::from_secs(1));
    assert!(!format!("{c:?}").contains(TOKEN));
}
