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
                    "不明なコマンドです: {first}（--help で使い方を表示）",
                    "Unknown command: {first} (run --help for usage)"
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
            return Err(Error::Usage(tr!(
                "余分な引数です: {arg}",
                "Unexpected argument: {arg}"
            )));
        }

        // --name=value 形式にも対応する
        let (name, mut value) = match arg.find('=') {
            Some(eq) if eq > 0 => (arg[..eq].to_lowercase(), Some(arg[eq + 1..].to_string())),
            _ => (arg.to_lowercase(), None),
        };

        if !kind.allowed_options().contains(&name.as_str()) {
            return Err(Error::Usage(tr!(
                "{command_name} では {name} は使えません（--help で使い方を表示）",
                "{name} cannot be used with {command_name} (run --help for usage)"
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
                        return Err(Error::Usage(tr!(
                            "{name} の値がありません",
                            "{name} requires a value"
                        )));
                    }
                }
            }
            if value.as_deref().is_none_or(|v| v.trim().is_empty()) {
                return Err(Error::Usage(tr!(
                    "{name} の値が空です",
                    "The value of {name} is empty"
                )));
            }
        } else if value.is_some() {
            return Err(Error::Usage(tr!(
                "{name} は値を取りません",
                "{name} does not take a value"
            )));
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
                        "--item は 32 文字の CWS アイテム ID で指定してください: {v}",
                        "--item must be a 32-character CWS item ID: {v}"
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
                        "--locale のロケールコードが正しくありません: {v}",
                        "Invalid locale code for --locale: {v}"
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
                        Error::Usage(
                            trs!(
                                "--wait は 1〜120 の分数で指定してください",
                                "--wait must be a number of minutes from 1 to 120"
                            )
                            .to_string(),
                        )
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
                trs!(
                    "submit には --item <アイテム ID> を 1 件指定してください",
                    "submit requires exactly one --item <item ID>"
                )
                .to_string(),
            ));
        }
        if cmd.item_ids.len() > 1 {
            return Err(Error::Usage(
                trs!(
                    "submit は 1 件ずつ実行してください（--item は 1 つだけ指定できます）",
                    "Run submit for one item at a time (only one --item is allowed)"
                )
                .to_string(),
            ));
        }
        if !cmd.yes {
            return Err(Error::Usage(
                trs!(
                    "submit は審査に送信します。実行するには --yes を付けてください",
                    "submit sends the item for review. Add --yes to run it"
                )
                .to_string(),
            ));
        }
    }

    Ok(cmd)
}

fn set_once(slot: &mut Option<String>, value: String, name: &str) -> Result<()> {
    if slot.is_some() {
        return Err(Error::Usage(tr!(
            "{name} が 2 回指定されています",
            "{name} is specified more than once"
        )));
    }
    *slot = Some(value);
    Ok(())
}

/// 使い方（日本語。表示には言語を選ぶ [`help_text`] を使う）
pub const HELP_TEXT: &str = r"Edictora — Chrome Web Store の掲載情報を正本ファイルと一括で比較・更新するツール

使い方:
  edictora login   [--config <file>] [--profile <dir>] [--chrome <exe>] [--wait <分>]
  edictora pull    [--item <id>]... [--locale <code>]... [共通オプション]
  edictora diff    [--item <id>]... [--locale <code>]... [共通オプション]   （既定のコマンド）
  edictora push    [--item <id>]... [--locale <code>]... --yes [共通オプション]
  edictora submit  --item <id> --yes [共通オプション]
  edictora update
  edictora mcp

コマンド:
  login    専用プロファイルの Chrome を開き、手動ログインしてダッシュボードに着くまで待ちます
  pull     ダッシュボードの現在値を表示します（読み取りのみ）
  diff     正本との差分を表示します（読み取りのみ）
  push     差分のある欄だけを書き換えて下書き保存し、読み直して照合します。--yes が無ければ差分を表示して止まります
  submit   1 件のアイテムを審査に送信します（「自動的に公開する」の設定は変更しません）
  update   新しい版があればダウンロードし、終了後に差し替えます（Velopack の Setup・Portable 版のとき。
           Setup で入れたときはログオン時にも自動で行います。Portable 版は自動更新を登録しません。
           pull などの実行中は適用を後に回します。https://edictora.kagayoi.com に接続します）
  mcp      AI エージェント向けの MCP サーバー（標準入出力）として動きます。list_items / diff / pull / push / submit を
           ツールとして公開します（画面の表示・login・プロファイルの指定はツールから選べません）

