//! Edictora CLI（edictora.exe）の入口。
//!
//! - 最初（引数の解釈より前）に Velopack のフック（`--veloapp-install` など）を処理し、すぐに終了コード 0 で終える
//!   （Run への更新用 exe の登録・PATH の追加と解除。`install::hooks`）
//! - Velopack に引数なしで起動された（初回起動・再起動の環境変数がある）ときは、既定の `diff` を走らせずに終える
//! - 保存先は環境（`%LOCALAPPDATA%` / `%APPDATA%`、検証用の `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT`）から決める
//! - `edictora mcp` は実行中ロックをプロセスの終了まで共有で持つ（待機中の MCP も更新の適用で強制終了させない）
//! - 1 回目の Ctrl+C は中断要求として扱い、Chrome を正常終了させてから終了コード 130 で抜ける
//! - `update` の配信元の差し替え（検証用）は環境変数 `EDICTORA_UPDATE_URL` から読み、CliApp へ値で渡す
//! - Rust の標準出力はコンソールへ UTF-16 で書くので、日本語は文字化けしない

use edictora::app_paths::AppPaths;
use edictora::cancel::{CancelToken, install_ctrl_c_handler};
use edictora::cli::{CliApp, open_real_browser, run_real_update};
use edictora::install::{self, hooks};
use edictora::logging::{self, LogLevel, log};
use edictora::mcp;
use edictora::update::lock::{LockFile, is_held_by_another};
use edictora::update::source::SOURCE_URL_ENV;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // 表示言語は初回の使用時に決まる（フック・引数の誤りより前でも失敗しない。判定順は i18n.rs）。
    // `edictora mcp` は AI が読む出力なので、利用者の言語に関わらず英語へ固定する
    if mcp::is_mcp_command(&args) {
        edictora::i18n::init(edictora::i18n::Language::English);
    }
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
            "{}",
            edictora::trs!("main.edictora_is_installed_run_edictora_help_in")
        );
        std::process::exit(0);
    }

    // ── MCP サーバー（`edictora mcp`）。標準入出力は JSON-RPC 専用なので、ここから先の CLI の出力は使わない ──
    // Ctrl+C の扱い（中断の合図）は入れない。クライアントは標準入力を閉じて止めるのが普通で、
    // そのときは serve が受け付け済みの呼び出しを終えてから戻る。呼び出しの中断は notifications/cancelled で行う
    if mcp::is_mcp_command(&args) {
        logging::init(&paths.log_directory());
        // ── 実行中の印をプロセスの終了まで共有で持つ ──
        // Velopack の `Update.exe apply` は、インストール先から動いている他のプロセスを強制終了してから
        // 差し替える。MCP はクライアント（Claude Code・Codex）のセッションの間ずっと常駐するので、
        // 待機中でも止められるとセッションから Edictora が消える。そこで CLI の login / pull などと同じ
        // `edictora-running.lock` を、呼び出しの間だけでなくサーバーの生存期間中ずっと共有で持ち、
        // 更新側（ログオン時の更新用 exe・`edictora update`）に適用を見送らせる（`AppInUse`。次のログオンへ回る）。
        // 共有同士は両立するので、複数のセッションの MCP や、ツール呼び出しの中の `CliApp::run` が
        // 同じファイルを共有で開き直しても衝突しない。
        // - 排他で持たれている（更新の適用が始まっている）: 起動しても Update.exe に止められるだけなので、
        //   理由を標準エラーとログに出して終了する（標準出力は JSON-RPC 専用なので使わない）
        // - ロックファイル自体を開けない（フォルダーを作れないなど）: CLI と同じく警告をログへ残して続行する
        let _running = match LockFile::try_acquire_shared(&paths.running_lock_path()) {
            Ok(lock) => Some(lock),
            Err(e) if is_held_by_another(&e) => {
                // ログは作者の調査用に日本語のまま、標準エラーは表示言語に合わせる
                log(
                    "edictora の更新を適用している最中のため、MCP サーバーを起動しません。少し待ってからクライアントでサーバーを起動し直してください。",
                    LogLevel::Warning,
                );
                eprintln!(
                    "{}",
                    edictora::trs!("main.error_an_edictora_update_is_being_applied")
                );
                logging::shutdown();
                std::process::exit(1);
            }
            Err(e) => {
                log(
                    &format!("実行中を知らせるロックファイルを開けませんでした（続行します）: {e}"),
                    LogLevel::Warning,
                );
                None
            }
        };
        log("MCP サーバーを開始します", LogLevel::Info);
        let stdin = std::io::stdin().lock();
        let stdout = std::io::stdout();
        mcp::serve(stdin, stdout, paths, &open_real_browser);
        log("MCP サーバーを終了します", LogLevel::Info);
        logging::shutdown();
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
