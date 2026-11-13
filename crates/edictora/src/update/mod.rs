//! 自動更新（Velopack でインストールされたときだけ動く）。
//!
//! インストール・アンインストール・ファイルの差し替えは Velopack の Setup.exe / Update.exe に任せ、
//! Edictora は「更新の確認とダウンロード」だけを自前で行う（velopack crate は ureq / rustls を引き込むため
//! 使わない。通信は `--jev` と共用の [`crate::net::winhttp`]）。約束は vendored の Velopack 1.2.161 の
//! Rust SDK に合わせる（各モジュールの説明に対応箇所を書いた）。
//!
//! 流れ（[`run_update`]）:
//! 1. `<packages>\.edictora-update.lock` を取る（取れなければ別の更新処理が動いているので何もしない）
//! 2. 配信元の `releases.{channel}.json` を取得して解釈し、現在の版より新しい完全パッケージを選ぶ
//! 3. `<packages>` へダウンロードし、サイズとハッシュを確かめてから改名する（[`download`]）
//! 4. 実行中の edictora（Chrome を開くコマンド）が無いことをロックで確かめ（あれば適用を後に回す）、
//!    Update.exe を `apply ... --waitPid <自分> --silent --norestart` で起動し、呼び出し側はすぐに終了する
//!    （Update.exe が自分の終了を待って `current` を差し替える）
//!
//! 入口は 2 つ: ログオン時に動く `edictora-updater.exe`（コンソールなし、結果はログへ）と、
//! 手動の `edictora update`（結果は標準エラーへ）。pull / diff / push / submit の中では更新を確かめない。
//!
//! 構成:
//! - [`version`] — SemVer の解釈と比較
//! - [`manifest`] — `current\sq.version`（nuspec XML）の解釈
//! - [`locate`] — インストールされているかの判定と各パス
//! - [`feed`] — フィードの解釈と更新先の選択
//! - [`source`] — 配信元の URL（本番は固定。検証用に環境変数で差し替え）
//! - [`hash`] — SHA-1 / SHA-256（Windows の CNG）
//! - [`lock`] — 排他用のロックファイル
//! - [`download`] — ダウンロードと検証
//! - [`apply`] — Update.exe の起動

pub mod apply;
pub mod download;
pub mod feed;
pub mod hash;
pub mod locate;
pub mod lock;
pub mod manifest;
pub mod source;
pub mod version;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::net::winhttp::{self as net, HttpError, HttpRequest, Method, ResponseSink};

use apply::{ApplyLauncher, ProcessApplyLauncher, apply_args};
use feed::{FeedAsset, UpdateCheck, check_for_updates, parse_feed};
use locate::{Installation, LocateError, locate_current};
use lock::{LockFile, UPDATE_LOCK_FILE};
use source::{UpdateSource, is_safe_channel};
use version::Version;

/// エラー文に出す相手の名前
pub(crate) const SERVICE: &str = "更新の配信元";

/// フィードの操作ごとのタイムアウト
const FEED_TIMEOUT: Duration = Duration::from_secs(30);
/// フィードの本文の上限（リリースノートを含めても十分な大きさ）
const MAX_FEED_BYTES: usize = 8 * 1024 * 1024;

/// 更新の失敗を作る
pub(crate) fn update_err(message: impl Into<String>) -> Error {
    Error::Update(message.into())
}

/// 通信の失敗を更新の失敗にする（`total` は全体の期限。タイムアウトの文に使う）
pub(crate) fn http_error(e: HttpError, total: Duration) -> Error {
    match e {
        HttpError::Cancelled => Error::Cancelled,
        HttpError::TimedOut => update_err(format!(
            "{SERVICE} の応答を {} 秒待ちましたが返りませんでした（タイムアウト）",
            total.as_secs()
        )),
        HttpError::Failed(m) => update_err(m),
    }
}

