//! MCP サーバーの Claude Code・Codex への登録・解除（`install::connect`）のテスト。
//!
//! **本物の `claude.exe` / `codex.exe` と、その設定（`~/.claude.json`・`~/.codex/config.toml`）には絶対に触れない。**
//! CLI の場所は引数（`CliResolver`）で渡し、ここではスタブの場所だけを返す。実際の PATH を読む
//! `resolve_from_environment` と、本物の保存先・HKCU を使う `run_hook` は呼ばない。
//!
//! スタブの CLI は、このテストバイナリ自身を作業フォルダー（target/tmp の下）に `claude.exe` / `codex.exe` として
//! 置いたもの。CRT の初期化子（main より前に走る）が「自分の名前が claude / codex で、隣に目印ファイルがある」
//! ときだけスタブとして振る舞い、libtest に引数を渡す前に終了する（libtest は `--scope` などを未知の
//! オプションとして弾くため、`#[test]` の再帰起動は使えない）。環境変数は使わない（並列テストで競合するため）。
//!
//! スタブは exe の隣のファイルに状態を持つ:
//! - `<名前>.registered` … 登録の内容（`mcp add` の `--` より後ろ）。`mcp get` はあれば `Command: …` を出して 0、無ければ 1
//! - `<名前>.calls` … 呼ばれた引数（1 行に 1 回）
//! - 引数 `__warmup` … 何も記録せずに 0 で終わる（配置の直後の空打ち用）
//! - `<名前>.mode` … 振る舞いの切り替え（`sleep` / `add-fails` / `remove-fails` / `remove-deletes-then-fails`）
// テスト名に CLI などの英大文字を含めるため
#![allow(non_snake_case)]
#![cfg(windows)]

mod common;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::work_dir;
use edictora::install::connect::{
    Budget, CallResult, Client, EnsureReport, LOCK_FILE, Plan, RegisterOutcome, STATE_FILE, State,
    UnregisterOutcome, ensure_registered, find_cli_in, forget_state, load_state, output_points_at,
    parse_state, plan, register_client, registered_executable, run_cli, save_state, unregister_all,
    unregister_client,
};
use edictora::update::lock::LockFile;

// ---------------------------------------------------------------------------
// スタブの CLI（main より前に走る）
// ---------------------------------------------------------------------------

/// スタブとして振る舞うフォルダーの目印
const STUB_MARKER: &str = "stub.marker";

/// `sleep` のスタブ（親と孫）が「まだ生きている」目印を書くまでの時間。
/// 並列のテストの負荷で taskkill の起動が遅れても、止める前に書いてしまわない長さにする
const MARKER_DELAY: Duration = Duration::from_secs(6);

/// CRT の初期化子として登録する（MSVC の `.CRT$XCU` は C++ の静的初期化と同じく main より前に呼ばれる）
#[used]
#[unsafe(link_section = ".CRT$XCU")]
static STUB_ENTRY: extern "C" fn() = stub_entry;

extern "C" fn stub_entry() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(stem) = exe
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
    else {
        return;
    };
    if stem != "claude" && stem != "codex" {
        return;
    }
    let Some(dir) = exe.parent() else {
        return;
    };
    if !dir.join(STUB_MARKER).is_file() {
        return;
    }
    let code = run_stub(&exe, dir, &stem);
    std::process::exit(code);
}

