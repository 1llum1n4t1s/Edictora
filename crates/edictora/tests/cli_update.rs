//! `edictora update` のテスト。
//!
//! 更新処理は偽物を差し込み、本物の配信元・Update.exe には届かない。CliApp の既定（`with_update` を
//! 呼ばない状態）は refuse_update で、テストからは構造的に本物の更新を呼べない。
//! ブラウザを開く関数には「呼ばれたら失敗する」ものを渡す（update が Chrome を開かないことを確かめる）。
// テスト名に Chrome などの英大文字を含めるため
#![allow(non_snake_case)]

mod common;

use std::cell::RefCell;
use std::path::Path;

use common::{test_paths, work_dir};
use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::cli::args::{CliCommandKind, parse};
use edictora::cli::{BrowserOpener, CliApp, UpdateRunner};
use edictora::dashboard::browser::{DashboardBrowser, DashboardBrowserOptions};
use edictora::update::UpdateOutcome;
use edictora::update::lock::{LockFile, is_held_by_another};
use edictora::update::source::UpdateSource;
use edictora::update::version::Version;

fn no_browser(_: &DashboardBrowserOptions, _: &CancelToken) -> edictora::Result<DashboardBrowser> {
    panic!("update では Chrome を開いてはいけません");
}

/// CliApp を実行し、(終了コード, 標準出力, 標準エラー) を返す
fn run(
    dir_name: &str,
    args: &[&str],
    runner: Option<&UpdateRunner>,
    source_override: Option<&str>,
    cancel: &CancelToken,
) -> (i32, String, String) {
    let dir = work_dir(dir_name);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let opener: &BrowserOpener = &no_browser;
    let mut app = CliApp::new(&mut out, &mut err, test_paths(&dir), opener);
    if let Some(runner) = runner {
        app = app.with_update(runner, source_override.map(str::to_string));
    }
    let code = app.run(args, cancel);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn parse_updateはオプションを取らない() {
    assert_eq!(parse(&["update"]).unwrap().kind, CliCommandKind::Update);
    assert_eq!(parse(&["UPDATE"]).unwrap().kind, CliCommandKind::Update);
    for args in [
        &["update", "--yes"][..],
        &["update", "--config", "c.json"],
        &["update", "x"],
    ] {
        assert!(matches!(parse(args), Err(Error::Usage(_))), "{args:?}");
    }
    // --help はどこにあっても使い方
    assert_eq!(
        parse(&["update", "--help"]).unwrap().kind,
        CliCommandKind::Help
    );
}

#[test]
fn run_ヘルプにupdateの説明がある() {
    let (code, output, _) = run(
        "cli-update-help",
        &["--help"],
        None,
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 0);
    assert!(output.contains("edictora update"));
    assert!(output.contains("edictora.kagayoi.com"));
    assert!(output.contains(r"%LOCALAPPDATA%\Edictora.Data\chrome-profile"));
}

#[test]
fn run_既定では本物の更新に届かず終了コード1() {
    let (code, out, err) = run(
        "cli-update-refuse",
        &["update"],
        None,
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("更新処理を使えません"), "{err}");
}

#[test]
fn run_updateは設定を読まずに更新処理を呼び_結果を標準エラーへ出す() {
    let seen = RefCell::new(Vec::new());
    let runner = |source: &UpdateSource,
                  _: &Path,
                  progress: &dyn Fn(&str),
                  _: &CancelToken|
     -> edictora::Result<UpdateOutcome> {
        seen.borrow_mut().push(source.display_url());
        progress("更新を確認しています");
        Ok(UpdateOutcome::UpToDate {
            current: v("1.0.6"),
            latest: v("1.0.6"),
        })
    };
    // 設定ファイルが無い作業フォルダーでも成功する（update は config を読まない）
    let (code, out, err) = run(
        "cli-update-uptodate",
        &["update"],
        Some(&runner as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 0);
    assert!(out.is_empty(), "結果は標準出力に出さない: {out}");
    assert!(err.contains("更新を確認しています"));
    assert!(err.contains("最新です"), "{err}");
    // 差し替えが無ければ本番の配信元
    assert_eq!(
        *seen.borrow(),
        vec!["https://edictora.kagayoi.com".to_string()]
    );
}

#[test]
fn run_更新を始めたら0_別の更新処理が動いていれば1() {
    let started = |_: &UpdateSource,
                   _: &Path,
                   _: &dyn Fn(&str),
                   _: &CancelToken|
     -> edictora::Result<UpdateOutcome> {
        Ok(UpdateOutcome::ApplyStarted {
            from: v("1.0.5"),
            to: v("1.0.6"),
            package: "Edictora-1.0.6-full.nupkg".into(),
        })
    };
    let (code, _, err) = run(
        "cli-update-started",
        &["update"],
        Some(&started as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 0);
    assert!(err.contains("1.0.5 → 1.0.6"), "{err}");

    let busy = |_: &UpdateSource,
                _: &Path,
                _: &dyn Fn(&str),
                _: &CancelToken|
     -> edictora::Result<UpdateOutcome> { Ok(UpdateOutcome::Busy) };
    let (code, _, err) = run(
        "cli-update-busy",
        &["update"],
        Some(&busy as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 1);
    assert!(err.contains("別の更新処理"));
}

#[test]
fn run_更新の失敗は1_中断は130() {
    let failing = |_: &UpdateSource,
                   _: &Path,
                   _: &dyn Fn(&str),
                   _: &CancelToken|
     -> edictora::Result<UpdateOutcome> {
        Err(Error::Update("検証に失敗しました".to_string()))
    };
    let (code, _, err) = run(
        "cli-update-fail",
        &["update"],
        Some(&failing as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 1);
    assert!(err.contains("エラー: 検証に失敗しました"));

    let cancelled = |_: &UpdateSource,
                     _: &Path,
                     _: &dyn Fn(&str),
                     _: &CancelToken|
     -> edictora::Result<UpdateOutcome> { Err(Error::Cancelled) };
    let (code, _, err) = run(
        "cli-update-cancel",
        &["update"],
        Some(&cancelled as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 130);
    assert!(err.contains("中断しました"));
}

#[test]
fn run_配信元の差し替えは検査し_誤りなら更新処理を呼ばず終了コード2() {
    let called = RefCell::new(0);
    let runner = |source: &UpdateSource,
                  _: &Path,
                  _: &dyn Fn(&str),
                  _: &CancelToken|
     -> edictora::Result<UpdateOutcome> {
        *called.borrow_mut() += 1;
        assert_eq!(source.display_url(), "http://127.0.0.1:8080");
        Ok(UpdateOutcome::Busy)
    };
    let (code, _, err) = run(
        "cli-update-badurl",
        &["update"],
        Some(&runner as &UpdateRunner),
        Some("http://example.com"),
        &CancelToken::new(),
    );
    assert_eq!(code, 2);
    assert!(err.contains("EDICTORA_UPDATE_URL"), "{err}");
    assert_eq!(*called.borrow(), 0);

    // 127.0.0.1 の平文は許す
    run(
        "cli-update-loopback",
        &["update"],
        Some(&runner as &UpdateRunner),
        Some("http://127.0.0.1:8080/"),
        &CancelToken::new(),
    );
    assert_eq!(*called.borrow(), 1);

    // オプションの誤りも 2（更新処理は呼ばない）
    let (code, _, _) = run(
        "cli-update-usage",
        &["update", "--yes"],
        Some(&runner as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 2);
    assert_eq!(*called.borrow(), 1);
}

// ───────── 実行中の印（更新の適用との排他）─────────

/// 有効な設定ファイルを書く（Chrome を開く手前まで進めるため）
fn write_config(dir_name: &str) -> String {
    let dir = work_dir(dir_name);
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{ "items": [ {{ "itemId": "{}", "locales": {{ "ja": {{ "homepageUrl": "https://example.com/" }} }} }} ] }}"#,
            common::ITEM_A
        ),
    )
    .unwrap();
    config.to_str().unwrap().to_string()
}

/// ブラウザを開く代わりに、呼ばれたことを記録して失敗する
fn run_diff(dir_name: &str, opened: &RefCell<u32>) -> (i32, String) {
    let config = write_config(dir_name);
    let dir = work_dir(dir_name);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let opener =
        |_: &DashboardBrowserOptions, _: &CancelToken| -> edictora::Result<DashboardBrowser> {
            *opened.borrow_mut() += 1;
            Err(Error::Dashboard("テスト用: ブラウザは開かない".to_string()))
        };
    let code = CliApp::new(&mut out, &mut err, test_paths(&dir), &opener)
        .run(&["diff", "--config", &config], &CancelToken::new());
    (code, String::from_utf8(err).unwrap())
}

#[test]
fn run_更新の適用中ならChromeを開かず終了コード1() {
    let name = "cli-running-applying";
    let lock_path = test_paths(&work_dir(name)).running_lock_path();
    // 更新側が適用の直前に排他で持っている状態
    let held = LockFile::try_acquire(&lock_path).unwrap();
    let opened = RefCell::new(0);
    let (code, err) = run_diff(name, &opened);
    assert_eq!(code, 1);
    assert!(err.contains("更新を適用している最中"), "{err}");
    assert_eq!(*opened.borrow(), 0);
    drop(held);

    // 排他が外れれば Chrome を開く手前まで進む（ここでは偽物が失敗する）
    let (code, _) = run_diff(name, &opened);
    assert_eq!(code, 1);
    assert_eq!(*opened.borrow(), 1);
}

#[test]
fn run_Chromeを開くコマンドは共有ロックを持ち_同士は同時に動ける() {
    let name = "cli-running-shared";
    let lock_path = test_paths(&work_dir(name)).running_lock_path();
    // 別の CLI が実行中（共有で持っている）でも動ける
    let other = LockFile::try_acquire_shared(&lock_path).unwrap();
    let opened = RefCell::new(0);
    let seen_exclusive_blocked = RefCell::new(None);
    {
        let config = write_config(name);
        let dir = work_dir(name);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let opener = |_: &DashboardBrowserOptions,
                      _: &CancelToken|
         -> edictora::Result<DashboardBrowser> {
            *opened.borrow_mut() += 1;
            // 実行中は更新側が排他で取れない
            *seen_exclusive_blocked.borrow_mut() = Some(match LockFile::try_acquire(&lock_path) {
                Err(e) => is_held_by_another(&e),
                Ok(_) => false,
            });
            Err(Error::Dashboard("テスト用".to_string()))
        };
        CliApp::new(&mut out, &mut err, test_paths(&dir), &opener)
            .run(&["diff", "--config", &config], &CancelToken::new());
    }
    assert_eq!(*opened.borrow(), 1);
    assert_eq!(*seen_exclusive_blocked.borrow(), Some(true));
    drop(other);
    // 終われば手放している（更新側が排他で取れる）
    assert!(LockFile::try_acquire(&lock_path).is_ok());
}

#[test]
fn run_updateは実行中の印を持たず_適用を後に回したら終了コード1() {
    let name = "cli-update-inuse";
    let expected = test_paths(&work_dir(name)).running_lock_path();
    let runner = |_: &UpdateSource,
                  lock: &Path,
                  _: &dyn Fn(&str),
                  _: &CancelToken|
     -> edictora::Result<UpdateOutcome> {
        assert_eq!(lock, expected.as_path());
        // update 自身は共有ロックを持っていない（持っていたら自分の適用を妨げる）
        assert!(LockFile::try_acquire(lock).is_ok());
        Ok(UpdateOutcome::AppInUse {
            from: v("1.0.5"),
            to: v("1.0.6"),
        })
    };
    let (code, _, err) = run(
        name,
        &["update"],
        Some(&runner as &UpdateRunner),
        None,
        &CancelToken::new(),
    );
    assert_eq!(code, 1);
    assert!(err.contains("実行中の edictora"), "{err}");
}
