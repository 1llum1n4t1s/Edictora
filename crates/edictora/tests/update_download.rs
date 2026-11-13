//! 自動更新の通信・ダウンロード・検証・適用の起動のテスト。
//!
//! 本物の配信元（edictora.kagayoi.com）には繋がず、127.0.0.1 の偽 HTTP サーバー（平文）だけを相手にする。
//! インストール先は target/tmp の下に作った偽物（ダミーの Update.exe と sq.version）で、Update.exe は起動せず、
//! 起動の引数を記録するだけの偽の [`ApplyLauncher`] を使う。
// テスト名に SHA256・HTTP などの英大文字を含めるため
#![allow(non_snake_case)]
#![cfg(windows)]

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use common::work_dir;
use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::update::apply::{ApplyLauncher, apply_args};
use edictora::update::download::{VerifyError, verify_package};
use edictora::update::feed::FeedAsset;
use edictora::update::hash::{bytes_digests, file_digests};
use edictora::update::locate::{Installation, LocateError, locate_from_exe, locate_from_root};
use edictora::update::lock::{LockFile, RUNNING_LOCK_FILE, UPDATE_LOCK_FILE};
use edictora::update::source::UpdateSource;
use edictora::update::{UpdateOutcome, run_update};
use sha2::{Digest, Sha256};

// ───────── 補助 ─────────

/// 実行ごとに別の作業フォルダー（前回の残りに影響されない。片付けは cargo clean に任せる）
fn fresh_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = work_dir(name).join(format!("{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 偽サーバーの応答 1 つ
#[derive(Clone)]
struct Route {
    status: &'static str,
    body: Vec<u8>,
}

fn ok(body: impl Into<Vec<u8>>) -> Route {
    Route {
        status: "200 OK",
        body: body.into(),
    }
}

/// 何度でも応答する偽サーバー。受け取った要求行を記録する
struct FakeServer {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeServer {
    fn start(routes: HashMap<String, Route>) -> FakeServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line.trim_end().is_empty() {
                        break;
                    }
                }
                let request_line = request_line.trim_end().to_string();
                log.lock().unwrap().push(request_line.clone());
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let route = routes.get(&path).cloned().unwrap_or(Route {
                    status: "404 Not Found",
                    body: b"not found".to_vec(),
                });
                let mut head = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    route.status,
                    route.body.len()
                );
                if route.status.starts_with('3') {
                    head.push_str("Location: http://127.0.0.1:1/elsewhere\r\n");
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&route.body);
                let _ = stream.flush();
            }
        });
        FakeServer { port, requests }
    }

    fn source(&self) -> UpdateSource {
        UpdateSource::parse(&format!("http://127.0.0.1:{}/base/", self.port)).unwrap()
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

/// Update.exe の起動を記録するだけの偽物
#[derive(Default)]
struct RecordingLauncher {
    calls: RefCell<Vec<(PathBuf, Vec<OsString>)>>,
}

impl ApplyLauncher for RecordingLauncher {
    fn launch(&self, update_exe: &Path, args: &[OsString]) -> edictora::Result<()> {
        self.calls
            .borrow_mut()
            .push((update_exe.to_path_buf(), args.to_vec()));
        Ok(())
    }
}

fn sq_version(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2010/07/nuspec.xsd">
  <metadata>
    <id>Edictora</id>
    <version>{version}</version>
    <title>Edictora</title>
    <authors>Kagayoi</authors>
    <mainExe>edictora.exe</mainExe>
    <channel>win</channel>
  </metadata>
</package>"#
    )
}

/// 偽のインストール先（`<dir>\Edictora` を根にする）
fn fake_install(dir: &Path, version: &str) -> Installation {
    let root = dir.join("Edictora");
    std::fs::create_dir_all(root.join("current")).unwrap();
    std::fs::create_dir_all(root.join("packages")).unwrap();
    std::fs::write(root.join("Update.exe"), b"dummy").unwrap();
    std::fs::write(root.join("current").join("edictora.exe"), b"dummy").unwrap();
    std::fs::write(root.join("current").join("sq.version"), sq_version(version)).unwrap();
    locate_from_exe(&root.join("current").join("edictora.exe")).unwrap()
}

