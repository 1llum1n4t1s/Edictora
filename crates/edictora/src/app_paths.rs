//! Edictora が使う既定の保存先（C# の `AppPaths`）。
//!
//! - 利用者が書く設定（正本の対応づけ）は移動プロファイル側の `%APPDATA%\Edictora` に置く。
//! - Chrome プロファイル・ログ・診断情報は端末ローカルの `%LOCALAPPDATA%\Edictora.Data` に置く。
//!   Chrome プロファイルにはログイン情報（Cookie）が入るため、ローミングさせない。
//! - `%LOCALAPPDATA%\Edictora` は Velopack のインストール先（packId `Edictora`）で、アンインストール時に
//!   フォルダーごと消える。利用者のデータ（特にログイン済みの Chrome プロファイル）を巻き込まないよう、
//!   データの根はインストール先と分けて `Edictora.Data` にする。旧場所からの自動移行はしない（配布前のため）。
//! - いずれもリポジトリの外。
//!
//! C# 版は static だったが、Rust 版は値として持ち回す（注入できるようにする）。
//! テストは一時ディレクトリを根にした `AppPaths` を渡し、実プロファイル・実設定を解決しない。
//! 環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きは `main` だけが読む。

use std::path::{Path, PathBuf};

/// アプリ名（保存先フォルダ名に使う）
pub const APP_NAME: &str = "Edictora";

/// ローカル側のデータの根のフォルダー名（`%LOCALAPPDATA%` の直下）。
/// インストール先（`%LOCALAPPDATA%\Edictora`）とは別にする（上のモジュール説明を参照）
pub const LOCAL_DATA_DIR_NAME: &str = "Edictora.Data";

/// ローカル側の根を上書きする環境変数（検証・動作確認用）
pub const LOCAL_ROOT_ENV: &str = "EDICTORA_LOCAL_ROOT";

/// 移動プロファイル側の根を上書きする環境変数（検証・動作確認用）
pub const ROAMING_ROOT_ENV: &str = "EDICTORA_ROAMING_ROOT";

/// 保存先の一式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// `%LOCALAPPDATA%\Edictora.Data`（Chrome プロファイル・ログ・診断情報）
    pub local_root: PathBuf,
    /// `%APPDATA%\Edictora`（設定ファイル）
    pub roaming_root: PathBuf,
}

impl AppPaths {
    /// 根を指定して作る（テスト用）
    pub fn new(local_root: impl Into<PathBuf>, roaming_root: impl Into<PathBuf>) -> AppPaths {
        AppPaths {
            local_root: local_root.into(),
            roaming_root: roaming_root.into(),
        }
    }

    /// 環境から既定の保存先を決める（`main` だけが呼ぶ）。
    /// `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` があればそれを根にする。
    pub fn from_environment() -> AppPaths {
        let local = std::env::var_os(LOCAL_ROOT_ENV)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| default_local_root(&folder("LOCALAPPDATA")));
        let roaming = std::env::var_os(ROAMING_ROOT_ENV)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| folder("APPDATA").join(APP_NAME));
        AppPaths {
            local_root: local,
            roaming_root: roaming,
        }
    }

    /// 既定の設定ファイル（`%APPDATA%\Edictora\config.json`）
    pub fn default_config_path(&self) -> PathBuf {
        self.roaming_root.join("config.json")
    }

    /// 既定の Chrome プロファイル（`%LOCALAPPDATA%\Edictora.Data\chrome-profile`）。秘密情報として扱う
    pub fn default_profile_directory(&self) -> PathBuf {
        self.local_root.join("chrome-profile")
    }

    /// ログ（`%LOCALAPPDATA%\Edictora.Data\logs`）
    pub fn log_directory(&self) -> PathBuf {
        self.local_root.join("logs")
    }

    /// 失敗時の画面保存先（`%LOCALAPPDATA%\Edictora.Data\diagnostics`）
    pub fn diagnostics_directory(&self) -> PathBuf {
        self.local_root.join("diagnostics")
    }

    /// Chrome を開くコマンドの実行中を知らせるロック（`%LOCALAPPDATA%\Edictora.Data\edictora-running.lock`）。
    /// 更新の適用側（`edictora update`・更新用 exe）も同じ場所を見る。`EDICTORA_LOCAL_ROOT` で根を変えた
    /// 実行同士は別のロックになる（検証用の差し替えなので許容する）
    pub fn running_lock_path(&self) -> PathBuf {
        self.local_root.join(crate::update::lock::RUNNING_LOCK_FILE)
    }
}

/// `%LOCALAPPDATA%` に当たるフォルダーから、ローカル側のデータの既定の根を決める（`<local_app_data>\Edictora.Data`）。
/// 環境変数を読まない純粋関数（テストで既定の位置を確かめるため）
pub fn default_local_root(local_app_data: &Path) -> PathBuf {
    local_app_data.join(LOCAL_DATA_DIR_NAME)
}

/// 既知フォルダーの環境変数（無ければカレントディレクトリ）
fn folder(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}
