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

/// 本物の exe を、表示言語を環境変数で固定して起動する（保存先は作業フォルダーの下。Chrome は開かない）。
/// ライブラリのテストの言語は `.cargo/config.toml` の `EDICTORA_LANGUAGE=ja` で日本語に固定されているので、
/// ほかの言語の出力は exe を起動して確かめる
fn run_exe(dir_name: &str, lang: &str, args: &[&str]) -> (i32, String, String) {
    let dir = work_dir(dir_name);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_edictora"))
        .args(args)
        .env(edictora::i18n::LANGUAGE_ENV, lang)
        .env(edictora::app_paths::LOCAL_ROOT_ENV, dir.join("local"))
        .env(edictora::app_paths::ROAMING_ROOT_ENV, dir.join("roaming"))
        // Velopack の初回起動の印が親から漏れても、引数ありなので通常の CLI として動くが、念のため消す
        .env_remove("VELOPACK_FIRSTRUN")
        .env_remove("VELOPACK_RESTART")
        .output()
        .expect("edictora を起動できません");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn exe_ヘルプはedictora_languageがenなら英語() {
    let (code, output, _) = run_exe("cli-help-en", "en", &["--help"]);
    assert_eq!(code, 0);
    assert!(output.contains("Usage:"), "{output}");
    assert!(output.contains("Chrome Web Store listings"), "{output}");
    assert!(output.contains("edictora submit  --item <id> --yes"));
    assert!(!output.contains("使い方"), "{output}");
}

#[test]
fn exe_ヘルプはedictora_languageがjaなら日本語() {
    // CI（英語の OS）でも日本語になることを確かめる
    let (code, output, _) = run_exe("cli-help-ja", "ja", &["--help"]);
    assert_eq!(code, 0);
    assert_eq!(output.trim_end(), edictora::cli::args::help_text());
}

#[test]
fn exe_引数の誤りはedictora_languageがenなら英語で終了コード2() {
    let (code, _, err) = run_exe("cli-usage-en", "en", &["frobnicate"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("Error: Unknown command: frobnicate"),
        "{err}"
    );

    let (code, _, err) = run_exe("cli-usage-ja", "ja", &["frobnicate"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("エラー: 不明なコマンドです: frobnicate"),
        "{err}"
    );
}

/// 環境変数を明示して exe を起動する（言語の判定順の確認用。親から言語の変数が漏れないよう先に全部消す）
fn run_exe_env(dir_name: &str, envs: &[(&str, &str)], args: &[&str]) -> (i32, String, String) {
    let dir = work_dir(dir_name);
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_edictora"));
    command
        .args(args)
        .env(edictora::app_paths::LOCAL_ROOT_ENV, dir.join("local"))
        .env(edictora::app_paths::ROAMING_ROOT_ENV, dir.join("roaming"))
        .env_remove("VELOPACK_FIRSTRUN")
        .env_remove("VELOPACK_RESTART");
    for name in [
        edictora::i18n::LANGUAGE_ENV,
        edictora::i18n::LEGACY_LANGUAGE_ENV,
        "LC_ALL",
        "LC_MESSAGES",
        "LANG",
    ] {
        command.env_remove(name);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.output().expect("edictora を起動できません");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn exe_旧名のedictora_langも読み_新名が優先される() {
    // 旧名だけ: 英語
    let (_, _, err) = run_exe_env("cli-legacy-en", &[("EDICTORA_LANG", "en")], &["frobnicate"]);
    assert!(err.starts_with("Error: Unknown command"), "{err}");
    // 両方あれば EDICTORA_LANGUAGE（日本語）が優先される
    let (_, _, err) = run_exe_env(
        "cli-legacy-both",
        &[("EDICTORA_LANG", "en"), ("EDICTORA_LANGUAGE", "ja")],
        &["frobnicate"],
    );
    assert!(err.starts_with("エラー: "), "{err}");
}

#[test]
fn exe_言語の判定はedictora_languageからlc_allからlangの順で未対応は英語() {
    let unknown = |dir: &str, envs: &[(&str, &str)]| run_exe_env(dir, envs, &["frobnicate"]).2;
    // EDICTORA_LANGUAGE は LC_ALL・LANG より優先
    let err = unknown(
        "cli-order-1",
        &[("EDICTORA_LANGUAGE", "en"), ("LC_ALL", "ja_JP.UTF-8")],
    );
    assert!(err.starts_with("Error: "), "{err}");
    // LC_ALL は LANG より優先、C は未設定扱いで次へ進む
    let err = unknown("cli-order-2", &[("LC_ALL", "C"), ("LANG", "ja_JP.UTF-8")]);
    assert!(err.starts_with("エラー: "), "{err}");
    // 未対応の言語（イタリア語）は英語
    let err = unknown("cli-order-3", &[("LANG", "it_IT.UTF-8")]);
    assert!(err.starts_with("Error: "), "{err}");
}

// ───────── 画像（--only / --image-scope）─────────

#[test]
fn parse_onlyとimage_scopeを読む() {
    use edictora::cli::args::Only;
    use edictora::images::ImageScope;

    let cmd = parse(&["diff"]).unwrap();
    assert_eq!((cmd.only, cmd.image_scope), (Only::All, ImageScope::All));

    let cmd = parse(&["push", "--only", "images", "--image-scope=global", "--yes"]).unwrap();
    assert_eq!(
        (cmd.only, cmd.image_scope),
        (Only::Images, ImageScope::Global)
    );
    assert!(cmd.yes);

    let cmd = parse(&["pull", "--only=TEXT", "--image-scope", "Localized"]).unwrap();
    assert_eq!(
        (cmd.only, cmd.image_scope),
        (Only::Text, ImageScope::Localized)
    );
    assert!(Only::All.text() && Only::All.images() && Only::All.privacy());
    assert!(Only::Text.text() && !Only::Text.images() && !Only::Text.privacy());
    assert!(!Only::Images.text() && Only::Images.images() && !Only::Images.privacy());
    assert!(!Only::Privacy.text() && !Only::Privacy.images() && Only::Privacy.privacy());
}

#[test]
fn parse_onlyとimage_scopeの誤りは使い方の誤り() {
    assert!(usage_error(&["diff", "--only", "image"]));
    assert!(usage_error(&["diff", "--only"]));
    assert!(usage_error(&["diff", "--image-scope", "everything"]));
    assert!(usage_error(&["diff", "--only", "text", "--only", "images"]));
    assert!(usage_error(&[
        "diff",
        "--image-scope",
        "all",
        "--image-scope",
        "global"
    ]));
    // 対象を持たないコマンドでは使えない（submit は画像を触らない）
    assert!(usage_error(&[
        "submit", "--item", ITEM, "--yes", "--only", "images"
    ]));
    assert!(usage_error(&["login", "--only", "images"]));
    assert!(usage_error(&["login", "--image-scope", "global"]));
}

#[test]
fn run_画像の設定の誤りはブラウザを開かず終了コード2() {
    let dir = work_dir("cli-images-config-error");
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM}", "baseDir": "{}", "images": {{ "marquee": "none.png" }} }} ] }}"#,
            dir.to_str().unwrap().replace('\\', "/")
        ),
    )
    .unwrap();
    let (code, _, err) = run(
        "cli-images-config-error",
        &[
            "diff",
            "--config",
            config.to_str().unwrap(),
            "--only",
            "images",
        ],
    );
    assert_eq!(code, 2);
    assert!(err.contains("none.png"), "{err}");

    // 規格外の画像（PNG でない）も Chrome を開く前に止まる
    let bad = dir.join("marquee.png");
    std::fs::write(&bad, b"not a png at all, just text").unwrap();
    std::fs::write(
        &config,
        format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM}", "baseDir": "{}", "images": {{ "marquee": "marquee.png" }} }} ] }}"#,
            dir.to_str().unwrap().replace('\\', "/")
        ),
    )
    .unwrap();
    let (code, _, err) = run(
        "cli-images-config-error",
        &["push", "--config", config.to_str().unwrap(), "--yes"],
    );
    assert_eq!(code, 2);
    assert!(err.contains("PNG"), "{err}");
}

