//! Edictora 全体で使うエラー型。
//!
//! C# 版の例外階層を 1 つの enum に写したもの。
//! - `Usage` / `Config` … 引数・設定の誤り（終了コード 2。Chrome を開く前に検出する）
//! - `Dashboard` / `WriteBlocked` / `LoginRequired` … ダッシュボード操作の失敗（C# の `DashboardException` 系）
//! - `Cdp` / `Timeout` / `JavaScript` / `Io` … 下位層の失敗（C# では `DashboardException` ではない例外）
//! - `Cancelled` … Ctrl+C による中断（終了コード 130）
//!
//! 手順側（`ListingService`）は「DashboardException 系か」で扱いを変えるため、
//! [`Error::is_dashboard`] で C# の `catch (DashboardException)` と同じ範囲を判定する。

use std::fmt;

/// ログインが必要なときの定型文（C# の `LoginRequiredException` と同じ文面）
pub const LOGIN_REQUIRED_MESSAGE: &str = "ダッシュボードにログインしていません。先に login を実行し、開いた Chrome で Google にログインしてください。";

/// Edictora のエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// コマンドライン引数の誤り（C# の `CliUsageException`）
    Usage(String),
    /// 設定ファイルの誤り（C# の `ConfigException`）
    Config(String),
    /// ダッシュボード操作の失敗（要素が見つからない・タイムアウト・照合不一致など）
    Dashboard(String),
    /// 書き込みを中止した（審査待ち・ロック中・状態を判定できない）。安全側に倒した結果
    WriteBlocked(String),
    /// ログインしていない（Google のログイン画面へ移った）
    LoginRequired,
    /// CDP コマンドがエラー応答を返した・接続が閉じた（C# の `CdpException`）
    Cdp(String),
    /// 応答・イベント待ちのタイムアウト（C# の `TimeoutException`）
    Timeout(String),
    /// ページ内スクリプトの実行失敗（C# の `JavaScriptException`）
    JavaScript(String),
    /// ファイル・プロセスなどの入出力の失敗
    Io(String),
    /// 利用者による中断（Ctrl+C）
    Cancelled,
}

impl Error {
    /// C# の `DashboardException`（とその派生）に当たるか
    pub fn is_dashboard(&self) -> bool {
        matches!(
            self,
            Error::Dashboard(_) | Error::WriteBlocked(_) | Error::LoginRequired
        )
    }

    /// メッセージ本文（種類の接頭辞を付けない）
    pub fn message(&self) -> String {
        match self {
            Error::Usage(m)
            | Error::Config(m)
            | Error::Dashboard(m)
            | Error::WriteBlocked(m)
            | Error::Cdp(m)
            | Error::Timeout(m)
            | Error::JavaScript(m)
            | Error::Io(m) => m.clone(),
            Error::LoginRequired => LOGIN_REQUIRED_MESSAGE.to_string(),
            Error::Cancelled => "中断しました。".to_string(),
        }
    }

    /// 種類の名前（ログ用）
    pub fn kind_name(&self) -> &'static str {
        match self {
            Error::Usage(_) => "Usage",
            Error::Config(_) => "Config",
            Error::Dashboard(_) => "Dashboard",
            Error::WriteBlocked(_) => "WriteBlocked",
            Error::LoginRequired => "LoginRequired",
            Error::Cdp(_) => "Cdp",
            Error::Timeout(_) => "Timeout",
            Error::JavaScript(_) => "JavaScript",
            Error::Io(_) => "Io",
            Error::Cancelled => "Cancelled",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// Edictora の Result
pub type Result<T> = std::result::Result<T, Error>;