fn run_stub(exe: &Path, dir: &Path, name: &str) -> i32 {
    use std::io::Write;
    let args: Vec<String> = std::env::args().skip(1).collect();
    // 空打ち（テストの補助が配置の直後に 1 回だけ呼ぶ）: 何も記録せずに終わる
    if args.first().map(String::as_str) == Some("__warmup") {
        return 0;
    }
    // 時間切れの検証用の孫プロセス: 親が木ごと止められなければ、しばらく後に目印を書く
    if args.first().map(String::as_str) == Some("__grandchild") {
        std::thread::sleep(MARKER_DELAY);
        let _ = std::fs::write(dir.join(format!("{name}.grandchild-alive")), b"alive");
        return 0;
    }
    let mut calls = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{name}.calls")))
        .expect("呼び出しの記録を開けません");
    let _ = writeln!(calls, "{}", args.join(" "));
    drop(calls);

    let mode = std::fs::read_to_string(dir.join(format!("{name}.mode"))).unwrap_or_default();
    let mode = mode.trim();
    let registered = dir.join(format!("{name}.registered"));
    if mode == "sleep" {
        // 孫を 1 つ起こしてから眠る（taskkill /T で孫まで止まることを確かめる）
        let _ = std::process::Command::new(exe).arg("__grandchild").spawn();
        std::thread::sleep(MARKER_DELAY);
        let _ = std::fs::write(dir.join(format!("{name}.parent-alive")), b"alive");
        std::thread::sleep(Duration::from_secs(30));
        return 0;
    }
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["mcp", "get", "edictora"] => match std::fs::read_to_string(&registered) {
            Ok(command) => {
                let mut out = std::io::stdout();
                let _ = write!(
                    out,
                    "edictora:\n  Scope: User config\n  Type: stdio\n  Command: {command}\n  Args: \n"
                );
                let _ = out.flush();
                0
            }
            Err(_) => 1,
        },
        ["mcp", "remove", "edictora", ..] => match mode {
            "remove-fails" => 1,
            "remove-deletes-then-fails" => {
                let _ = std::fs::remove_file(&registered);
                1
            }
            _ => {
                if std::fs::remove_file(&registered).is_ok() {
                    0
                } else {
                    1
                }
            }
        },
        ["mcp", "add", ..] => {
            if mode == "add-fails" {
                return 1;
            }
            let Some(separator) = words.iter().position(|w| *w == "--") else {
                return 2;
            };
            let _ = std::fs::write(&registered, words[separator + 1..].join(" "));
            0
        }
        _ => 2,
    }
}

// ---------------------------------------------------------------------------
// テストの補助
// ---------------------------------------------------------------------------

/// スタブ一式（実行ごとに別のフォルダー。前回の残りやまだ動いているスタブに影響されない）
struct Stub {
    dir: PathBuf,
    bin: PathBuf,
}

impl Stub {
    fn new(name: &str) -> Stub {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = work_dir("install_connect")
            .join(name)
            .join(format!("{}-{nanos}", std::process::id()));
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(STUB_MARKER), b"").unwrap();
        let me = std::env::current_exe().unwrap();
        for client in [Client::Claude, Client::Codex] {
            let target = bin.join(client.exe_name());
            if std::fs::hard_link(&me, &target).is_err() {
                std::fs::copy(&me, &target).unwrap();
            }
            // 時間を測らずに 1 回空打ちする。置いたばかりの exe の初回起動はウイルス対策の検査などで
            // 数秒遅れることがあり、それを時間の上限の検証に含めないため
            let status = std::process::Command::new(&target)
                .arg("__warmup")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("スタブを起動できません");
            assert!(status.success(), "スタブの空打ちが失敗しました: {status:?}");
        }
        Stub { dir, bin }
    }

    /// 登録する exe（実在しなくてよい。照合と登録の内容にだけ使う）
    fn executable(&self) -> PathBuf {
        registered_executable(&self.dir.join("Edictora"))
    }

    /// 状態ファイルを置くフォルダー（データの根に当たる）
    fn state_dir(&self) -> PathBuf {
        self.dir.join("Edictora.Data")
    }

    fn cli(&self, client: Client) -> PathBuf {
        self.bin.join(client.exe_name())
    }

    fn file(&self, client: Client, suffix: &str) -> PathBuf {
        self.bin.join(format!("{}.{suffix}", client.name()))
    }

    fn set_registered(&self, client: Client, command: &str) {
        std::fs::write(self.file(client, "registered"), command).unwrap();
    }

    fn registered(&self, client: Client) -> Option<String> {
        std::fs::read_to_string(self.file(client, "registered")).ok()
    }

    fn set_mode(&self, client: Client, mode: &str) {
        std::fs::write(self.file(client, "mode"), mode).unwrap();
    }

    fn calls(&self, client: Client) -> Vec<String> {
        std::fs::read_to_string(self.file(client, "calls"))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// 両方のスタブを返す CLI の場所（本物の PATH は見ない）
    fn resolver(&self) -> impl Fn(Client) -> Option<PathBuf> + '_ {
        move |client| Some(self.cli(client))
    }

    /// 自分を指す登録の内容（`mcp add` の `--` より後ろ）
    fn own_command(&self) -> String {
        format!("{} mcp", self.executable().display())
    }
}

