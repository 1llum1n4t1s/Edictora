//! Velopack のフック（`--veloapp-*`）の解釈と処理。
//!
//! vendored の SDK（app.rs の `VelopackApp::run` と constants.rs）と同じ約束にする:
//! - 引数が 2 つ以上あり、1 つ目を小文字にしたものが `--veloapp-install` / `--veloapp-updated` /
//!   `--veloapp-obsolete` / `--veloapp-uninstall` のどれかならフック。2 つ目は版（SemVer）。
//! - 版を解釈できればフックの処理を行い、できなくても処理の後は必ず終了コード 0 で終える（`call_fast_hook`）。
//! - 時間制限: install と uninstall は 30 秒、updated と obsolete は 15 秒で Update.exe に止められる
//!   （`on_after_install_fast_callback` などの説明）。重い処理はしない。
//!
//! Edictora の処理:
//! - install / updated: HKCU の Run に更新用 exe（`<根>\current\edictora-updater.exe`）を登録し、
//!   ユーザー環境変数 PATH に `<根>\current` を足す（どちらも既にあれば変えない）。
//!   Portable 版（根に `.portable`）では登録しない。Run の同じ名前の値が別の場所の更新用 exe を
//!   指していれば上書きしない（警告をログへ）。
//! - uninstall: その両方を取り除く（Run は内容が自分の値と一致するときだけ。根のフォルダーは Velopack が消す。
//!   データの `Edictora.Data` は残す）。
//! - obsolete: 何もしない。
//!
//! Run の値の名前と PATH の要素は、インストール先の根から決める（packId を変えた検証用インストールと
//! 本番のインストールが互いの登録を上書き・削除しないように）。Velopack でインストールされた場所
//! （`<根>\Update.exe` と `<根>\current\sq.version` がある）から起動されていなければ、レジストリに一切触れない。

use std::path::Path;

use crate::logging::{LogLevel, log};
use crate::update::locate::locate_current;
use crate::update::version::Version;

/// 更新用 exe の名前（Velopack のパッケージに edictora.exe と並べて入れる）
pub const UPDATER_EXE: &str = "edictora-updater.exe";

/// フックの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    /// `--veloapp-install`（インストール直後）
    Install,
    /// `--veloapp-updated`（更新後、新しい版で）
    Updated,
    /// `--veloapp-obsolete`（更新前、古い版で）
    Obsolete,
    /// `--veloapp-uninstall`（アンインストール前）
    Uninstall,
}

/// フック 1 回分
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    pub kind: HookKind,
    /// 版（解釈できなければ None。その場合は処理せずに終了する）
    pub version: Option<Version>,
}

/// 引数がフックなら解釈する（SDK と同じく 2 つ以上・1 つ目だけを小文字にして照合）
pub fn parse_hook<S: AsRef<str>>(args: &[S]) -> Option<Hook> {
    if args.len() < 2 {
        return None;
    }
    let kind = match args[0].as_ref().to_ascii_lowercase().as_str() {
        "--veloapp-install" => HookKind::Install,
        "--veloapp-updated" => HookKind::Updated,
        "--veloapp-obsolete" => HookKind::Obsolete,
        "--veloapp-uninstall" => HookKind::Uninstall,
        _ => return None,
    };
    Some(Hook {
        kind,
        version: Version::parse(args[1].as_ref()),
    })
}

/// インストール先ごとの登録内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// Run の値の名前（例 `Edictora Updater`。根のフォルダー名から作る）
    pub run_value_name: String,
    /// Run の値（更新用 exe を引用符で囲んだもの）
    pub run_command: String,
    /// PATH に足す要素（`<根>\current`）
    pub path_entry: String,
}

/// 根から登録内容を決める
pub fn registration_for(root: &Path) -> Registration {
    let leaf = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| crate::app_paths::APP_NAME.to_string());
    let current = root.join("current");
    Registration {
        run_value_name: format!("{leaf} Updater"),
        run_command: format!("\"{}\"", current.join(UPDATER_EXE).display()),
        path_entry: current.display().to_string(),
    }
}

