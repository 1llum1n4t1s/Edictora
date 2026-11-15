//! MSIX パッケージのアプリ専用領域へのリダイレクト検出（`redirect`）のテスト。
//!
//! 判定はパス文字列だけの純粋関数でテストし、候補の列挙は作業フォルダーの下に作った偽の `Packages` で確かめる
//! （実際の `%LOCALAPPDATA%\Packages` や `%APPDATA%\Edictora` には触れない）。

mod common;

use std::path::{Path, PathBuf};

use common::{config_err, test_paths, work_dir};
use edictora::config;
use edictora::redirect::{
    detect_redirect, find_package_configs, find_redirect, package_family_of, resolve_final_path,
    strip_verbatim_prefix,
};

const REAL: &str = r"C:\Users\IMT\AppData\Roaming\Edictora";
const VIRTUAL: &str =
    r"\\?\C:\Users\IMT\AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Edictora";

#[test]
fn 専用領域へ付け替えられていればその実パスを返す() {
    let actual = detect_redirect(Path::new(REAL), Path::new(VIRTUAL)).expect("リダイレクト");
    assert_eq!(
        actual,
        PathBuf::from(
            r"C:\Users\IMT\AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Edictora"
        )
    );
}

#[test]
fn ローカル側の専用領域も判定する() {
    let requested = r"C:\Users\IMT\AppData\Local\Edictora.Data\chrome-profile";
    let resolved = r"\\?\C:\Users\IMT\AppData\Local\Packages\Claude_x\LocalCache\Local\Edictora.Data\chrome-profile";
    assert!(detect_redirect(Path::new(requested), Path::new(resolved)).is_some());
}

#[test]
fn 同じパスなら判定しない_表記の揺れも同じとみなす() {
    assert_eq!(detect_redirect(Path::new(REAL), Path::new(REAL)), None);
    // 拡張プレフィックス・大文字小文字・末尾の区切り・スラッシュ
    let verbatim = format!(r"\\?\{}\", REAL.to_uppercase());
    assert_eq!(detect_redirect(Path::new(REAL), Path::new(&verbatim)), None);
    let slashes = REAL.replace('\\', "/");
    assert_eq!(detect_redirect(Path::new(&slashes), Path::new(REAL)), None);
}

#[test]
fn 利用者が最初から専用領域のパスを指定したなら判定しない() {
    let p = VIRTUAL.trim_start_matches(r"\\?\");
    assert_eq!(detect_redirect(Path::new(p), Path::new(VIRTUAL)), None);
}

#[test]
fn 専用領域以外への付け替えは判定しない() {
    // ジャンクションなどで別のドライブへ向いているだけ
    assert_eq!(
        detect_redirect(Path::new(REAL), Path::new(r"\\?\D:\Data\Roaming\Edictora")),
        None
    );
    // Packages の下でも LocalCache でなければ（例: LocalState）対象外
    assert_eq!(
        detect_redirect(
            Path::new(REAL),
            Path::new(r"C:\Users\IMT\AppData\Local\Packages\Claude_x\LocalState\Edictora")
        ),
        None
    );
    // AppData\Local\Packages の並びでなければ対象外
    assert_eq!(
        detect_redirect(
            Path::new(REAL),
            Path::new(r"C:\Work\Packages\Claude_x\LocalCache\Edictora")
        ),
        None
    );
}

#[test]
fn パッケージ_ファミリー名を取り出す() {
    assert_eq!(
        package_family_of(Path::new(VIRTUAL)).as_deref(),
        Some("claude_pzs8sxrjxfjjc")
    );
    assert_eq!(package_family_of(Path::new(REAL)), None);
    assert_eq!(
        package_family_of(Path::new(
            r"C:\Users\x\AppData\Local\Packages\LocalCache\foo"
        )),
        None
    );
}

#[test]
fn 拡張プレフィックスを外す() {
    assert_eq!(strip_verbatim_prefix(r"\\?\C:\a"), r"C:\a");
    assert_eq!(
        strip_verbatim_prefix(r"\\?\UNC\srv\share\a"),
        r"\\srv\share\a"
    );
    assert_eq!(strip_verbatim_prefix(r"C:\a"), r"C:\a");
}

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
fn 専用領域の設定ファイルの候補を列挙する() {
    let dir = work_dir("redirect_candidates");
    let root = dir.join("Packages");
    reset(&root);
    fake_package(&root, "B_pkg", true);
    fake_package(&root, "A_pkg", true);
    fake_package(&root, "C_pkg_no_config", false);
    std::fs::write(root.join("stray-file"), "x").unwrap();

    let found = find_package_configs(&root, &dir.join("config.json"));
    let names: Vec<_> = found
        .iter()
        .map(|p| {
            p.strip_prefix(&root)
                .unwrap()
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["A_pkg", "B_pkg"]);

    // 根が無ければ空
    assert!(find_package_configs(&dir.join("missing"), &dir.join("config.json")).is_empty());
    // 除外指定の候補は出さない（大文字小文字は区別しない）
    let excluded = found[0].to_string_lossy().to_uppercase();
    let left = find_package_configs(&root, Path::new(&excluded));
    assert_eq!(left, vec![found[1].clone()]);
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