/// パッケージの中身（中身は何でもよい。zip である必要はない）と、それを指すフィード
fn package_and_feed(version: &str, body: &[u8], tweak: impl Fn(&mut FeedAsset)) -> String {
    let d = bytes_digests(body).unwrap();
    let mut asset = FeedAsset {
        package_id: "Edictora".to_string(),
        version: version.to_string(),
        kind: "Full".to_string(),
        file_name: format!("Edictora-{version}-full.nupkg"),
        sha1: d.sha1,
        sha256: d.sha256,
        size: body.len() as u64,
    };
    tweak(&mut asset);
    serde_json::json!({
        "Assets": [{
            "PackageId": asset.package_id,
            "Version": asset.version,
            "Type": asset.kind,
            "FileName": asset.file_name,
            "SHA1": asset.sha1,
            "SHA256": asset.sha256,
            "Size": asset.size,
            "NotesMarkdown": "",
            "NotesHtml": ""
        }]
    })
    .to_string()
}

fn package_body() -> Vec<u8> {
    // 読み取りの区切り（256 KiB）をまたぐ大きさにする
    (0..300_000u32).map(|i| (i % 251) as u8).collect()
}

fn routes(feed: String, package: Option<(&str, Vec<u8>)>) -> HashMap<String, Route> {
    let mut map = HashMap::new();
    map.insert("/base/releases.win.json".to_string(), ok(feed));
    if let Some((name, body)) = package {
        map.insert(format!("/base/{name}"), ok(body));
    }
    map
}

fn quiet(_: &str) {}

