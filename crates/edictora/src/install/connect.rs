//! Setup（Velopack）版の MCP サーバー（`edictora mcp`）を、Claude Code と Codex へ登録・解除する。
//!
//! 共通仕様（インストール直後に Claude Code・Codex へ自動登録、アンインストール前に自動解除）に揃えるための処理。
//! 方式は Remnifold（`native/src/connect.rs` と `app_update/registration.rs`）に倣う:
//!
//! - 登録名は `edictora`。登録する exe は `<根>\current\edictora.exe`（更新で変わらない固定パス）、引数は `mcp`。
//! - 公式 CLI（`claude mcp …` / `codex mcp …`）だけで登録・解除し、ほかのアプリの設定ファイルを直接書き換えない。
//!   CLI は PATH から `claude.exe` / `codex.exe` だけを探す（`.cmd` / `.ps1` のラッパーは使わない）。
//! - 既に自分を指す登録は触らない（`mcp get` の出力に自分の exe が含まれれば何もしない）。それ以外は
//!   remove（失敗は無視）→ add。解除は自分を指す登録だけを remove し、別の場所を指す登録は残す。
//! - CLI 1 回ごとの上限と、呼び出し全体の締め切りを持つ。Velopack は install / uninstall のフックを 30 秒、
//!   updated を 15 秒で打ち切るため、どの呼び出しも時間で打ち切れるようにする（時間切れは `taskkill /T /F` で
//!   子プロセスの木ごと止める）。remove と add の間で打ち切られると登録が消えたままになるので、
//!   残り時間が「1 回の上限 × 2」に満たなければ remove → add を始めない。
//! - 状態ファイル（データの根の `setup-registration.json`）に、登録した exe と登録まで済んだ CLI を覚える。
//!   済んだ CLI は二度と呼ばない（利用者が意図して外した登録を、更新のたびに復活させないため）。
//!   未完了の CLI（見つからない・失敗・時間切れ）は、次の更新フックやログオン時の更新用 exe で再試行する。
//!
//! - Toolport ゲートウェイ経由で既に Edictora が提供されている環境（[`super::toolport`] の判定が `Served`
//!   = 自分と同じ実体の実行ファイル、または `ServedElsewhere` = 自分以外の実行ファイル）では、新しい直接登録を
//!   省く（状態ファイルの `skipped`。理由は `toolport` / `toolport-foreign`）。既にある直接登録は外さない。
//!   `ServedElsewhere` は二重に起動しないよう直接登録を省いたまま、ログ（Warning）で Toolport の command を
//!   自分の実行ファイルへ向け直すよう促す。省いた後でゲートウェイがクライアントの設定から消えた、または
//!   Toolport が指す実行ファイルが無くなった（どちらも `NotConnected`）ら、次の更新フックかログオン時の
//!   更新用 exe で直接登録を足す。`toolport` / `toolport-foreign` で省いたものは、CLI の有無に関係なく
//!   毎回判定し直し、`toolport` が別の実行ファイルでの提供に変われば `toolport-foreign` に、
//!   `toolport-foreign` が自分と同じ実体に変われば `toolport` に記録を変える（同じ理由のままなら何もしない。
//!   知らない理由は旧版が読み捨てるので逆シリアル化は失敗しない）。前回未完了の
//!   CLI も、auto なら CLI があるときに同じ判定をする。判定は Windows・macOS・Linux で行い、
//!   対応外の OS では判定せず直接登録する。
//!   `%APPDATA%\Edictora\config.json` の `mcpRegistration`（[`Mode`]: auto / direct / off）で挙動を選べる。
//!   `done`（直接登録済み）の意味は変えず、どのモードでも外さない。判定は正本
//!   `install/toolport.rs`（Remnifold・Plumivox へバイト単位で同一に複製する）で行う。
//!
//! テストから本物の CLI・本物の設定に触れないよう、CLI の場所（[`CliResolver`]）・Toolport の判定
//! （[`Detector`]）・動作モード・状態フォルダーはすべて引数で受ける。実際の PATH を読むのは
//! [`resolve_from_environment`]、実際の環境で判定するのは [`detect_from_environment`]、設定を読むのは
//! [`mode_from_config_file`] だけで、これらを使うのはフック（`hooks::run_hook`）とログオン時の更新用 exe
//! （`updater_main.rs`）の本番経路に限る。

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::toolport::{self, ClientId, Detection, Env, Product};
use crate::logging::{LogLevel, log};
use crate::update::lock::{LockFile, is_held_by_another};
use crate::util::{decode_text, kill_process_tree, strip_json_comments_and_trailing_commas};

