//! ログ出力（C# の `LoggerService` 相当の簡単な実装）。
//!
//! - `%LOCALAPPDATA%\Edictora.Data\logs\Edictora_yyyy-MM-dd.log` へ 1 行ずつ追記する（UTF-8）
//! - 形式: `yyyy-MM-dd HH:mm:ss.ffff [LEVEL] message`
//! - 保持は 7 日。初期化時に、日付がそれより古い自アプリのログファイルだけを消す
//! - [`init`] を呼ぶまでは何も書かない（テストはログを書かない）
//! - ダッシュボードの URL に含まれる publisherId は、書く直前に [`redact`] で必ず伏せる
//!
//! C# 版（SuperLightLogger）にあった 10 MB ごとの分割は持たない（1 回の実行で書く量が小さいため）。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use regex::Regex;

use crate::util::LocalTime;

/// ログレベル
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn label(self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// ログファイル名の接頭辞
const FILE_PREFIX: &str = "Edictora";

/// ログ保持日数
const RETENTION_DAYS: i64 = 7;

/// 最小ログレベル（デバッグビルドは Debug、リリースは Info）
const MIN_LEVEL: LogLevel = if cfg!(debug_assertions) {
    LogLevel::Debug
} else {
    LogLevel::Info
};

/// 出力先（未初期化なら None）
static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// ロガーを初期化する（フォルダーを作れなければファイルログは無効のまま）
pub fn init(directory: &Path) {
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    if let Ok(mut guard) = LOG_DIR.lock() {
        *guard = Some(directory.to_path_buf());
    }
    cleanup_old_log_files(directory);
    log("Logger initialized", LogLevel::Debug);
}

/// ログの出力を止める
pub fn shutdown() {
    if let Ok(mut guard) = LOG_DIR.lock() {
        *guard = None;
    }
}

/// ログを出力する（publisherId は自動で伏せる）
pub fn log(message: &str, level: LogLevel) {
    if level < MIN_LEVEL {
        return;
    }
    write_line(message, level);
}

/// 詳細付きのエラーログ（常に Error レベル）
pub fn log_error_detail(message: &str, detail: &str) {
    write_line(&format!("{message}\n{detail}"), LogLevel::Error);
}

fn write_line(message: &str, level: LogLevel) {
    let Ok(guard) = LOG_DIR.lock() else { return };
    let Some(dir) = guard.as_ref() else { return };
    let now = LocalTime::now();
    let path = dir.join(format!("{FILE_PREFIX}_{}.log", now.date_string()));
    // 書式は `yyyy-MM-dd HH:mm:ss.ffff [LEVEL] message`（時刻部分は LocalTime::log_timestamp）
    let line = format!(
        "{} [{}] {}\n",
        now.log_timestamp(),
        level.label(),
        redact(message)
    );
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 文字列中のダッシュボード URL から publisherId を伏せる。
/// 「devconsole/」の直後のパス要素を `<publisher>` に置き換える。
///
/// C# 版の正規表現にあった否定先読み `(?!<publisher>)` は、Rust の regex では使えない。
/// ただし置き換える文字の集合が `<` を含まないため、`<publisher>` は元から一致しない（先読みなしで同じ結果）。
pub fn redact(text: &str) -> String {
    // 固定の正規表現は初回参照時に 1 度だけコンパイルする
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(?P<prefix>devconsole/)[^/\s?#"'<>]+"#).expect("固定の正規表現")
    });
    RE.replace_all(text, "${prefix}<publisher>").into_owned()
}

/// 保持期間を超えた古いログファイルを片付ける（自アプリのログファイルだけが対象）
fn cleanup_old_log_files(directory: &Path) {
    let cutoff = LocalTime::now().days() - RETENTION_DAYS;
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".log") else {
            continue;
        };
        if !stem.starts_with(&format!("{FILE_PREFIX}_")) {
            continue;
        }
        let parts: Vec<&str> = stem.split('_').collect();
        if parts.len() < 2 {
            continue;
        }
        if let Some(days) = parse_date(parts[1])
            && days < cutoff
        {
            // 使用中なら次回に回す
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// "yyyy-MM-dd" を日数にする
fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i32, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(crate::util::days_from_civil(y, m, d))
}
