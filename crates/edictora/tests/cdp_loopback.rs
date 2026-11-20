//! CDP クライアントの送受信を、127.0.0.1 に立てた偽の WebSocket サーバーで確かめる（Chrome は使わない）。
//! Rust 版で足したテスト（C# 版には無い）。
//! - 応答待ちの間にもイベントを受け取れる
//! - 分割して遅れて届く大きなメッセージを組み立てられる
//! - 切断時に応答待ちのコマンドが（時間切れを待たずに）失敗する
//! - 遷移中の beforeunload ダイアログは「留まる」を選んで中止する

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::cdp::{CdpClient, CdpPage};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

/// 偽サーバーを立て、接続 1 本を `serve` に渡す
fn fake_server(
    serve: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let ws = tungstenite::accept(stream).unwrap();
        serve(ws);
    });
    (
        format!("ws://127.0.0.1:{port}/devtools/browser/test"),
        handle,
    )
}

fn read_command(ws: &mut WebSocket<TcpStream>) -> Option<Value> {
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => return serde_json::from_str(t.as_str()).ok(),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => continue,
        }
    }
}

fn send_json(ws: &mut WebSocket<TcpStream>, v: Value) {
    ws.send(Message::text(v.to_string())).unwrap();
}

/// 相手が閉じるまで読み捨てる
fn drain(mut ws: WebSocket<TcpStream>) {
    while ws.read().is_ok() {}
}

#[test]
fn 応答待ちの間に届いたイベントを受け取れる() {
    let (url, server) = fake_server(|mut ws| {
        let cmd = read_command(&mut ws).unwrap();
        let id = cmd["id"].as_u64().unwrap();
        assert_eq!(cmd["method"], "Test.slow");
        assert_eq!(cmd["sessionId"], "S1");
        // 応答より先にイベントを送り、応答は少し遅らせる
        send_json(
            &mut ws,
            json!({ "method": "Page.loadEventFired", "params": { "timestamp": 1 }, "sessionId": "S1" }),
        );
        std::thread::sleep(Duration::from_millis(200));
        send_json(&mut ws, json!({ "id": id, "result": { "ok": true } }));
        // エラー応答も確かめる
        let cmd = read_command(&mut ws).unwrap();
        send_json(
            &mut ws,
            json!({ "id": cmd["id"], "error": { "code": -32000, "message": "だめです" } }),
        );
        drain(ws);
    });

    let client = CdpClient::connect(&url).unwrap();
    let sub = client.subscribe(|e| e.method == "Page.loadEventFired");
    let result = client
        .send("Test.slow", None, Some("S1"), Duration::from_secs(5))
        .unwrap();
    assert_eq!(result["ok"], true);
    // 応答を待っている間に届いたイベントが、購読に入っている
    let event = sub
        .recv(Duration::from_millis(10), None)
        .unwrap()
        .expect("イベント");
    assert_eq!(event.session_id.as_deref(), Some("S1"));

    let err = client.send("Test.fail", None, None, Duration::from_secs(5));
    assert_eq!(err, Err(Error::Cdp("だめです".to_string())));

    client.close();
    server.join().unwrap();
}

