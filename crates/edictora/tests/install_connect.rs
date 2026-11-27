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
use std::time::{Duration, Instant};

use common::{Scratch, work_dir};
use edictora::install::connect::{
    Action, Budget, CallResult, CliResolver, Client, EnsureReport, LOCK_FILE, Mode, Plan,
    RegisterOutcome, STATE_FILE, SkipReason, State, UnregisterOutcome, ensure_registered,
    find_cli_in, forget_state, load_state, mode_from_config_file, output_points_at, parse_mode,
    parse_state, plan, register_client, registered_executable, run_cli, save_state, unregister_all,
    unregister_client,
};
use edictora::install::toolport::Detection;
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
    /// 手放すと一式を消す
    _scratch: Scratch,
}

impl Stub {
    fn new(name: &str) -> Stub {
        let scratch = Scratch::under(&work_dir("install_connect").join(name));
        let dir = scratch.to_path_buf();
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
        Stub {
            dir,
            bin,
            _scratch: scratch,
        }
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
        skipped: Default::default(),
    }
}

fn state_with_skipped(
    executable: &Path,
    done: &[Client],
    skipped: &[(Client, SkipReason)],
) -> State {
    State {
        skipped: skipped.iter().copied().collect(),
        ..state_of(executable, done)
    }
}

/// 常に同じ結果を返す Toolport の判定（本物の設定は読まない）
fn served() -> Detection {
    Detection::Served {
        server_id: "edictora".to_string(),
        command: r"C:\Tools\edictora.exe".to_string(),
    }
}

/// 直接登録（判定を使わないモード）で ensure する。判定が呼ばれたらテストを失敗させる
fn ensure_direct(
    dir: &Path,
    exe: &Path,
    resolve: CliResolver,
    budget: &Budget,
    label: &str,
) -> EnsureReport {
    let never = |_: Client| -> Detection { panic!("Direct では Toolport を判定しません") };
    ensure_registered(dir, exe, Mode::Direct, resolve, &never, budget, label)
}

/// 指定のモードと判定で ensure する
fn ensure_with(stub: &Stub, mode: Mode, detection: Detection) -> EnsureReport {
    let resolve = stub.resolver();
    let detect = move |_: Client| detection.clone();
    ensure_registered(
        &stub.state_dir(),
        &stub.executable(),
        mode,
        &resolve,
        &detect,
        &generous(),
        "test",
    )
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
        actions: vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register),
        ],
    });
    let unknown = |_: Client| Detection::Unknown("x".to_string());
    // 状態なし・別の exe（CLI が見つからなくても全部）
    assert_eq!(
        plan(&State::default(), exe, Mode::Auto, |_| false, unknown),
        all
    );
    assert_eq!(
        plan(
            &state_of(
                Path::new(r"D:\Moved\current\edictora.exe"),
                &[Client::Claude, Client::Codex]
            ),
            exe,
            Mode::Auto,
            |_| true,
            unknown
        ),
        all
    );
    // 同じ exe で全部済み → 何もしない（大小・区切りの違いは同じとみなす）
    let done = State {
        executable: Some(r"c:/a/edictora/current/EDICTORA.exe".to_string()),
        done: [Client::Claude, Client::Codex].into_iter().collect(),
        skipped: Default::default(),
    };
    assert_eq!(plan(&done, exe, Mode::Auto, |_| true, unknown), None);
    // 未完了でも CLI が見つからなければ呼ばない
    let partial = state_of(exe, &[Client::Claude]);
    assert_eq!(plan(&partial, exe, Mode::Auto, |_| false, unknown), None);
    assert_eq!(
        plan(&partial, exe, Mode::Auto, |_| true, unknown),
        Some(Plan {
            fresh: false,
            actions: vec![(Client::Codex, Action::Register)],
        })
    );
}