共通オプション:
  --config <file>   設定ファイル（既定: %APPDATA%\Edictora\config.json）
  --profile <dir>   Chrome プロファイル（既定: 設定ファイルの profileDir、なければ %LOCALAPPDATA%\Edictora.Data\chrome-profile）
  --chrome <exe>    chrome.exe の場所（既定: 設定ファイルの chromePath、なければ自動検出）
  --item <id>       対象アイテム（複数可。省略時は設定ファイルの全アイテム）
  --locale <code>   対象ロケール（複数可。例: ja, en）
  --show-browser    Chrome のウィンドウを表示して操作する（既定はウィンドウなしで動き、作業中に前面へ出てこない）。
                    人が目で見るためのもの。AI エージェントや自動実行では付けないこと（利用者の画面にウィンドウが出る。
                    失敗の原因はエラー文と、失敗時に保存する画面 %LOCALAPPDATA%\Edictora.Data\diagnostics で調べる）
  --wait <分>       login で待つ時間（既定 10 分）。login は常にウィンドウを表示します
  --help            この説明を表示
  --version         バージョンを表示

終了コード:
  0  成功（差分なし。update では最新・更新の開始）
  1  実行時エラー（画面操作の失敗・照合の不一致・更新の失敗・更新の適用中など）
  2  引数・設定の誤り
  3  差分あり（diff、または --yes なしの push）
  4  書き込みを中止（審査待ち・状態を判定できない）
  5  ログインが必要（login を実行してください）
  130 中断（Ctrl+C）";

/// 使い方（英語表示用。内容は [`HELP_TEXT`] と同じに保つ）
pub const HELP_TEXT_EN: &str = r"Edictora - compare and update Chrome Web Store listings against source files in bulk

Usage:
  edictora login   [--config <file>] [--profile <dir>] [--chrome <exe>] [--wait <minutes>]
  edictora pull    [--item <id>]... [--locale <code>]... [common options]
  edictora diff    [--item <id>]... [--locale <code>]... [common options]   (default command)
  edictora push    [--item <id>]... [--locale <code>]... --yes [common options]
  edictora submit  --item <id> --yes [common options]
  edictora update
  edictora mcp

Commands:
  login    Opens Chrome with the dedicated profile and waits until you sign in manually and reach the dashboard
  pull     Shows the current values on the dashboard (read-only)
  diff     Shows the differences from the source files (read-only)
  push     Updates only the fields that differ, saves as draft, then reloads and verifies. Without --yes it shows the differences and stops
  submit   Submits one item for review (does not change the publish-automatically setting)
  update   Downloads a newer version if available and replaces the app after it exits (Velopack Setup or Portable builds.
           Setup builds also update automatically at sign-in. Portable builds do not register automatic updates.
           Applying is postponed while pull and other commands are running. Connects to https://edictora.kagayoi.com)
  mcp      Runs as an MCP server (stdio) for AI agents. Exposes list_items / diff / pull / push / submit
           as tools (showing the browser, login, and choosing a profile are not available from the tools)

Common options:
  --config <file>   Config file (default: %APPDATA%\Edictora\config.json)
  --profile <dir>   Chrome profile (default: profileDir in the config file, otherwise %LOCALAPPDATA%\Edictora.Data\chrome-profile)
  --chrome <exe>    Location of chrome.exe (default: chromePath in the config file, otherwise detected automatically)
  --item <id>       Target item (repeatable. Default: all items in the config file)
  --locale <code>   Target locale (repeatable. e.g. ja, en)
  --show-browser    Show the Chrome window while working (by default it runs without a window and never comes to the front).
                    Meant for people watching. Do not use it from AI agents or automation (a window appears on the user's screen.
                    Investigate failures with the error message and the screenshots saved at %LOCALAPPDATA%\Edictora.Data\diagnostics)
  --wait <minutes>  How long login waits (default 10 minutes). login always shows the window
  --help            Show this help
  --version         Show the version

Exit codes:
  0  Success (no differences. For update: up to date, or the update has started)
  1  Runtime error (page operation failed, verification mismatch, update failed, an update is being applied, etc.)
  2  Invalid arguments or config
  3  Differences found (diff, or push without --yes)
  4  Write aborted (pending review, or the state could not be determined)
  5  Sign-in required (run login)
  130 Cancelled (Ctrl+C)";

/// 表示言語に合わせた使い方
pub fn help_text() -> &'static str {
    trs!(HELP_TEXT, HELP_TEXT_EN)
}
