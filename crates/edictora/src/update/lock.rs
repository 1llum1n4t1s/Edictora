//! 排他用のロックファイル。
//!
//! Velopack の SDK（vendored の lockfile.rs の `windows_exclusive_lock`）と同じく、ファイルを共有なし
//! （share_mode 0）で開いたままにして他のプロセスを締め出す。drop で閉じれば解放される。
//! ファイル自体は消さない（SDK も `FILE_FLAG_DELETE_ON_CLOSE` を使っていない）。
//!
//! 使う場所:
//! - `<packages>\.velopack_lock` … ダウンロード中だけ持つ（SDK の `download_updates` と同じ。Update.exe を
//!   起動する前に手放す）
//! - `<packages>\.edictora-update.lock` … 更新処理の全体（確認〜Update.exe の起動）で持つ。ログオン時の
//!   更新用 exe と `edictora update` が同時に動かないようにする
//! - `<データの根>\edictora-running.lock`（[`RUNNING_LOCK_FILE`]）… Chrome を開くコマンド（login / pull / diff /
//!   push / submit）が実行中だけ共有で持つ。更新側は Update.exe を起動する直前にこれを排他で取り、取れなければ
//!   適用しない（Update.exe がインストール先配下のプロセスを止めて push の途中で落とすのを防ぐ）。
//!   排他で取れたら、プロセスが終わるまで持ち続ける（終わった後に Update.exe が差し替える）。
//!   インストール先に依存しない場所に置くので、インストールされていない実行（cargo run など）でも無害

use std::fs::File;
use std::path::{Path, PathBuf};

/// SDK と共有するダウンロード中のロックのファイル名
pub const VELOPACK_LOCK_FILE: &str = ".velopack_lock";

/// Edictora の更新処理の二重起動を防ぐロックのファイル名
pub const UPDATE_LOCK_FILE: &str = ".edictora-update.lock";

/// 実行中の edictora を知らせるロックのファイル名（データの根 `Edictora.Data` に置く）
pub const RUNNING_LOCK_FILE: &str = "edictora-running.lock";

/// ロックを取れなかった理由が「他のプロセスが開いたまま」か（共有違反 32・ロック違反 33）
pub fn is_held_by_another(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32) | Some(33))
}

/// 取得したロック（drop で解放）
#[derive(Debug)]
pub struct LockFile {
    _file: File,
    path: PathBuf,
}

impl LockFile {
    /// 排他で取る。すぐに取れなければ Err（待たない）
    pub fn try_acquire(path: &Path) -> std::io::Result<LockFile> {
        LockFile::open_with_share(path, 0)
    }

    /// 共有で取る（共有同士は同時に取れる。排他で持たれていれば Err、待たない）。
    /// Chrome を開くコマンドが実行中であることを、更新の適用側に知らせるのに使う
    pub fn try_acquire_shared(path: &Path) -> std::io::Result<LockFile> {
        /// FILE_SHARE_READ | FILE_SHARE_WRITE
        const SHARE_READ_WRITE: u32 = 0x1 | 0x2;
        LockFile::open_with_share(path, SHARE_READ_WRITE)
    }

    fn open_with_share(path: &Path, share: u32) -> std::io::Result<LockFile> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(share);
        }
        #[cfg(not(windows))]
        let _ = share;
        let file = options.open(path)?;
        Ok(LockFile {
            _file: file,
            path: path.to_path_buf(),
        })
    }

    /// ロックファイルの場所
    pub fn path(&self) -> &Path {
        &self.path
    }
}