#[test]
fn planは新規ならモードと判定でクライアントごとに決める() {
    let exe = Path::new(r"C:\A\Edictora\current\edictora.exe");
    let fresh = State::default();
    let only_claude_served = |c: Client| {
        if c == Client::Claude {
            served()
        } else {
            Detection::NotServed
        }
    };
    let actions = |mode, detect: &dyn Fn(Client) -> Detection| {
        plan(&fresh, exe, mode, |_| true, detect).unwrap().actions
    };
    assert_eq!(
        actions(Mode::Auto, &only_claude_served),
        vec![
            (Client::Claude, Action::Skip(SkipReason::Toolport)),
            (Client::Codex, Action::Register),
        ]
    );
    for mode_detection in [
        Detection::NotConnected,
        Detection::NotServed,
        Detection::Unknown("x".to_string()),
    ] {
        let detect = |_: Client| mode_detection.clone();
        assert_eq!(
            actions(Mode::Auto, &detect),
            vec![
                (Client::Claude, Action::Register),
                (Client::Codex, Action::Register)
            ]
        );
    }
    assert_eq!(
        actions(Mode::Direct, &|_| served()),
        vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register)
        ]
    );
    assert_eq!(
        actions(Mode::Off, &|_| served()),
        vec![
            (Client::Claude, Action::Skip(SkipReason::Off)),
            (Client::Codex, Action::Skip(SkipReason::Off))
        ]
    );
}

#[test]
fn planは省いたものをモードに応じて扱いdoneには触れない() {
    let exe = Path::new(r"C:\A\Edictora\current\edictora.exe");
    let skipped_toolport = state_with_skipped(
        exe,
        &[],
        &[
            (Client::Claude, SkipReason::Toolport),
            (Client::Codex, SkipReason::Toolport),
        ],
    );
    let run = |state: &State, mode, detection: Detection, available: bool| {
        plan(state, exe, mode, |_| available, move |_| detection.clone()).map(|p| p.actions)
    };
    // auto: 提供中・無効化・不明は何もしない
    for detection in [
        served(),
        Detection::NotServed,
        Detection::Unknown("x".to_string()),
    ] {
        assert_eq!(run(&skipped_toolport, Mode::Auto, detection, true), None);
    }
    // auto: ゲートウェイが消えたら直接登録（CLI が無ければ待つ）
    assert_eq!(
        run(&skipped_toolport, Mode::Auto, Detection::NotConnected, true),
        Some(vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register)
        ])
    );
    assert_eq!(
        run(
            &skipped_toolport,
            Mode::Auto,
            Detection::NotConnected,
            false
        ),
        None
    );
    // direct: 省いたものを判定せず直接登録
    assert_eq!(
        run(&skipped_toolport, Mode::Direct, served(), true),
        Some(vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register)
        ])
    );
    // off: 省いたものは何もしない。未完了のものは CLI の有無に関係なく off として記録する
    assert_eq!(run(&skipped_toolport, Mode::Off, served(), true), None);
    for available in [true, false] {
        assert_eq!(
            run(&state_of(exe, &[]), Mode::Off, served(), available),
            Some(vec![
                (Client::Claude, Action::Skip(SkipReason::Off)),
                (Client::Codex, Action::Skip(SkipReason::Off))
            ])
        );
    }
    // off で省いていたものを auto で新規扱いにする
    let skipped_off = state_with_skipped(
        exe,
        &[],
        &[
            (Client::Claude, SkipReason::Off),
            (Client::Codex, SkipReason::Off),
        ],
    );
    assert_eq!(
        run(&skipped_off, Mode::Auto, served(), true),
        Some(vec![
            (Client::Claude, Action::Skip(SkipReason::Toolport)),
            (Client::Codex, Action::Skip(SkipReason::Toolport))
        ])
    );
    assert_eq!(
        run(&skipped_off, Mode::Auto, Detection::NotServed, true),
        Some(vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register)
        ])
    );
    // done のものは、どのモードでも何もしない
    let done = state_of(exe, &[Client::Claude, Client::Codex]);
    for mode in [Mode::Auto, Mode::Direct, Mode::Off] {
        assert_eq!(run(&done, mode, served(), true), None);
    }
}