/// フックでレジストリに対して行うこと
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookAction {
    /// Run と PATH を登録する
    Register,
    /// 自分の Run と PATH を取り除く
    Unregister,
    /// 何もしない
    Nothing,
}

/// フックの種類と Portable かどうかから、行うことを決める。
///
/// Portable 版（根に `.portable` がある。Velopack の Portable.zip を展開したもの）は、利用者が置いた
/// 場所をログオン時に起動させたり PATH に足したりしない（自動更新の登録をしない。手動の
/// `edictora update` は使える）。uninstall は自分の値（内容が一致するものだけ）の掃除なので Portable でも行う
pub fn hook_action(kind: HookKind, portable: bool) -> HookAction {
    match kind {
        HookKind::Install | HookKind::Updated if portable => HookAction::Nothing,
        HookKind::Install | HookKind::Updated => HookAction::Register,
        HookKind::Uninstall => HookAction::Unregister,
        HookKind::Obsolete => HookAction::Nothing,
    }
}

/// Portable 版の目印（根の `.portable`。vendored の locator.rs の `create_config_from_root_dir` と同じ）
pub fn is_portable(root: &Path) -> bool {
    root.join(".portable").exists()
}

/// Run の値の扱いの結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunChange {
    /// 書いた・消した
    Changed,
    /// 既に同じ（登録）・無い（解除）ので何もしなかった
    Unchanged,
    /// 同じ名前の値が別の内容（別の場所の更新用 exe など）なので触らなかった（その値）
    KeptOther(String),
}

/// 登録・解除の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryReport {
    pub run: RunChange,
    /// PATH を変えたか
    pub path_changed: bool,
}

/// Run の値の比較用（前後の空白と引用符を外し、大文字小文字を区別しない）
fn normalize_command(command: &str) -> String {
    command.trim().trim_matches('"').trim().to_lowercase()
}

/// Run の値が更新用 exe を指しているか（別の場所の Edictora の登録かを見分ける）
fn points_to_updater(command: &str) -> bool {
    normalize_command(command).ends_with(&UPDATER_EXE.to_lowercase())
}

/// Run の値が「いま実在する」別の場所の更新用 exe を指しているか。
/// アンインストールせずにフォルダーだけ消した古い登録（指す先の exe が無い）は守る必要がないので、
/// 上書きしてよい（守り続けると、同じフォルダー名の新しいインストールが自動更新を登録できない）
fn points_to_existing_updater(command: &str) -> bool {
    points_to_updater(command)
        && std::path::Path::new(command.trim().trim_matches('"').trim()).is_file()
}

#[cfg(windows)]
mod apply {
    use std::io;

    use super::{
        Registration, RegistryReport, RunChange, normalize_command, points_to_existing_updater,
    };
    use crate::install::registry::{
        RegKey, add_path_entry, delete_run_value, read_run_value, remove_path_entry, set_run_value,
    };

    /// 登録する（Run と PATH）。
    ///
    /// Run の同じ名前の値が、別の場所に実在する更新用 exe を指していれば上書きしない（根のフォルダー名が
    /// 同じ別のインストールを壊さない）。それ以外（無い・同じ・更新用 exe ではない値・指す先の exe が
    /// 消えている古い登録）なら自分の値にする
    pub fn register(
        root: &RegKey,
        run_subkey: &str,
        env_subkey: &str,
        reg: &Registration,
    ) -> io::Result<RegistryReport> {
        let run = match read_run_value(root, run_subkey, &reg.run_value_name)? {
            Some(current)
                if normalize_command(&current) != normalize_command(&reg.run_command)
                    && points_to_existing_updater(&current) =>
            {
                RunChange::KeptOther(current)
            }
            _ => {
                if set_run_value(root, run_subkey, &reg.run_value_name, &reg.run_command)? {
                    RunChange::Changed
                } else {
                    RunChange::Unchanged
                }
            }
        };
        let path_changed = add_path_entry(root, env_subkey, &reg.path_entry)?;
        Ok(RegistryReport { run, path_changed })
    }

