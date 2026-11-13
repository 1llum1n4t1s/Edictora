//! Velopack でインストールされているかの判定と、インストール先の各パスの決定。
//!
//! vendored の SDK（locator.rs）と同じ約束にする:
//! - `auto_locate_app_manifest(FromSpecifiedAppExecutable)`: exe の親フォルダーに `Update.exe` があれば
//!   そこが根。無ければ exe のパスの最後の `\current\` より前を根の候補にし、そこに `Update.exe` があれば根。
//! - `create_config_from_root_dir`: `Update.exe` = `<根>\Update.exe`、パッケージ = `<根>\packages`、
//!   マニフェスト = `<根>\current\sq.version`、アプリの exe = `<根>\current`。
//! - `VelopackLocator::new`: `Update.exe` とマニフェストの両方が無ければ「インストールされていない」。
//!
//! SDK は根に書き込めないとき（MSI で Program Files に入れた場合）だけ `%LOCALAPPDATA%\<id>\packages` へ
//! 逃がすが、Edictora は利用者ごとの `%LOCALAPPDATA%\<packId>` に Setup で入れるので扱わない（常に `<根>\packages`）。

use std::path::{Path, PathBuf};

use super::manifest::{Manifest, parse_manifest};

/// インストール先の一式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    /// 根（例 `%LOCALAPPDATA%\Edictora`）
    pub root: PathBuf,
    /// `<根>\Update.exe`
    pub update_exe: PathBuf,
    /// `<根>\packages`（ダウンロードしたパッケージの置き場）
    pub packages_dir: PathBuf,
    /// `<根>\current`（アプリの exe があるフォルダー。更新で中身が差し替わる）
    pub current_dir: PathBuf,
    /// `<根>\current\sq.version` の内容
    pub manifest: Manifest,
}

/// インストール先を決められなかった理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateError {
    /// Velopack でインストールされていない（cargo run・展開しただけの exe など）
    NotInstalled(String),
    /// インストール先らしいがマニフェストが読めない・正しくない
    InvalidManifest(String),
}

impl LocateError {
    /// 利用者向けの文
    pub fn message(&self) -> String {
        match self {
            LocateError::NotInstalled(m) | LocateError::InvalidManifest(m) => m.clone(),
        }
    }
}

/// 根からインストール先の各パスを作る（存在は確かめない）
fn layout(root: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let current = root.join("current");
    (
        root.join("Update.exe"),
        root.join("packages"),
        current.join("sq.version"),
        current,
    )
}

/// exe のパスから根の候補を探す（SDK の `FromSpecifiedAppExecutable` と同じ順）
fn find_root(exe: &Path) -> Option<PathBuf> {
    if let Some(parent) = exe.parent()
        && parent.join("Update.exe").exists()
    {
        return Some(parent.to_path_buf());
    }
    let text = exe.to_string_lossy();
    let idx = text.rfind("\\current\\")?;
    let maybe_root = PathBuf::from(&text[..idx]);
    maybe_root.join("Update.exe").exists().then_some(maybe_root)
}

/// 指定の exe（通常は自分自身）からインストール先を決める
pub fn locate_from_exe(exe: &Path) -> Result<Installation, LocateError> {
    let root = find_root(exe).ok_or_else(|| {
        LocateError::NotInstalled(
            "Velopack でインストールされた場所から起動されていません（Update.exe が見つかりません）"
                .to_string(),
        )
    })?;
    locate_from_root(&root)
}

/// 根を指定してインストール先を決める
pub fn locate_from_root(root: &Path) -> Result<Installation, LocateError> {
    let (update_exe, packages_dir, manifest_path, current_dir) = layout(root);
    if !update_exe.exists() {
        return Err(LocateError::NotInstalled(format!(
            "Update.exe がありません（{}）",
            update_exe.display()
        )));
    }
    if !manifest_path.exists() {
        return Err(LocateError::NotInstalled(format!(
            "マニフェストがありません（{}）",
            manifest_path.display()
        )));
    }
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| {
        LocateError::InvalidManifest(format!(
            "マニフェストを読めません（{}）: {e}",
            manifest_path.display()
        ))
    })?;
    let manifest = parse_manifest(&text).map_err(LocateError::InvalidManifest)?;
    Ok(Installation {
        root: root.to_path_buf(),
        update_exe,
        packages_dir,
        current_dir,
        manifest,
    })
}

/// 自分自身（実行中の exe）からインストール先を決める
pub fn locate_current() -> Result<Installation, LocateError> {
    let exe = std::env::current_exe().map_err(|e| {
        LocateError::NotInstalled(format!("実行中の exe の場所を取得できません: {e}"))
    })?;
    locate_from_exe(&exe)
}
