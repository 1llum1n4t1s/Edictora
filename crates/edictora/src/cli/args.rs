//! コマンドライン引数の解釈（C# の `CliArguments`）。外部パーサを使わない小さな実装。
//!
//! ここではブラウザも設定ファイルも触らず、形式の検証だけを行う。誤りはブラウザを開く前に `Usage` エラーで止める。

use std::time::Duration;

use crate::config::{is_valid_item_id, is_valid_locale_code, normalize_locale_code};
use crate::error::{Error, Result};
use crate::jev::DEFAULT_MODEL;
use crate::util::eq_ic;

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
                "--jev",
                "--jev-model",
            ],
            CliCommandKind::Push => &[
                "--config",
                "--profile",
                "--chrome",
                "--item",
                "--locale",
                "--yes",
                "--show-browser",
                "--jev",
                "--jev-model",
            ],
            CliCommandKind::Submit => &[
                "--config",
                "--profile",
                "--chrome",
                "--item",
                "--yes",
                "--show-browser",
                "--jev",
                "--jev-model",
            ],
            // update はオプションを取らない（配信元の差し替えは検証用の環境変数だけ）
            CliCommandKind::Help | CliCommandKind::Version | CliCommandKind::Update => &[],
        }
    }
}

/// 値を取るオプション
const VALUE_OPTIONS: [&str; 7] = [
    "--config",
    "--profile",
    "--chrome",
    "--item",
    "--locale",
    "--wait",
    "--jev-model",
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
    /// `--jev`: 操作対象の選択を TypeSafe Jev にも確かめさせる（opt-in。login では使えない）
    pub jev: bool,
    /// `--jev-model`（未指定なら None。使うときは [`CliCommand::jev_model_name`]）
    pub jev_model: Option<String>,
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
            jev: false,
            jev_model: None,
        }
    }

    /// 使う Jev のモデル名（既定 `jev-latest`）
    pub fn jev_model_name(&self) -> &str {
        self.jev_model.as_deref().unwrap_or(DEFAULT_MODEL)
    }
}

