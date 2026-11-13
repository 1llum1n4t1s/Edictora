//! C# の CliArguments.test.cs（CliArgumentsTests・CliAppTests）の移植。
//! CliApp のテストは、ブラウザを開く関数に「呼ばれたら失敗する」ものを渡す（Chrome を開く前に終わることを確かめる）。

mod common;

use std::time::Duration;

use common::{ITEM_A as ITEM, ITEM_B as ITEM2, test_paths, work_dir};
use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::cli::args::{CliCommandKind, parse};
use edictora::cli::{BrowserOpener, CliApp};
use edictora::dashboard::browser::{DashboardBrowser, DashboardBrowserOptions};

fn usage_error(args: &[&str]) -> bool {
    matches!(parse(args), Err(Error::Usage(_)))
}

#[test]
fn parse_引数なしは差分表示() {
    assert_eq!(parse::<&str>(&[]).unwrap().kind, CliCommandKind::Diff);
    assert_eq!(parse(&["--item", ITEM]).unwrap().kind, CliCommandKind::Diff);
}

#[test]
fn parse_ヘルプとバージョン() {
    assert_eq!(parse(&["--help"]).unwrap().kind, CliCommandKind::Help);
    assert_eq!(parse(&["push", "-h"]).unwrap().kind, CliCommandKind::Help);
    assert_eq!(parse(&["--version"]).unwrap().kind, CliCommandKind::Version);
}

#[test]
fn parse_pushのオプションを読む() {
    let upper = ITEM.to_uppercase();
    let cmd = parse(&[
        "push",
        "--item",
        &upper,
        "--locale=ja",
        "--locale",
        "en",
        "--yes",
        "--config",
        "c.json",
        "--profile",
        "p",
    ])
    .unwrap();

    assert_eq!(cmd.kind, CliCommandKind::Push);
    assert_eq!(cmd.item_ids, vec![ITEM.to_string()]);
    assert_eq!(cmd.locale_codes, vec!["ja".to_string(), "en".to_string()]);
    assert!(cmd.yes);
    assert_eq!(cmd.config_path.as_deref(), Some("c.json"));
    assert_eq!(cmd.profile_dir.as_deref(), Some("p"));
}

#[test]
fn parse_submitは1件とyesが必須() {
    assert!(usage_error(&["submit", "--yes"]));
    assert!(usage_error(&["submit", "--item", ITEM]));
    assert!(usage_error(&[
        "submit", "--item", ITEM, "--item", ITEM2, "--yes"
    ]));
    assert!(usage_error(&[
        "submit", "--item", ITEM, "--locale", "ja", "--yes"
    ]));

    let cmd = parse(&["submit", "--item", ITEM, "--yes"]).unwrap();
    assert_eq!(cmd.kind, CliCommandKind::Submit);
}

#[test]
fn parse_コマンドに無いオプションや誤った値は拒否する() {
    assert!(usage_error(&["diff", "--yes"]));
    assert!(usage_error(&["pull", "--item", "short"]));
    assert!(usage_error(&["pull", "--locale", "日本語"]));
    assert!(usage_error(&["pull", "--item"]));
    assert!(usage_error(&["pull", "--config", "a", "--config", "b"]));
    assert!(usage_error(&["publish"]));
    assert!(usage_error(&["pull", "extra"]));
    assert!(usage_error(&["login", "--wait", "0"]));
    assert!(usage_error(&["push", "--yes=true"]));
}

#[test]
fn parse_show_browserは既定でオフで指定時だけオン_loginでは使えない() {
    assert!(!parse(&["diff"]).unwrap().show_browser);
    assert!(parse(&["diff", "--show-browser"]).unwrap().show_browser);
    assert!(
        parse(&["push", "--show-browser", "--yes"])
            .unwrap()
            .show_browser
    );
    // login は常にウィンドウを表示するので、指定させない
    assert!(usage_error(&["login", "--show-browser"]));
    assert!(usage_error(&["diff", "--show-browser=true"]));
}

#[test]
fn parse_loginの待ち時間() {
    assert_eq!(
        parse(&["login", "--wait", "5"]).unwrap().login_wait,
        Some(Duration::from_secs(5 * 60))
    );
}

// ───────── CliApp ─────────

/// Chrome を開いてはいけないテストで渡す関数（呼ばれたら失敗させる）
fn no_browser(_: &DashboardBrowserOptions, _: &CancelToken) -> edictora::Result<DashboardBrowser> {
    panic!("このテストでは Chrome を開いてはいけません");
}

