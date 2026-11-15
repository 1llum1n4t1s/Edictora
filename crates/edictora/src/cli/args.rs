//! コマンドライン引数の解釈（C# の `CliArguments`）。外部パーサを使わない小さな実装。
//!
//! ここではブラウザも設定ファイルも触らず、形式の検証だけを行う。誤りはブラウザを開く前に `Usage` エラーで止める。

use std::time::Duration;

use crate::config::{is_valid_item_id, is_valid_locale_code, normalize_locale_code};
use crate::error::{Error, Result};
use crate::util::eq_ic;
use crate::{tr, trs};

/// サブコマンド
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommandKind {
    /// 使い方を表示
    Help,
    /// バージョンを表示
    Version,
    /// Chrome を開いて手動ログインを待つ
    Login,
    /// 現在値を表示（読み取りのみ）
    Pull,
    /// 正本との差分を表示（読み取りのみ・既定）
    Diff,
    /// 差分のある欄を書き換えて下書き保存（--yes 必須）
    Push,
    /// 審査に送信（--item 1 件と --yes 必須）
    Submit,
    /// 自動更新を手動で行う（Velopack でインストールされたときだけ。Chrome も設定も使わない）
    Update,
}

impl CliCommandKind {
    /// コマンド名（小文字）
    pub fn name(self) -> &'static str {
        match self {
            CliCommandKind::Help => "help",
            CliCommandKind::Version => "version",
            CliCommandKind::Login => "login",
            CliCommandKind::Pull => "pull",
            CliCommandKind::Diff => "diff",
            CliCommandKind::Push => "push",
            CliCommandKind::Submit => "submit",
            CliCommandKind::Update => "update",
        }
    }

    /// コマンドごとに使えるオプション
    fn allowed_options(self) -> &'static [&'static str] {
        match self {
            CliCommandKind::Login => &["--config", "--profile", "--chrome", "--wait"],
            CliCommandKind::Pull | CliCommandKind::Diff => &[
                "--config",
                "--profile",
                "--chrome",
                "--item",
                "--locale",
                "--show-browser",
            ],
            CliCommandKind::Push => &[
                "--config",
                "--profile",
                "--chrome",
                "--item",
                "--locale",
                "--yes",
                "--show-browser",
            ],
            CliCommandKind::Submit => &[
                "--config",
                "--profile",
                "--chrome",
                "--item",
                "--yes",
                "--show-browser",
            ],
            // update はオプションを取らない（配信元の差し替えは検証用の環境変数だけ）
            CliCommandKind::Help | CliCommandKind::Version | CliCommandKind::Update => &[],
        }
    }
}

/// 値を取るオプション
const VALUE_OPTIONS: [&str; 6] = [
    "--config",
    "--profile",
    "--chrome",
    "--item",
    "--locale",
    "--wait",
];

/// 解釈済みのコマンドライン
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCommand {
    pub kind: CliCommandKind,
    pub config_path: Option<String>,
    pub profile_dir: Option<String>,
    pub chrome_path: Option<String>,
    /// 指定されたアイテム（未指定なら空。小文字・重複なし）
    pub item_ids: Vec<String>,
    /// 指定されたロケール（未指定なら空。「-」区切りに正規化・重複なし）
    pub locale_codes: Vec<String>,
    pub yes: bool,
    pub login_wait: Option<Duration>,
    pub show_browser: bool,
}

impl CliCommand {
    fn of(kind: CliCommandKind) -> CliCommand {
        CliCommand {
            kind,
            config_path: None,
            profile_dir: None,
            chrome_path: None,
            item_ids: Vec::new(),
            locale_codes: Vec::new(),
            yes: false,
            login_wait: None,
            show_browser: false,
        }
    }
}

