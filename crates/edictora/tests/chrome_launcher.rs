//! C# の ChromeLauncher.test.cs の移植（起動引数の組み立てだけを確かめる。Chrome は起動しない）
//!
//! 起動引数は「含む・含まない」の部分確認ではなく、引数列全体を完全一致で比べる。
//! 1 つでも足し引き・並べ替えがあれば落ちるので、AGENTS.md の「起動引数は最小限」の契約
//! （変える場合は実画面で Google ログインを再検証する）を変更時に必ず意識させられる。

use std::path::PathBuf;

use edictora::chrome::{ChromeLaunchOptions, build_args};

/// テストの入力にするプロファイルのパス（実プロファイルではない架空の場所）
const PROFILE: &str = r"C:\profiles\edictora";
/// テストの入力にする起動時の URL
const START_URL: &str = "about:blank";

fn args(headless: bool) -> Vec<String> {
    build_args(&ChromeLaunchOptions {
        chrome_path: PathBuf::from("chrome.exe"),
        user_data_dir: PathBuf::from(PROFILE),
        start_url: START_URL.to_string(),
        headless,
    })
}

/// 表示・ヘッドレスに共通する引数（この順で並ぶ）。
/// - `--user-data-dir`: ログイン状態を保つ専用プロファイルを入力どおりに渡す
/// - `--remote-debugging-port=0`: ポートは Chrome に自動で割り当てさせる（固定ポートの衝突を避ける）
/// - `--no-first-run` / `--no-default-browser-check`: 初回案内や既定ブラウザ確認の画面を出さない
/// - 描画の間引き抑止 3 つと `CalculateNativeWinOcclusion` の無効化: Chrome を前面に出さず、
///   裏に回ったままでも描画とクリックが応答するようにする（2026-11-13 の実画面で確認）
fn common_args() -> Vec<String> {
    vec![
        format!("--user-data-dir={PROFILE}"),
        "--remote-debugging-port=0".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-backgrounding-occluded-windows".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-features=CalculateNativeWinOcclusion".to_string(),
    ]
}

#[test]
fn build_args_表示時は最小限の引数だけを実装どおりの順で付ける() {
    // 守る契約:
    // - `--enable-automation` などの自動化フラグを付けない（navigator.webdriver が立ち、Google ログインを弾かれうる）
    // - 画面外配置（--window-position）を付けない（login では利用者が画面で操作する）
    // - 表示時は --headless / --window-size を付けない
    // 完全一致なので、上のどれかが紛れ込めば必ず落ちる
    let mut expected = common_args();
    expected.push(START_URL.to_string());
    assert_eq!(args(false), expected);
}

#[test]
fn build_args_ヘッドレス時だけheadlessと画面サイズを足す() {
    // 守る契約:
    // - ヘッドレスは `--headless=new`（同じプロファイルのログイン状態を使える方式）
    // - 既定の 800x600 では狭いので、ダッシュボードの通常表示に近い 1280x900 にする
    // - それ以外は表示時と同じで、自動化フラグなどは足さない
    let mut expected = common_args();
    expected.push("--headless=new".to_string());
    expected.push("--window-size=1280,900".to_string());
    expected.push(START_URL.to_string());
    assert_eq!(args(true), expected);
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