/// 実行中の edictora を知らせるロック（データの根の代わりに作業フォルダーの下）
fn running(dir: &Path) -> PathBuf {
    dir.join("data").join(RUNNING_LOCK_FILE)
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ───────── ハッシュ ─────────

#[test]
fn ハッシュは既知の値とsha2に一致する() {
    // SDK のテスト（misc.rs）と同じ「hello world」の値（大文字の 16 進）
    let d = bytes_digests(b"hello world").unwrap();
    assert_eq!(d.sha1, "2AAE6C35C94FCFB415DBE95F408B9CE91EE846ED");
    assert_eq!(
        d.sha256,
        "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9"
    );
    let empty = bytes_digests(b"").unwrap();
    assert_eq!(empty.sha1, "DA39A3EE5E6B4B0D3255BFEF95601890AFD80709");

    // 読み取りの区切り（64 KiB）をまたぐファイルでも sha2 と同じ
    let dir = fresh_dir("update-hash");
    let body = package_body();
    let file = dir.join("data.bin");
    std::fs::write(&file, &body).unwrap();
    let expected: String = Sha256::digest(&body)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect();
    assert_eq!(file_digests(&file).unwrap().sha256, expected);
}

#[test]
fn 検証はサイズ_SHA256_無ければSHA1の順() {
    let dir = fresh_dir("update-verify");
    let body = b"package".to_vec();
    let file = dir.join("p.nupkg");
    std::fs::write(&file, &body).unwrap();
    let d = bytes_digests(&body).unwrap();
    let asset = FeedAsset {
        size: body.len() as u64,
        sha1: d.sha1.clone(),
        sha256: d.sha256.to_lowercase(),
        ..FeedAsset::default()
    };
    // 大文字小文字は区別しない
    assert_eq!(verify_package(&file, &asset), Ok(()));
    assert_eq!(
        verify_package(
            &file,
            &FeedAsset {
                size: 1,
                ..asset.clone()
            }
        ),
        Err(VerifyError::Size {
            expected: 1,
            actual: 7
        })
    );
    // SHA256 があれば SHA1 が合っていても SHA256 で判定する
    assert_eq!(
        verify_package(
            &file,
            &FeedAsset {
                sha256: "00".repeat(32),
                ..asset.clone()
            }
        ),
        Err(VerifyError::Checksum {
            algorithm: "SHA256"
        })
    );
    // SHA256 が無ければ SHA1
    assert_eq!(
        verify_package(
            &file,
            &FeedAsset {
                sha256: String::new(),
                ..asset.clone()
            }
        ),
        Ok(())
    );
    // 両方とも無ければ失敗（SHA1 の空文字と比べる）
    assert_eq!(
        verify_package(
            &file,
            &FeedAsset {
                sha256: String::new(),
                sha1: String::new(),
                ..asset.clone()
            }
        ),
        Err(VerifyError::Checksum { algorithm: "SHA1" })
    );
}

// ───────── インストール先 ─────────

#[test]
fn インストール先はUpdate_exeとsq_versionで判定する() {
    let dir = fresh_dir("update-locate");
    // Update.exe が無ければインストールされていない（cargo run と同じ状況）
    let exe = dir.join("Edictora").join("current").join("edictora.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, b"dummy").unwrap();
    assert!(matches!(
        locate_from_exe(&exe),
        Err(LocateError::NotInstalled(_))
    ));
    // Update.exe だけでマニフェストが無い
    std::fs::write(dir.join("Edictora").join("Update.exe"), b"dummy").unwrap();
    assert!(matches!(
        locate_from_exe(&exe),
        Err(LocateError::NotInstalled(_))
    ));
    // マニフェストが壊れている
    std::fs::write(exe.with_file_name("sq.version"), "<package/>").unwrap();
    assert!(matches!(
        locate_from_exe(&exe),
        Err(LocateError::InvalidManifest(_))
    ));
    // 正しいマニフェスト
    std::fs::write(exe.with_file_name("sq.version"), sq_version("1.0.5")).unwrap();
    let inst = locate_from_exe(&exe).unwrap();
    let root = dir.join("Edictora");
    assert_eq!(inst.root, root);
    assert_eq!(inst.update_exe, root.join("Update.exe"));
    assert_eq!(inst.packages_dir, root.join("packages"));
    assert_eq!(inst.current_dir, root.join("current"));
    assert_eq!(inst.manifest.version.to_string(), "1.0.5");
    assert_eq!(locate_from_root(&root).unwrap(), inst);
    // current の下の深い場所にある exe からも根を見つける
    let nested = root.join("current").join("sub").join("x.exe");
    assert_eq!(locate_from_exe(&nested).unwrap().root, root);
}

// ───────── 更新の全体 ─────────

#[test]
fn 新しい版をダウンロードして検証し_Update_exeにapplyを任せる() {
    let dir = fresh_dir("update-apply");
    let inst = fake_install(&dir, "1.0.5");
    // ダウンロード前からある古いパッケージ・途中のファイル・差分は成功後に片付く
    std::fs::write(inst.packages_dir.join("Edictora-1.0.5-full.nupkg"), b"old").unwrap();
    std::fs::write(inst.packages_dir.join("Edictora-1.0.4-full.partial"), b"x").unwrap();
    std::fs::write(inst.packages_dir.join("Edictora-1.0.5-delta.nupkg"), b"d").unwrap();

    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |_| {}),
        Some((name, body.clone())),
    ));
    let launcher = RecordingLauncher::default();
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        4242,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();

    let package = inst.packages_dir.join(name);
    match &outcome {
        UpdateOutcome::ApplyStarted {
            from,
            to,
            package: p,
        } => {
            assert_eq!(from.to_string(), "1.0.5");
            assert_eq!(to.to_string(), "1.0.6");
            assert_eq!(p, &package);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read(&package).unwrap(), body);
    // ロックファイルは残る（SDK と同じく消さない）。パッケージは新しいものだけ
    assert_eq!(
        files_in(&inst.packages_dir),
        vec![
            ".edictora-update.lock".to_string(),
            ".velopack_lock".to_string(),
            name.to_string()
        ]
    );

    // Update.exe の引数は SDK（manager.rs の unsafe_apply_updates）と同じ順
    let calls = launcher.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, inst.update_exe);
    let expected: Vec<OsString> = [
        "apply".into(),
        "--package".into(),
        package.clone().into_os_string(),
        "--waitPid".into(),
        "4242".into(),
        "--silent".into(),
        "--norestart".into(),
        "--root".into(),
        inst.root.clone().into_os_string(),
        "--packageDir".into(),
        inst.packages_dir.clone().into_os_string(),
    ]
    .to_vec();
    assert_eq!(calls[0].1, expected);
    assert_eq!(apply_args(&inst, &package, 4242), expected);

    // フィードの問い合わせ文字列は版と id だけ（stagingId は送らない）
    let requests = server.requests();
    assert_eq!(
        requests,
        vec![
            "GET /base/releases.win.json?localVersion=1.0.5&id=Edictora HTTP/1.1".to_string(),
            format!("GET /base/{name} HTTP/1.1"),
        ]
    );
}