/// 引数を解釈する。誤りは `Usage` エラー
pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<CliCommand> {
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    if args
        .iter()
        .any(|a| matches!(*a, "--help" | "-h" | "/?" | "help"))
    {
        return Ok(CliCommand::of(CliCommandKind::Help));
    }
    if args
        .iter()
        .any(|a| matches!(*a, "--version" | "-v" | "version"))
    {
        return Ok(CliCommand::of(CliCommandKind::Version));
    }

    let mut index = 0;
    let mut kind = CliCommandKind::Diff; // 既定は差分表示（読み取りのみ）
    if let Some(first) = args.first()
        && !first.starts_with('-')
    {
        kind = match first.to_lowercase().as_str() {
            "login" => CliCommandKind::Login,
            "pull" => CliCommandKind::Pull,
            "diff" => CliCommandKind::Diff,
            "push" => CliCommandKind::Push,
            "submit" => CliCommandKind::Submit,
            "update" => CliCommandKind::Update,
            _ => {
                return Err(Error::Usage(tr!(
                    "cli.unknown_command_run_help_for_usage",
                    first = first
                )));
            }
        };
        index = 1;
    }

    let command_name = kind.name();
    let mut cmd = CliCommand::of(kind);

    while index < args.len() {
        let arg = args[index];
        if !arg.starts_with("--") {
            return Err(Error::Usage(tr!("cli.unexpected_argument", arg = arg)));
        }

        // --name=value 形式にも対応する
        let (name, mut value) = match arg.find('=') {
            Some(eq) if eq > 0 => (arg[..eq].to_lowercase(), Some(arg[eq + 1..].to_string())),
            _ => (arg.to_lowercase(), None),
        };

        if !kind.allowed_options().contains(&name.as_str()) {
            return Err(Error::Usage(tr!(
                "cli.cannot_be_used_with_run_help_for",
                name = name,
                command_name = command_name
            )));
        }

        if VALUE_OPTIONS.contains(&name.as_str()) {
            if value.is_none() {
                match args.get(index + 1) {
                    Some(next) if !next.starts_with("--") => {
                        value = Some(next.to_string());
                        index += 1;
                    }
                    _ => {
                        return Err(Error::Usage(tr!("cli.requires_a_value", name = name)));
                    }
                }
            }
            if value.as_deref().is_none_or(|v| v.trim().is_empty()) {
                return Err(Error::Usage(tr!("cli.the_value_of_is_empty", name = name)));
            }
        } else if value.is_some() {
            return Err(Error::Usage(tr!("cli.does_not_take_a_value", name = name)));
        }

        let v = value.unwrap_or_default();
        match name.as_str() {
            "--config" => set_once(&mut cmd.config_path, v, &name)?,
            "--profile" => set_once(&mut cmd.profile_dir, v, &name)?,
            "--chrome" => set_once(&mut cmd.chrome_path, v, &name)?,
            "--item" => {
                let id = v.trim().to_lowercase();
                if !is_valid_item_id(&id) {
                    return Err(Error::Usage(tr!(
                        "cli.item_must_be_a_32_character_cws",
                        value = v
                    )));
                }
                if !cmd.item_ids.contains(&id) {
                    cmd.item_ids.push(id);
                }
            }
            "--locale" => {
                let code = normalize_locale_code(&v);
                if !is_valid_locale_code(&code) {
                    return Err(Error::Usage(tr!(
                        "cli.invalid_locale_code_for_locale",
                        value = v
                    )));
                }
                if !cmd.locale_codes.iter().any(|c| eq_ic(c, &code)) {
                    cmd.locale_codes.push(code);
                }
            }
            "--wait" => {
                // C# の int.TryParse と同じく前後の空白と符号を許す
                let minutes = v
                    .trim_matches(|c: char| {
                        matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
                    })
                    .parse::<i32>()
                    .ok()
                    .filter(|m| (1..=120).contains(m))
                    .ok_or_else(|| {
                        Error::Usage(trs!("cli.wait_must_be_a_number_of_minutes").to_string())
                    })?;
                cmd.login_wait = Some(Duration::from_secs(minutes as u64 * 60));
            }
            "--yes" => cmd.yes = true,
            "--show-browser" => cmd.show_browser = true,
            _ => {}
        }
        index += 1;
    }

    if kind == CliCommandKind::Submit {
        // 審査送信は取り返しがつかないので、複数アイテムの一括送信はさせない
        if cmd.item_ids.is_empty() {
            return Err(Error::Usage(
                trs!("cli.submit_requires_exactly_one_item_item_id").to_string(),
            ));
        }
        if cmd.item_ids.len() > 1 {
            return Err(Error::Usage(
                trs!("cli.run_submit_for_one_item_at_a").to_string(),
            ));
        }
        if !cmd.yes {
            return Err(Error::Usage(
                trs!("cli.submit_sends_the_item_for_review_add").to_string(),
            ));
        }
    }

    Ok(cmd)
}

fn set_once(slot: &mut Option<String>, value: String, name: &str) -> Result<()> {
    if slot.is_some() {
        return Err(Error::Usage(tr!(
            "cli.is_specified_more_than_once",
            name = name
        )));
    }
    *slot = Some(value);
    Ok(())
}

/// 表示言語に合わせた使い方
pub fn help_text() -> &'static str {
    trs!("cli.help")
}
