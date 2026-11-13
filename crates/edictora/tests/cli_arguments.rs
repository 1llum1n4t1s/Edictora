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
    // 未知のオプション（廃止したオプションも同じ扱い）は不明として止める
    assert!(usage_error(&["diff", "--unknown-option"]));
    assert!(usage_error(&["diff", "--unknown-option=value"]));
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

/// CliApp を実行し、(終了コード, 標準出力, 標準エラー) を返す
fn run(dir_name: &str, args: &[&str]) -> (i32, String, String) {
    let dir = work_dir(dir_name);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let opener: &BrowserOpener = &no_browser;
    let code =
        CliApp::new(&mut out, &mut err, test_paths(&dir), opener).run(args, &CancelToken::new());
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn run_ヘルプは成功で使い方を出す() {
    let (code, output, _) = run("cli-help", &["--help"]);
    assert_eq!(code, 0);
    assert!(output.contains("edictora submit  --item <id> --yes"));
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

#[test]
fn run_未知のオプションはブラウザを開かず終了コード2() {
    // 廃止したオプションを指定しても不明として止まる（Chrome は開かない）
    let (code, _, err) = run("cli-unknown-option", &["diff", "--unknown-option"]);
    assert_eq!(code, 2);
    assert!(err.contains("--unknown-option"), "{err}");
}