#[test]
fn 分割して遅れて届く大きなメッセージを組み立てられる() {
    let big = "あ".repeat(1_000_000); // 3 MB
    let expected = big.clone();
    let (url, server) = fake_server(move |mut ws| {
        let cmd = read_command(&mut ws).unwrap();
        let payload = json!({ "id": cmd["id"], "result": { "data": big } })
            .to_string()
            .into_bytes();
        // テキストフレーム（FIN・マスクなし・64 bit 長）を手で組み、少しずつ間をあけて書く
        let mut frame = vec![0x81u8, 127];
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        frame.extend_from_slice(&payload);
        let stream = ws.get_mut();
        for chunk in frame.chunks(256 * 1024) {
            stream.write_all(chunk).unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        drain(ws);
    });

    let client = CdpClient::connect(&url).unwrap();
    let result = client
        .send("Runtime.evaluate", None, None, Duration::from_secs(10))
        .unwrap();
    assert_eq!(result["data"].as_str().unwrap(), expected);
    client.close();
    server.join().unwrap();
}

#[test]
fn 切断されたら応答待ちのコマンドはすぐ失敗する() {
    let (url, server) = fake_server(|mut ws| {
        let _ = read_command(&mut ws);
        // Close フレームを送らずに TCP を切る
        let _ = ws.get_mut().shutdown(std::net::Shutdown::Both);
    });

    let client = CdpClient::connect(&url).unwrap();
    let started = Instant::now();
    let r = client.send("Browser.getVersion", None, None, Duration::from_secs(20));
    assert!(matches!(r, Err(Error::Cdp(_))), "{r:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "時間切れまで待ってしまった"
    );
    server.join().unwrap();

    // 切断後は新しいコマンドもすぐ失敗し、接続は切れた扱いになる
    let deadline = Instant::now() + Duration::from_secs(2);
    while client.is_connected() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!client.is_connected());
    assert!(matches!(
        client.send("Browser.getVersion", None, None, Duration::from_secs(1)),
        Err(Error::Cdp(_))
    ));
}

#[test]
fn 遷移中のbeforeunloadは留まるを選んで中止する() {
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen_server = seen.clone();
    let (url, server) = fake_server(move |mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            seen_server.lock().unwrap().push(cmd.clone());
            let id = cmd["id"].clone();
            match cmd["method"].as_str().unwrap() {
                "Page.navigate" => {
                    send_json(&mut ws, json!({ "id": id, "result": { "frameId": "F" } }));
                    send_json(
                        &mut ws,
                        json!({ "method": "Page.javascriptDialogOpening", "params": { "type": "beforeunload", "message": "" }, "sessionId": "S1" }),
                    );
                }
                _ => send_json(&mut ws, json!({ "id": id, "result": {} })),
            }
        }
    });

    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    let r = page.navigate("https://example.com/", &CancelToken::new());
    assert_eq!(
        r,
        Err(Error::Cdp(
            "未保存の変更があるため、ページの移動を中止しました".to_string()
        ))
    );
    client.close();
    server.join().unwrap();

    let seen = seen.lock().unwrap();
    let dialog = seen
        .iter()
        .find(|c| c["method"] == "Page.handleJavaScriptDialog")
        .expect("ダイアログへの応答");
    assert_eq!(dialog["params"]["accept"], false);
    assert_eq!(dialog["sessionId"], "S1");
}

#[test]
fn 遷移はload_event_firedを待って終わる() {
    let (url, server) = fake_server(move |mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            let id = cmd["id"].clone();
            send_json(&mut ws, json!({ "id": id, "result": {} }));
            if cmd["method"] == "Page.reload" {
                // 別セッションのイベントは無視されること
                send_json(
                    &mut ws,
                    json!({ "method": "Page.loadEventFired", "params": {}, "sessionId": "OTHER" }),
                );
                std::thread::sleep(Duration::from_millis(100));
                send_json(
                    &mut ws,
                    json!({ "method": "Page.loadEventFired", "params": {}, "sessionId": "S1" }),
                );
            }
        }
    });

    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    page.reload(&CancelToken::new()).unwrap();
    client.close();
    server.join().unwrap();
}