/// Jev のモデル名として受け付けるか（英数字と `.` `_` `-`、64 文字まで）。
/// 送信本文に入るだけだが、打ち間違いや制御文字を早めに止める
pub fn is_valid_model_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
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
                return Err(Error::Usage(format!(
                    "不明なコマンドです: {first}（--help で使い方を表示）"
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
            return Err(Error::Usage(format!("余分な引数です: {arg}")));
        }

        // --name=value 形式にも対応する
        let (name, mut value) = match arg.find('=') {
            Some(eq) if eq > 0 => (arg[..eq].to_lowercase(), Some(arg[eq + 1..].to_string())),
            _ => (arg.to_lowercase(), None),
        };

        if !kind.allowed_options().contains(&name.as_str()) {
            return Err(Error::Usage(format!(
                "{command_name} では {name} は使えません（--help で使い方を表示）"
            )));
        }

        if VALUE_OPTIONS.contains(&name.as_str()) {
            if value.is_none() {
                match args.get(index + 1) {
                    Some(next) if !next.starts_with("--") => {
                        value = Some(next.to_string());
                        index += 1;
                    }
                    _ => return Err(Error::Usage(format!("{name} の値がありません"))),
                }
            }
            if value.as_deref().is_none_or(|v| v.trim().is_empty()) {
                return Err(Error::Usage(format!("{name} の値が空です")));
            }
        } else if value.is_some() {
            return Err(Error::Usage(format!("{name} は値を取りません")));
        }

        let v = value.unwrap_or_default();
        match name.as_str() {
            "--config" => set_once(&mut cmd.config_path, v, &name)?,
            "--profile" => set_once(&mut cmd.profile_dir, v, &name)?,
            "--chrome" => set_once(&mut cmd.chrome_path, v, &name)?,
            "--item" => {
                let id = v.trim().to_lowercase();
                if !is_valid_item_id(&id) {
                    return Err(Error::Usage(format!(
                        "--item は 32 文字の CWS アイテム ID で指定してください: {v}"
                    )));
                }
                if !cmd.item_ids.contains(&id) {
                    cmd.item_ids.push(id);
                }
            }
            "--locale" => {
                let code = normalize_locale_code(&v);
                if !is_valid_locale_code(&code) {
                    return Err(Error::Usage(format!(
                        "--locale のロケールコードが正しくありません: {v}"
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
                        Error::Usage("--wait は 1〜120 の分数で指定してください".to_string())
                    })?;
                cmd.login_wait = Some(Duration::from_secs(minutes as u64 * 60));
            }
            "--jev-model" => {
                let model = v.trim().to_string();
                if !is_valid_model_name(&model) {
                    return Err(Error::Usage(format!(
                        "--jev-model のモデル名が正しくありません（英数字と . _ - で 64 文字まで）: {v}"
                    )));
                }
                set_once(&mut cmd.jev_model, model, &name)?;
            }
            "--yes" => cmd.yes = true,
            "--show-browser" => cmd.show_browser = true,
            "--jev" => cmd.jev = true,
            _ => {}
        }
        index += 1;
    }

    if cmd.jev_model.is_some() && !cmd.jev {
        return Err(Error::Usage(
            "--jev-model は --jev と一緒に指定してください".to_string(),
        ));
    }

    if kind == CliCommandKind::Submit {
        // 審査送信は取り返しがつかないので、複数アイテムの一括送信はさせない
        if cmd.item_ids.is_empty() {
            return Err(Error::Usage(
                "submit には --item <アイテム ID> を 1 件指定してください".to_string(),
            ));
        }
        if cmd.item_ids.len() > 1 {
            return Err(Error::Usage(
                "submit は 1 件ずつ実行してください（--item は 1 つだけ指定できます）".to_string(),
            ));
        }
        if !cmd.yes {
            return Err(Error::Usage(
                "submit は審査に送信します。実行するには --yes を付けてください".to_string(),
            ));
        }
    }

    Ok(cmd)
}

fn set_once(slot: &mut Option<String>, value: String, name: &str) -> Result<()> {
    if slot.is_some() {
        return Err(Error::Usage(format!("{name} が 2 回指定されています")));
    }
    *slot = Some(value);
    Ok(())
}

/// 使い方
pub const HELP_TEXT: &str = r"Edictora — Chrome Web Store の掲載情報を正本ファイルと一括で比較・更新するツール

使い方:
  edictora login   [--config <file>] [--profile <dir>] [--chrome <exe>] [--wait <分>]
  edictora pull    [--item <id>]... [--locale <code>]... [共通オプション]
  edictora diff    [--item <id>]... [--locale <code>]... [共通オプション]   （既定のコマンド）
  edictora push    [--item <id>]... [--locale <code>]... --yes [共通オプション]
  edictora submit  --item <id> --yes [共通オプション]
  edictora update

コマンド:
  login    専用プロファイルの Chrome を開き、手動ログインしてダッシュボードに着くまで待ちます
  pull     ダッシュボードの現在値を表示します（読み取りのみ）
  diff     正本との差分を表示します（読み取りのみ）
  push     差分のある欄だけを書き換えて下書き保存し、読み直して照合します。--yes が無ければ差分を表示して止まります
  submit   1 件のアイテムを審査に送信します（「自動的に公開する」の設定は変更しません）
  update   新しい版があればダウンロードし、終了後に差し替えます（Velopack の Setup・Portable 版のとき。
           Setup で入れたときはログオン時にも自動で行います。Portable 版は自動更新を登録しません。
           pull などの実行中は適用を後に回します。https://edictora.kagayoi.com に接続します）

共通オプション:
  --config <file>   設定ファイル（既定: %APPDATA%\Edictora\config.json）
  --profile <dir>   Chrome プロファイル（既定: 設定ファイルの profileDir、なければ %LOCALAPPDATA%\Edictora.Data\chrome-profile）
  --chrome <exe>    chrome.exe の場所（既定: 設定ファイルの chromePath、なければ自動検出）
  --item <id>       対象アイテム（複数可。省略時は設定ファイルの全アイテム）
  --locale <code>   対象ロケール（複数可。例: ja, en）
  --show-browser    Chrome のウィンドウを表示して操作する（既定はウィンドウなしで動き、作業中に前面へ出てこない）
  --jev             入力欄・言語の選択欄・言語の選択肢を選ぶたびに TypeSafe Jev にも確かめさせる（login では使えません）。
                    API トークンは環境変数 EDICTORA_JEV_API_TOKEN で渡します。画面の要素情報（役割・ラベル行など。
                    欄の値・URL・パブリッシャー ID は含まない）を api.typesafe.ai へ送ります
  --jev-model <名前> --jev で使うモデル（既定: jev-latest）
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