#[test]
fn plan_は未完了のクライアントをモードに応じて判定してから登録する() {
    let exe = Path::new(r"C:\A\Edictora\current\edictora.exe");
    // 前回 CLI が無かった・失敗した（done にも skipped にも無い）
    let unfinished = state_of(exe, &[]);
    let run = |mode, detection: Detection, available: bool| {
        let detect = move |_: Client| detection.clone();
        plan(&unfinished, exe, mode, |_| available, detect).map(|p| p.actions)
    };
    // auto: CLI があるときだけ判定する。提供中なら省き、それ以外は直接登録
    assert_eq!(
        run(Mode::Auto, served(), true),
        Some(vec![
            (Client::Claude, Action::Skip(SkipReason::Toolport)),
            (Client::Codex, Action::Skip(SkipReason::Toolport))
        ])
    );
    for detection in [
        Detection::NotConnected,
        Detection::NotServed,
        Detection::Unknown("x".to_string()),
    ] {
        assert_eq!(
            run(Mode::Auto, detection, true),
            Some(vec![
                (Client::Claude, Action::Register),
                (Client::Codex, Action::Register)
            ])
        );
    }
    // auto: CLI が無ければ判定もしない（次の機会に回す）
    let never = |_: Client| -> Detection { panic!("CLI が無いときは判定しません") };
    assert_eq!(
        plan(&unfinished, exe, Mode::Auto, |_| false, never).map(|p| p.actions),
        None
    );
    // direct: 判定せず直接登録
    let plan_direct = plan(&unfinished, exe, Mode::Direct, |_| true, never);
    assert_eq!(
        plan_direct.map(|p| p.actions),
        Some(vec![
            (Client::Claude, Action::Register),
            (Client::Codex, Action::Register)
        ])
    );
}

/// 自分以外の実行ファイルでの提供
fn served_elsewhere() -> Detection {
    Detection::ServedElsewhere {
        server_id: "edictora".to_string(),
        command: r"C:\Other\edictora.exe".to_string(),
    }
}

#[test]
fn plan_はtoolport_foreignの遷移を表のとおりに決める() {
    let exe = Path::new(r"C:\A\Edictora\current\edictora.exe");
    let both = |skip: SkipReason| {
        vec![
            (Client::Claude, Action::Skip(skip)),
            (Client::Codex, Action::Skip(skip)),
        ]
    };
    let register = vec![
        (Client::Claude, Action::Register),
        (Client::Codex, Action::Register),
    ];
    let state_of_reason = |reason| {
        state_with_skipped(
            exe,
            &[],
            &[(Client::Claude, reason), (Client::Codex, reason)],
        )
    };
    let run = |state: &State, mode, detection: Detection, available: bool| {
        plan(state, exe, mode, |_| available, move |_| detection.clone()).map(|p| p.actions)
    };
    // 新規（初回・CLI が無い未完了）: 別の実行ファイルでの提供は foreign、同じ実体は toolport
    let fresh = Some(both(SkipReason::ToolportForeign));
    assert_eq!(
        run(&State::default(), Mode::Auto, served_elsewhere(), true),
        fresh
    );
    assert_eq!(
        run(&state_of(exe, &[]), Mode::Auto, served_elsewhere(), true),
        fresh
    );
    assert_eq!(
        run(&State::default(), Mode::Auto, served(), true),
        Some(both(SkipReason::Toolport))
    );
    // direct は判定せず直接登録
    assert_eq!(
        run(&State::default(), Mode::Direct, served_elsewhere(), true),
        Some(register.clone())
    );

    // toolport → 別の実行ファイルに変われば foreign（CLI の有無に関係なく判定する）
    let toolport = state_of_reason(SkipReason::Toolport);
    for available in [true, false] {
        assert_eq!(
            run(&toolport, Mode::Auto, served_elsewhere(), available),
            fresh
        );
    }
    // foreign → 同じ実体なら toolport（CLI の有無に関係なく）
    let foreign = state_of_reason(SkipReason::ToolportForeign);
    for available in [true, false] {
        assert_eq!(
            run(&foreign, Mode::Auto, served(), available),
            Some(both(SkipReason::Toolport))
        );
    }
    // foreign のまま: 何もしない
    for detection in [
        served_elsewhere(),
        Detection::NotServed,
        Detection::Unknown("x".to_string()),
    ] {
        assert_eq!(run(&foreign, Mode::Auto, detection, true), None);
    }
    // foreign → NotConnected は CLI があれば直接登録、無ければ何もしない
    assert_eq!(
        run(&foreign, Mode::Auto, Detection::NotConnected, true),
        Some(register.clone())
    );
    assert_eq!(
        run(&foreign, Mode::Auto, Detection::NotConnected, false),
        None
    );
    // foreign: direct は直接登録、off は何もしない
    assert_eq!(
        run(&foreign, Mode::Direct, served_elsewhere(), true),
        Some(register)
    );
    assert_eq!(run(&foreign, Mode::Off, served_elsewhere(), true), None);
    // off で省いていたものを auto で新規扱いにするときも foreign になる
    assert_eq!(
        run(
            &state_of_reason(SkipReason::Off),
            Mode::Auto,
            served_elsewhere(),
            true
        ),
        fresh
    );
}