#[test]
fn 中断されたら応答待ちをやめる() {
    let (url, server) = fake_server(|mut ws| {
        // 応答を返さない
        let _ = read_command(&mut ws);
        drain(ws);
    });
    let client = CdpClient::connect(&url).unwrap();
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        c2.cancel();
    });
    let started = Instant::now();
    let r = client.send_with_cancel(
        "Test.never",
        None,
        None,
        Duration::from_secs(20),
        Some(&cancel),
    );
    assert_eq!(r, Err(Error::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
    client.close();
    server.join().unwrap();
}

#[test]
fn ファイル入力へファイルを選ぶ_パスはcdpの引数で渡し解放する() {
    // 式は returnByValue: false で評価して objectId を得て、DOM.setFileInputFiles へ渡し、終わったら解放する。
    // パスはページ内スクリプトに埋め込まれず、CDP の引数だけに入る
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen2 = seen.clone();
    let (url, server) = fake_server(move |mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            let id = cmd["id"].clone();
            let result = match cmd["method"].as_str().unwrap() {
                "Runtime.evaluate" => {
                    json!({ "result": { "type": "object", "objectId": "OBJ-1" } })
                }
                _ => json!({}),
            };
            seen2.lock().unwrap().push(cmd);
            send_json(&mut ws, json!({ "id": id, "result": result }));
        }
    });

    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    let path = std::path::PathBuf::from(r"D:\images\screenshot-01.png");
    page.set_file_input_files(
        "window.__edictora.element(\"fileInput\", 2)",
        std::slice::from_ref(&path),
        &CancelToken::new(),
    )
    .unwrap();
    client.close();
    server.join().unwrap();

    let seen = seen.lock().unwrap();
    let methods: Vec<&str> = seen.iter().map(|c| c["method"].as_str().unwrap()).collect();
    assert_eq!(
        methods,
        vec![
            "Runtime.evaluate",
            "DOM.setFileInputFiles",
            "Runtime.releaseObjectGroup"
        ]
    );
    assert_eq!(seen[0]["params"]["returnByValue"], false);
    assert!(
        !seen[0]["params"]["expression"]
            .as_str()
            .unwrap()
            .contains("screenshot-01")
    );
    assert_eq!(seen[1]["params"]["objectId"], "OBJ-1");
    assert_eq!(
        seen[1]["params"]["files"],
        json!([r"D:\images\screenshot-01.png"])
    );
    assert_eq!(seen[1]["sessionId"], "S1");
    assert_eq!(
        seen[2]["params"]["objectGroup"],
        seen[0]["params"]["objectGroup"]
    );
}

#[test]
fn ファイル入力の式が要素を返さなければ選ばずに失敗して解放する() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen2 = seen.clone();
    let (url, server) = fake_server(move |mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            let id = cmd["id"].clone();
            seen2
                .lock()
                .unwrap()
                .push(cmd["method"].as_str().unwrap().to_string());
            // objectId の無い結果（要素でなく値が返った）
            send_json(
                &mut ws,
                json!({ "id": id, "result": { "result": { "type": "undefined" } } }),
            );
        }
    });
    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    let r = page.set_file_input_files(
        "1",
        &[std::path::PathBuf::from("a.png")],
        &CancelToken::new(),
    );
    assert!(matches!(r, Err(Error::Cdp(_))), "{r:?}");
    client.close();
    server.join().unwrap();
    // DOM.setFileInputFiles は呼ばない
    assert_eq!(
        *seen.lock().unwrap(),
        vec!["Runtime.evaluate", "Runtime.releaseObjectGroup"]
    );
}

#[test]
fn ページ内で例外が出たらjavascriptエラーで選ばない() {
    let (url, server) = fake_server(|mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            let id = cmd["id"].clone();
            send_json(
                &mut ws,
                json!({ "id": id, "result": { "exceptionDetails": { "text": "Uncaught", "exception": { "description": "Error: EDICTORA_ELEMENT_GONE:fileInput" } } } }),
            );
        }
    });
    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    let r = page.set_file_input_files(
        "x",
        &[std::path::PathBuf::from("a.png")],
        &CancelToken::new(),
    );
    assert!(
        matches!(&r, Err(Error::JavaScript(m)) if m.contains("EDICTORA_ELEMENT_GONE")),
        "{r:?}"
    );
    client.close();
    server.join().unwrap();
}

#[test]
fn ホバーはマウス移動だけでクリックしない() {
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen2 = seen.clone();
    let (url, server) = fake_server(move |mut ws| {
        while let Some(cmd) = read_command(&mut ws) {
            let id = cmd["id"].clone();
            seen2.lock().unwrap().push(cmd);
            send_json(&mut ws, json!({ "id": id, "result": {} }));
        }
    });
    let client = CdpClient::connect(&url).unwrap();
    let page = CdpPage::new(client.clone(), "S1".to_string(), "T1".to_string());
    page.hover_at(10.0, 20.0, &CancelToken::new()).unwrap();
    client.close();
    server.join().unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["params"]["type"], "mouseMoved");
    assert_eq!(seen[0]["params"]["x"], 10.0);
}
