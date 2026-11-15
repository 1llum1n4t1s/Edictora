//! パッケージのダウンロードと検証。
//!
//! vendored の SDK（manager.rs の `download_updates` と `verify_package_checksum`）と同じ約束にする:
//! - ダウンロード中は `<packages>\.velopack_lock` を持つ。
//! - 保存先は `<packages>\<FileName>`。まず `<FileName の拡張子を .partial にしたもの>` に書き、
//!   サイズとハッシュを確かめてから改名する。
//! - 検証: サイズがフィードの Size と一致し、SHA256 があればそれ、無ければ SHA1 が一致すること
//!   （大文字小文字は区別しない）。両方とも無ければ SHA1 の空文字と比べることになり失敗する（SDK と同じ）。
//! - 改名できたら、ダウンロード前からあった `*.nupkg`・`*.partial` と `*-delta.nupkg` を消す。
//!
//! SDK と違う点（意図的）:
//! - 保存先に同名のファイルが既にあるとき、SDK は検証せずにそのまま使うが、ここでは検証し直し、
//!   合わなければ消して取り直す（検証していないものを適用しない）。
//! - SDK はダウンロード後に nupkg から `Squirrel.exe` を取り出して `Update.exe` を差し替えるが、
//!   zip の展開を持たないため行わない（Update.exe はインストール時の版のまま。結合確認で影響を確かめる）。
//! - ダウンロードは宣言の Size を上限に打ち切る（大きすぎる応答を書き続けない）。

use std::path::{Path, PathBuf};

use crate::cancel::CancelToken;
use crate::error::Result;
use crate::net::winhttp::{self as net, HttpRequest, ResponseSink};

use super::feed::{FeedAsset, is_safe_file_name};
use super::hash::file_digests;
use super::lock::{LockFile, VELOPACK_LOCK_FILE};
use super::source::UpdateSource;
use super::{http_error, service, update_err};
use std::time::Duration;

/// ダウンロードの操作ごとのタイムアウト（接続・受信の 1 回ごと）
const DOWNLOAD_OP_TIMEOUT: Duration = Duration::from_secs(60);
/// ダウンロード全体の期限
const DOWNLOAD_TOTAL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 検証の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// サイズが違う
    Size { expected: u64, actual: u64 },
    /// ハッシュが違う（どちらのハッシュで比べたか）
    Checksum { algorithm: &'static str },
    /// 読めない
    Io(String),
}

impl VerifyError {
    pub fn message(&self) -> String {
        match self {
            VerifyError::Size { expected, actual } => {
                crate::tr!(
                    "update.size_mismatch_feed_bytes_actual_bytes",
                    expected = expected,
                    actual = actual
                )
            }
            VerifyError::Checksum { algorithm } => {
                crate::tr!("update.mismatch", algorithm = algorithm)
            }
            VerifyError::Io(m) => {
                crate::tr!("update.cannot_read_the_file", detail = m)
            }
        }
    }
}

/// ファイルがフィードの項目と一致するかを確かめる（SDK の `verify_package_checksum` と同じ順と規則）
pub fn verify_package(file: &Path, asset: &FeedAsset) -> std::result::Result<(), VerifyError> {
    let actual = file
        .metadata()
        .map_err(|e| VerifyError::Io(e.to_string()))?
        .len();
    if actual != asset.size {
        return Err(VerifyError::Size {
            expected: asset.size,
            actual,
        });
    }
    let digests = file_digests(file).map_err(|e| VerifyError::Io(e.to_string()))?;
    if !asset.sha256.is_empty() {
        if !digests.sha256.eq_ignore_ascii_case(&asset.sha256) {
            return Err(VerifyError::Checksum {
                algorithm: "SHA256",
            });
        }
    } else if !digests.sha1.eq_ignore_ascii_case(&asset.sha1) {
        return Err(VerifyError::Checksum { algorithm: "SHA1" });
    }
    Ok(())
}