#[test]
fn 最新なら何もダウンロードせず適用もしない() {
    let dir = fresh_dir("update-uptodate");
    let inst = fake_install(&dir, "1.0.6");
    let server = FakeServer::start(routes(package_and_feed("1.0.6", b"x", |_| {}), None));
    let launcher = RecordingLauncher::default();
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(
        matches!(outcome, UpdateOutcome::UpToDate { .. }),
        "{outcome:?}"
    );
    assert!(launcher.calls.borrow().is_empty());
    assert_eq!(server.requests().len(), 1);

    // 完全パッケージが無いフィード
    let server = FakeServer::start(routes(r#"{"Assets":[]}"#.to_string(), None));
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(outcome, UpdateOutcome::RemoteIsEmpty { .. }));
    assert!(launcher.calls.borrow().is_empty());
}

/// 失敗したとき、パッケージ置き場に新しいファイルが残らず、古いパッケージは消えず、適用も起動しない
fn assert_failed_without_side_effects(
    result: edictora::Result<UpdateOutcome>,
    inst: &Installation,
    launcher: &RecordingLauncher,
    expected: &str,
) {
    match result {
        Err(Error::Update(m)) => assert!(m.contains(expected), "{m}"),
        other => panic!("Update エラーを期待しました: {other:?}"),
    }
    assert!(launcher.calls.borrow().is_empty());
    let files = files_in(&inst.packages_dir);
    assert!(
        files.contains(&"Edictora-1.0.5-full.nupkg".to_string()),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|f| f.contains("1.0.6")),
        "新しい版のファイルが残っている: {files:?}"
    );
}

#[test]
fn ハッシュが合わなければ適用しない() {
    let dir = fresh_dir("update-badhash");
    let inst = fake_install(&dir, "1.0.5");
    std::fs::write(inst.packages_dir.join("Edictora-1.0.5-full.nupkg"), b"old").unwrap();
    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |a| a.sha256 = "AB".repeat(32)),
        Some((name, body.clone())),
    ));
    let launcher = RecordingLauncher::default();
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "SHA256");
}

#[test]
fn SHA256が無ければSHA1で検証して適用する() {
    let dir = fresh_dir("update-sha1");
    let inst = fake_install(&dir, "1.0.5");
    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |a| a.sha256.clear()),
        Some((name, body.clone())),
    ));
    let launcher = RecordingLauncher::default();
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(outcome, UpdateOutcome::ApplyStarted { .. }));
    assert_eq!(launcher.calls.borrow().len(), 1);
}

#[test]
fn 宣言のサイズより大きい応答は打ち切って適用しない() {
    let dir = fresh_dir("update-oversize");
    let inst = fake_install(&dir, "1.0.5");
    std::fs::write(inst.packages_dir.join("Edictora-1.0.5-full.nupkg"), b"old").unwrap();
    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |a| a.size = 1000),
        Some((name, body.clone())),
    ));
    let launcher = RecordingLauncher::default();
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "1000");
}