#[test]
fn toolport_foreignは状態に残り_foreignの間は記録もログも増やさない() {
    assert_eq!(SkipReason::ToolportForeign.name(), "toolport-foreign");
    let stub = Stub::new("toolport_foreign");
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served_elsewhere()),
        EnsureReport::Ran(vec![
            (
                Client::Claude,
                RegisterOutcome::Skipped(SkipReason::ToolportForeign)
            ),
            (
                Client::Codex,
                RegisterOutcome::Skipped(SkipReason::ToolportForeign)
            ),
        ])
    );
    assert!(stub.calls(Client::Claude).is_empty());
    let foreign_state = state_with_skipped(
        &stub.executable(),
        &[],
        &[
            (Client::Claude, SkipReason::ToolportForeign),
            (Client::Codex, SkipReason::ToolportForeign),
        ],
    );
    assert_eq!(load_state(&stub.state_dir()), foreign_state);
    // 起動のたびに判定し直し、別の実行ファイルのままなら何もしない
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served_elsewhere()),
        EnsureReport::NothingToDo
    );
    assert_eq!(load_state(&stub.state_dir()), foreign_state);
    // Toolport 側を直して自分と同じ実体になれば toolport へ
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served()),
        EnsureReport::Ran(vec![
            (
                Client::Claude,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
            (
                Client::Codex,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
        ])
    );
    // さらに Toolport から外れたら直接登録して done に移し、skipped から外す
    let foreign_again = state_with_skipped(
        &stub.executable(),
        &[],
        &[
            (Client::Claude, SkipReason::ToolportForeign),
            (Client::Codex, SkipReason::ToolportForeign),
        ],
    );
    save_state(&stub.state_dir(), &foreign_again).expect("save");
    assert_eq!(
        ensure_with(&stub, Mode::Auto, Detection::NotConnected),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    assert_eq!(
        load_state(&stub.state_dir()),
        state_of(&stub.executable(), &[Client::Claude, Client::Codex])
    );
}

// ---------------------------------------------------------------------------
// Toolport 経由の提供（スタブの CLI と固定の判定）
// ---------------------------------------------------------------------------

#[test]
fn auto_で提供中なら新規インストールでもCLIを呼ばず省く() {
    let stub = Stub::new("toolport_served");
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served()),
        EnsureReport::Ran(vec![
            (
                Client::Claude,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
            (
                Client::Codex,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
        ])
    );
    assert!(stub.calls(Client::Claude).is_empty());
    assert!(stub.calls(Client::Codex).is_empty());
    assert_eq!(
        load_state(&stub.state_dir()),
        state_with_skipped(
            &stub.executable(),
            &[],
            &[
                (Client::Claude, SkipReason::Toolport),
                (Client::Codex, SkipReason::Toolport)
            ]
        )
    );
    // 次の起動でも、提供中のままなら何もしない
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served()),
        EnsureReport::NothingToDo
    );
    assert!(stub.calls(Client::Claude).is_empty());
}

#[test]
fn 提供中でなければ従来どおり直接登録する() {
    for (name, detection) in [
        ("toolport_not_served", Detection::NotServed),
        ("toolport_not_connected", Detection::NotConnected),
        ("toolport_unknown", Detection::Unknown("x".to_string())),
    ] {
        let stub = Stub::new(name);
        assert_eq!(
            ensure_with(&stub, Mode::Auto, detection),
            EnsureReport::Ran(vec![
                (Client::Claude, RegisterOutcome::Registered),
                (Client::Codex, RegisterOutcome::Registered),
            ]),
            "{name}"
        );
        assert_eq!(
            load_state(&stub.state_dir()),
            state_of(&stub.executable(), &[Client::Claude, Client::Codex])
        );
    }
}

