//! MSIX パッケージのアプリ専用領域へのリダイレクト検出（`redirect`）のテスト。
//!
//! 実ファイルでの最終パス解決と、設定が無いときの候補の案内を、作業フォルダーの下に作った偽の `Packages` で確かめる
//! （実際の `%LOCALAPPDATA%\Packages` や `%APPDATA%\Edictora` には触れない）。

mod common;

use std::path::Path;

use common::{config_err, test_paths, work_dir};
use edictora::config;
use edictora::redirect::{find_redirect, resolve_final_path};

#[test]
fn 通常のフォルダーとファイルはリダイレクトされない() {
    let dir = work_dir("redirect_plain");
    let file = dir.join("a.txt");
    std::fs::write(&file, "x").unwrap();
    assert_eq!(find_redirect(&file), None);
    assert_eq!(find_redirect(&dir), None);
    // 無いパスは判定できない（作らない・エラーにしない）
    assert_eq!(find_redirect(&dir.join("none")), None);
    assert!(!dir.join("none").exists());
    #[cfg(windows)]
    {
        let resolved = resolve_final_path(&file).expect("最終パス");
        assert!(resolved.ends_with("a.txt"), "{resolved:?}");
        assert!(!resolved.to_string_lossy().starts_with(r"\\?\"));
    }
}

/// 偽の Packages: <root>/<name>/LocalCache/Roaming/Edictora/config.json を作る
fn fake_package(root: &Path, name: &str, with_config: bool) {
    let dir = root
        .join(name)
        .join("LocalCache")
        .join("Roaming")
        .join("Edictora");
    std::fs::create_dir_all(&dir).unwrap();
    if with_config {
        std::fs::write(dir.join("config.json"), "{}").unwrap();
    }
}

fn reset(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
}

#[test]
fn 設定が無いとき専用領域の候補をエラーに添える() {
    let dir = work_dir("redirect_hint");
    let root = dir.join("Packages");
    reset(&root);
    fake_package(&root, "Claude_x", true);
    let paths = test_paths(&dir).with_packages_root(&root);
    let _ = std::fs::remove_file(paths.default_config_path());

    let message = config_err(config::load(None, None, None, true, &paths));
    assert!(
        message.contains(&paths.default_config_path().display().to_string()),
        "{message}"
    );
    let candidate = root
        .join("Claude_x")
        .join("LocalCache")
        .join("Roaming")
        .join("Edictora")
        .join("config.json");
    assert!(
        message.contains(&candidate.display().to_string()),
        "{message}"
    );
    // 読み取りだけ: 候補は元の場所に残り、本物の場所には何も作られない
    assert!(candidate.is_file());
    assert!(!paths.default_config_path().exists());
}

#[test]
fn 候補が無い_根が無い_設定を明示したときは案内を添えない() {
    let dir = work_dir("redirect_nohint");
    let root = dir.join("Packages");
    reset(&root);
    fake_package(&root, "Claude_x", false);
    let with_root = test_paths(&dir).with_packages_root(&root);
    let plain = config_err(config::load(None, None, None, true, &test_paths(&dir)));

    // 候補なし
    let msg = config_err(config::load(None, None, None, true, &with_root));
    assert_eq!(msg, plain);

    // 候補あり・根なし（テスト・保存先を差し替えた実行）
    fake_package(&root, "Claude_y", true);
    let msg = config_err(config::load(None, None, None, true, &test_paths(&dir)));
    assert_eq!(msg, plain);

    // 候補あり・--config を明示 → 明示したファイルが無いだけ
    let explicit = dir.join("explicit.json");
    let _ = std::fs::remove_file(&explicit);
    let msg = config_err(config::load(
        Some(explicit.to_str().unwrap()),
        None,
        None,
        true,
        &with_root,
    ));
    assert!(!msg.contains("Claude_y"), "{msg}");
}