/// CliApp を実行し、(終了コード, 標準出力, 標準エラー) を返す。
/// `token` は環境変数 EDICTORA_JEV_API_TOKEN の値の代わり（プロセスの環境変数には触れない）
fn run_with_token(dir_name: &str, args: &[&str], token: Option<&str>) -> (i32, String, String) {
    let dir = work_dir(dir_name);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let opener: &BrowserOpener = &no_browser;
    let code = CliApp::new(&mut out, &mut err, test_paths(&dir), opener)
        .with_jev_token(token.map(str::to_string))
        .run(args, &CancelToken::new());
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// トークンなし（EDICTORA_JEV_API_TOKEN が無い状態）で実行する。
/// CliApp::new の既定もトークンなしなので、`with_jev_token(None)` を通しても同じ状態になる
fn run(dir_name: &str, args: &[&str]) -> (i32, String, String) {
    run_with_token(dir_name, args, None)
}

#[test]
fn run_ヘルプは成功で使い方を出す() {
    let (code, output, _) = run("cli-help", &["--help"]);
    assert_eq!(code, 0);
    assert!(output.contains("edictora submit  --item <id> --yes"));
    // --jev とトークンの環境変数・モデル指定の説明もある
    assert!(output.contains("--jev"));
    assert!(output.contains("EDICTORA_JEV_API_TOKEN"));
    assert!(output.contains("--jev-model"));
}

#[test]
fn run_引数の誤りは終了コード2() {
    let (code, _, err) = run("cli-usage", &["submit", "--item", "abc"]);
    assert_eq!(code, 2);
    assert!(err.contains("--item"));
}

#[test]
fn run_設定ファイルが無ければブラウザを開かず終了コード2() {
    let dir = work_dir("cli-missing-config");
    let missing = dir.join("none.json");
    let (code, _, err) = run(
        "cli-missing-config",
        &["diff", "--config", missing.to_str().unwrap()],
    );
    assert_eq!(code, 2);
    assert!(err.contains("設定ファイルがありません"));
    // --jev なしなら、EDICTORA_JEV_API_TOKEN が無くてもトークンを求めず従来どおり進む
    assert!(!err.contains("EDICTORA_JEV_API_TOKEN"));
}

fn write_config(dir_name: &str) -> String {
    let dir = work_dir(dir_name);
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM}", "locales": {{ "ja": {{ "homepageUrl": "https://example.com/" }} }} }} ] }}"#
        ),
    )
    .unwrap();
    config.to_str().unwrap().to_string()
}

#[test]
fn run_設定に無いアイテムはブラウザを開かず終了コード2() {
    let config = write_config("cli-unknown-item");
    let (code, _, err) = run(
        "cli-unknown-item",
        &["push", "--config", &config, "--item", ITEM2, "--yes"],
    );
    assert_eq!(code, 2);
    assert!(err.contains(ITEM2));
}

#[test]
fn run_指定ロケールを持たないアイテムを明示したら終了コード2() {
    let config = write_config("cli-unknown-locale");
    let (code, _, err) = run(
        "cli-unknown-locale",
        &[
            "diff", "--config", &config, "--item", ITEM, "--locale", "fr",
        ],
    );
    assert_eq!(code, 2);
    assert!(err.contains("fr"));
}

// ───────── Rust 版で足したもの ─────────

#[test]
fn run_バージョンはパッケージの版をedictora_版の書式で出す() {
    // 版は Cargo.toml の [workspace.package] version に一元化しているので、テストに版番号を直書きしない
    let (code, output, _) = run("cli-version", &["--version"]);
    assert_eq!(code, 0);
    assert_eq!(
        output.trim(),
        format!("edictora {}", env!("CARGO_PKG_VERSION"))
    );
}

// ───────── --jev ─────────

#[test]
fn parse_jevは既定でオフでpull_diff_push_submitで使えloginでは使えない() {
    assert!(!parse(&["diff"]).unwrap().jev);
    for args in [
        vec!["pull", "--jev"],
        vec!["diff", "--jev"],
        vec!["push", "--jev", "--yes"],
        vec!["submit", "--item", ITEM, "--yes", "--jev"],
    ] {
        let cmd = parse(&args).unwrap();
        assert!(cmd.jev, "{args:?}");
        assert_eq!(cmd.jev_model_name(), "jev-latest");
    }
    assert!(usage_error(&["login", "--jev"]));
    assert!(usage_error(&["diff", "--jev=true"]));
}

#[test]
fn parse_jev_modelは任意で_jevなしや不正な名前は拒否する() {
    let cmd = parse(&["diff", "--jev", "--jev-model", "jev-2026-11"]).unwrap();
    assert_eq!(cmd.jev_model_name(), "jev-2026-11");
    let cmd = parse(&["diff", "--jev-model=jev.v2", "--jev"]).unwrap();
    assert_eq!(cmd.jev_model_name(), "jev.v2");

    assert!(usage_error(&["diff", "--jev-model", "jev-latest"]));
    assert!(usage_error(&["diff", "--jev", "--jev-model"]));
    assert!(usage_error(&["diff", "--jev", "--jev-model", "a b"]));
    assert!(usage_error(&["diff", "--jev", "--jev-model", "日本語"]));
    assert!(usage_error(&[
        "diff",
        "--jev",
        "--jev-model",
        "a",
        "--jev-model",
        "b"
    ]));
    assert!(usage_error(&["login", "--jev-model", "x"]));
}

#[test]
fn run_jevでトークンが無ければブラウザを開かず終了コード2() {
    let config = write_config("cli-jev-no-token");
    for token in [None, Some(""), Some("   ")] {
        let (code, _, err) = run_with_token(
            "cli-jev-no-token",
            &["diff", "--jev", "--config", &config],
            token,
        );
        assert_eq!(code, 2);
        assert!(err.contains("EDICTORA_JEV_API_TOKEN"), "{err}");
    }
}

#[test]
fn run_jevのトークンはエラー出力に出さない() {
    // 改行入りの不正なトークン
    let (code, _, err) = run_with_token(
        "cli-jev-bad-token",
        &["diff", "--jev"],
        Some("secret-value\nX"),
    );
    assert_eq!(code, 2);
    assert!(!err.contains("secret-value"), "{err}");

    // 正しいトークンでも、設定の誤りで止まるときに値を出さない（Chrome も開かない）
    let dir = work_dir("cli-jev-token-ok");
    let missing = dir.join("none.json");
    let (code, out, err) = run_with_token(
        "cli-jev-token-ok",
        &["diff", "--jev", "--config", missing.to_str().unwrap()],
        Some("secret-value-123"),
    );
    assert_eq!(code, 2);
    assert!(err.contains("設定ファイルがありません"), "{err}");
    assert!(!err.contains("secret-value-123") && !out.contains("secret-value-123"));
}