/// 更新処理の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// フィードに完全パッケージが無い
    RemoteIsEmpty { current: Version },
    /// 最新（配信元の最新が現在の版以下）
    UpToDate { current: Version, latest: Version },
    /// 新しい版をダウンロードし、Update.exe に適用を任せた（呼び出し側はすぐに終了すること）
    ApplyStarted {
        from: Version,
        to: Version,
        package: PathBuf,
    },
    /// 別の更新処理が動いている（何もしなかった）
    Busy,
    /// 新しい版はダウンロード・検証したが、Chrome を開くコマンド（login / pull / diff / push / submit）が
    /// 実行中なので適用（Update.exe の起動）を後に回した。次回はダウンロード済みのものを検証して使う
    AppInUse { from: Version, to: Version },
}

impl UpdateOutcome {
    /// 利用者向けの 1 行
    pub fn message(&self) -> String {
        match self {
            UpdateOutcome::RemoteIsEmpty { current } => {
                format!("配信元に更新がありません（現在 {current}）。")
            }
            UpdateOutcome::UpToDate { current, latest } => {
                format!("最新です（現在 {current}、配信元の最新 {latest}）。")
            }
            UpdateOutcome::ApplyStarted { from, to, .. } => format!(
                "{from} → {to} の更新をダウンロードしました。このプロセスの終了後に Update.exe が差し替えます。"
            ),
            UpdateOutcome::Busy => "別の更新処理が動いているため、何もしませんでした。".to_string(),
            UpdateOutcome::AppInUse { from, to } => format!(
                "{from} → {to} の更新はダウンロード済みですが、実行中の edictora（login / pull / diff / push / submit）があるため適用を後に回しました。終わってから edictora update を実行してください。"
            ),
        }
    }
}

/// 配信元からフィードを取得して解釈する
pub fn fetch_feed(
    source: &UpdateSource,
    installation: &Installation,
    cancel: &CancelToken,
) -> Result<Vec<FeedAsset>> {
    let manifest = &installation.manifest;
    if !is_safe_channel(&manifest.channel) {
        return Err(update_err(format!(
            "マニフェストのチャンネル名が正しくありません: {}",
            manifest.channel
        )));
    }
    let response = net::send(
        HttpRequest {
            method: Method::Get,
            target: source.feed_target(
                &manifest.channel,
                &manifest.version.to_string(),
                &manifest.id,
            ),
            headers: vec!["Accept: application/json".to_string()],
            body: Vec::new(),
            timeout: FEED_TIMEOUT,
            total_timeout: FEED_TIMEOUT + Duration::from_secs(2),
            service: SERVICE,
        },
        ResponseSink::Memory {
            max_bytes: MAX_FEED_BYTES,
        },
        cancel,
    )
    .map_err(|e| http_error(e, FEED_TIMEOUT + Duration::from_secs(2)))?;
    if response.status != 200 {
        return Err(update_err(format!(
            "フィード（{}）を取得できませんでした（HTTP {}）",
            source::feed_file_name(&manifest.channel),
            response.status
        )));
    }
    let text = String::from_utf8(response.body)
        .map_err(|_| update_err("フィードが UTF-8 ではありません"))?;
    parse_feed(&text).map_err(update_err)
}