#[test]
fn run_対象が無い組み合わせはブラウザを開かず終了コード2() {
    let config = write_config("cli-images-nothing");
    // テキストだけのアイテムに --only images
    let (code, _, err) = run(
        "cli-images-nothing",
        &[
            "diff", "--config", &config, "--item", ITEM, "--only", "images",
        ],
    );
    assert_eq!(code, 2);
    assert!(err.contains(ITEM), "{err}");
}

#[test]
fn ヘルプにonlyとimage_scopeがある() {
    let (code, output, _) = run("cli-help-images", &["--help"]);
    assert_eq!(code, 0);
    assert!(output.contains("--only"));
    assert!(output.contains("--image-scope"));
}

#[test]
fn only_privacyを受け付ける() {
    use edictora::cli::args::Only;

    let cmd = parse(&["diff", "--only", "privacy"]).unwrap();
    assert_eq!(cmd.only, Only::Privacy);
    let cmd = parse(&["push", "--only=PRIVACY", "--yes"]).unwrap();
    assert_eq!(cmd.only, Only::Privacy);
    // 値の誤りは使い方の誤り
    assert!(parse(&["diff", "--only", "perms"]).is_err());
    // submit には --only は無い
    assert!(
        parse(&[
            "submit",
            "--item",
            "abcdefghijklmnopabcdefghijklmnop",
            "--yes",
            "--only",
            "privacy"
        ])
        .is_err()
    );
}
