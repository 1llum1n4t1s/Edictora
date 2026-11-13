//! テスト共通の補助。
//!
//! 作業フォルダーは cargo が用意する `target/tmp`（CARGO_TARGET_TMPDIR）の下に、テストごとの固定名で作る。
//! 利用者の TEMP・実プロファイル（%LOCALAPPDATA%\Edictora.Data）・実設定（%APPDATA%\Edictora）は使わない。
//! 保存先（AppPaths）も作業フォルダーの下を指すものを渡す。

#![allow(dead_code)]

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use edictora::Error;
use edictora::app_paths::AppPaths;

/// テスト用の作業フォルダー（固定名。中のファイルは毎回書き直す）
pub fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("edictora-tests")
        .join(name);
    std::fs::create_dir_all(&dir).expect("作業フォルダーを作れません");
    dir
}

/// 作業フォルダーの下を根にした保存先（実プロファイル・実設定を解決しない）
pub fn test_paths(dir: &Path) -> AppPaths {
    AppPaths::new(dir.join("local"), dir.join("roaming"))
}

/// フィクスチャのパス
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ReplaceTranslator")
        .join(name)
}

/// 設定の誤り（`Error::Config`）であることを確かめ、その文言を返す。
/// 成功や別種のエラーなら、中身を表示してテストを失敗させる
pub fn config_err<T: Debug>(r: edictora::Result<T>) -> String {
    match r {
        Err(Error::Config(m)) => m,
        other => panic!("Config エラーを期待しました: {other:?}"),
    }
}

/// 架空のアイテム ID
pub const ITEM_A: &str = "abcdefghijklmnopabcdefghijklmnop";
/// 架空のアイテム ID（2 つ目）
pub const ITEM_B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";