/// Toolport の判定で探す製品（`edictora mcp`）
pub const TOOLPORT_PRODUCT: Product = Product {
    binary_stem: "edictora",
    mode_arg: "mcp",
};

/// MCP の登録名
pub const SERVER_NAME: &str = "edictora";

/// 登録する exe のファイル名（`<根>\current\edictora.exe`）
pub const MAIN_EXE: &str = "edictora.exe";

/// 登録の状態を覚えるファイル（データの根 `Edictora.Data` の直下。アンインストールでも残る場所）
pub const STATE_FILE: &str = "setup-registration.json";

/// 登録の二重実行を防ぐロック（データの根の直下）
pub const LOCK_FILE: &str = "setup-registration.lock";

/// フックでの CLI 1 回の上限
pub const HOOK_CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// install フックでの全体の締め切り（Velopack は 30 秒で打ち切る。Run・PATH の処理の分を残す）
pub const INSTALL_HOOK_TOTAL: Duration = Duration::from_secs(20);

/// uninstall フックでの全体の締め切り（Velopack は 30 秒で打ち切る）
pub const UNINSTALL_HOOK_TOTAL: Duration = Duration::from_secs(20);

/// updated フックでの全体の締め切り（Velopack は 15 秒で打ち切る）
pub const UPDATED_HOOK_TOTAL: Duration = Duration::from_secs(10);

/// ログオン時の更新用 exe での CLI 1 回の上限（Velopack の打ち切りが無いので長め。フックで
/// 終わらなかった登録をここで完了させる）
pub const UPDATER_CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// ログオン時の更新用 exe での全体の締め切り
pub const UPDATER_TOTAL: Duration = Duration::from_secs(120);

/// CLI の標準出力として受け取る上限（超えた分は読み捨てて、子プロセスを詰まらせない）
const MAX_OUTPUT: usize = 64 * 1024;

/// 子プロセスの終了を確かめる間隔
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 子プロセスが終わった後、標準出力の読み終わりを待つ猶予の上限。
/// 孫プロセスが標準出力を受け継いで生き残ると EOF が来ないため、無期限には待たない
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// 登録先の AI クライアント
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Client {
    /// Claude Code（`claude mcp … --scope user`）
    Claude,
    /// Codex（`codex mcp …`）
    Codex,
}

/// 登録の対象にするクライアント（処理する順）
pub const CLIENTS: [Client; 2] = [Client::Claude, Client::Codex];

impl Client {
    /// 状態ファイルとログでの名前（CLI のコマンド名と同じ）
    pub fn name(self) -> &'static str {
        match self {
            Client::Claude => "claude",
            Client::Codex => "codex",
        }
    }

    /// PATH から探す実行ファイル名（`.exe` だけ。スクリプトのラッパーは使わない）
    pub fn exe_name(self) -> &'static str {
        match self {
            Client::Claude => "claude.exe",
            Client::Codex => "codex.exe",
        }
    }

    /// 名前から引く（状態ファイルの読み込み用）
    pub fn from_name(name: &str) -> Option<Client> {
        CLIENTS.into_iter().find(|c| c.name() == name)
    }

    /// Toolport 側のクライアント ID
    pub fn toolport_id(self) -> ClientId {
        match self {
            Client::Claude => ClientId::ClaudeCode,
            Client::Codex => ClientId::Codex,
        }
    }

    /// 登録の確認（`mcp get edictora`）
    pub fn get_args(self) -> Vec<String> {
        vec!["mcp".into(), "get".into(), SERVER_NAME.into()]
    }

    /// 登録の削除（Claude Code は利用者単位の登録だけを消すので `--scope user` を付ける）
    pub fn remove_args(self) -> Vec<String> {
        let mut args = vec!["mcp".into(), "remove".into(), SERVER_NAME.into()];
        if self == Client::Claude {
            args.extend(["--scope".into(), "user".into()]);
        }
        args
    }

    /// 登録（`mcp add [--scope user] edictora -- <exe> mcp`）
    pub fn add_args(self, executable: &Path) -> Vec<String> {
        let mut args = vec!["mcp".to_string(), "add".to_string()];
        if self == Client::Claude {
            args.extend(["--scope".into(), "user".into()]);
        }
        args.extend([
            SERVER_NAME.to_string(),
            "--".to_string(),
            executable.to_string_lossy().into_owned(),
            "mcp".to_string(),
        ]);
        args
    }
}

/// CLI の場所を決める関数（見つからなければ None）。テストはスタブの場所を返すものを渡す
pub type CliResolver<'a> = &'a dyn Fn(Client) -> Option<PathBuf>;