#[test]
fn パッケージやフィードが取れなければエラーにする() {
    let dir = fresh_dir("update-404");
    let inst = fake_install(&dir, "1.0.5");
    std::fs::write(inst.packages_dir.join("Edictora-1.0.5-full.nupkg"), b"old").unwrap();
    let launcher = RecordingLauncher::default();

    // パッケージが 404
    let server = FakeServer::start(routes(package_and_feed("1.0.6", b"abc", |_| {}), None));
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "HTTP 404");

    // フィードが 500
    let mut map = HashMap::new();
    map.insert(
        "/base/releases.win.json".to_string(),
        Route {
            status: "500 Internal Server Error",
            body: b"oops".to_vec(),
        },
    );
    let server = FakeServer::start(map);
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "HTTP 500");

    // フィードがリダイレクト（追わない）
    let mut map = HashMap::new();
    map.insert(
        "/base/releases.win.json".to_string(),
        Route {
            status: "302 Found",
            body: Vec::new(),
        },
    );
    let server = FakeServer::start(map);
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "HTTP 302");
    assert_eq!(server.requests().len(), 1);

    // フィードが JSON でない
    let server = FakeServer::start(routes("<html>".to_string(), None));
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "JSON");

    // フィードのファイル名が素のファイル名でない
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", b"abc", |a| {
            a.file_name = "../evil.nupkg".to_string()
        }),
        None,
    ));
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    );
    assert_failed_without_side_effects(r, &inst, &launcher, "ファイル名");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn ダウンロード済みで検証が通ればそれを使い_壊れていれば取り直す() {
    let dir = fresh_dir("update-existing");
    let inst = fake_install(&dir, "1.0.5");
    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let launcher = RecordingLauncher::default();

    // 正しいものが既にある → 配信元からは取らない（パッケージの経路が無くても成功する）
    std::fs::write(inst.packages_dir.join(name), &body).unwrap();
    let server = FakeServer::start(routes(package_and_feed("1.0.6", &body, |_| {}), None));
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(outcome, UpdateOutcome::ApplyStarted { .. }));
    assert_eq!(server.requests().len(), 1);

    // 1 回目の適用で実行中ロックは（本物ならプロセス終了まで）持ったままなので、
    // 2 回目は別のプロセスとみなして別の場所を使う
    // 壊れたものがある → 取り直す
    std::fs::write(inst.packages_dir.join(name), b"broken").unwrap();
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |_| {}),
        Some((name, body.clone())),
    ));
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir.join("second")),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(outcome, UpdateOutcome::ApplyStarted { .. }));
    assert_eq!(server.requests().len(), 2);
    assert_eq!(std::fs::read(inst.packages_dir.join(name)).unwrap(), body);
}

#[test]
fn 別の更新処理が動いていれば何もしない() {
    let dir = fresh_dir("update-busy");
    let inst = fake_install(&dir, "1.0.5");
    let _held = LockFile::try_acquire(&inst.packages_dir.join(UPDATE_LOCK_FILE)).unwrap();
    let server = FakeServer::start(routes(package_and_feed("1.0.6", b"x", |_| {}), None));
    let launcher = RecordingLauncher::default();
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(outcome, UpdateOutcome::Busy);
    assert!(server.requests().is_empty());
    assert!(launcher.calls.borrow().is_empty());
    // 同じロックは同時に 2 つ取れない
    assert!(LockFile::try_acquire(&inst.packages_dir.join(UPDATE_LOCK_FILE)).is_err());
}

#[test]
fn 中断されていればCancelledで抜ける() {
    let dir = fresh_dir("update-cancel");
    let inst = fake_install(&dir, "1.0.5");
    let server = FakeServer::start(routes(package_and_feed("1.0.6", b"x", |_| {}), None));
    let cancel = CancelToken::new();
    cancel.cancel();
    let launcher = RecordingLauncher::default();
    let r = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &cancel,
    );
    assert_eq!(r.unwrap_err(), Error::Cancelled);
    assert!(launcher.calls.borrow().is_empty());
}