fn generous() -> Budget {
    Budget::new(Duration::from_secs(20), Duration::from_secs(60))
}

fn get_line() -> String {
    "mcp get edictora".to_string()
}

fn remove_line(client: Client) -> String {
    match client {
        Client::Claude => "mcp remove edictora --scope user".to_string(),
        Client::Codex => "mcp remove edictora".to_string(),
    }
}

fn add_line(client: Client, executable: &Path) -> String {
    match client {
        Client::Claude => format!(
            "mcp add --scope user edictora -- {} mcp",
            executable.display()
        ),
        Client::Codex => format!("mcp add edictora -- {} mcp", executable.display()),
    }
}

fn state_of(executable: &Path, done: &[Client]) -> State {
    State {
        executable: Some(executable.to_string_lossy().into_owned()),
        done: done.iter().copied().collect(),
    }
}

// ---------------------------------------------------------------------------
// スタブと CLI の探索
// ---------------------------------------------------------------------------

#[test]
fn スタブのgetは未登録なら1で終わり登録があれば内容を出す() {
    let stub = Stub::new("stub_get");
    let cli = stub.cli(Client::Claude);
    let args = vec!["mcp".into(), "get".into(), "edictora".into()];
    assert_eq!(
        run_cli(&cli, &args, Duration::from_secs(20)),
        CallResult::Failed
    );
    stub.set_registered(Client::Claude, &stub.own_command());
    match run_cli(&cli, &args, Duration::from_secs(20)) {
        CallResult::Success(output) => assert!(output_points_at(&output, &stub.executable())),
        other => panic!("成功を期待しました: {other:?}"),
    }
    assert_eq!(stub.calls(Client::Claude), vec![get_line(), get_line()]);
}

#[test]
fn CLIはPATHの中のexeだけを探す() {
    let stub = Stub::new("find_cli");
    // .cmd のラッパーだけがあるフォルダーは飛ばす
    let wrappers = stub.dir.join("wrappers");
    std::fs::create_dir_all(&wrappers).unwrap();
    std::fs::write(wrappers.join("claude.cmd"), b"@echo off").unwrap();
    std::fs::write(wrappers.join("codex.ps1"), b"").unwrap();
    let mut path = OsString::from(&wrappers);
    path.push(";;");
    path.push(&stub.bin);
    assert_eq!(
        find_cli_in(Client::Claude, Some(&path)),
        Some(stub.cli(Client::Claude))
    );
    assert_eq!(
        find_cli_in(Client::Codex, Some(&path)),
        Some(stub.cli(Client::Codex))
    );
    assert_eq!(
        find_cli_in(Client::Claude, Some(wrappers.as_os_str())),
        None
    );
    assert_eq!(find_cli_in(Client::Claude, None), None);
}

#[test]
fn 登録する引数と固定パス() {
    let exe = Path::new(r"C:\Users\u\AppData\Local\Edictora\current\edictora.exe");
    assert_eq!(
        registered_executable(Path::new(r"C:\Users\u\AppData\Local\Edictora")),
        exe
    );
    assert_eq!(
        Client::Claude.add_args(exe),
        vec![
            "mcp",
            "add",
            "--scope",
            "user",
            "edictora",
            "--",
            exe.to_str().unwrap(),
            "mcp"
        ]
    );
    assert_eq!(
        Client::Codex.add_args(exe),
        vec!["mcp", "add", "edictora", "--", exe.to_str().unwrap(), "mcp"]
    );
    assert_eq!(
        Client::Claude.remove_args(),
        vec!["mcp", "remove", "edictora", "--scope", "user"]
    );
    assert_eq!(
        Client::Codex.remove_args(),
        vec!["mcp", "remove", "edictora"]
    );
    assert_eq!(Client::Codex.get_args(), vec!["mcp", "get", "edictora"]);
    // 照合は区切りと大小を区別しない
    assert!(output_points_at(
        "Command: c:/users/U/appdata/local/edictora/CURRENT/edictora.exe",
        exe
    ));
    assert!(!output_points_at(
        r"Command: D:\Other\current\edictora.exe",
        exe
    ));
}

// ---------------------------------------------------------------------------
// 登録
// ---------------------------------------------------------------------------

