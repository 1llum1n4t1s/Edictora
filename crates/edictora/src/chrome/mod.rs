//! Chrome の検出と起動（C# の `Edictora.Core.Chrome`）。
//!
//! 専用プロファイルで起動するか、そのプロファイルの Chrome がデバッグポート付きで開いていれば接続する。
//! **プロファイルを削除する経路は持たない**（ログイン済みの秘密情報のため）。

pub mod launcher;
pub mod locator;

pub use launcher::{ChromeInstance, ChromeLaunchOptions, build_args, launch_or_attach, probe};
pub use locator::find_chrome;