/// Toolport の判定をする関数。テストは固定の結果を返すものを渡す（本物の設定を読ませない）
pub type Detector<'a> = &'a dyn Fn(Client) -> Detection;

/// 実際の環境（クライアントの設定・Toolport の registry）で判定する。**フックと更新用 exe の本番経路だけが使う**
/// `executable` は直接登録に使うのと同じ絶対パス（Toolport の提供先が自分と同じ実体かの比較に使う）
pub fn detect_from_environment(client: Client, executable: &Path) -> Detection {
    toolport::detect(
        &TOOLPORT_PRODUCT,
        client.toolport_id(),
        &Env::process(),
        &executable.to_string_lossy(),
    )
}

/// 自動登録の動作モード（`config.json` の `mcpRegistration`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Toolport が提供していれば直接登録を省き、そうでなければ直接登録する（既定）
    #[default]
    Auto,
    /// 判定せず、常に直接登録する
    Direct,
    /// 自動の追加・修復をしない（アンインストール時の解除は続ける）
    Off,
}

impl Mode {
    /// 設定値から引く（小文字の `auto` / `direct` / `off` だけ）
    pub fn from_name(name: &str) -> Option<Mode> {
        match name {
            "auto" => Some(Mode::Auto),
            "direct" => Some(Mode::Direct),
            "off" => Some(Mode::Off),
            _ => None,
        }
    }
}

/// 設定ファイルの本文から `mcpRegistration` を寛容に読む（コメント・末尾のカンマを許す。項目が無ければ
/// 既定の `Auto`）。本文が壊れている・オブジェクトでない・不正な値は Err（呼び出し側がログに残して
/// `Auto` として扱う）
pub fn parse_mode(text: &str) -> Result<Mode, String> {
    let cleaned = strip_json_comments_and_trailing_commas(text.trim_start_matches('\u{feff}'));
    let value = serde_json::from_str::<Value>(&cleaned)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| {
            "設定ファイルの JSON が壊れているか、オブジェクトではありません".to_string()
        })?;
    match value.get("mcpRegistration") {
        None | Some(Value::Null) => Ok(Mode::Auto),
        Some(Value::String(name)) => Mode::from_name(name).ok_or_else(|| {
            format!("mcpRegistration の値が不正です（auto / direct / off）: {name}")
        }),
        Some(_) => Err("mcpRegistration が文字列ではありません".to_string()),
    }
}

/// 既定の設定ファイルを読んで動作モードを決める（見つからない・読めない・不正な値は `Auto`。
/// 読めない・不正な値はログに残す）。`--config` で指定した別の設定は対象外
pub fn mode_from_config_file(config_path: &Path, label: &str) -> Mode {
    let bytes = match std::fs::read(config_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Mode::Auto,
        Err(e) => {
            log(
                &format!(
                    "{label}: 設定ファイルを読めないため mcpRegistration は auto とします: {e}"
                ),
                LogLevel::Warning,
            );
            return Mode::Auto;
        }
    };
    match parse_mode(&decode_text(&bytes)) {
        Ok(mode) => mode,
        Err(message) => {
            log(
                &format!("{label}: {message}。mcpRegistration は auto とします"),
                LogLevel::Warning,
            );
            Mode::Auto
        }
    }
}

/// PATH の文字列（`;` 区切り）から、そのクライアントの `.exe` を探す。純粋関数ではないが、
/// 環境変数は読まない（PATH は引数で受ける）
pub fn find_cli_in(client: Client, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(client.exe_name()))
        .find(|candidate| candidate.is_file())
}

/// 実際の環境変数 PATH から CLI を探す。**フックと更新用 exe の本番経路だけが使う**
/// （テストで使うと、利用者の `~/.claude.json` や `~/.codex/config.toml` を書き換えてしまう）
pub fn resolve_from_environment(client: Client) -> Option<PathBuf> {
    find_cli_in(client, std::env::var_os("PATH").as_deref())
}

/// インストール先の根から、登録する exe（`<根>\current\edictora.exe`）を決める
pub fn registered_executable(root: &Path) -> PathBuf {
    root.join("current").join(MAIN_EXE)
}

/// 時間の予算（CLI 1 回の上限と、全体の締め切り）
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    per_call: Duration,
    deadline: Instant,
}

impl Budget {
    /// 今から `total` 後を締め切りにする
    pub fn new(per_call: Duration, total: Duration) -> Budget {
        Budget {
            per_call,
            deadline: Instant::now() + total,
        }
    }

