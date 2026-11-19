//! 表示つきのログインを求めたのに、同じプロファイルの画面なし Chrome に接続した場合の判定（純粋関数）。

use edictora::dashboard::browser::is_headless_user_agent;

#[test]
fn ヘッドレスのuaは画面なしと判定する() {
    assert!(is_headless_user_agent(Some(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/130.0.0.0 Safari/537.36"
    )));
}

#[test]
fn 通常のuaは画面ありと判定する() {
    assert!(!is_headless_user_agent(Some(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
    )));
}

#[test]
fn uaが読めないときは止めない() {
    assert!(!is_headless_user_agent(None));
    assert!(!is_headless_user_agent(Some("")));
}