/// 更新を確認し、新しい版があればダウンロードして Update.exe に適用を任せる。
///
/// `running_lock` は Chrome を開くコマンドが実行中に共有で持つロック（[`crate::app_paths::AppPaths::running_lock_path`]）。
/// Update.exe を起動する直前にこれを排他で取り、取れなければ適用せず [`UpdateOutcome::AppInUse`] を返す
/// （Update.exe がインストール先配下のプロセスを止めて、push の途中で落とすのを防ぐ）。
/// 取れたら、このプロセスが終わるまで持ち続ける（その間に新しい CLI が Chrome を開き始めないように）
pub fn run_update(
    installation: &Installation,
    source: &UpdateSource,
    launcher: &dyn ApplyLauncher,
    wait_pid: u32,
    running_lock: &Path,
    progress: &dyn Fn(&str),
    cancel: &CancelToken,
) -> Result<UpdateOutcome> {
    let lock_path = installation.packages_dir.join(UPDATE_LOCK_FILE);
    let _lock = match LockFile::try_acquire(&lock_path) {
        Ok(lock) => lock,
        // 共有違反・ロック違反は「別の更新処理が開いたまま」。それ以外（フォルダーを作れない・
        // 書き込めないなど）は恒常的な失敗なので、Busy に紛らわせず理由つきで失敗にする
        Err(e) if lock::is_held_by_another(&e) => return Ok(UpdateOutcome::Busy),
        Err(e) => {
            return Err(update_err(format!(
                "更新用のロックファイルを開けません（{}）: {e}",
                lock_path.display()
            )));
        }
    };

    let current = installation.manifest.version.clone();
    progress(&format!(
        "更新を確認しています（現在 {current}、配信元 {}、チャンネル {}）",
        source.display_url(),
        installation.manifest.channel
    ));
    let assets = fetch_feed(source, installation, cancel)?;
    let (asset, target) = match check_for_updates(&assets, &current) {
        UpdateCheck::RemoteIsEmpty => return Ok(UpdateOutcome::RemoteIsEmpty { current }),
        UpdateCheck::NoUpdateAvailable { latest } => {
            return Ok(UpdateOutcome::UpToDate { current, latest });
        }
        UpdateCheck::UpdateAvailable { asset, version } => (asset, version),
    };
    progress(&format!("新しい版があります: {current} → {target}"));

    let package =
        download::ensure_package(&installation.packages_dir, source, &asset, progress, cancel)?;
    cancel.check()?;

    // 実行中の edictora が無いことを確かめてから適用を任せる（ダウンロードと検証は先に済ませてよい）
    let app_lock = match LockFile::try_acquire(running_lock) {
        Ok(lock) => lock,
        Err(e) if lock::is_held_by_another(&e) => {
            return Ok(UpdateOutcome::AppInUse {
                from: current,
                to: target,
            });
        }
        Err(e) => {
            return Err(update_err(format!(
                "実行中の edictora を確かめるロックファイルを開けません（{}）: {e}",
                running_lock.display()
            )));
        }
    };

    let args = apply_args(installation, &package, wait_pid);
    launcher.launch(&installation.update_exe, &args)?;
    // プロセスの終了まで排他で持ち続ける（ハンドルは終了時に OS が閉じる）。
    // ここで閉じると、Update.exe が差し替えを始める前に新しい CLI が Chrome を開き始めうる
    std::mem::forget(app_lock);
    Ok(UpdateOutcome::ApplyStarted {
        from: current,
        to: target,
        package,
    })
}

/// 実行中の exe のインストール先で、本物の Update.exe を使って更新する（`edictora update` と更新用 exe の本体）。
/// インストールされていなければ [`Error::Update`]（理由つき）
pub fn run_installed_update(
    source: &UpdateSource,
    running_lock: &Path,
    progress: &dyn Fn(&str),
    cancel: &CancelToken,
) -> Result<UpdateOutcome> {
    let installation = locate_current().map_err(not_installed_error)?;
    run_update(
        &installation,
        source,
        &ProcessApplyLauncher,
        std::process::id(),
        running_lock,
        progress,
        cancel,
    )
}

/// インストール先を決められなかったときの利用者向けのエラー
pub fn not_installed_error(e: LocateError) -> Error {
    match e {
        LocateError::NotInstalled(m) => update_err(format!(
            "Velopack でインストールされた Edictora ではないため更新できません（Setup で入れたものだけが自動更新の対象です）: {m}"
        )),
        LocateError::InvalidManifest(m) => {
            update_err(format!("インストール先のマニフェストを読めません: {m}"))
        }
    }
}
