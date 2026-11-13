//! C# の ChromeLauncher.test.cs の移植（起動引数の組み立てだけを確かめる。Chrome は起動しない）

use std::path::PathBuf;

use edictora::chrome::{ChromeLaunchOptions, build_args};

fn args(headless: bool) -> Vec<String> {
    build_args(&ChromeLaunchOptions {
        chrome_path: PathBuf::from("chrome.exe"),
        user_data_dir: PathBuf::from(r"C:\profiles\edictora"),
        start_url: "about:blank".to_string(),
        headless,
    })
}

#[test]
fn build_args_ヘッドレス指定のときだけheadlessと画面サイズを付ける() {
    let headless = args(true);
    assert!(headless.contains(&"--headless=new".to_string()));
    assert!(headless.contains(&"--window-size=1280,900".to_string()));
    assert!(
        !headless
            .iter()
            .any(|a| a.starts_with("--enable-automation"))
    );

    let visible = args(false);
    assert!(!visible.iter().any(|a| a.starts_with("--headless")));
    assert!(!visible.iter().any(|a| a.starts_with("--window-size")));
}

#[test]
fn build_args_裏に回っても描画を止めないよう隠れたウィンドウの検出を止める() {
    // 前面に出さずに操作するための前提（2026-11-13 の実画面で確認）
    assert!(args(false).contains(&"--disable-features=CalculateNativeWinOcclusion".to_string()));
    assert!(args(true).contains(&"--disable-features=CalculateNativeWinOcclusion".to_string()));
}

#[test]
fn build_args_専用プロファイルとポート自動割り当てを指定する() {
    let a = args(false);
    assert!(a.contains(&r"--user-data-dir=C:\profiles\edictora".to_string()));
    assert!(a.contains(&"--remote-debugging-port=0".to_string()));
    assert!(a.contains(&"--no-first-run".to_string()));
    assert!(a.contains(&"--no-default-browser-check".to_string()));
}

#[test]
fn build_args_ログインを妨げる自動化フラグや画面外配置を付けない() {
    let a = args(false);
    assert!(!a.iter().any(|x| x.starts_with("--enable-automation")));
    assert!(!a.iter().any(|x| x.starts_with("--headless")));
    assert!(!a.iter().any(|x| x.starts_with("--window-position")));
    assert!(!a.iter().any(|x| x.to_lowercase().contains("extension")));
}

#[test]
fn build_args_許可した引数だけを付ける() {
    let allowed = [
        "--user-data-dir=",
        "--remote-debugging-port=0",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
        "--disable-background-timer-throttling",
        "--disable-features=CalculateNativeWinOcclusion",
        "--headless=new",
        "--window-size=1280,900",
        "about:blank",
    ];
    for arg in args(false).into_iter().chain(args(true)) {
        assert!(
            allowed.iter().any(|a| arg.starts_with(a)),
            "想定外の起動引数: {arg}"
        );
    }
}

// ───────── Rust 版で足したもの ─────────

#[test]
fn find_chromeはパニックせず見つかれば実在するファイルを返す() {
    // レジストリと既知の場所を読むだけ（Chrome は起動しない）
    if let Some(path) = edictora::chrome::find_chrome() {
        eprintln!("find_chrome: {}", path.display());
        assert!(path.is_file(), "{}", path.display());
        assert!(
            path.to_string_lossy()
                .to_lowercase()
                .ends_with("chrome.exe")
        );
    }
}
