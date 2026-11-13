//! Edictora CLI（edictora.exe）の入口。
//!
//! - 最初（引数の解釈より前）に Velopack のフック（`--veloapp-install` など）を処理し、すぐに終了コード 0 で終える
//!   （Run への更新用 exe の登録・PATH の追加と解除。`install::hooks`）
//! - Velopack に引数なしで起動された（初回起動・再起動の環境変数がある）ときは、既定の `diff` を走らせずに終える
//! - 保存先は環境（`%LOCALAPPDATA%` / `%APPDATA%`、検証用の `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT`）から決める
//! - 1 回目の Ctrl+C は中断要求として扱い、Chrome を正常終了させてから終了コード 130 で抜ける
//! - `update` の配信元の差し替え（検証用）は環境変数 `EDICTORA_UPDATE_URL` から読み、CliApp へ値で渡す
//! - Rust の標準出力はコンソールへ UTF-16 で書くので、日本語は文字化けしない

use edictora::app_paths::AppPaths;
use edictora::cancel::{CancelToken, install_ctrl_c_handler};
use edictora::cli::{CliApp, open_real_browser, run_real_update};
use edictora::install::{self, hooks};
use edictora::logging::{self, LogLevel, log};
use edictora::update::source::SOURCE_URL_ENV;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = AppPaths::from_environment();

    // ── Velopack のフック（引数の解釈より前。処理後は必ず 0 で終える）──
    if let Some(hook) = hooks::parse_hook(&args) {
        logging::init(&paths.log_directory());
        let code = hooks::run_hook(&hook);
        logging::shutdown();
        std::process::exit(code);
    }

    // ── Velopack に引数なしで起動された（Setup 直後など）。Chrome を開く diff を走らせない ──
    let firstrun = std::env::var(install::FIRSTRUN_ENV).ok();
    let restart = std::env::var(install::RESTART_ENV).ok();
    if install::is_launched_by_velopack_without_command(
        &args,
        firstrun.as_deref(),
        restart.as_deref(),
    ) {
        logging::init(&paths.log_directory());
        log(
            "Velopack から引数なしで起動されたため、何もせずに終了します",
            LogLevel::Info,
        );
        logging::shutdown();
        eprintln!(
            "Edictora をインストールしました。コマンドプロンプトで edictora --help を実行すると使い方を表示します。"
        );
        std::process::exit(0);
    }

    let cancel = CancelToken::new();
    install_ctrl_c_handler(&cancel);

    logging::init(&paths.log_directory());
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let code = {
        let mut app = CliApp::new(&mut stdout, &mut stderr, paths, &open_real_browser)
            .with_update(&run_real_update, std::env::var(SOURCE_URL_ENV).ok());
        app.run(&args, &cancel)
    };
    logging::shutdown();
    std::process::exit(code);
}
