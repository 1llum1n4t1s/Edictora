//! プロファイルのロック（同じ Chrome プロファイルを 2 つの edictora が同時に使わない）のテスト。
//!
//! ブラウザを開く関数は偽物（呼ばれた回数を数えて失敗する）で、Chrome は起動しない。
//! ロックファイルは test_paths の一時領域の既定プロファイルにだけ作る。
// テスト名に Chrome などの英大文字を含めるため
#![allow(non_snake_case)]

mod common;

use std::cell::RefCell;
use std::path::PathBuf;

use common::{test_paths, work_dir};
use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::cli::CliApp;
use edictora::dashboard::browser::{DashboardBrowser, DashboardBrowserOptions};
use edictora::update::lock::{LockFile, is_held_by_another, profile_lock_path};

/// 有効な設定ファイルを書く（Chrome を開く手前まで進めるため）
fn write_config(dir_name: &str) -> String {
    let config = work_dir(dir_name).join("config.json");
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

/// 既定のプロファイル（設定に profileDir が無いとき）のロックの場所
fn lock_path_of(dir_name: &str) -> PathBuf {
    profile_lock_path(&test_paths(&work_dir(dir_name)).default_profile_directory())
}

/// diff を実行する。opener が呼ばれたら、そのときプロファイルのロックが排他で持たれているかを記録する
fn run_diff(dir_name: &str, opened: &RefCell<Vec<bool>>) -> (i32, String) {
    let config = write_config(dir_name);
    let lock_path = lock_path_of(dir_name);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let opener =
        |_: &DashboardBrowserOptions, _: &CancelToken| -> edictora::Result<DashboardBrowser> {
            let held = match LockFile::try_acquire(&lock_path) {
                Err(e) => is_held_by_another(&e),
                Ok(_) => false,
            };
            opened.borrow_mut().push(held);
            Err(Error::Dashboard("テスト用: ブラウザは開かない".to_string()))
        };
    let code = CliApp::new(&mut out, &mut err, test_paths(&work_dir(dir_name)), &opener)
        .run(&["diff", "--config", &config], &CancelToken::new());
    (code, String::from_utf8(err).unwrap())
}

#[test]
fn run_別のedictoraがプロファイルを使っていればChromeを開かず終了コード1() {
    let name = "cli-profile-lock-busy";
    let lock_path = lock_path_of(name);
    let held = LockFile::try_acquire(&lock_path).unwrap();
    let opened = RefCell::new(Vec::new());
    let (code, err) = run_diff(name, &opened);
    assert_eq!(code, 1);
    assert!(err.contains("別の edictora"), "{err}");
    assert!(opened.borrow().is_empty());
    drop(held);

    // 放せば Chrome を開く手前まで進み、その間は自分が排他で持っている
    let (code, _) = run_diff(name, &opened);
    assert_eq!(code, 1);
    assert_eq!(*opened.borrow(), vec![true]);
}

#[test]
fn run_終わればプロファイルのロックを手放している() {
    let name = "cli-profile-lock-release";
    let opened = RefCell::new(Vec::new());
    run_diff(name, &opened);
    assert_eq!(opened.borrow().len(), 1);
    assert!(LockFile::try_acquire(&lock_path_of(name)).is_ok());
}

#[test]
fn run_更新の適用中はプロファイルのロックより先に止まる() {
    let name = "cli-profile-lock-applying";
    let running = test_paths(&work_dir(name)).running_lock_path();
    let held = LockFile::try_acquire(&running).unwrap();
    let opened = RefCell::new(Vec::new());
    let (code, err) = run_diff(name, &opened);
    drop(held);
    assert_eq!(code, 1);
    assert!(err.contains("更新を適用している最中"), "{err}");
    assert!(opened.borrow().is_empty());
    // プロファイルのロックファイルは作っていない
    assert!(!lock_path_of(name).exists());
}
