//! MSIX パッケージ化されたアプリ（Claude デスクトップなど）の中から起動されたときの、保存先のリダイレクトの検出。
//!
//! 背景: MSIX のファイルシステム仮想化により、パッケージ内のプロセスが `%APPDATA%` / `%LOCALAPPDATA%` の下に
//! 「本物にまだ無いフォルダー」を新規作成すると、本物ではなくパッケージ専用領域
//! `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalCache\{Roaming,Local}\...` に作られる。
//! 本物に既にあるフォルダーへの書き込みは本物に入り、両方にあるとパッケージ内のプロセスは専用領域を優先して読む。
//! その結果、Edictora の設定（`%APPDATA%\Edictora\config.json`）や Chrome プロファイルが専用領域にだけできて、
//! 利用者の通常のターミナルからは「無い」ように見える。
//!
//! 方針:
//! - 実際の置き場所は、パスを開いて `GetFinalPathNameByHandleW` で得た最終パスで知る（仮想化は開いたハンドルの
//!   最終パスに現れる。2026-11-15 に Claude デスクトップの Bash から実測済み）。
//! - 「リダイレクトされているか」の判定はパス文字列だけの純粋関数（[`detect_redirect`]）に分け、テストで固める。
//! - 見つかったら警告するだけで、自動で読み込み直したり移したりしない（利用者のデータなので勝手に動かさない）。
//! - 設定が無いときは、専用領域側の候補を読み取りだけで探して案内する（[`find_package_configs`]）。
//! - パッケージ内から本物の場所へ確実にフォルダーを作る手段は無い（子プロセスもパッケージの仮想化を受ける）。
//!   そのため「先に作る」のはパッケージの外（Setup・ログオン時の更新用 exe）で動く経路だけにとどめ、
//!   作った後の検出と警告を主な対策にする（[`ensure_data_folders`]）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::logging::{LogLevel, log};
use crate::tr;

/// 専用領域の目印となる、パス中の連続した名前（小文字。`...\AppData\Local\Packages\<name>\LocalCache\...`）。
/// `<name>` の位置は任意（パッケージ ファミリー名）
const PACKAGE_PREFIX: [&str; 3] = ["appdata", "local", "packages"];

/// 検証用: パッケージ専用領域の根（`...\Packages`）を差し替える環境変数
pub const PACKAGES_ROOT_ENV: &str = "EDICTORA_PACKAGES_ROOT";

// ───────── パス文字列だけで決まる部分（純粋関数） ─────────

/// `\\?\` 形式（`\\?\C:\...`・`\\?\UNC\server\share`）を通常の表記へ戻す
pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

/// 比較用の表記（拡張子プレフィックスを外し、`/` を `\` に寄せ、末尾の区切りを落とし、小文字にする）
fn normalized(path: &Path) -> String {
    strip_verbatim_prefix(&path.to_string_lossy())
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// パスが `...\AppData\Local\Packages\<name>\LocalCache\...` の下なら、そのパッケージ ファミリー名
/// （小文字化したもの）を返す
pub fn package_family_of(path: &Path) -> Option<String> {
    let text = normalized(path);
    let parts: Vec<&str> = text.split('\\').collect();
    parts.windows(5).find_map(|w| {
        (w[..3] == PACKAGE_PREFIX && !w[3].is_empty() && w[4] == "localcache")
            .then(|| w[3].to_string())
    })
}

/// 要求したパス `requested` を開いて得た最終パス `resolved` が、アプリ専用領域へのリダイレクトなら
/// その実際のパス（`\\?\` を外したもの）を返す。
///
/// 条件: 2 つが（大文字小文字・区切り・`\\?\` を除いて）異なり、かつ `resolved` が
/// `\AppData\Local\Packages\<name>\LocalCache\` の下にあること。
/// 利用者が最初から専用領域のパスを指定した（2 つが同じ）場合や、ジャンクションなど専用領域以外への
/// 付け替えは対象にしない（それは利用者の意図した配置で、見えない場所へ消える問題ではない）。
pub fn detect_redirect(requested: &Path, resolved: &Path) -> Option<PathBuf> {
    if normalized(requested) == normalized(resolved) {
        return None;
    }
    package_family_of(resolved)?;
    Some(PathBuf::from(strip_verbatim_prefix(
        &resolved.to_string_lossy(),
    )))
}

// ───────── 実パスの解決（Windows API） ─────────

/// 既存のファイル・フォルダーを開き、最終パス（`\\?\` を外したもの）を返す。
/// 存在しない・開けない場合や Windows 以外では `None`。読み取りのみで、何も作らない・変えない
pub fn resolve_final_path(path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        win::final_path(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// `path` がアプリ専用領域へリダイレクトされていれば、その実際のパスを返す
pub fn find_redirect(path: &Path) -> Option<PathBuf> {
    let resolved = resolve_final_path(path)?;
    detect_redirect(path, &resolved)
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    use super::strip_verbatim_prefix;

    type Handle = *mut c_void;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    /// 属性・パスの取得だけで足りるので読み書きの権限は要らない
    const NO_ACCESS: u32 = 0;
    /// 他のプロセスの読み書き・削除を妨げない
    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    const OPEN_EXISTING: u32 = 3;
    /// フォルダーを開くのに必要
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    /// 正規化したパス（ボリューム名は DOS 形式）
    const FILE_NAME_NORMALIZED: u32 = 0;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *mut c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: Handle,
        ) -> Handle;
        fn GetFinalPathNameByHandleW(
            file: Handle,
            file_path: *mut u16,
            file_path_len: u32,
            flags: u32,
        ) -> u32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub fn final_path(path: &Path) -> Option<PathBuf> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return None;
        }
        wide.push(0);
        // SAFETY: wide は NUL 終端の UTF-16。戻りのハンドルは下で必ず閉じる
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                NO_ACCESS,
                FILE_SHARE_ALL,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 1024];
        let mut result = None;
        // 長さが足りないときは、戻り値（必要な長さ）で 1 回だけ取り直す
        for _ in 0..2 {
            // SAFETY: buffer は書き込み可能な buffer.len() 個の u16
            let n = unsafe {
                GetFinalPathNameByHandleW(
                    handle,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    FILE_NAME_NORMALIZED,
                )
            } as usize;
            if n == 0 {
                break;
            }
            if n < buffer.len() {
                let text = std::ffi::OsString::from_wide(&buffer[..n]);
                result = Some(PathBuf::from(strip_verbatim_prefix(
                    &text.to_string_lossy(),
                )));
                break;
            }
            buffer = vec![0u16; n + 1];
        }
        // SAFETY: CreateFileW が返した有効なハンドル
        unsafe { CloseHandle(handle) };
        result
    }
}