    /// 取り除く（Run と PATH）。Run は現在の値が自分の値と一致するときだけ消す。
    /// 片方が失敗しても、もう片方は試す
    pub fn unregister(
        root: &RegKey,
        run_subkey: &str,
        env_subkey: &str,
        reg: &Registration,
    ) -> io::Result<RegistryReport> {
        let run = (|| -> io::Result<RunChange> {
            match read_run_value(root, run_subkey, &reg.run_value_name)? {
                None => Ok(RunChange::Unchanged),
                Some(current)
                    if normalize_command(&current) == normalize_command(&reg.run_command) =>
                {
                    delete_run_value(root, run_subkey, &reg.run_value_name)?;
                    Ok(RunChange::Changed)
                }
                Some(other) => Ok(RunChange::KeptOther(other)),
            }
        })();
        let path = remove_path_entry(root, env_subkey, &reg.path_entry);
        Ok(RegistryReport {
            run: run?,
            path_changed: path?,
        })
    }
}

#[cfg(windows)]
pub use apply::{register, unregister};

/// フックを処理する（本物の HKCU を使う）。終了コードは常に 0（SDK と同じ）。結果はログへ残す
pub fn run_hook(hook: &Hook) -> i32 {
    let Some(version) = &hook.version else {
        log(
            &format!(
                "Velopack のフック {:?}: 版を解釈できないため何もしません",
                hook.kind
            ),
            LogLevel::Warning,
        );
        return 0;
    };
    log(
        &format!("Velopack のフック {:?}（版 {version}）", hook.kind),
        LogLevel::Info,
    );
    if hook.kind == HookKind::Obsolete {
        return 0;
    }
    let installation = match locate_current() {
        Ok(i) => i,
        Err(e) => {
            // 開発中の exe などを誤って登録しない
            log(
                &format!(
                    "インストール先から起動されていないため、登録を変更しません: {}",
                    e.message()
                ),
                LogLevel::Warning,
            );
            return 0;
        }
    };
    let portable = is_portable(&installation.root);
    let action = hook_action(hook.kind, portable);
    if action == HookAction::Nothing {
        log(
            "Portable 版のため、ログオン時の自動更新と PATH は登録しません（手動の edictora update は使えます）",
            LogLevel::Info,
        );
        return 0;
    }
    let reg = registration_for(&installation.root);
    apply_to_current_user(action, &reg);
    0
}

#[cfg(windows)]
fn apply_to_current_user(action: HookAction, reg: &Registration) {
    use crate::install::registry::{
        ENVIRONMENT_SUBKEY, RUN_SUBKEY, RegKey, broadcast_environment_change,
    };
    let root = RegKey::current_user();
    let result = match action {
        HookAction::Register => {
            register(&root, RUN_SUBKEY, ENVIRONMENT_SUBKEY, reg).map(|r| ("登録", r))
        }
        HookAction::Unregister => {
            unregister(&root, RUN_SUBKEY, ENVIRONMENT_SUBKEY, reg).map(|r| ("登録の解除", r))
        }
        HookAction::Nothing => return,
    };
    match result {
        Ok((what, report)) => {
            if let RunChange::KeptOther(other) = &report.run {
                log(
                    &format!(
                        "Run の {} は別の内容（{other}）なので変更しません（別のインストールの登録の可能性があります）",
                        reg.run_value_name
                    ),
                    LogLevel::Warning,
                );
            }
            log(
                &format!(
                    "ログオン時の自動更新（Run の {}: {:?}）と PATH（{}）の{what}を行いました",
                    reg.run_value_name, report.run, reg.path_entry
                ),
                LogLevel::Info,
            );
            // レジストリを書き終えてから通知する（時間制限で止められても登録は残る）
            if report.path_changed {
                broadcast_environment_change();
            }
        }
        Err(e) => log(
            &format!("ログオン時の自動更新・PATH の変更に失敗しました: {e}"),
            LogLevel::Error,
        ),
    }
}

#[cfg(not(windows))]
fn apply_to_current_user(_action: HookAction, _reg: &Registration) {}
