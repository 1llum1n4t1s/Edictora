//! Chrome DevTools Protocol クライアント（C# の `CdpClient` / `CdpSession`）。
//!
//! ブラウザレベルの WebSocket エンドポイント（localhost のみ・TLS なし）に接続し、
//! flatten モードの sessionId ルーティングでコマンド送信・イベント受信を行う。
//!
//! 送受信の作り:
//! - WebSocket は 1 本の入出力スレッドだけが持つ。ソケットは非ブロッキングにし、
//!   tungstenite が再開可能として扱う `WouldBlock` の経路で読み書きする（読み取りタイムアウトは使わない）。
//! - コマンドは送信用のチャネルで入出力スレッドへ渡し、応答は id ごとのチャネルで受け取る。
//! - イベントは購読（[`Subscription`]）ごとのチャネルへ配る。コマンドの応答待ちの間も届く
//!   （`Page.loadEventFired` と beforeunload の `Page.javascriptDialogOpening` を取りこぼさないため）。
//! - 接続が切れたら、応答待ちのコマンドと購読はすべて失敗させる。

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::logging::{LogLevel, log};
use crate::util::parse_absolute_url;

/// 既定の応答待ち時間
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// CDP イベント（method + params + 発生元 sessionId）
#[derive(Debug, Clone, PartialEq)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

type EventFilter = Box<dyn Fn(&CdpEvent) -> bool + Send>;

struct Subscriber {
    id: u64,
    filter: EventFilter,
    tx: Sender<CdpEvent>,
}

/// 入出力スレッドと共有する状態
struct Shared {
    pending: Mutex<HashMap<u64, Sender<Result<Value>>>>,
    subscribers: Mutex<Vec<Subscriber>>,
    alive: AtomicBool,
}

enum Outgoing {
    Text(String),
    Close,
}

/// CDP クライアント
pub struct CdpClient {
    shared: Arc<Shared>,
    outgoing: Mutex<Option<Sender<Outgoing>>>,
    next_id: AtomicU64,
    next_subscriber: AtomicU64,
    thread: Mutex<Option<JoinHandle<()>>>,
    disposed: AtomicBool,
}