// ───────── 見つからないときの候補 ─────────

/// パッケージ専用領域にある設定ファイルの候補を列挙する（読み取りのみ）:
/// `<packages_root>\*\LocalCache\Roaming\Edictora\config.json`（実在するファイルだけ。名前順）。
/// `exclude` は候補から外すパス（要求した本物の場所。大文字小文字を区別しない）
pub fn find_package_configs(packages_root: &Path, exclude: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(packages_root) else {
        return Vec::new();
    };
    let excluded = normalized(exclude);
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| {
            e.path()
                .join("LocalCache")
                .join("Roaming")
                .join(crate::app_paths::APP_NAME)
                .join("config.json")
        })
        .filter(|p| p.is_file() && normalized(p) != excluded)
        .collect();
    found.sort();
    found
}

// ───────── 利用者向けの警告（1 プロセスにつき種類ごとに 1 回） ─────────

static CONFIG_WARNED: AtomicBool = AtomicBool::new(false);
static PROFILE_WARNED: AtomicBool = AtomicBool::new(false);

/// 既定の設定ファイルがリダイレクトされていたときの警告文（その種類で最初の 1 回だけ。2 回目以降と未検出は `None`）。
/// 標準エラーへ出す。MCP の標準出力（JSON-RPC）には書かない
pub fn config_warning(requested: &Path) -> Option<String> {
    let actual = find_redirect(requested)?;
    if CONFIG_WARNED.swap(true, Ordering::Relaxed) {
        return None;
    }
    log(
        &format!(
            "設定ファイルがアプリ専用領域へリダイレクトされています: 要求 {} → 実際 {}",
            requested.display(),
            actual.display()
        ),
        LogLevel::Warning,
    );
    Some(tr!(
        "redirect.config_is_redirected",
        actual = actual.display(),
        requested_dir = requested.parent().unwrap_or(requested).display()
    ))
}

/// Chrome プロファイルがリダイレクトされていたときの警告文（[`config_warning`] と同じ規則）
pub fn profile_warning(requested: &Path) -> Option<String> {
    let actual = find_redirect(requested)?;
    if PROFILE_WARNED.swap(true, Ordering::Relaxed) {
        return None;
    }
    log(
        &format!(
            "Chrome プロファイルがアプリ専用領域へリダイレクトされています: 要求 {} → 実際 {}",
            requested.display(),
            actual.display()
        ),
        LogLevel::Warning,
    );
    Some(tr!(
        "redirect.profile_is_redirected",
        actual = actual.display(),
        requested = requested.display()
    ))
}

/// 設定が無いときに添える案内文（候補が無ければ `None`）
pub fn package_config_hint(candidates: &[PathBuf], requested: &Path) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let list = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    Some(tr!(
        "redirect.config_found_in_app_storage",
        candidates = list,
        requested = requested.display()
    ))
}

// ───────── 起きにくくする ─────────

/// 設定フォルダー（`%APPDATA%\Edictora`）とデータの根（`%LOCALAPPDATA%\Edictora.Data`）を先に用意する（ベストエフォート）。
///
/// パッケージの外（Setup・ログオン時の更新用 exe）から実行されれば本物の場所に作られ、その後にパッケージ内の
/// プロセスが書いても本物へ入る。パッケージ内から呼ばれた場合は専用領域へ作られてしまうため、作成後に
/// リダイレクトを検出して警告をログへ残す（フックには標準エラーの表示先が無い）。失敗しても何もしない。
pub fn ensure_data_folders(paths: &crate::app_paths::AppPaths) {
    for dir in [&paths.roaming_root, &paths.local_root] {
        if std::fs::create_dir_all(dir).is_err() {
            continue;
        }
        if let Some(actual) = find_redirect(dir) {
            log(
                &format!(
                    "保存先のフォルダー {} がアプリ専用領域 {} へリダイレクトされました（パッケージ内のプロセスから作成された可能性があります）",
                    dir.display(),
                    actual.display()
                ),
                LogLevel::Warning,
            );
        }
    }
}