#[test]
fn 未登録ならremoveしてからaddする() {
    let stub = Stub::new("register_new");
    let exe = stub.executable();
    for client in [Client::Claude, Client::Codex] {
        let cli = stub.cli(client);
        assert_eq!(
            register_client(client, Some(&cli), &exe, &generous()),
            RegisterOutcome::Registered
        );
        assert_eq!(
            stub.calls(client),
            vec![get_line(), remove_line(client), add_line(client, &exe)]
        );
        assert_eq!(stub.registered(client), Some(stub.own_command()));
    }
}

#[test]
fn 既に自分を指す登録はgetだけで変えない() {
    let stub = Stub::new("register_unchanged");
    let exe = stub.executable();
    // 区切りと大小が違っても同じ exe とみなす
    let same = stub.own_command().replace('\\', "/").to_uppercase();
    stub.set_registered(Client::Codex, &same);
    assert_eq!(
        register_client(
            Client::Codex,
            Some(&stub.cli(Client::Codex)),
            &exe,
            &generous()
        ),
        RegisterOutcome::Unchanged
    );
    assert_eq!(stub.calls(Client::Codex), vec![get_line()]);
    assert_eq!(stub.registered(Client::Codex), Some(same));
}

#[test]
fn 別の場所を指す登録は置き換える() {
    let stub = Stub::new("register_other");
    let exe = stub.executable();
    stub.set_registered(Client::Claude, r"D:\Old\Edictora\current\edictora.exe mcp");
    assert_eq!(
        register_client(
            Client::Claude,
            Some(&stub.cli(Client::Claude)),
            &exe,
            &generous()
        ),
        RegisterOutcome::Registered
    );
    assert_eq!(
        stub.calls(Client::Claude),
        vec![
            get_line(),
            remove_line(Client::Claude),
            add_line(Client::Claude, &exe)
        ]
    );
    assert_eq!(stub.registered(Client::Claude), Some(stub.own_command()));
}

#[test]
fn addの失敗とCLIなしは未完了() {
    let stub = Stub::new("register_failures");
    let exe = stub.executable();
    stub.set_mode(Client::Codex, "add-fails");
    let outcome = register_client(
        Client::Codex,
        Some(&stub.cli(Client::Codex)),
        &exe,
        &generous(),
    );
    assert!(matches!(outcome, RegisterOutcome::Failed(_)), "{outcome:?}");
    assert!(!outcome.is_done());
    assert_eq!(
        register_client(Client::Claude, None, &exe, &generous()),
        RegisterOutcome::NoCli
    );
    let missing = stub.dir.join("missing").join("claude.exe");
    let outcome = register_client(Client::Claude, Some(&missing), &exe, &generous());
    assert!(matches!(outcome, RegisterOutcome::Failed(_)), "{outcome:?}");
}

// ---------------------------------------------------------------------------
// 時間の上限と締め切り
// ---------------------------------------------------------------------------

#[test]
fn 時間切れは子プロセスの木ごと止めて未完了にする() {
    let stub = Stub::new("timeout");
    let exe = stub.executable();
    stub.set_mode(Client::Claude, "sleep");
    let budget = Budget::new(Duration::from_millis(500), Duration::from_secs(30));
    let started = Instant::now();
    let outcome = register_client(
        Client::Claude,
        Some(&stub.cli(Client::Claude)),
        &exe,
        &budget,
    );
    let elapsed = started.elapsed();
    assert_eq!(outcome, RegisterOutcome::TimedOut);
    assert!(!outcome.is_done());
    // 上限（0.5 秒）＋ taskkill の分だけで戻る（スタブは 30 秒眠る）
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    // get が時間切れなら remove / add はしない
    assert_eq!(stub.calls(Client::Claude), vec![get_line()]);
    // 親も孫も止まっている（生きていれば起動の MARKER_DELAY 後に目印を書く）
    std::thread::sleep((MARKER_DELAY + Duration::from_secs(1)).saturating_sub(started.elapsed()));
    assert!(!stub.file(Client::Claude, "parent-alive").exists());
    assert!(!stub.file(Client::Claude, "grandchild-alive").exists());
}