#[test]
fn direct_は提供中でも直接登録し_offは何もせず記録する() {
    let direct = Stub::new("toolport_direct");
    assert_eq!(
        ensure_with(&direct, Mode::Direct, served()),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    let off = Stub::new("toolport_off");
    assert_eq!(
        ensure_with(&off, Mode::Off, Detection::NotConnected),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Skipped(SkipReason::Off)),
            (Client::Codex, RegisterOutcome::Skipped(SkipReason::Off)),
        ])
    );
    assert!(off.calls(Client::Claude).is_empty());
    assert!(off.calls(Client::Codex).is_empty());
    assert_eq!(
        load_state(&off.state_dir()),
        state_with_skipped(
            &off.executable(),
            &[],
            &[
                (Client::Claude, SkipReason::Off),
                (Client::Codex, SkipReason::Off)
            ]
        )
    );
    // off のままなら何度起動しても何もしない
    assert_eq!(
        ensure_with(&off, Mode::Off, Detection::NotConnected),
        EnsureReport::NothingToDo
    );
}

#[test]
fn 省いたあとゲートウェイが消えたら直接登録に戻りdoneにする() {
    let stub = Stub::new("toolport_not_connected_later");
    ensure_with(&stub, Mode::Auto, served());
    // 無効化（NotServed）・不明では足さない
    for detection in [Detection::NotServed, Detection::Unknown("x".to_string())] {
        assert_eq!(
            ensure_with(&stub, Mode::Auto, detection),
            EnsureReport::NothingToDo
        );
    }
    assert!(stub.calls(Client::Claude).is_empty());
    // NotConnected → 直接登録（既存の登録は触らない設計なので、無ければ remove → add）
    assert_eq!(
        ensure_with(&stub, Mode::Auto, Detection::NotConnected),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    assert_eq!(
        stub.calls(Client::Claude),
        vec![
            get_line(),
            remove_line(Client::Claude),
            add_line(Client::Claude, &stub.executable())
        ]
    );
    assert_eq!(
        load_state(&stub.state_dir()),
        state_of(&stub.executable(), &[Client::Claude, Client::Codex])
    );
}

#[test]
fn モード変更後は省いたものを直接登録し_doneは外さない() {
    // 省いた後に direct にする → 直接登録
    let stub = Stub::new("toolport_mode_direct");
    ensure_with(&stub, Mode::Auto, served());
    assert_eq!(
        ensure_with(&stub, Mode::Direct, served()),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::Registered),
        ])
    );
    // 直接登録済みのあと off / auto + 提供中でも done は外れず、CLI も呼ばない
    let before = stub.calls(Client::Claude);
    for (mode, detection) in [(Mode::Off, served()), (Mode::Auto, served())] {
        assert_eq!(
            ensure_with(&stub, mode, detection),
            EnsureReport::NothingToDo
        );
    }
    assert_eq!(stub.calls(Client::Claude), before);
    assert_eq!(
        load_state(&stub.state_dir()),
        state_of(&stub.executable(), &[Client::Claude, Client::Codex])
    );
    // off で省いた後に auto にする → 提供中なら toolport として省く
    let stub = Stub::new("toolport_mode_off_to_auto");
    ensure_with(&stub, Mode::Off, served());
    assert_eq!(
        ensure_with(&stub, Mode::Auto, served()),
        EnsureReport::Ran(vec![
            (
                Client::Claude,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
            (
                Client::Codex,
                RegisterOutcome::Skipped(SkipReason::Toolport)
            ),
        ])
    );
    assert!(stub.calls(Client::Claude).is_empty());
}

