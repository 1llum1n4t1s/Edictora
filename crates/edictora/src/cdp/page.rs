//! 1 つのページ（タブ）に対する低レベル操作: スクリプト評価・マウス・キー入力・遷移（C# の `CdpPage`）。
//!
//! puppeteer が裏でやっていた処理を CDP で直接行う。注意点:
//! - クリックは mouseMoved → mousePressed → mouseReleased を clickCount=1 で送る（clickCount が無いと click イベントが起きない）
//! - Runtime.evaluate は returnByValue で値を受け取り、exceptionDetails があれば失敗として扱う
//! - 読み込み完了イベントは、遷移コマンドを送る「前」に待ち始める（取りこぼし防止）
//! - beforeunload の確認ダイアログが出たら遷移を中止する（未保存の変更を勝手に捨てない）

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::cancel::CancelToken;
use crate::cdp::client::{CdpClient, DEFAULT_TIMEOUT};
use crate::error::{Error, Result};
use crate::util::base64_decode;

/// flatten モードで attach したタブ
pub struct CdpPage {
    client: Arc<CdpClient>,
    session_id: String,
    target_id: String,
}

impl CdpPage {
    /// 作る
    pub fn new(client: Arc<CdpClient>, session_id: String, target_id: String) -> CdpPage {
        CdpPage {
            client,
            session_id,
            target_id,
        }
    }

    /// flatten セッション ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// このセッション宛てに CDP コマンドを送信する
    pub fn send(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
        cancel: &CancelToken,
    ) -> Result<Value> {
        self.client.send_with_cancel(
            method,
            params,
            Some(&self.session_id),
            timeout,
            Some(cancel),
        )
    }

    /// ページのドメインを有効化する（読み込み・ダイアログのイベントを受けるため）。
    ///
    /// Runtime.enable は呼ばない。Runtime.evaluate は有効化なしで使え、Runtime.enable はページ側から
    /// 自動操作の検知に使われることがあるため（ログインを弾かれる要因を増やさない）。
    pub fn enable(&self, cancel: &CancelToken) -> Result<()> {
        self.send("Page.enable", None, DEFAULT_TIMEOUT, cancel)
            .map(|_| ())
    }

    /// ページがフォーカスを持っている扱いにする（操作の前に毎回呼ぶ）。
    ///
    /// 以前は Page.bringToFront でウィンドウを前面に出していたが、利用者の作業中に Chrome が最前面へ割り込むため廃止した。
    /// 前面に出さなくても、フォーカスのエミュレーションと起動引数（CalculateNativeWinOcclusion の無効化など）で
    /// クリック・言語切替が動くことを 2026-11-13 に実画面で確認している。
    pub fn prepare_for_input(&self, cancel: &CancelToken) -> Result<()> {
        self.send(
            "Emulation.setFocusEmulationEnabled",
            Some(json!({ "enabled": true })),
            DEFAULT_TIMEOUT,
            cancel,
        )
        .map(|_| ())
    }