#[test]
fn 締め切りが近いとremoveとaddを始めない() {
    let stub = Stub::new("deadline");
    let exe = stub.executable();
    let other = r"D:\Old\Edictora\current\edictora.exe mcp";
    stub.set_registered(Client::Codex, other);
    // 残りが「1 回の上限 × 2」に満たない
    let budget = Budget::new(Duration::from_secs(5), Duration::from_secs(8));
    assert_eq!(
        register_client(Client::Codex, Some(&stub.cli(Client::Codex)), &exe, &budget),
        RegisterOutcome::Deferred
    );
    assert_eq!(stub.calls(Client::Codex), vec![get_line()]);
    assert_eq!(stub.registered(Client::Codex).as_deref(), Some(other));
    // 残りが無ければ CLI を起動しない
    let none = Budget::new(Duration::from_secs(5), Duration::ZERO);
    assert_eq!(
        register_client(Client::Claude, Some(&stub.cli(Client::Claude)), &exe, &none),
        RegisterOutcome::Deferred
    );
    assert!(stub.calls(Client::Claude).is_empty());
}

// ---------------------------------------------------------------------------
// 状態ファイルと plan
// ---------------------------------------------------------------------------

#[test]
fn planは状態のexeが違えば全部を対象にし同じなら未完了で見つかるものだけ() {
    let exe = Path::new(r"C:\A\Edictora\current\edictora.exe");
    let all = Some(Plan {
        fresh: true,
        clients: vec![Client::Claude, Client::Codex],
    });
    // 状態なし・別の exe（CLI が見つからなくても全部）
    assert_eq!(plan(&State::default(), exe, |_| false), all);
    assert_eq!(
        plan(
            &state_of(
                Path::new(r"D:\Moved\current\edictora.exe"),
                &[Client::Claude, Client::Codex]
            ),
            exe,
            |_| true
        ),
        all
    );
    // 同じ exe で全部済み → 何もしない（大小・区切りの違いは同じとみなす）
    let done = State {
        executable: Some(r"c:/a/edictora/current/EDICTORA.exe".to_string()),
        done: [Client::Claude, Client::Codex].into_iter().collect(),
    };
    assert_eq!(plan(&done, exe, |_| true), None);
    // 未完了でも CLI が見つからなければ呼ばない
    let partial = state_of(exe, &[Client::Claude]);
    assert_eq!(plan(&partial, exe, |_| false), None);
    assert_eq!(
        plan(&partial, exe, |_| true),
        Some(Plan {
            fresh: false,
            clients: vec![Client::Codex],
        })
    );
}

#[test]
fn 状態ファイルは往復でき壊れていれば未登録扱い() {
    let stub = Stub::new("state_file");
    let dir = stub.state_dir();
    assert_eq!(load_state(&dir), State::default());
    let state = state_of(&stub.executable(), &[Client::Codex]);
    save_state(&dir, &state).unwrap();
    assert_eq!(load_state(&dir), state);
    let text = std::fs::read_to_string(dir.join(STATE_FILE)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["done"], serde_json::json!(["codex"]));
    // 上書きしても一時ファイルは残らない
    save_state(&dir, &state_of(&stub.executable(), &[])).unwrap();
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n != STATE_FILE)
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // 壊れた・形の違う内容は未登録として扱う（知らない名前は読み捨てる）
    std::fs::write(dir.join(STATE_FILE), b"{ broken").unwrap();
    assert_eq!(load_state(&dir), State::default());
    assert_eq!(parse_state(b"[1,2]"), State::default());
    assert_eq!(
        parse_state(br#"{"executable":"x","done":["claude","other",3]}"#).done,
        BTreeSet::from([Client::Claude])
    );
    assert!(forget_state(&dir).unwrap());
    assert!(!forget_state(&dir).unwrap());
}

// ---------------------------------------------------------------------------
// ensure（状態ファイルつきの登録）
// ---------------------------------------------------------------------------

#[test]
fn 初回は全部を登録し済んだものは二度と呼ばない() {
    let stub = Stub::new("ensure_fresh");
    let exe = stub.executable();
    let dir = stub.state_dir();
    let resolve = stub.resolver();
    let report = ensure_registered(&dir, &exe, &resolve, &generous(), "test");
    assert_eq!(
        report,
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    assert_eq!(
        load_state(&dir),
        state_of(&exe, &[Client::Claude, Client::Codex])
    );
    // 利用者が登録を外しても、済んだ CLI は呼ばない（復活させない）
    std::fs::remove_file(stub.file(Client::Claude, "registered")).unwrap();
    let before = (stub.calls(Client::Claude), stub.calls(Client::Codex));
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::NothingToDo
    );
    assert_eq!(
        (stub.calls(Client::Claude), stub.calls(Client::Codex)),
        before
    );
    assert_eq!(stub.registered(Client::Claude), None);
}