#[test]
fn 古い状態ファイルを読め_skippedを保存して読み直せる() {
    let old = parse_state(br#"{"executable":"x","done":["claude"]}"#);
    assert_eq!(old.done, BTreeSet::from([Client::Claude]));
    assert!(old.skipped.is_empty());
    let parsed = parse_state(
        br#"{"executable":"x","done":[],"skipped":{"claude":"toolport","codex":"off","other":"off","x":3}}"#,
    );
    assert_eq!(
        parsed.skipped.into_iter().collect::<Vec<_>>(),
        vec![
            (Client::Claude, SkipReason::Toolport),
            (Client::Codex, SkipReason::Off)
        ]
    );
    let stub = Stub::new("state_skipped_roundtrip");
    let state = state_with_skipped(
        &stub.executable(),
        &[Client::Codex],
        &[(Client::Claude, SkipReason::Toolport)],
    );
    save_state(&stub.state_dir(), &state).unwrap();
    assert_eq!(load_state(&stub.state_dir()), state);
}

// ---------------------------------------------------------------------------
// 設定ファイルの mcpRegistration
// ---------------------------------------------------------------------------

#[test]
fn mcpRegistrationを寛容に読み不正な値はエラーにする() {
    assert_eq!(parse_mode("{}"), Ok(Mode::Auto));
    assert_eq!(
        parse_mode(r#"{"mcpRegistration":"direct"}"#),
        Ok(Mode::Direct)
    );
    assert_eq!(parse_mode(r#"{"mcpRegistration":"off"}"#), Ok(Mode::Off));
    assert_eq!(parse_mode(r#"{"mcpRegistration":"auto"}"#), Ok(Mode::Auto));
    // コメント・末尾のカンマ・BOM を許す
    assert_eq!(
        parse_mode("\u{feff}{\n  // 設定\n  \"mcpRegistration\": \"off\", /* x */\n}\n"),
        Ok(Mode::Off)
    );
    // 壊れた本文・オブジェクトでない本文はエラー（呼び出し側が「auto とする」とログに残す）
    assert!(parse_mode("{ broken").is_err());
    assert!(parse_mode("[1]").is_err());
    assert!(parse_mode("").is_err());
    // 不正な値
    assert!(parse_mode(r#"{"mcpRegistration":"Direct"}"#).is_err());
    assert!(parse_mode(r#"{"mcpRegistration":1}"#).is_err());
}

#[test]
fn 設定ファイルが無い読めない不正なときはautoになる() {
    let dir = work_dir("install_connect_mode");
    let path = dir.join("config.json");
    let _ = std::fs::remove_file(&path);
    assert_eq!(mode_from_config_file(&path, "test"), Mode::Auto);
    std::fs::write(&path, r#"{"mcpRegistration":"off"}"#).unwrap();
    assert_eq!(mode_from_config_file(&path, "test"), Mode::Off);
    std::fs::write(&path, r#"{"mcpRegistration":"bogus"}"#).unwrap();
    assert_eq!(mode_from_config_file(&path, "test"), Mode::Auto);
    // 壊れた JSON → auto（ログに残す）。UTF-16（BOM つき）の設定も読む
    std::fs::write(&path, "{ broken").unwrap();
    assert_eq!(mode_from_config_file(&path, "test"), Mode::Auto);
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in r#"{"mcpRegistration":"off"}"#.encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(&path, utf16).unwrap();
    assert_eq!(mode_from_config_file(&path, "test"), Mode::Off);
    // フォルダーは読めない → auto
    assert_eq!(mode_from_config_file(&dir, "test"), Mode::Auto);
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
    let report = ensure_direct(&dir, &exe, &resolve, &generous(), "test");
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
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
    let report = ensure_direct(&dir, &exe, &resolve, &generous(), "test");
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
        ensure_direct(&dir, &exe, &only_claude, &generous(), "test"),
        EnsureReport::Ran(vec![
            (Client::Claude, RegisterOutcome::Registered),
            (Client::Codex, RegisterOutcome::NoCli),
        ])
    );
    assert_eq!(load_state(&dir), state_of(&exe, &[Client::Claude]));
    // まだ見つからない → 何もしない
    assert_eq!(
        ensure_direct(&dir, &exe, &only_claude, &generous(), "test"),
        EnsureReport::NothingToDo
    );
    // 見つかった → その CLI だけ
    let resolve = stub.resolver();
    assert_eq!(
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
        ensure_direct(&dir, &exe, &resolve, &budget, "test"),
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
        ensure_direct(&dir, &exe, &resolve, &none, "test"),
        EnsureReport::Ran(vec![(Client::Claude, RegisterOutcome::Deferred)])
    );
    assert_eq!(load_state(&dir), state_of(&exe, &[Client::Codex]));
    // 次の機会に完了する
    assert_eq!(
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
        ensure_direct(&dir, &exe, &resolve, &generous(), "test"),
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