#[test]
fn ロックを開けない恒常的な失敗はBusyにせずエラーにする() {
    let dir = fresh_dir("update-lockfail");
    let inst = fake_install(&dir, "1.0.5");
    // ロックファイルの場所にフォルダーがあると開けない（共有違反ではない）
    std::fs::create_dir_all(inst.packages_dir.join(UPDATE_LOCK_FILE)).unwrap();
    let server = FakeServer::start(routes(package_and_feed("1.0.6", b"x", |_| {}), None));
    let launcher = RecordingLauncher::default();
    match run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    ) {
        Err(Error::Update(m)) => assert!(m.contains("ロックファイル"), "{m}"),
        other => panic!("Update エラーを期待しました: {other:?}"),
    }
    assert!(server.requests().is_empty());
    assert!(launcher.calls.borrow().is_empty());
}

#[test]
fn Portable版の目印は根の_portable() {
    use edictora::install::hooks::is_portable;
    let dir = fresh_dir("update-portable");
    let inst = fake_install(&dir, "1.0.5");
    assert!(!is_portable(&inst.root));
    std::fs::write(inst.root.join(".portable"), b"").unwrap();
    assert!(is_portable(&inst.root));
}

#[test]
fn 実行中ロックは共有同士は同時に取れ_排他とは両立しない() {
    let dir = fresh_dir("update-running-lock");
    let path = running(&dir);
    let a = LockFile::try_acquire_shared(&path).unwrap();
    let b = LockFile::try_acquire_shared(&path).unwrap();
    let e = LockFile::try_acquire(&path).unwrap_err();
    assert!(edictora::update::lock::is_held_by_another(&e), "{e:?}");
    drop((a, b));
    let x = LockFile::try_acquire(&path).unwrap();
    let e = LockFile::try_acquire_shared(&path).unwrap_err();
    assert!(edictora::update::lock::is_held_by_another(&e), "{e:?}");
    drop(x);
}

#[test]
fn 実行中のedictoraがあればダウンロードと検証まででapplyを起動しない() {
    let dir = fresh_dir("update-inuse");
    let inst = fake_install(&dir, "1.0.5");
    let body = package_body();
    let name = "Edictora-1.0.6-full.nupkg";
    let server = FakeServer::start(routes(
        package_and_feed("1.0.6", &body, |_| {}),
        Some((name, body.clone())),
    ));
    let launcher = RecordingLauncher::default();

    // pull などが実行中（共有で持っている）
    let cli = LockFile::try_acquire_shared(&running(&dir)).unwrap();
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    match &outcome {
        UpdateOutcome::AppInUse { from, to } => {
            assert_eq!(
                (from.to_string(), to.to_string()),
                ("1.0.5".into(), "1.0.6".into())
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(launcher.calls.borrow().is_empty());
    // ダウンロードと検証は済んでいる
    assert_eq!(std::fs::read(inst.packages_dir.join(name)).unwrap(), body);
    drop(cli);

    // 終われば、ダウンロード済みのものを検証して使い、適用を任せる（取り直さない）
    let outcome = run_update(
        &inst,
        &server.source(),
        &launcher,
        1,
        &running(&dir),
        &quiet,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(outcome, UpdateOutcome::ApplyStarted { .. }));
    assert_eq!(launcher.calls.borrow().len(), 1);
    let requests = server.requests();
    assert_eq!(
        requests.iter().filter(|r| r.contains(name)).count(),
        1,
        "{requests:?}"
    );
    // 適用を任せた後は、プロセスが終わるまで排他で持ち続ける（新しい CLI は始まれない）
    let e = LockFile::try_acquire_shared(&running(&dir)).unwrap_err();
    assert!(edictora::update::lock::is_held_by_another(&e));
}
