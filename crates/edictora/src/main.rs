//! Edictora CLI（edictora.exe）の入口。
//!
//! - 保存先は環境（`%LOCALAPPDATA%` / `%APPDATA%`、検証用の `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT`）から決める
//! - 1 回目の Ctrl+C は中断要求として扱い、Chrome を正常終了させてから終了コード 130 で抜ける
//! - Rust の標準出力はコンソールへ UTF-16 で書くので、日本語は文字化けしない

use edictora::app_paths::AppPaths;
use edictora::cancel::{CancelToken, install_ctrl_c_handler};
use edictora::cli::{CliApp, open_real_browser};
use edictora::logging;

fn main() {
    let paths = AppPaths::from_environment();
    let cancel = CancelToken::new();
    install_ctrl_c_handler(&cancel);

    logging::init(&paths.log_directory());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let code = {
        let mut app = CliApp::new(&mut stdout, &mut stderr, paths, &open_real_browser);
        app.run(&args, &cancel)
    };
    logging::shutdown();
    std::process::exit(code);
}