#[test]
fn 済んでいないものだけを呼ぶ() {
    let stub = Stub::new("ensure_partial");
    let exe = stub.executable();
    let dir = stub.state_dir();
    save_state(&dir, &state_of(&exe, &[Client::Claude])).unwrap();
    let resolve = stub.resolver();
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::Ran(vec![(Client::Codex, RegisterOutcome::Registered)])
    );
    assert!(stub.calls(Client::Claude).is_empty());
    assert_eq!(
        load_state(&dir),
        state_of(&exe, &[Client::Claude, Client::Codex])
    );
}

#[test]
fn 状態のexeが変わると全部やり直す() {
    let stub = Stub::new("ensure_moved");
    let exe = stub.executable();
    let dir = stub.state_dir();
    save_state(
        &dir,
        &state_of(
            Path::new(r"D:\Old\Edictora\current\edictora.exe"),
            &[Client::Claude, Client::Codex],
        ),
    )
    .unwrap();
    // Claude は既に新しい場所を指している（unchanged も済んだ扱い）
    stub.set_registered(Client::Claude, &stub.own_command());
    let resolve = stub.resolver();
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Unchanged),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    assert_eq!(
        load_state(&dir),
        state_of(&exe, &[Client::Claude, Client::Codex])
    );
}

#[test]
fn 壊れた状態ファイルは未登録として登録し直す() {
    let stub = Stub::new("ensure_broken");
    let exe = stub.executable();
    let dir = stub.state_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(STATE_FILE), b"\xff\xfe not json").unwrap();
    let resolve = stub.resolver();
    let report = ensure_registered(&dir, &exe, &resolve, &generous(), "test");
    assert!(
        matches!(report, EnsureReport::Ran(ref r) if r.len() == 2),
        "{report:?}"
    );
    assert_eq!(
        load_state(&dir),
        state_of(&exe, &[Client::Claude, Client::Codex])
    );
}

#[test]
fn CLIが見つからないものは未完了で見つかってから登録する() {
    let stub = Stub::new("ensure_missing_cli");
    let exe = stub.executable();
    let dir = stub.state_dir();
    let only_claude = |client: Client| (client == Client::Claude).then(|| stub.cli(client));
    assert_eq!(
        ensure_registered(&dir, &exe, &only_claude, &generous(), "test"),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::NoCli),
        ])
    );
    assert_eq!(load_state(&dir), state_of(&exe, &[Client::Claude]));
    // まだ見つからない → 何もしない
    assert_eq!(
        ensure_registered(&dir, &exe, &only_claude, &generous(), "test"),
        EnsureReport::NothingToDo
    );
    // 見つかった → その CLI だけ
    let resolve = stub.resolver();
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::Ran(vec![(Client::Codex, RegisterOutcome::Registered)])
    );
    assert_eq!(stub.calls(Client::Claude).len(), 3);
}

#[test]
fn 時間切れのものは未完了のまま保存して次に再試行する() {
    let stub = Stub::new("ensure_timeout");
    let exe = stub.executable();
    let dir = stub.state_dir();
    stub.set_mode(Client::Claude, "sleep");
    let resolve = stub.resolver();
    // 1 回の上限は、並列のテストの負荷でもスタブの通常の呼び出しが収まる長さにする
    let budget = Budget::new(Duration::from_secs(3), Duration::from_secs(30));
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &budget, "test"),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::TimedOut),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    assert_eq!(load_state(&dir), state_of(&exe, &[Client::Codex]));
    // 締め切りで手を付けなかったものも未完了のまま
    std::fs::remove_file(stub.file(Client::Claude, "mode")).unwrap();
    let none = Budget::new(Duration::from_secs(3), Duration::ZERO);
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &none, "test"),
        EnsureReport::Ran(vec![(Client::Claude, RegisterOutcome::Deferred)])
    );
    assert_eq!(load_state(&dir), state_of(&exe, &[Client::Codex]));
    // 次の機会に完了する
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::Ran(vec![(Client::Claude, RegisterOutcome::Registered)])
    );
}

