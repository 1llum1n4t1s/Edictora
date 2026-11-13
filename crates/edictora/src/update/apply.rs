//! Update.exe での適用の起動。
//!
//! vendored の SDK（manager.rs の `unsafe_apply_updates`）と同じ引数で Update.exe を起動する:
//! `apply --package <パッケージ> --waitPid <自分の PID> --silent --norestart --root <根> --packageDir <packages>`
//! - `--waitPid`: Update.exe は指定のプロセスが終わるのを待ってから `current` を差し替える
//!   （SDK の `ApplyWaitMode::WaitCurrentProcess`）。起動した側はすぐに終了すること。
//! - `--silent`: Update.exe に画面（進捗・エラーのダイアログ）を出させない。ログオン時の更新も
//!   `edictora update` も利用者の操作を待たずに終わらせるため。
//! - `--norestart`: 適用後にアプリを起動し直さない（CLI なので起動し直す意味がない）。
//! - 起動は SDK の `run_process`（show_window = false）と同じく `CREATE_NO_WINDOW`、作業フォルダーは
//!   Update.exe のあるフォルダー。完了は待たない。

use std::ffi::OsString;
use std::path::Path;

use crate::error::Result;

use super::locate::Installation;
use super::update_err;

/// Update.exe を起動する相手（本物は [`ProcessApplyLauncher`]。テストでは記録するだけの偽物を使う）
pub trait ApplyLauncher {
    /// `update_exe` を `args` で起動する（完了は待たない）
    fn launch(&self, update_exe: &Path, args: &[OsString]) -> Result<()>;
}

/// `apply` の引数（SDK と同じ順）
pub fn apply_args(installation: &Installation, package: &Path, wait_pid: u32) -> Vec<OsString> {
    vec![
        "apply".into(),
        "--package".into(),
        package.as_os_str().to_os_string(),
        "--waitPid".into(),
        wait_pid.to_string().into(),
        "--silent".into(),
        "--norestart".into(),
        "--root".into(),
        installation.root.as_os_str().to_os_string(),
        "--packageDir".into(),
        installation.packages_dir.as_os_str().to_os_string(),
    ]
}

/// 実際に Update.exe を起動する
pub struct ProcessApplyLauncher;

impl ApplyLauncher for ProcessApplyLauncher {
    fn launch(&self, update_exe: &Path, args: &[OsString]) -> Result<()> {
        let mut command = std::process::Command::new(update_exe);
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if let Some(dir) = update_exe.parent() {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            /// コンソールを作らない（SDK の run_process と同じ）
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map(|_| ()).map_err(|e| {
            update_err(format!(
                "Update.exe を起動できませんでした（{}）: {e}",
                update_exe.display()
            ))
        })
    }
}
