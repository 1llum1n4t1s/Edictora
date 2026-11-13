//! 専用プロファイルの Chrome と、その 1 タブへの CDP 接続をまとめたもの（C# の `DashboardBrowser`）。
//!
//! 使い方: [`DashboardBrowser::open`] → [`DashboardBrowser::ensure_logged_in`]（または [`DashboardBrowser::wait_for_login`]）
//! → [`DashboardBrowser::create_item_page`] で編集画面を操作 → [`DashboardBrowser::close`]（drop でも閉じる）。
//! publisherId はログイン確認時に URL から取り出してメモリにだけ持つ。外へは出さない。
//!
//! ログイン待ちの間はタブに接続しない（2026-11-13 の検証で手動ログインできたのは、タブに何も接続していない状態だったため）。
//! ブラウザ全体の接続から Target.getTargets で URL を見るだけにし、タブへの接続はログイン確認後の操作時に行う。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::cancel::CancelToken;
use crate::cdp::{CdpClient, CdpPage, DEFAULT_TIMEOUT};
use crate::chrome::{ChromeInstance, ChromeLaunchOptions, find_chrome, launch_or_attach};
use crate::dashboard::item_page::DashboardItemPage;
use crate::dashboard::urls;
use crate::error::{Error, Result};
use crate::logging::{LogLevel, log};
use crate::util::starts_with_ic;

/// ダッシュボード用ブラウザの開き方
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardBrowserOptions {
    /// 専用プロファイル（ログイン状態を保持する。削除しない）
    pub profile_directory: PathBuf,
    /// chrome.exe（None なら自動検出）
    pub chrome_path: Option<PathBuf>,
    /// ウィンドウを表示するか。false（既定）ならヘッドレスで起動し、作業中に前面へ出てこない。
    /// ログインは利用者の操作が要るので true で開くこと
    pub show_browser: bool,
    /// 失敗時の画面保存先
    pub diagnostics_directory: PathBuf,
}

impl DashboardBrowserOptions {
    /// 同じ指定でウィンドウを表示する版（ログイン用）
    pub fn visible(&self) -> DashboardBrowserOptions {
        DashboardBrowserOptions {
            show_browser: true,
            ..self.clone()
        }
    }
}

/// 専用プロファイルの Chrome と 1 タブへの接続
pub struct DashboardBrowser {
    chrome: ChromeInstance,
    client: Arc<CdpClient>,
    page: Option<CdpPage>,
    publisher_id: Option<String>,
    diagnostics_directory: PathBuf,
    disposed: bool,
}

impl DashboardBrowser {
    /// Chrome を起動（または既存に接続）し、ブラウザ全体の CDP に接続する。タブにはまだ接続しない。
    /// ウィンドウを表示して新しく起動する場合は最初からダッシュボードの入口を開く
    pub fn open(
        options: &DashboardBrowserOptions,
        cancel: &CancelToken,
    ) -> Result<DashboardBrowser> {
        let chrome_path = options.chrome_path.clone().or_else(find_chrome).ok_or_else(|| {
            Error::Dashboard(
                "Chrome が見つかりません。設定ファイルの chromePath か --chrome で chrome.exe を指定してください。".to_string(),
            )
        })?;
        if !chrome_path.is_file() {
            return Err(Error::Dashboard(format!(
                "chrome.exe がありません: {}",
                chrome_path.display()
            )));
        }

        let mut chrome = launch_or_attach(
            &ChromeLaunchOptions {
                chrome_path,
                user_data_dir: options.profile_directory.clone(),
                // ヘッドレスでは UA の差し替え（get_page）より前にダッシュボードへ通信しないよう、空のページで起動する。
                // 入口へは ensure_logged_in が移動する
                start_url: if options.show_browser {
                    urls::ROOT.to_string()
                } else {
                    "about:blank".to_string()
                },
                headless: !options.show_browser,
            },
            cancel,
        )?;

        match CdpClient::connect(chrome.browser_ws_url()) {
            Ok(client) => Ok(DashboardBrowser {
                chrome,
                client,
                page: None,
                publisher_id: None,
                diagnostics_directory: options.diagnostics_directory.clone(),
                disposed: false,
            }),
            Err(e) => {
                chrome.close(None);
                Err(e)
            }
        }
    }

    /// Chrome との接続が生きているか（利用者が Chrome を閉じると false）
    pub fn is_connected(&self) -> bool {
        !self.disposed && self.client.is_connected()
    }

