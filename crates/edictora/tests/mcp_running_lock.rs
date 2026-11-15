//! `edictora mcp` が実行中ロック（`edictora-running.lock`）をプロセスの生存期間中ずっと共有で持つことの結合テスト。
//!
//! 守りたい契約:
//! - MCP サーバーが待機しているだけでも、更新側（排他で取ろうとする）が取れない
//!   （取れると、ログオン時の `Update.exe apply` がセッションの MCP を強制終了してしまう）
//! - 複数のセッションの MCP は同時に起動できる（共有同士は両立する）
//! - クライアントが標準入力を閉じてサーバーが終わったら、更新側が取れるようになる
//! - 更新の適用中（排他で持たれている）に起動した MCP は、標準出力に何も書かずに終了コード 1 で終わる
//!
//! 本物の exe（`CARGO_BIN_EXE_edictora`）を起動するが、保存先は環境変数（プロセス単位）で作業フォルダーの
//! 下へ向けるので、利用者の `%LOCALAPPDATA%\Edictora.Data` には触れない。Chrome は開かない（initialize だけ送る）。

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use edictora::app_paths::{LOCAL_ROOT_ENV, ROAMING_ROOT_ENV};
use edictora::update::lock::{LockFile, is_held_by_another};

mod common;
use common::{test_paths, work_dir};

/// 起動した MCP サーバー（drop で必ず止める。テストが途中で失敗しても子プロセスを残さない）
struct Server {
    child: Child,
    stdout: BufReader<ChildStdout>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_server(dir: &Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_edictora"))
        .arg("mcp")
        .env(LOCAL_ROOT_ENV, dir.join("local"))
        .env(ROAMING_ROOT_ENV, dir.join("roaming"))
        // Velopack の初回起動の印が親から漏れてきても、引数があるので MCP として動く（念のため消しておく）
        .env_remove("VELOPACK_FIRSTRUN")
        .env_remove("VELOPACK_RESTART")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("edictora mcp を起動できません");
    let stdout = BufReader::new(child.stdout.take().expect("標準出力"));
    Server { child, stdout }
}

/// initialize を送り、応答が返るのを待つ（応答が返った時点で、ロックは serve より前に取られている）
fn initialize(server: &mut Server) {
    let stdin = server.child.stdin.as_mut().expect("標準入力");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#
    )
    .expect("書き込み");
    stdin.flush().expect("flush");
    let mut line = String::new();
    server.stdout.read_line(&mut line).expect("応答の読み取り");
    let value: serde_json::Value = serde_json::from_str(&line).expect("応答が JSON ではありません");
    assert_eq!(value["id"], 1, "{line}");
    assert!(value["result"]["protocolVersion"].is_string(), "{line}");
}

/// 標準入力を閉じて（クライアントの通常の止め方）終了を待つ
fn close_and_wait(mut server: Server) -> i32 {
    drop(server.child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = server.child.try_wait().expect("終了の確認") {
            return status.code().unwrap_or(-1);
        }
        assert!(
            Instant::now() < deadline,
            "標準入力を閉じても MCP が終わりません"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn lock_path(dir: &Path) -> PathBuf {
    test_paths(dir).running_lock_path()
}

/// 更新側と同じく排他で取ろうとして、他のプロセスに持たれているかを返す
fn held_by_server(lock: &Path) -> bool {
    match LockFile::try_acquire(lock) {
        Ok(_) => false,
        Err(e) if is_held_by_another(&e) => true,
        Err(e) => panic!("ロックファイルを開けません: {e}"),
    }
}

#[test]
fn idle_mcp_server_keeps_updates_from_being_applied() {
    let dir = work_dir("mcp_running_lock_idle");
    let lock = lock_path(&dir);

    let mut first = spawn_server(&dir);
    initialize(&mut first);
    assert!(
        held_by_server(&lock),
        "待機中の MCP が実行中ロックを持っていません（更新の適用で強制終了されます）"
    );

    // 2 つ目のセッションの MCP も起動でき、共有で両立する
    let mut second = spawn_server(&dir);
    initialize(&mut second);
    assert!(
        LockFile::try_acquire_shared(&lock).is_ok(),
        "共有のロックが両立しません"
    );

    // 1 つ目が終わっても、2 つ目が動いている間は更新側が取れない
    assert_eq!(close_and_wait(first), 0);
    assert!(held_by_server(&lock), "残った MCP のロックが外れています");

    // すべて終われば更新側が取れる
    assert_eq!(close_and_wait(second), 0);
    assert!(
        !held_by_server(&lock),
        "MCP が終わったのに実行中ロックが外れていません"
    );
}

#[test]
fn mcp_server_refuses_to_start_while_an_update_is_being_applied() {
    let dir = work_dir("mcp_running_lock_applying");
    let lock = lock_path(&dir);
    // 更新側が Update.exe を起動する直前に取るのと同じ排他のロック
    let _applying = LockFile::try_acquire(&lock).expect("排他のロックを取れません");

    let server = spawn_server(&dir);
    let mut server = server;
    // 標準出力（JSON-RPC 専用）には何も書かずに終わる
    let mut out = String::new();
    server
        .stdout
        .read_line(&mut out)
        .expect("標準出力の読み取り");
    assert_eq!(out, "", "適用中なのに標準出力へ書きました");
    assert_eq!(close_and_wait(server), 1);
}