    /// 締め切りまでの残り
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// 次の 1 回に使える上限（min(1 回の上限, 残り時間)）。残りが無ければ None（起動しない）
    fn next_call(&self) -> Option<Duration> {
        let remaining = self.remaining();
        (!remaining.is_zero()).then(|| self.per_call.min(remaining))
    }

    /// あと `calls` 回分（1 回の上限 × 回数）の時間が残っているか
    fn has_room_for(&self, calls: u32) -> bool {
        self.remaining() >= self.per_call.saturating_mul(calls)
    }
}

/// CLI 1 回の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResult {
    /// 終了コード 0（標準出力。上限までの分）
    Success(String),
    /// 終了コードが 0 以外
    Failed,
    /// 上限を超えたので木ごと止めた（または、終了後も標準出力が閉じなかった）
    TimedOut,
    /// 起動できなかった（理由）
    NotStarted(String),
}

/// CLI を 1 回、時間の上限つきで実行する。
///
/// - `CREATE_NO_WINDOW`、標準入力は null、標準エラーは捨てる。
/// - 標準出力は別スレッドで読み、上限（64KiB）を超えた分は読み捨てる（子を詰まらせない）。
/// - 上限を超えたら `taskkill /T /F` で子プロセスの木ごと止めて [`CallResult::TimedOut`]。
pub fn run_cli(cli: &Path, args: &[String], timeout: Duration) -> CallResult {
    let mut command = Command::new(cli);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::util::CREATE_NO_WINDOW);
    }
    let started = Instant::now();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return CallResult::NotStarted(e.to_string()),
    };
    let Some(stdout) = child.stdout.take() else {
        kill_process_tree(&mut child);
        return CallResult::NotStarted("標準出力を受け取れません".to_string());
    };
    // 読み終わりは channel で受ける（join しない。孫プロセスが標準出力を握ったままだと終わらないため）
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_capped(stdout, MAX_OUTPUT));
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(_) => {
                kill_process_tree(&mut child);
                return CallResult::TimedOut;
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            kill_process_tree(&mut child);
            return CallResult::TimedOut;
        }
        std::thread::sleep(POLL_INTERVAL.min(timeout - elapsed));
    };
    // 子は終わった。出力の読み終わりは、残り時間（最低でも少し）と猶予の小さいほうだけ待つ
    let wait = timeout
        .saturating_sub(started.elapsed())
        .max(Duration::from_millis(200))
        .min(OUTPUT_GRACE);
    let output = match receiver.recv_timeout(wait) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => return CallResult::TimedOut,
    };
    if status.success() {
        CallResult::Success(output)
    } else {
        CallResult::Failed
    }
}

/// 上限まで読み、その後は EOF まで読み捨てる
fn read_capped(mut reader: impl Read, cap: usize) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = cap.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(room)]);
            }
        }
    }
    kept
}

/// パスを比較用に正規化する（区切りを `/` に揃えて小文字にする。Windows のパスは大小を区別しない）
pub fn normalize_for_match(text: &str) -> String {
    text.replace('\\', "/").to_lowercase()
}

/// `mcp get` の出力が、指定の exe を指しているか
pub fn output_points_at(output: &str, executable: &Path) -> bool {
    normalize_for_match(output).contains(&normalize_for_match(&executable.to_string_lossy()))
}

/// 登録の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// remove → add で登録した
    Registered,
    /// 既に自分を指していたので何もしなかった
    Unchanged,
    /// CLI が見つからない
    NoCli,
    /// 締め切りが近いので始めなかった（登録は変えていない）
    Deferred,
    /// 時間切れ
    TimedOut,
    /// 失敗（理由）
    Failed(String),
    /// 直接登録を省いた（CLI は呼んでいない）
    Skipped(SkipReason),
}

impl RegisterOutcome {
    /// 登録まで済んだか（状態ファイルの done に入れるか）
    pub fn is_done(&self) -> bool {
        matches!(
            self,
            RegisterOutcome::Registered | RegisterOutcome::Unchanged
        )
    }
}