/// パッケージを `<packages>` に用意する（既にあって検証が通ればそれを使う）。用意できたパスを返す
pub fn ensure_package(
    packages_dir: &Path,
    source: &UpdateSource,
    asset: &FeedAsset,
    progress: &dyn Fn(&str),
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if !is_safe_file_name(&asset.file_name) {
        return Err(update_err(crate::tr!(
            "update.invalid_file_name_in_the_feed",
            file_name = asset.file_name
        )));
    }
    std::fs::create_dir_all(packages_dir).map_err(|e| {
        update_err(crate::tr!(
            "update.cannot_create_the_package_folder",
            packages_dir = packages_dir.display(),
            error = e
        ))
    })?;
    let _lock = LockFile::try_acquire(&packages_dir.join(VELOPACK_LOCK_FILE)).map_err(|e| {
        update_err(crate::tr!(
            "update.the_package_folder_is_in_use_another",
            error = e
        ))
    })?;

    let final_path = packages_dir.join(&asset.file_name);
    let partial_path = final_path.with_extension("partial");

    if final_path.exists() {
        match verify_package(&final_path, asset) {
            Ok(()) => {
                progress(&crate::tr!(
                    "update.using_the_already_downloaded_package",
                    file_name = asset.file_name
                ));
                return Ok(final_path);
            }
            Err(e) => {
                progress(&crate::tr!(
                    "update.the_downloaded_package_does_not_match_so",
                    error = e.message()
                ));
                std::fs::remove_file(&final_path).map_err(|e| {
                    update_err(crate::tr!(
                        "update.cannot_delete_the_mismatched_package",
                        final_path = final_path.display(),
                        error = e
                    ))
                })?;
            }
        }
    }

    // ダウンロード前からある古いパッケージ・途中のファイル（成功後に消す）
    let to_delete = list_files(packages_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".nupkg") || lower.ends_with(".partial")
    })
    .into_iter()
    .filter(|p| p != &final_path && p != &partial_path)
    .collect::<Vec<_>>();

    progress(&crate::tr!(
        "update.downloading_the_package_bytes",
        file_name = asset.file_name,
        size = asset.size
    ));
    let downloaded = download_to(&partial_path, source, asset, cancel);
    if let Err(e) = downloaded {
        let _ = std::fs::remove_file(&partial_path);
        return Err(e);
    }
    if let Err(e) = verify_package(&partial_path, asset) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(update_err(crate::tr!(
            "update.the_downloaded_package_failed_verification_so_it",
            error = e.message()
        )));
    }
    std::fs::rename(&partial_path, &final_path).map_err(|e| {
        update_err(crate::tr!(
            "update.cannot_rename_the_downloaded_package",
            final_path = final_path.display(),
            error = e
        ))
    })?;

    // 差分パッケージも片付ける（SDK と同じく改名の後で集める）
    let deltas = list_files(packages_dir, |name| {
        name.to_ascii_lowercase().ends_with("-delta.nupkg")
    });
    for path in to_delete.into_iter().chain(deltas) {
        if path != final_path {
            // 使用中などで消せなければ次回に回す
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(final_path)
}

/// 配信元から一時ファイルへダウンロードする
fn download_to(
    partial_path: &Path,
    source: &UpdateSource,
    asset: &FeedAsset,
    cancel: &CancelToken,
) -> Result<()> {
    let response = net::send(
        HttpRequest {
            target: source.package_target(&asset.file_name),
            headers: Vec::new(),
            timeout: DOWNLOAD_OP_TIMEOUT,
            total_timeout: DOWNLOAD_TOTAL_TIMEOUT,
            service: service(),
        },
        ResponseSink::File {
            path: partial_path.to_path_buf(),
            max_bytes: asset.size,
        },
        cancel,
    )
    .map_err(|e| http_error(e, DOWNLOAD_TOTAL_TIMEOUT))?;
    if response.status != 200 {
        return Err(update_err(crate::tr!(
            "update.could_not_fetch_the_package_http",
            status = response.status,
            file_name = asset.file_name
        )));
    }
    Ok(())
}

/// フォルダー直下のファイルのうち、名前が条件に合うもの
fn list_files(dir: &Path, matches: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| matches(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect()
}