#[test]
fn ほかのプロセスが登録中なら何もしない() {
    let stub = Stub::new("ensure_locked");
    let exe = stub.executable();
    let dir = stub.state_dir();
    let _held = LockFile::try_acquire(&dir.join(LOCK_FILE)).unwrap();
    let resolve = stub.resolver();
    assert_eq!(
        ensure_registered(&dir, &exe, &resolve, &generous(), "test"),
        EnsureReport::Busy
    );
    assert!(stub.calls(Client::Claude).is_empty());
    assert!(stub.calls(Client::Codex).is_empty());
    assert_eq!(load_state(&dir), State::default());
}

// ---------------------------------------------------------------------------
// 解除
// ---------------------------------------------------------------------------

#[test]
fn 解除は自分を指すときだけremoveする() {
    let stub = Stub::new("unregister");
    let exe = stub.executable();
    // 自分を指す → remove
    stub.set_registered(Client::Claude, &stub.own_command());
    assert_eq!(
        unregister_client(
            Client::Claude,
            Some(&stub.cli(Client::Claude)),
            &exe,
            &generous()
        ),
        UnregisterOutcome::Removed
    );
    assert_eq!(
        stub.calls(Client::Claude),
        vec![get_line(), remove_line(Client::Claude)]
    );
    assert_eq!(stub.registered(Client::Claude), None);
    // 別の場所を指す → 触らない
    let other = r"D:\Other\current\edictora.exe mcp";
    stub.set_registered(Client::Codex, other);
    let outcome = unregister_client(
        Client::Codex,
        Some(&stub.cli(Client::Codex)),
        &exe,
        &generous(),
    );
    assert!(matches!(outcome, UnregisterOutcome::Kept(_)), "{outcome:?}");
    assert_eq!(stub.calls(Client::Codex), vec![get_line()]);
    assert_eq!(stub.registered(Client::Codex).as_deref(), Some(other));
    // 登録が無い → 触らない。CLI が無い → 何もしない
    let outcome = unregister_client(
        Client::Claude,
        Some(&stub.cli(Client::Claude)),
        &exe,
        &generous(),
    );
    assert!(matches!(outcome, UnregisterOutcome::Kept(_)), "{outcome:?}");
    assert_eq!(
        unregister_client(Client::Codex, None, &exe, &generous()),
        UnregisterOutcome::NoCli
    );
}

#[test]
fn removeが非0でも登録が無くなっていれば成功扱い() {
    let stub = Stub::new("unregister_absent");
    let exe = stub.executable();
    stub.set_registered(Client::Codex, &stub.own_command());
    stub.set_mode(Client::Codex, "remove-deletes-then-fails");
    assert_eq!(
        unregister_client(
            Client::Codex,
            Some(&stub.cli(Client::Codex)),
            &exe,
            &generous()
        ),
        UnregisterOutcome::Absent
    );
    assert_eq!(
        stub.calls(Client::Codex),
        vec![get_line(), remove_line(Client::Codex), get_line()]
    );
    // 登録が残っていれば失敗
    stub.set_registered(Client::Claude, &stub.own_command());
    stub.set_mode(Client::Claude, "remove-fails");
    let outcome = unregister_client(
        Client::Claude,
        Some(&stub.cli(Client::Claude)),
        &exe,
        &generous(),
    );
    assert!(
        matches!(outcome, UnregisterOutcome::Failed(_)),
        "{outcome:?}"
    );
    assert_eq!(stub.registered(Client::Claude), Some(stub.own_command()));
}

#[test]
fn アンインストールは自分の登録だけを消して状態ファイルも消す() {
    let stub = Stub::new("unregister_all");
    let exe = stub.executable();
    let dir = stub.state_dir();
    save_state(&dir, &state_of(&exe, &[Client::Claude, Client::Codex])).unwrap();
    stub.set_registered(Client::Claude, &stub.own_command());
    let other = r"D:\Other\current\edictora.exe mcp";
    stub.set_registered(Client::Codex, other);
    let resolve = stub.resolver();
    let results = unregister_all(&dir, &exe, &resolve, &generous(), "test");
    assert_eq!(results[0], (Client::Claude, UnregisterOutcome::Removed));
    assert!(matches!(
        results[1],
        (Client::Codex, UnregisterOutcome::Kept(_))
    ));
    assert_eq!(stub.registered(Client::Claude), None);
    assert_eq!(stub.registered(Client::Codex).as_deref(), Some(other));
    assert!(!dir.join(STATE_FILE).exists());
}