    /// ダッシュボードの入口を開き、ログイン済みか確かめる。未ログインなら `LoginRequired`
    pub fn ensure_logged_in(&mut self, cancel: &CancelToken) -> Result<()> {
        if !self.is_connected() {
            return Err(Error::Dashboard(
                "Chrome との接続が切れています。開き直してください。".to_string(),
            ));
        }
        self.get_page(cancel)?;
        if self.publisher_id.is_some() {
            return Ok(());
        }
        let page = self.page.as_ref().expect("get_page 済み");
        page.prepare_for_input(cancel)?;
        page.navigate(urls::ROOT, cancel)?;

        // ダッシュボードは読み込み後に publisherId 付きの URL へ移る。ログイン画面へ移れば未ログイン
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let url = page.get_url(cancel)?;
            if let Some(pid) = urls::try_parse_publisher_id(url.as_deref()) {
                self.publisher_id = Some(pid);
                log("ダッシュボードへのログインを確認しました", LogLevel::Info);
                return Ok(());
            }
            if urls::is_login_page(url.as_deref()) {
                return Err(Error::LoginRequired);
            }
            cancel.sleep(Duration::from_millis(500))?;
        }
        Err(Error::Dashboard(
            "ダッシュボードに到達できませんでした（30 秒）。login を実行してログイン状態を確認してください。".to_string(),
        ))
    }

    /// 利用者が手動でログインしてダッシュボードに着くまで待つ。
    /// パスワードや確認コードには一切触れない（入力は利用者が Chrome の画面で行う）。タブにも接続しない
    pub fn wait_for_login(
        &mut self,
        timeout: Duration,
        progress: &dyn Fn(&str),
        cancel: &CancelToken,
    ) -> Result<()> {
        // 既に開いていた Chrome に接続した場合は、入口のタブを新しく開く（起動した場合は起動時に開いている）
        if !self.chrome.owned_by_tool() && self.find_dashboard_url(cancel)?.is_none() {
            self.browser_send(
                "Target.createTarget",
                Some(json!({ "url": urls::ROOT })),
                cancel,
            )?;
        }
        progress(
            "開いた Chrome で Google にログインし、デベロッパー ダッシュボードを表示してください。",
        );

        let deadline = Instant::now() + timeout;
        let mut announced = false;
        while Instant::now() < deadline {
            if !self.is_connected() {
                return Err(Error::Dashboard(
                    "Chrome が閉じられたため、ログインの確認を中止しました。".to_string(),
                ));
            }
            let (dashboard_url, on_login_page) = self.scan_targets(cancel)?;
            if let Some(pid) = urls::try_parse_publisher_id(dashboard_url.as_deref()) {
                self.publisher_id = Some(pid);
                progress("ダッシュボードへのログインを確認しました。");
                log("login: ダッシュボード到達を確認", LogLevel::Info);
                return Ok(());
            }
            if !announced && on_login_page {
                progress(
                    "Google のログイン画面を表示しています。ログインが終わるまで待っています…",
                );
                announced = true;
            }
            cancel.sleep(Duration::from_secs(1))?;
        }
        Err(Error::Dashboard(format!(
            "ログインの完了を {} 分待ちましたが、ダッシュボードに到達しませんでした。",
            timeout.as_secs() / 60
        )))
    }

    /// アイテムの編集画面を操作するオブジェクトを作る（先にログイン確認が必要）
    pub fn create_item_page(&self) -> Result<DashboardItemPage<'_>> {
        match (&self.publisher_id, &self.page) {
            (Some(pid), Some(page)) => Ok(DashboardItemPage::new(
                page,
                pid.clone(),
                self.diagnostics_directory.clone(),
            )),
            _ => Err(Error::Dashboard(
                "先に ensure_logged_in でログインを確認してください".to_string(),
            )),
        }
    }

    fn browser_send(
        &self,
        method: &str,
        params: Option<Value>,
        cancel: &CancelToken,
    ) -> Result<Value> {
        self.client
            .send_with_cancel(method, params, None, DEFAULT_TIMEOUT, Some(cancel))
    }

    fn target_infos(&self, cancel: &CancelToken) -> Result<Vec<Value>> {
        let mut targets = self.browser_send("Target.getTargets", None, cancel)?;
        // 配列を所有権ごと取り出す（clone しない）。配列でなければ（欠落・別の型）空として扱う
        Ok(match targets.get_mut("targetInfos").map(Value::take) {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        })
    }

    /// ブラウザ全体の接続から、全タブの URL を見る（タブには接続しない）
    fn scan_targets(&self, cancel: &CancelToken) -> Result<(Option<String>, bool)> {
        let mut dashboard = None;
        let mut login = false;
        for t in self.target_infos(cancel)? {
            if t.get("type").and_then(Value::as_str) != Some("page") {
                continue;
            }
            let url = t.get("url").and_then(Value::as_str);
            if dashboard.is_none() && urls::try_parse_publisher_id(url).is_some() {
                dashboard = url.map(str::to_string);
            }
            if urls::is_login_page(url) {
                login = true;
            }
        }
        Ok((dashboard, login))
    }

    fn find_dashboard_url(&self, cancel: &CancelToken) -> Result<Option<String>> {
        for t in self.target_infos(cancel)? {
            let url = t.get("url").and_then(Value::as_str).unwrap_or("");
            if t.get("type").and_then(Value::as_str) == Some("page")
                && starts_with_ic(url, urls::ROOT)
            {
                return Ok(Some(url.to_string()));
            }
        }
        Ok(None)
    }

    /// 操作に使うタブへ接続する（初回だけ）
    fn get_page(&mut self, cancel: &CancelToken) -> Result<()> {
        if self.page.is_some() {
            return Ok(());
        }
        let target_id = self.pick_page_target(cancel)?;
        let attach = self.browser_send(
            "Target.attachToTarget",
            Some(json!({ "targetId": target_id, "flatten": true })),
            cancel,
        )?;
        let session_id = attach
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Dashboard("タブへの接続に失敗しました".to_string()))?
            .to_string();

        let page = CdpPage::new(self.client.clone(), session_id, target_id);
        page.enable(cancel)?;
        self.hide_headless_user_agent(&page, cancel)?;
        page.prepare_for_input(cancel)?;
        self.page = Some(page);
        Ok(())
    }

    /// ヘッドレスの UA に含まれる "HeadlessChrome" を "Chrome" に置き換え、通常の Chrome と同じ見え方にする。
    ///
    /// 2026-11-13 の検証ではこの置き換えをした状態でダッシュボードを操作できた。置き換えない状態は試していないため、
    /// サイト側の判定を変えないよう常に置き換える。ウィンドウ表示時の UA には含まれないので何もしない。
    fn hide_headless_user_agent(&self, page: &CdpPage, cancel: &CancelToken) -> Result<()> {
        let version = self.browser_send("Browser.getVersion", None, cancel)?;
        let Some(ua) = version.get("userAgent").and_then(Value::as_str) else {
            return Ok(());
        };
        if !ua.contains("HeadlessChrome") {
            return Ok(());
        }
        page.send(
            "Emulation.setUserAgentOverride",
            Some(json!({ "userAgent": ua.replace("HeadlessChrome", "Chrome") })),
            DEFAULT_TIMEOUT,
            cancel,
        )
        .map(|_| ())
    }

    /// 操作に使うタブを選ぶ。ダッシュボードを開いているタブがあればそれを、なければ通常のタブを使う。
    /// タブが 1 つも無ければ新しく作る
    fn pick_page_target(&self, cancel: &CancelToken) -> Result<String> {
        let mut fallback: Option<String> = None;
        for t in self.target_infos(cancel)? {
            if t.get("type").and_then(Value::as_str) != Some("page") {
                continue;
            }
            let Some(id) = t.get("targetId").and_then(Value::as_str) else {
                continue;
            };
            let url = t.get("url").and_then(Value::as_str).unwrap_or("");
            if starts_with_ic(url, urls::ROOT) {
                return Ok(id.to_string());
            }
            if !starts_with_ic(url, "devtools://") && fallback.is_none() {
                fallback = Some(id.to_string());
            }
        }
        if let Some(f) = fallback {
            return Ok(f);
        }
        let created = self.browser_send(
            "Target.createTarget",
            Some(json!({ "url": "about:blank" })),
            cancel,
        )?;
        created
            .get("targetId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Error::Dashboard("タブを作れませんでした".to_string()))
    }

    /// 終了する。自分で起動した Chrome は Browser.close で正常終了させる（プロファイルは残す）。
    /// 既に開いていた Chrome には接続を切るだけで、ウィンドウは閉じない。中断の合図は見ない
    pub fn close(&mut self) {
        if std::mem::replace(&mut self.disposed, true) {
            return;
        }
        self.publisher_id = None;
        if self.chrome.owned_by_tool() {
            self.chrome.close(Some(&self.client));
        } else if let Some(page) = &self.page {
            // 切断済みなら何もしない
            let _ = self.client.send(
                "Target.detachFromTarget",
                Some(json!({ "sessionId": page.session_id() })),
                None,
                Duration::from_secs(5),
            );
        }
        self.client.close();
    }
}

impl Drop for DashboardBrowser {
    fn drop(&mut self) {
        self.close();
    }
}