/// 1 つのクライアントへ登録する。既に自分を指す登録は触らない。
/// それ以外（無い・別の場所を指す）は remove（失敗は無視）→ add
pub fn register_client(
    client: Client,
    cli: Option<&Path>,
    executable: &Path,
    budget: &Budget,
) -> RegisterOutcome {
    let Some(cli) = cli else {
        return RegisterOutcome::NoCli;
    };
    let Some(timeout) = budget.next_call() else {
        return RegisterOutcome::Deferred;
    };
    match run_cli(cli, &client.get_args(), timeout) {
        CallResult::Success(output) if output_points_at(&output, executable) => {
            return RegisterOutcome::Unchanged;
        }
        // 無い（非 0）・別の場所を指す → 登録し直す
        CallResult::Success(_) | CallResult::Failed => {}
        // get が時間内に終わらない CLI に remove → add はさせない（次の機会に再試行）
        CallResult::TimedOut => return RegisterOutcome::TimedOut,
        CallResult::NotStarted(reason) => {
            return RegisterOutcome::Failed(format!("CLI を起動できません: {reason}"));
        }
    }
    // remove の後に打ち切られて登録が消えたままになるのを避ける
    if !budget.has_room_for(2) {
        return RegisterOutcome::Deferred;
    }
    let Some(timeout) = budget.next_call() else {
        return RegisterOutcome::Deferred;
    };
    // 登録が無くて失敗しても構わない（外した後に登録し直すだけ）
    let _ = run_cli(cli, &client.remove_args(), timeout);
    let Some(timeout) = budget.next_call() else {
        return RegisterOutcome::Deferred;
    };
    match run_cli(cli, &client.add_args(executable), timeout) {
        CallResult::Success(_) => RegisterOutcome::Registered,
        CallResult::Failed => RegisterOutcome::Failed("mcp add が失敗しました".to_string()),
        CallResult::TimedOut => RegisterOutcome::TimedOut,
        CallResult::NotStarted(reason) => {
            RegisterOutcome::Failed(format!("CLI を起動できません: {reason}"))
        }
    }
}

/// 解除の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnregisterOutcome {
    /// 自分を指す登録を消した
    Removed,
    /// remove は非 0 だったが、その後の確認で登録が無かった
    Absent,
    /// 自分を指していない・確かめられないので触らなかった（理由）
    Kept(String),
    /// CLI が見つからない
    NoCli,
    /// 締め切りのため行わなかった
    Deferred,
    /// 失敗（理由）
    Failed(String),
}

/// 1 つのクライアントから、自分を指す登録だけを解除する
pub fn unregister_client(
    client: Client,
    cli: Option<&Path>,
    executable: &Path,
    budget: &Budget,
) -> UnregisterOutcome {
    let Some(cli) = cli else {
        return UnregisterOutcome::NoCli;
    };
    let Some(timeout) = budget.next_call() else {
        return UnregisterOutcome::Deferred;
    };
    match run_cli(cli, &client.get_args(), timeout) {
        CallResult::Success(output) if output_points_at(&output, executable) => {}
        CallResult::Success(_) => {
            return UnregisterOutcome::Kept("別の場所を指す登録です".to_string());
        }
        CallResult::Failed => return UnregisterOutcome::Kept("登録がありません".to_string()),
        CallResult::TimedOut => {
            return UnregisterOutcome::Kept("確認が時間切れになりました".to_string());
        }
        CallResult::NotStarted(reason) => {
            return UnregisterOutcome::Kept(format!("CLI を起動できません: {reason}"));
        }
    }
    let Some(timeout) = budget.next_call() else {
        return UnregisterOutcome::Deferred;
    };
    match run_cli(cli, &client.remove_args(), timeout) {
        CallResult::Success(_) => return UnregisterOutcome::Removed,
        CallResult::TimedOut => {
            return UnregisterOutcome::Failed("mcp remove が時間切れになりました".to_string());
        }
        CallResult::NotStarted(reason) => {
            return UnregisterOutcome::Failed(format!("CLI を起動できません: {reason}"));
        }
        CallResult::Failed => {}
    }
    // 登録が無いときに非 0 で終わる実装がある。その後の確認で無ければ「既に無い」として成功扱い
    let Some(timeout) = budget.next_call() else {
        return UnregisterOutcome::Failed("mcp remove が失敗しました".to_string());
    };
    match run_cli(cli, &client.get_args(), timeout) {
        CallResult::Failed => UnregisterOutcome::Absent,
        _ => UnregisterOutcome::Failed("mcp remove が失敗しました".to_string()),
    }
}

/// 直接登録を省いた理由（状態ファイルの `skipped`）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Toolport ゲートウェイ経由で、自分と同じ実体の実行ファイルが提供されている
    Toolport,
    /// Toolport ゲートウェイ経由で、自分以外の実行ファイルが提供されている（直接登録は省いたまま警告する）
    ToolportForeign,
    /// `mcpRegistration` が `off`
    Off,
}

