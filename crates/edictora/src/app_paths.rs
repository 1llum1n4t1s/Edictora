//! Edictora が使う既定の保存先（C# の `AppPaths`）。
//!
//! - 利用者が書く設定（正本の対応づけ）は移動プロファイル側の `%APPDATA%\Edictora` に置く。
//! - Chrome プロファイル・ログ・診断情報は端末ローカルの `%LOCALAPPDATA%\Edictora` に置く。
//!   Chrome プロファイルにはログイン情報（Cookie）が入るため、ローミングさせない。
//! - いずれもリポジトリの外。
//!
//! C# 版は static だったが、Rust 版は値として持ち回す（注入できるようにする）。
//! テストは一時ディレクトリを根にした `AppPaths` を渡し、実プロファイル・実設定を解決しない。
//! 環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きは `main` だけが読む。

use std::path::{Path, PathBuf};

/// アプリ名（保存先フォルダ名に使う）
pub const APP_NAME: &str = "Edictora";

/// ローカル側の根を上書きする環境変数（検証・動作確認用）
pub const LOCAL_ROOT_ENV: &str = "EDICTORA_LOCAL_ROOT";

/// 移動プロファイル側の根を上書きする環境変数（検証・動作確認用）
pub const ROAMING_ROOT_ENV: &str = "EDICTORA_ROAMING_ROOT";

/// 保存先の一式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// `%LOCALAPPDATA%\Edictora`（Chrome プロファイル・ログ・診断情報）
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
            .unwrap_or_else(|| folder("LOCALAPPDATA").join(APP_NAME));
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

    /// 既定の Chrome プロファイル（`%LOCALAPPDATA%\Edictora\chrome-profile`）。秘密情報として扱う
    pub fn default_profile_directory(&self) -> PathBuf {
        self.local_root.join("chrome-profile")
    }

    /// ログ（`%LOCALAPPDATA%\Edictora\logs`）
    pub fn log_directory(&self) -> PathBuf {
        self.local_root.join("logs")
    }

    /// 失敗時の画面保存先（`%LOCALAPPDATA%\Edictora\diagnostics`）
    pub fn diagnostics_directory(&self) -> PathBuf {
        self.local_root.join("diagnostics")
    }
}

/// 既知フォルダーの環境変数（無ければカレントディレクトリ）
fn folder(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}