impl CdpClient {
    /// ブラウザレベルの WebSocket エンドポイント（`ws://127.0.0.1:<port>/devtools/browser/<id>`）に接続する
    pub fn connect(ws_url: &str) -> Result<Arc<CdpClient>> {
        let parsed = parse_absolute_url(ws_url)
            .ok_or_else(|| Error::Cdp(format!("WebSocket の URL が正しくありません: {ws_url}")))?;
        if parsed.scheme != "ws" {
            return Err(Error::Cdp(format!(
                "ws:// 以外の接続先には接続しません: {ws_url}"
            )));
        }
        let authority = ws_url
            .trim()
            .strip_prefix("ws://")
            .and_then(|r| r.split('/').next())
            .unwrap_or_default()
            .to_string();
        let addr = authority
            .to_socket_addrs()
            .map_err(|e| Error::Cdp(format!("接続先を解決できません: {e}")))?
            .next()
            .ok_or_else(|| Error::Cdp("接続先を解決できません".to_string()))?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .map_err(|e| Error::Cdp(format!("CDP に接続できません: {e}")))?;
        let _ = stream.set_nodelay(true);
        // ハンドシェイクは同期で済ませ、その後で非ブロッキングに切り替える
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let (ws, _response) = tungstenite::client(ws_url, stream)
            .map_err(|e| Error::Cdp(format!("CDP の WebSocket 接続に失敗しました: {e}")))?;
        ws.get_ref().set_read_timeout(None).map_err(Error::from)?;
        ws.get_ref().set_nonblocking(true).map_err(Error::from)?;

        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            subscribers: Mutex::new(Vec::new()),
            alive: AtomicBool::new(true),
        });
        let (tx, rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let handle = std::thread::Builder::new()
            .name("edictora-cdp".to_string())
            .spawn(move || io_loop(ws, rx, thread_shared))
            .map_err(Error::from)?;

        Ok(Arc::new(CdpClient {
            shared,
            outgoing: Mutex::new(Some(tx)),
            next_id: AtomicU64::new(0),
            next_subscriber: AtomicU64::new(0),
            thread: Mutex::new(Some(handle)),
            disposed: AtomicBool::new(false),
        }))
    }

    /// 接続が生きているか（利用者が Chrome を閉じると false になる）
    pub fn is_connected(&self) -> bool {
        !self.disposed.load(Ordering::SeqCst) && self.shared.alive.load(Ordering::SeqCst)
    }

    /// CDP コマンドを送信して result を返す（中断は見ない。終了処理の `Browser.close` などに使う）
    pub fn send(
        &self,
        method: &str,
        params: Option<Value>,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value> {
        self.send_with_cancel(method, params, session_id, timeout, None)
    }

    /// CDP コマンドを送信して result を返す。エラー応答は `Cdp`、時間切れは `Timeout`、中断は `Cancelled`
    pub fn send_with_cancel(
        &self,
        method: &str,
        params: Option<Value>,
        session_id: Option<&str>,
        timeout: Duration,
        cancel: Option<&CancelToken>,
    ) -> Result<Value> {
        if !self.is_connected() {
            return Err(Error::Cdp("CDP 接続が閉じられました".to_string()));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = mpsc::channel();
        self.shared.pending.lock().expect("pending").insert(id, tx);

        let mut payload = json!({ "id": id, "method": method });
        if let Some(p) = params {
            payload["params"] = p;
        }
        if let Some(s) = session_id {
            payload["sessionId"] = Value::String(s.to_string());
        }

        let sent = self
            .outgoing
            .lock()
            .expect("outgoing")
            .as_ref()
            .map(|o| o.send(Outgoing::Text(payload.to_string())).is_ok())
            .unwrap_or(false);
        let result = if sent {
            wait_channel(&rx, timeout, cancel).unwrap_or_else(|e| match e {
                WaitError::Timeout => Err(Error::Timeout(format!(
                    "CDP コマンド {method} の応答待ちがタイムアウトまたはキャンセルされました"
                ))),
                WaitError::Cancelled => Err(Error::Cancelled),
                WaitError::Closed => Err(Error::Cdp("CDP 接続が閉じられました".to_string())),
            })
        } else {
            Err(Error::Cdp("CDP 接続が閉じられました".to_string()))
        };
        self.shared.pending.lock().expect("pending").remove(&id);
        result
    }

    /// イベントを購読する（`filter` に一致するものだけが届く）。コマンドを送る「前」に購読すること（取りこぼし防止）
    pub fn subscribe(&self, filter: impl Fn(&CdpEvent) -> bool + Send + 'static) -> Subscription {
        let id = self.next_subscriber.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.shared
            .subscribers
            .lock()
            .expect("subscribers")
            .push(Subscriber {
                id,
                filter: Box::new(filter),
                tx,
            });
        // 既に切断されていたら、購読を残さず即座に「閉じた」扱いにする
        if !self.shared.alive.load(Ordering::SeqCst) {
            self.shared
                .subscribers
                .lock()
                .expect("subscribers")
                .retain(|s| s.id != id);
        }
        Subscription {
            id,
            rx,
            shared: self.shared.clone(),
        }
    }

    /// 接続を閉じる（多重呼び出しは無視）。入出力スレッドの終了を待つ
    pub fn close(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(tx) = self.outgoing.lock().expect("outgoing").take() {
            let _ = tx.send(Outgoing::Close);
        }
        if let Some(handle) = self.thread.lock().expect("thread").take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CdpClient {
    fn drop(&mut self) {
        self.close();
    }
}

/// イベントの購読（drop すると購読をやめる）
pub struct Subscription {
    id: u64,
    rx: Receiver<CdpEvent>,
    shared: Arc<Shared>,
}

impl Subscription {
    /// 次のイベントを待つ。時間切れは `Ok(None)`、切断は `Cdp`、中断は `Cancelled`
    pub fn recv(
        &self,
        timeout: Duration,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<CdpEvent>> {
        match wait_channel(&self.rx, timeout, cancel) {
            Ok(e) => Ok(Some(e)),
            Err(WaitError::Timeout) => Ok(None),
            Err(WaitError::Cancelled) => Err(Error::Cancelled),
            Err(WaitError::Closed) => Err(Error::Cdp("CDP 接続が閉じられました".to_string())),
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut subs) = self.shared.subscribers.lock() {
            subs.retain(|s| s.id != self.id);
        }
    }
}

enum WaitError {
    Timeout,
    Cancelled,
    Closed,
}

/// チャネルを、中断を見ながら待つ
fn wait_channel<T>(
    rx: &Receiver<T>,
    timeout: Duration,
    cancel: Option<&CancelToken>,
) -> std::result::Result<T, WaitError> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(WaitError::Cancelled);
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(WaitError::Timeout);
        }
        let slice = (deadline - now).min(Duration::from_millis(50));
        match rx.recv_timeout(slice) {
            Ok(v) => return Ok(v),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Err(WaitError::Closed),
        }
    }
}

/// 入出力スレッド本体
fn io_loop(mut ws: WebSocket<TcpStream>, outgoing: Receiver<Outgoing>, shared: Arc<Shared>) {
    let mut needs_flush = false;
    let mut closing_since: Option<Instant> = None;
    let reason = loop {
        // 1) 届いているメッセージをすべて読む
        let mut idle = false;
        match ws.read() {
            Ok(Message::Text(text)) => handle_message(text.as_str(), &shared),
            Ok(Message::Binary(bytes)) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    handle_message(text, &shared);
                }
            }
            Ok(Message::Close(_)) => break None,
            // tungstenite が Pong を送信待ちに積むので書き出す
            Ok(Message::Ping(_)) => needs_flush = true,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => idle = true,
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                break None;
            }
            Err(e) => break Some(e.to_string()),
        }

        // 終了要求後は、相手の Close を一定時間だけ待つ
        if let Some(since) = closing_since
            && since.elapsed() > Duration::from_secs(2)
        {
            break None;
        }

        // 2) 送信待ちを書き出す（読むものが無いときは少し待って CPU を空ける）
        let next = if closing_since.is_some() {
            // 終了要求の後は送るものが無い（送信側は切れていて即座に戻るので、空回りしないよう少し待つ）
            if idle {
                std::thread::sleep(Duration::from_millis(5));
            }
            None
        } else if idle && !needs_flush {
            match outgoing.recv_timeout(Duration::from_millis(5)) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => Some(Outgoing::Close),
            }
        } else {
            match outgoing.try_recv() {
                Ok(m) => Some(m),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Outgoing::Close),
            }
        };
        let mut batch: Vec<Outgoing> = next.into_iter().collect();
        while let Ok(m) = outgoing.try_recv() {
            batch.push(m);
        }
        let mut write_error = None;
        for m in batch {
            let res = match m {
                Outgoing::Text(t) => ws.write(Message::text(t)),
                Outgoing::Close => {
                    if closing_since.is_none() {
                        closing_since = Some(Instant::now());
                        ws.close(None)
                    } else {
                        Ok(())
                    }
                }
            };
            match res {
                Ok(()) => needs_flush = true,
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {
                    needs_flush = true
                }
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {}
                Err(e) => {
                    write_error = Some(e.to_string());
                    break;
                }
            }
        }
        if let Some(e) = write_error {
            break Some(e);
        }
        if needs_flush {
            match ws.flush() {
                Ok(()) => needs_flush = false,
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    break None;
                }
                Err(e) => break Some(e.to_string()),
            }
        }
        if idle && needs_flush {
            // 書き込みが詰まっているときは少し待つ
            std::thread::sleep(Duration::from_millis(1));
        }
    };

    if let Some(r) = reason
        && closing_since.is_none()
    {
        log(
            &format!("CDP 受信ループが切断されました: {r}"),
            LogLevel::Warning,
        );
    }
    shared.alive.store(false, Ordering::SeqCst);
    // 先に送信チャネルの受け口を閉じる。これより後に登録された応答待ちは、送信に失敗してすぐエラーになる
    // （順序を逆にすると、片付けの直後に登録された応答待ちが時間切れまで残りうる）
    drop(outgoing);
    // 応答待ちと購読をすべて失敗させる（送信側を落とすと受信側は Disconnected になる）
    let pending: Vec<_> = shared
        .pending
        .lock()
        .map(|mut p| p.drain().collect())
        .unwrap_or_default();
    for (_, tx) in pending {
        let _ = tx.send(Err(Error::Cdp("CDP 接続が閉じられました".to_string())));
    }
    if let Ok(mut subs) = shared.subscribers.lock() {
        subs.clear();
    }
}

/// 受信メッセージ 1 件を応答（id あり）またはイベント（method あり）として処理する
fn handle_message(text: &str, shared: &Shared) {
    let Ok(root) = serde_json::from_str::<Value>(text) else {
        log(
            "CDP から解釈できないメッセージを受け取りました",
            LogLevel::Warning,
        );
        return;
    };
    if let Some(id) = root.get("id").and_then(Value::as_u64) {
        let tx = shared.pending.lock().ok().and_then(|mut p| p.remove(&id));
        let Some(tx) = tx else { return };
        let result = match root.get("error") {
            Some(err) => Err(Error::Cdp(
                err.get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| err.to_string()),
            )),
            None => Ok(root.get("result").cloned().unwrap_or(Value::Null)),
        };
        let _ = tx.send(result);
    } else if let Some(method) = root.get("method").and_then(Value::as_str) {
        let event = CdpEvent {
            method: method.to_string(),
            params: root.get("params").cloned().unwrap_or(Value::Null),
            session_id: root
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        if let Ok(mut subs) = shared.subscribers.lock() {
            // 受け手が居なくなった購読は片付ける
            subs.retain(|s| !(s.filter)(&event) || s.tx.send(event.clone()).is_ok());
        }
    }
}