impl SkipReason {
    /// 状態ファイルでの名前
    pub fn name(self) -> &'static str {
        match self {
            SkipReason::Toolport => "toolport",
            SkipReason::ToolportForeign => "toolport-foreign",
            SkipReason::Off => "off",
        }
    }

    fn from_name(name: &str) -> Option<SkipReason> {
        match name {
            "toolport" => Some(SkipReason::Toolport),
            "toolport-foreign" => Some(SkipReason::ToolportForeign),
            "off" => Some(SkipReason::Off),
            _ => None,
        }
    }
}

/// 前回の登録の状態（`setup-registration.json`）
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct State {
    /// 登録した exe
    pub executable: Option<String>,
    /// 登録まで済んだクライアント（直接登録済み。どのモードでも外さない）
    pub done: BTreeSet<Client>,
    /// 直接登録を省いたクライアントと理由（古い状態ファイルには無く、その場合は空）
    pub skipped: BTreeMap<Client, SkipReason>,
}

/// JSON から状態を作る。壊れていれば既定（未登録）として扱い、決して失敗しない。
/// 知らないクライアント名・理由は読み捨てる
pub fn parse_state(bytes: &[u8]) -> State {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return State::default();
    };
    State {
        executable: value["executable"].as_str().map(str::to_owned),
        done: value["done"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(Client::from_name)
                    .collect()
            })
            .unwrap_or_default(),
        skipped: value["skipped"]
            .as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(name, reason)| {
                        Some((
                            Client::from_name(name)?,
                            SkipReason::from_name(reason.as_str()?)?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// 状態ファイルを読む。無い・読めない・壊れているときは既定（未登録）
pub fn load_state(state_dir: &Path) -> State {
    let path = state_dir.join(STATE_FILE);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => std::fs::read(&path)
            .map(|bytes| parse_state(&bytes))
            .unwrap_or_default(),
        _ => State::default(),
    }
}

/// 状態ファイルを書く（一時ファイルに書いてから rename で置き換える）
pub fn save_state(state_dir: &Path, state: &State) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let value = json!({
        "executable": state.executable,
        "done": state.done.iter().map(|c| c.name()).collect::<Vec<_>>(),
        "skipped": state
            .skipped
            .iter()
            .map(|(c, reason)| (c.name().to_string(), json!(reason.name())))
            .collect::<serde_json::Map<String, Value>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    let path = state_dir.join(STATE_FILE);
    let temp = state_dir.join(format!("{STATE_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&temp, &bytes)?;
    std::fs::rename(&temp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// 状態ファイルを消す（install では状態を忘れてから登録し直し、uninstall では次の導入に備えて消す）。
/// 無ければ何もしない（false）
pub fn forget_state(state_dir: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(state_dir.join(STATE_FILE)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// クライアント 1 つに対するするべきこと
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 直接登録する（CLI を呼ぶ）
    Register,
    /// 直接登録を省いて、理由を状態に残す（CLI は呼ばない）
    Skip(SkipReason),
}

/// するべきこと
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 状態を作り直す（exe が違う: 初回・移動・状態なし）
    pub fresh: bool,
    /// クライアントごとのするべきこと（処理する順）
    pub actions: Vec<(Client, Action)>,
}

/// Toolport が提供しているなら省く、そうでなければ直接登録する（新規扱いの判断）
fn decide_new(client: Client, detect: &impl Fn(Client) -> Detection) -> Action {
    match detect(client) {
        Detection::Served { .. } => Action::Skip(SkipReason::Toolport),
        Detection::ServedElsewhere { .. } => Action::Skip(SkipReason::ToolportForeign),
        _ => Action::Register,
    }
}

/// 同じ exe の状態にある 1 クライアントのするべきこと（done 済みは何もしない）
fn decide_existing(
    client: Client,
    state: &State,
    mode: Mode,
    available: &impl Fn(Client) -> bool,
    detect: &impl Fn(Client) -> Detection,
) -> Option<Action> {
    if state.done.contains(&client) {
        return None;
    }
    if mode == Mode::Off {
        // 未完了のものは off として記録する（CLI は呼ばない）。済み・省いたものは何もしない
        let untouched = !state.skipped.contains_key(&client);
        return untouched.then_some(Action::Skip(SkipReason::Off));
    }
    let register = || available(client).then_some(Action::Register);
    match (state.skipped.get(&client), mode) {
        // 省いていたものを直接登録する
        (Some(_), Mode::Direct) => register(),
        // Toolport で省いていたものは、CLI の有無に関係なく毎回判定し直す。
        // 別の実行ファイルでの提供に変わったら foreign へ、ゲートウェイが設定から消えた・実行ファイルが
        // 無くなったら直接登録に戻す。ほか（提供中・無効化・不明）は何もしない
        (Some(SkipReason::Toolport), _) => match detect(client) {
            Detection::ServedElsewhere { .. } => Some(Action::Skip(SkipReason::ToolportForeign)),
            Detection::NotConnected => register(),
            _ => None,
        },
        // foreign も毎回判定し直す。自分と同じ実体の提供に変わったら toolport へ（通常の省略）、
        // 引き続き別の実行ファイルなら何もしない（記録もログも無し）、消えたら直接登録に戻す
        (Some(SkipReason::ToolportForeign), _) => match detect(client) {
            Detection::Served { .. } => Some(Action::Skip(SkipReason::Toolport)),
            Detection::NotConnected => register(),
            _ => None,
        },
        // off で省いていたものは、auto になったので新規扱いで判定する
        (Some(SkipReason::Off), _) => match decide_new(client, detect) {
            Action::Skip(reason) => Some(Action::Skip(reason)),
            Action::Register => register(),
        },
        // 未完了（CLI が見つからない・失敗・時間切れ）は、今 CLI があるときだけ再試行する。
        // direct は判定せず直接登録、auto は CLI があるときだけ判定する（提供中なら省く）
        (None, Mode::Direct) => register(),
        (None, _) => available(client).then(|| decide_new(client, detect)),
    }
}

/// 前回の状態・今の exe・動作モードから、するべきことを決める。何もしなくてよければ None（CLI を一切呼ばない）。
///
/// - exe が違う → 全クライアントを対象にし、done・skipped を空から作り直す。off なら `Skip(Off)`、auto で
///   Toolport が提供していれば `Skip(Toolport)`、それ以外は `Register`（CLI が見つからないものも対象。
///   結果は未完了になるが、exe を保存して次回から「未完了かつ見つかるもの」だけにする）。
/// - 同じ → [`decide_existing`] のとおり。done 済みは二度と呼ばず、off は何もしない。
pub fn plan(
    state: &State,
    executable: &Path,
    mode: Mode,
    available: impl Fn(Client) -> bool,
    detect: impl Fn(Client) -> Detection,
) -> Option<Plan> {
    let current = normalize_for_match(&executable.to_string_lossy());
    let same = state
        .executable
        .as_deref()
        .is_some_and(|previous| normalize_for_match(previous) == current);
    if !same {
        let actions = CLIENTS
            .into_iter()
            .map(|c| {
                let action = match mode {
                    Mode::Off => Action::Skip(SkipReason::Off),
                    Mode::Direct => Action::Register,
                    Mode::Auto => decide_new(c, &detect),
                };
                (c, action)
            })
            .collect();
        return Some(Plan {
            fresh: true,
            actions,
        });
    }
    let actions: Vec<(Client, Action)> = CLIENTS
        .into_iter()
        .filter_map(|c| Some((c, decide_existing(c, state, mode, &available, &detect)?)))
        .collect();
    (!actions.is_empty()).then_some(Plan {
        fresh: false,
        actions,
    })
}

/// [`ensure_registered`] の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureReport {
    /// するべきことが無かった（CLI を呼んでいない）
    NothingToDo,
    /// ほかのプロセスが登録中、またはロックを取れなかった（何もしていない）
    Busy,
    /// 登録を試みた・省いた（クライアントごとの結果）
    Ran(Vec<(Client, RegisterOutcome)>),
}

/// 判定したクライアントごとに、結果の種類と（`Unknown` なら）理由を 1 行ログへ残す（結果はそのまま返す。
/// 秘密値は結果に含まれない）
fn logged_detection<'a>(detect: Detector<'a>, label: &'a str) -> impl Fn(Client) -> Detection + 'a {
    move |client| {
        let detection = detect(client);
        let message = match &detection {
            Detection::Unknown(reason) => format!(
                "{label}: Toolport の判定（{}）: {}。確かめられません: {reason}",
                client.name(),
                detection.kind()
            ),
            _ => format!(
                "{label}: Toolport の判定（{}）: {}",
                client.name(),
                detection.kind()
            ),
        };
        log(&message, LogLevel::Info);
        detection
    }
}

/// 必要なら登録する。失敗はすべてログに残すだけで、呼び出し側を止めない。
///
/// `state_dir` はデータの根（本番は `AppPaths.local_root`）、`mode` は動作モード、`resolve` は CLI の場所、
/// `detect` は Toolport の判定（必要なときだけ呼ぶ）、`label` はログの見出し。
pub fn ensure_registered(
    state_dir: &Path,
    executable: &Path,
    mode: Mode,
    resolve: CliResolver,
    detect: Detector,
    budget: &Budget,
    label: &str,
) -> EnsureReport {
    // 普段は状態ファイル 1 つと PATH の走査だけで終わる（ロックも取らない）
    if plan(
        &load_state(state_dir),
        executable,
        mode,
        |c| resolve(c).is_some(),
        detect,
    )
    .is_none()
    {
        return EnsureReport::NothingToDo;
    }
    let _lock = match LockFile::try_acquire(&state_dir.join(LOCK_FILE)) {
        Ok(lock) => lock,
        Err(e) => {
            let (message, level) = if is_held_by_another(&e) {
                (
                    "ほかのプロセスが MCP の登録中のため任せます".to_string(),
                    LogLevel::Info,
                )
            } else {
                (
                    format!("MCP の登録のロックを取れないため登録しません: {e}"),
                    LogLevel::Warning,
                )
            };
            log(&format!("{label}: {message}"), level);
            return EnsureReport::Busy;
        }
    };
    // ロックを待つ間にほかのプロセスが終えていることがある。読み直して決め直す
    let previous = load_state(state_dir);
    let Some(plan) = plan(
        &previous,
        executable,
        mode,
        |c| resolve(c).is_some(),
        logged_detection(detect, label),
    ) else {
        return EnsureReport::NothingToDo;
    };
    let mut state = if plan.fresh {
        State {
            executable: Some(executable.to_string_lossy().into_owned()),
            ..State::default()
        }
    } else {
        previous
    };
    let mut results = Vec::new();
    for (client, action) in plan.actions {
        let outcome = match action {
            Action::Skip(reason) => {
                state.skipped.insert(client, reason);
                RegisterOutcome::Skipped(reason)
            }
            Action::Register => {
                let cli = resolve(client);
                let outcome = register_client(client, cli.as_deref(), executable, budget);
                if outcome.is_done() {
                    state.done.insert(client);
                    state.skipped.remove(&client);
                } else {
                    state.done.remove(&client);
                }
                outcome
            }
        };
        let (message, level) = match &outcome {
            RegisterOutcome::Skipped(SkipReason::Toolport) => (
                "への直接登録を省きました（Toolport ゲートウェイ経由で提供されています）"
                    .to_string(),
                LogLevel::Info,
            ),
            RegisterOutcome::Skipped(SkipReason::ToolportForeign) => (
                format!(
                    "への直接登録を省きました（Toolport が別の実行ファイルで提供しています。Toolport の {SERVER_NAME} の command を {} へ向け直してください）",
                    executable.display()
                ),
                LogLevel::Warning,
            ),
            RegisterOutcome::Skipped(SkipReason::Off) => (
                "への直接登録を省きました（mcpRegistration が off です）".to_string(),
                LogLevel::Info,
            ),
            RegisterOutcome::Registered | RegisterOutcome::Unchanged | RegisterOutcome::NoCli => {
                (format!("への登録: {outcome:?}"), LogLevel::Info)
            }
            _ => (format!("への登録: {outcome:?}"), LogLevel::Warning),
        };
        log(
            &format!(
                "{label}: MCP（{SERVER_NAME}）の {} {message}",
                client.name()
            ),
            level,
        );
        results.push((client, outcome));
    }
    if let Err(e) = save_state(state_dir, &state) {
        log(
            &format!("{label}: MCP の登録の状態を保存できません: {e}"),
            LogLevel::Warning,
        );
    }
    EnsureReport::Ran(results)
}

/// 自分を指す登録だけを全クライアントから解除し、状態ファイルを消す（アンインストール用）。
/// 状態ファイルは結果に関係なく最後に消す（次に導入したときに登録し直すため）
pub fn unregister_all(
    state_dir: &Path,
    executable: &Path,
    resolve: CliResolver,
    budget: &Budget,
    label: &str,
) -> Vec<(Client, UnregisterOutcome)> {
    let mut results = Vec::new();
    for client in CLIENTS {
        let cli = resolve(client);
        let outcome = unregister_client(client, cli.as_deref(), executable, budget);
        let level = match &outcome {
            UnregisterOutcome::Failed(_) | UnregisterOutcome::Deferred => LogLevel::Warning,
            _ => LogLevel::Info,
        };
        log(
            &format!(
                "{label}: MCP（{SERVER_NAME}）の {} からの解除: {outcome:?}",
                client.name()
            ),
            level,
        );
        results.push((client, outcome));
    }
    if let Err(e) = forget_state(state_dir) {
        log(
            &format!("{label}: MCP の登録の状態を消せません: {e}"),
            LogLevel::Warning,
        );
    }
    results
}
