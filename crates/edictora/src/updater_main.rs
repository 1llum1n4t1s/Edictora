//! 更新用 exe（edictora-updater.exe）の入口。ログオン時に HKCU の Run から起動される。
//!
//! - コンソールを出さない（`windows_subsystem = "windows"`）。結果は標準エラーでなくログ（`Edictora.Data\logs`）へ残す。
//! - 最初に Velopack のフック（`--veloapp-*`）を処理する（主 exe をどちらにしても同じ登録が行われるように）。
//! - Velopack でインストールされた場所から起動されていなければ（cargo run など）何もしない。
//! - 二重起動は `<packages>\.edictora-update.lock` で防ぐ（`edictora update` とも排他）。
//! - ログオン直後はネットワークがまだ使えないことがあるので、失敗したら間を空けて 2 回まで試し直す。
//!   Chrome を開くコマンド（login / pull / diff / push / submit）が実行中で適用を後に回したときも同じ。
//! - 新しい版があればダウンロード・検証して Update.exe を `--waitPid <自分>` で起動し、すぐに終了する。

#![windows_subsystem = "windows"]

use std::time::Duration;

use edictora::Error;
use edictora::app_paths::AppPaths;
use edictora::cancel::CancelToken;
use edictora::install::hooks;
use edictora::logging::{self, LogLevel, log};
use edictora::update::apply::ProcessApplyLauncher;
use edictora::update::locate::{LocateError, locate_current};
use edictora::update::source::{SOURCE_URL_ENV, UpdateSource};
use edictora::update::{UpdateOutcome, run_update};

/// 失敗したときの待ち時間（ログオン直後のネットワーク待ち）。この回数だけ試し直す
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(30), Duration::from_secs(90)];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = AppPaths::from_environment();
    logging::init(&paths.log_directory());

    let code = match hooks::parse_hook(&args) {
        Some(hook) => hooks::run_hook(&hook),
        None => run_logon_update(&paths),
    };
    logging::shutdown();
    std::process::exit(code);
}

fn run_logon_update(paths: &AppPaths) -> i32 {
    let source = match UpdateSource::from_override(std::env::var(SOURCE_URL_ENV).ok().as_deref()) {
        Ok(s) => s,
        Err(m) => {
            log(&format!("updater: {m}"), LogLevel::Error);
            return 2;
        }
    };
    let installation = match locate_current() {
        Ok(i) => i,
        Err(LocateError::NotInstalled(m)) => {
            log(
                &format!("updater: インストールされていないため何もしません（{m}）"),
                LogLevel::Info,
            );
            return 0;
        }
        Err(e) => {
            log(&format!("updater: {}", e.message()), LogLevel::Error);
            return 1;
        }
    };

    // コンソールが無いので Ctrl+C は来ない（中断の合図は立たない）
    let cancel = CancelToken::new();
    let progress = |m: &str| log(&format!("updater: {m}"), LogLevel::Info);
    let mut attempt = 0;
    loop {
        match run_update(
            &installation,
            &source,
            &ProcessApplyLauncher,
            std::process::id(),
            &paths.running_lock_path(),
            &progress,
            &cancel,
        ) {
            // 実行中の edictora があって適用を後に回したときも、間を空けて試し直す
            // （ダウンロード済みのものを検証して使うので、取り直しはしない）
            Ok(outcome @ UpdateOutcome::AppInUse { .. }) if attempt < RETRY_DELAYS.len() => {
                log(
                    &format!(
                        "updater: {} {} 秒後に試し直します",
                        outcome.message(),
                        RETRY_DELAYS[attempt].as_secs()
                    ),
                    LogLevel::Info,
                );
                std::thread::sleep(RETRY_DELAYS[attempt]);
                attempt += 1;
            }
            Ok(outcome) => {
                log(&format!("updater: {}", outcome.message()), LogLevel::Info);
                return 0;
            }
            Err(Error::Update(m)) if attempt < RETRY_DELAYS.len() => {
                log(
                    &format!(
                        "updater: 更新に失敗しました。{} 秒後に試し直します: {m}",
                        RETRY_DELAYS[attempt].as_secs()
                    ),
                    LogLevel::Warning,
                );
                std::thread::sleep(RETRY_DELAYS[attempt]);
                attempt += 1;
            }
            Err(e) => {
                log(
                    &format!("updater: 更新に失敗しました: {}", e.message()),
                    LogLevel::Error,
                );
                return 1;
            }
        }
    }
}