    /// 今の URL（Target 情報から取る。遷移中でも失敗しにくい）
    pub fn get_url(&self, cancel: &CancelToken) -> Result<Option<String>> {
        let info = self.client.send_with_cancel(
            "Target.getTargetInfo",
            Some(json!({ "targetId": self.target_id })),
            None,
            DEFAULT_TIMEOUT,
            Some(cancel),
        )?;
        Ok(info
            .get("targetInfo")
            .and_then(|t| t.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    /// 式を評価して値を返す。Promise は待つ。ページ側で例外が起きたら `JavaScript` エラー
    pub fn evaluate(&self, expression: &str, cancel: &CancelToken) -> Result<Value> {
        let mut result = self.send(
            "Runtime.evaluate",
            Some(json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true,
                "userGesture": false,
            })),
            Duration::from_secs(20),
            cancel,
        )?;
        if let Some(ex) = result.get("exceptionDetails") {
            let text = ex
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(Value::as_str)
                .or_else(|| ex.get("text").and_then(Value::as_str))
                .unwrap_or(crate::trs!("cdp.unknown_error"));
            return Err(Error::JavaScript(crate::tr!(
                "cdp.the_in_page_script_failed",
                text = text
            )));
        }
        // 例外が無いときだけ、評価結果の値を所有権ごと取り出す（大きな戻り値を深いコピーしない）
        Ok(result
            .get_mut("result")
            .and_then(|r| r.get_mut("value"))
            .map(Value::take)
            .unwrap_or(Value::Null))
    }

    /// ビューポート座標 (x, y) へマウスを動かす（ホバーで現れるボタンを出すため。クリックはしない）
    pub fn hover_at(&self, x: f64, y: f64, cancel: &CancelToken) -> Result<()> {
        self.mouse("mouseMoved", x, y, 0, cancel)
    }

    /// `expression` が返す `input[type=file]` に、ファイルを選んだ扱いにする（`DOM.setFileInputFiles`）。
    ///
    /// 式は要素そのものを返す（`returnByValue: false` で objectId を得る）。パスは CDP の引数で渡し、
    /// ページ内スクリプトには埋め込まない。objectId は使い終えたら `Runtime.releaseObjectGroup` で解放する。
    /// 非表示の入力でも選べる。ファイルの存在確認は呼び出し側（Chrome は存在しないパスをエラーにする）
    pub fn set_file_input_files(
        &self,
        expression: &str,
        files: &[std::path::PathBuf],
        cancel: &CancelToken,
    ) -> Result<()> {
        const GROUP: &str = "edictora-file-input";
        let result = self.send(
            "Runtime.evaluate",
            Some(json!({
                "expression": expression,
                "returnByValue": false,
                "awaitPromise": false,
                "userGesture": false,
                "objectGroup": GROUP,
            })),
            Duration::from_secs(20),
            cancel,
        )?;
        if let Some(ex) = result.get("exceptionDetails") {
            let text = ex
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(Value::as_str)
                .or_else(|| ex.get("text").and_then(Value::as_str))
                .unwrap_or(crate::trs!("cdp.unknown_error"));
            let _ = self.release_object_group(GROUP, cancel);
            return Err(Error::JavaScript(crate::tr!(
                "cdp.the_in_page_script_failed",
                text = text
            )));
        }
        let object_id = result
            .get("result")
            .and_then(|r| r.get("objectId"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(object_id) = object_id else {
            let _ = self.release_object_group(GROUP, cancel);
            return Err(Error::Cdp(
                crate::trs!("cdp.the_file_input_element_was_not_returned").to_string(),
            ));
        };
        let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        let set = self.send(
            "DOM.setFileInputFiles",
            Some(json!({ "files": paths, "objectId": object_id })),
            Duration::from_secs(30),
            cancel,
        );
        let _ = self.release_object_group(GROUP, cancel);
        set.map(|_| ())
    }

    fn release_object_group(&self, group: &str, cancel: &CancelToken) -> Result<()> {
        self.send(
            "Runtime.releaseObjectGroup",
            Some(json!({ "objectGroup": group })),
            DEFAULT_TIMEOUT,
            cancel,
        )
        .map(|_| ())
    }

    /// ビューポート座標 (x, y) を左クリックする（実際の入力イベントとして届く）
    pub fn click_at(&self, x: f64, y: f64, cancel: &CancelToken) -> Result<()> {
        self.mouse("mouseMoved", x, y, 0, cancel)?;
        self.mouse("mousePressed", x, y, 1, cancel)?;
        self.mouse("mouseReleased", x, y, 1, cancel)
    }

    fn mouse(
        &self,
        kind: &str,
        x: f64,
        y: f64,
        click_count: i32,
        cancel: &CancelToken,
    ) -> Result<()> {
        let mut p = json!({
            "type": kind,
            "x": x,
            "y": y,
            "button": if kind == "mouseMoved" { "none" } else { "left" },
            "buttons": if kind == "mousePressed" { 1 } else { 0 },
        });
        if click_count > 0 {
            p["clickCount"] = json!(click_count);
        }
        self.send("Input.dispatchMouseEvent", Some(p), DEFAULT_TIMEOUT, cancel)
            .map(|_| ())
    }

    /// フォーカス中の欄へ文字列を入れる（選択範囲は置き換わる）。ページには通常の入力として届く
    pub fn insert_text(&self, text: &str, cancel: &CancelToken) -> Result<()> {
        self.send(
            "Input.insertText",
            Some(json!({ "text": text })),
            Duration::from_secs(30),
            cancel,
        )
        .map(|_| ())
    }

    /// 特殊キーを 1 回押す（Delete / Escape など）
    pub fn press_key(
        &self,
        key: &str,
        windows_virtual_key_code: i32,
        cancel: &CancelToken,
    ) -> Result<()> {
        for kind in ["rawKeyDown", "keyUp"] {
            self.send(
                "Input.dispatchKeyEvent",
                Some(json!({
                    "type": kind,
                    "key": key,
                    "code": key,
                    "windowsVirtualKeyCode": windows_virtual_key_code,
                    "nativeVirtualKeyCode": windows_virtual_key_code,
                })),
                DEFAULT_TIMEOUT,
                cancel,
            )?;
        }
        Ok(())
    }

    /// URL へ移動し、load イベントを待つ
    pub fn navigate(&self, url: &str, cancel: &CancelToken) -> Result<()> {
        self.run_navigation(
            || {
                let res = self.send(
                    "Page.navigate",
                    Some(json!({ "url": url })),
                    DEFAULT_TIMEOUT,
                    cancel,
                )?;
                match res.get("errorText").and_then(Value::as_str) {
                    Some(err) if !err.is_empty() => Err(Error::Cdp(crate::tr!(
                        "cdp.could_not_open_the_page",
                        error = err
                    ))),
                    _ => Ok(()),
                }
            },
            cancel,
        )
    }

    /// 読み込み直し、load イベントを待つ
    pub fn reload(&self, cancel: &CancelToken) -> Result<()> {
        self.run_navigation(
            || {
                self.send(
                    "Page.reload",
                    Some(json!({ "ignoreCache": false })),
                    DEFAULT_TIMEOUT,
                    cancel,
                )
                .map(|_| ())
            },
            cancel,
        )
    }

    /// 遷移を実行し、load イベントか beforeunload ダイアログのどちらかを待つ。
    /// beforeunload が出たら「留まる」を選んで中止する（未保存の変更を捨てない）
    fn run_navigation(
        &self,
        start: impl FnOnce() -> Result<()>,
        cancel: &CancelToken,
    ) -> Result<()> {
        // 先に待ち始めてから遷移を送る（イベントの取りこぼし防止）
        let session = self.session_id.clone();
        let sub = self.client.subscribe(move |e| {
            e.session_id.as_deref() == Some(session.as_str())
                && (e.method == "Page.loadEventFired"
                    || (e.method == "Page.javascriptDialogOpening"
                        && e.params.get("type").and_then(Value::as_str) == Some("beforeunload")))
        });

        start()?;

        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::Timeout(
                    crate::trs!("cdp.waiting_for_the_cdp_event_page_loadeventfired").to_string(),
                ));
            }
            match sub.recv(left, Some(cancel))? {
                Some(e) if e.method == "Page.javascriptDialogOpening" => {
                    self.send(
                        "Page.handleJavaScriptDialog",
                        Some(json!({ "accept": false })),
                        DEFAULT_TIMEOUT,
                        cancel,
                    )?;
                    return Err(Error::Cdp(
                        crate::trs!("cdp.navigation_was_cancelled_because_there_are_unsaved")
                            .to_string(),
                    ));
                }
                Some(_) => return Ok(()),
                None => continue,
            }
        }
    }

    /// 画面を PNG で取得する
    pub fn capture_screenshot(&self, cancel: &CancelToken) -> Result<Vec<u8>> {
        let res = self.send(
            "Page.captureScreenshot",
            Some(json!({ "format": "png" })),
            Duration::from_secs(30),
            cancel,
        )?;
        let data = res.get("data").and_then(Value::as_str).unwrap_or("");
        base64_decode(data).ok_or_else(|| {
            Error::Cdp(crate::trs!("cdp.could_not_decode_the_screenshot_data").to_string())
        })
    }
}
