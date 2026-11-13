//! CLI（C# の `Edictora.Cli`）。手順の中身は [`crate::services`] に任せ、ここは入出力と終了コードだけを扱う。
//!
//! 設定・引数の誤りは、Chrome を開く前にすべて検出して終了コード 2 で止める。
//! 画面へ出す文字列にも publisherId が混ざらないよう、エラーは `logging::redact` を通して出す。
//! 進捗は標準エラー、結果は標準出力へ出す（スクリプトから扱いやすくする）。

pub mod args;
pub mod diff_formatter;

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::app_paths::AppPaths;
use crate::cancel::CancelToken;
use crate::config::{self, EdictoraConfig};
use crate::dashboard::browser::{DashboardBrowser, DashboardBrowserOptions};
use crate::dashboard::item_page::DashboardItemPage;
use crate::dashboard::jev_assist::JevAssist;
use crate::error::{Error, Result};
use crate::jev::client::{JevClient, JevToken};
use crate::listing::{ComparisonStatus, ExpectedItem, ListingField, build_expected};
use crate::logging::{LogLevel, log, log_error_detail, redact};
use crate::services::{self, LocalePushOutcome};
use crate::update::UpdateOutcome;
use crate::update::lock::{LockFile, is_held_by_another};
use crate::update::source::UpdateSource;
use crate::util::{eq_ic, format_n0, utf16_len};
use args::{CliCommand, CliCommandKind, HELP_TEXT};

/// 終了コード
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const FAILURE: i32 = 1;
    pub const USAGE: i32 = 2;
    pub const DIFFERENCES: i32 = 3;
    pub const BLOCKED: i32 = 4;
    pub const LOGIN_REQUIRED: i32 = 5;
    pub const CANCELLED: i32 = 130;
}

/// login の既定の待ち時間
const DEFAULT_LOGIN_WAIT: Duration = Duration::from_secs(10 * 60);

/// ブラウザを開く関数（テストでは Chrome を開かないものに差し替える）
pub type BrowserOpener<'a> =
    dyn Fn(&DashboardBrowserOptions, &CancelToken) -> Result<DashboardBrowser> + 'a;

/// 実際に Chrome を開く既定の関数
pub fn open_real_browser(
    options: &DashboardBrowserOptions,
    cancel: &CancelToken,
) -> Result<DashboardBrowser> {
    DashboardBrowser::open(options, cancel)
}

/// 更新処理を行う関数（配信元・実行中ロックの場所・進捗・中断を受け取る）。
/// 本物は [`run_real_update`]。既定（テスト）は [`refuse_update`] で、ネットワークにも Update.exe にも届かない
pub type UpdateRunner<'a> =
    dyn Fn(&UpdateSource, &Path, &dyn Fn(&str), &CancelToken) -> Result<UpdateOutcome> + 'a;

/// 実行中の exe のインストール先で、本物の配信元・Update.exe を使って更新する（`main` が渡す）
pub fn run_real_update(
    source: &UpdateSource,
    running_lock: &Path,
    progress: &dyn Fn(&str),
    cancel: &CancelToken,
) -> Result<UpdateOutcome> {
    crate::update::run_installed_update(source, running_lock, progress, cancel)
}

/// 既定の更新処理（何もせずに失敗する）。`main` 以外（テスト）から本物の更新に届かないようにする
pub fn refuse_update(
    _source: &UpdateSource,
    _running_lock: &Path,
    _progress: &dyn Fn(&str),
    _cancel: &CancelToken,
) -> Result<UpdateOutcome> {
    Err(Error::Update(
        "この実行環境では更新処理を使えません".to_string(),
    ))
}

/// `--jev` の設定（トークンは検証済み。値は表示しない）
struct JevSettings {
    token: JevToken,
    model: String,
}

/// CLI のコマンド実行
pub struct CliApp<'a> {
    out: RefCell<&'a mut dyn Write>,
    err: RefCell<&'a mut dyn Write>,
    paths: AppPaths,
    open_browser: &'a BrowserOpener<'a>,
    show_browser: bool,
    /// 環境変数 EDICTORA_JEV_API_TOKEN の生の値（main が読んで渡す。テストは任意の値を渡せる）
    jev_token_source: Option<String>,
    /// `--jev` のときだけ Some
    jev: Option<JevSettings>,
    /// `update` で使う更新処理（既定は refuse_update）
    update_runner: &'a UpdateRunner<'a>,
    /// 環境変数 EDICTORA_UPDATE_URL の生の値（main が読んで渡す。None なら本番の配信元）
    update_source_override: Option<String>,
}

impl<'a> CliApp<'a> {
    /// 作る。`paths` は保存先、`open_browser` はブラウザを開く関数
    pub fn new(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        paths: AppPaths,
        open_browser: &'a BrowserOpener<'a>,
    ) -> CliApp<'a> {
        CliApp {
            out: RefCell::new(out),
            err: RefCell::new(err),
            paths,
            open_browser,
            show_browser: false,
            jev_token_source: None,
            jev: None,
            update_runner: &refuse_update,
            update_source_override: None,
        }
    }

    /// `update` で使う更新処理と、配信元を差し替える値（環境変数 EDICTORA_UPDATE_URL の値）を渡す。
    /// 環境変数を読むのは main だけにし、テストでは偽の更新処理を差し込む
    pub fn with_update(
        mut self,
        runner: &'a UpdateRunner<'a>,
        source_override: Option<String>,
    ) -> CliApp<'a> {
        self.update_runner = runner;
        self.update_source_override = source_override;
        self
    }

    /// `--jev` で使う API トークン（環境変数 EDICTORA_JEV_API_TOKEN の値）を渡す。
    /// 環境変数を読むのは main だけにし、テストではプロセスの環境変数に触れずに値を差し込む
    pub fn with_jev_token(mut self, token: Option<String>) -> CliApp<'a> {
        self.jev_token_source = token;
        self
    }

    fn outln(&self, text: &str) {
        let mut w = self.out.borrow_mut();
        let _ = writeln!(w, "{text}");
        let _ = w.flush();
    }

    fn out_raw(&self, text: &str) {
        let mut w = self.out.borrow_mut();
        let _ = write!(w, "{text}");
        let _ = w.flush();
    }

    fn errln(&self, text: &str) {
        let mut w = self.err.borrow_mut();
        let _ = writeln!(w, "{text}");
        let _ = w.flush();
    }

    /// 進捗は標準エラーへ（publisherId は伏せる）
    fn progress(&self) -> impl Fn(&str) + '_ {
        move |m: &str| self.errln(&redact(m))
    }

    /// 引数を解釈して実行し、終了コードを返す
    pub fn run<S: AsRef<str>>(&mut self, args: &[S], cancel: &CancelToken) -> i32 {
        let command = match args::parse(args) {
            Ok(c) => c,
            Err(e) => {
                self.errln(&format!("エラー: {}", e.message()));
                return exit_codes::USAGE;
            }
        };

        match command.kind {
            CliCommandKind::Help => {
                self.outln(HELP_TEXT);
                return exit_codes::SUCCESS;
            }
            CliCommandKind::Version => {
                self.outln(&format!("edictora {}", env!("CARGO_PKG_VERSION")));
                return exit_codes::SUCCESS;
            }
            // 更新は設定を読まない（設定が壊れていても更新できるように）。Chrome も開かない
            CliCommandKind::Update => return self.update(cancel),
            _ => {}
        }

        // ── --jev のトークン（まだ Chrome は開かない。値はどこにも出さない）──
        if command.jev {
            match JevToken::parse(self.jev_token_source.as_deref()) {
                Ok(token) => {
                    self.jev = Some(JevSettings {
                        token,
                        model: command.jev_model_name().to_string(),
                    });
                }
                Err(e) => {
                    self.errln(&format!("エラー: {}", e.message()));
                    return exit_codes::USAGE;
                }
            }
        }

        // ── ここから先は設定の検証（まだ Chrome は開かない）──
        let prepared = config::load(
            command.config_path.as_deref(),
            command.profile_dir.as_deref(),
            command.chrome_path.as_deref(),
            command.kind != CliCommandKind::Login,
            &self.paths,
        )
        .and_then(|config| {
            let targets = if command.kind != CliCommandKind::Login {
                select_targets(&config, &command)?
            } else {
                Vec::new()
            };
            Ok((config, targets))
        });
        let (config, targets) = match prepared {
            Ok(v) => v,
            Err(e @ (Error::Config(_) | Error::Usage(_))) => {
                self.errln(&format!("エラー: {}", e.message()));
                return exit_codes::USAGE;
            }
            Err(e) => {
                // 設定の段階で起きるのは Config / Usage だけのはずだが、念のため実行時エラーとして扱う
                self.errln(&format!("エラー: {}", redact(&e.message())));
                return exit_codes::FAILURE;
            }
        };

        // ── 実行中の印（更新の適用側に、Chrome を開くコマンドが動いていることを知らせる）──
        // Update.exe の apply がインストール先配下のプロセスを止めて push の途中で落とすのを防ぐため、
        // ブラウザ操作の間だけ共有ロックを持つ（CLI 同士は同時に動ける）。更新の適用中（排他で持たれている）
        // なら Chrome を開かずに止める。ロックファイル自体を開けない（フォルダーを作れないなど）ときは
        // 使えなくしないよう、ログに残して続ける（インストールされていない実行でも無害）
        let _running = match LockFile::try_acquire_shared(&self.paths.running_lock_path()) {
            Ok(lock) => Some(lock),
            Err(e) if is_held_by_another(&e) => {
                self.errln(
                    "エラー: edictora の更新を適用している最中です。少し待ってから実行し直してください。",
                );
                return exit_codes::FAILURE;
            }
            Err(e) => {
                log(
                    &format!("実行中を知らせるロックファイルを開けませんでした（続行します）: {e}"),
                    LogLevel::Warning,
                );
                None
            }
        };

        // ── ブラウザ操作 ──
        self.show_browser = command.show_browser;
        if let Some(jev) = &self.jev {
            self.errln(&format!(
                "Jev モード: 入力欄・言語の選択欄・言語の選択肢を選ぶたびに TypeSafe Jev（モデル {}）にも確かめます。画面の要素情報（役割・ラベル行など）を api.typesafe.ai へ送ります。",
                jev.model
            ));
        }
        let result = match command.kind {
            CliCommandKind::Login => self.login(&config, &command, cancel),
            CliCommandKind::Pull => self.pull(&config, &targets, cancel),
            CliCommandKind::Diff => self.diff(&config, &targets, cancel, false),
            CliCommandKind::Push if command.yes => self.push(&config, &targets, cancel),
            CliCommandKind::Push => self.diff(&config, &targets, cancel, true),
            CliCommandKind::Submit => self.submit(&config, &targets[0], cancel),
            CliCommandKind::Help | CliCommandKind::Version | CliCommandKind::Update => {
                Ok(exit_codes::USAGE)
            }
        };
        match result {
            Ok(code) => code,
            Err(Error::Cancelled) => {
                self.errln("中断しました。");
                exit_codes::CANCELLED
            }
            Err(Error::LoginRequired) => {
                self.errln(&format!("エラー: {}", Error::LoginRequired.message()));
                exit_codes::LOGIN_REQUIRED
            }
            Err(Error::WriteBlocked(m)) => {
                self.errln(&format!("中止: {}", redact(&m)));
                exit_codes::BLOCKED
            }
            Err(e) => {
                log_error_detail(
                    &format!("{} の実行に失敗", command.kind.name()),
                    &format!("{}: {}", e.kind_name(), e.message()),
                );
                self.errln(&format!("エラー: {}", redact(&e.message())));
                exit_codes::FAILURE
            }
        }
    }

    /// `update`: 更新を確認し、新しい版があればダウンロードして Update.exe に適用を任せる。
    /// 結果は標準エラーへ出す（標準出力は使わない）。最新・更新の開始は 0、失敗・別の更新処理が動いているときは 1
    fn update(&self, cancel: &CancelToken) -> i32 {
        let source = match UpdateSource::from_override(self.update_source_override.as_deref()) {
            Ok(s) => s,
            Err(m) => {
                self.errln(&format!("エラー: {m}"));
                return exit_codes::USAGE;
            }
        };
        let progress = |m: &str| {
            log(&format!("update: {m}"), LogLevel::Info);
            self.errln(m);
        };
        let running_lock = self.paths.running_lock_path();
        match (self.update_runner)(&source, &running_lock, &progress, cancel) {
            Ok(outcome) => {
                let message = outcome.message();
                log(&format!("update: {message}"), LogLevel::Info);
                self.errln(&message);
                match outcome {
                    UpdateOutcome::Busy | UpdateOutcome::AppInUse { .. } => exit_codes::FAILURE,
                    _ => exit_codes::SUCCESS,
                }
            }
            Err(Error::Cancelled) => {
                self.errln("中断しました。");
                exit_codes::CANCELLED
            }
            Err(e) => {
                log_error_detail(
                    "update の実行に失敗",
                    &format!("{}: {}", e.kind_name(), e.message()),
                );
                self.errln(&format!("エラー: {}", e.message()));
                exit_codes::FAILURE
            }
        }
    }

    fn browser_options(
        &self,
        config: &EdictoraConfig,
        show_browser: bool,
    ) -> DashboardBrowserOptions {
        DashboardBrowserOptions {
            profile_directory: config.profile_directory.clone(),
            chrome_path: config.chrome_path.clone(),
            show_browser,
            diagnostics_directory: self.paths.diagnostics_directory(),
        }
    }

    fn login(
        &self,
        config: &EdictoraConfig,
        command: &CliCommand,
        cancel: &CancelToken,
    ) -> Result<i32> {
        self.outln(&format!(
            "Chrome を開きます（プロファイル: {}）",
            config.profile_directory.display()
        ));
        // ログインは利用者が画面で操作するので、常にウィンドウを表示する
        let options = self.browser_options(config, false).visible();
        let mut browser = (self.open_browser)(&options, cancel)?;
        let progress = self.progress();
        let result = browser.wait_for_login(
            command.login_wait.unwrap_or(DEFAULT_LOGIN_WAIT),
            &progress,
            cancel,
        );
        browser.close();
        result?;
        self.outln(
            "ログイン状態をプロファイルに保存しました。以後は pull / diff / push が使えます。",
        );
        Ok(exit_codes::SUCCESS)
    }

    /// 編集画面の操作を作る。`--jev` なら Jev の選択役を差し込む（採用結果と確信度は進捗へ出す）
    fn item_page<'b>(&'b self, browser: &'b DashboardBrowser) -> Result<DashboardItemPage<'b>> {
        let page = browser.create_item_page()?;
        Ok(match &self.jev {
            None => page,
            Some(jev) => page.with_jev(JevAssist::new(
                Box::new(JevClient::new(jev.token.clone(), &jev.model)),
                Box::new(self.progress()),
            )),
        })
    }

    /// ログイン済みのブラウザを開く。既定はヘッドレス（作業中に前面へ出てこない）。--show-browser のときだけウィンドウを表示する
    fn open_logged_in(
        &self,
        config: &EdictoraConfig,
        cancel: &CancelToken,
    ) -> Result<DashboardBrowser> {
        let mut browser =
            (self.open_browser)(&self.browser_options(config, self.show_browser), cancel)?;
        // 失敗したら drop で閉じる（自分で起動した Chrome は Browser.close で正常終了させる）
        browser.ensure_logged_in(cancel)?;
        Ok(browser)
    }

    fn pull(
        &self,
        config: &EdictoraConfig,
        targets: &[ExpectedItem],
        cancel: &CancelToken,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = self.item_page(&browser)?;
        let progress = self.progress();
        let mut failed = false;

        for expected in targets {
            let snapshot = services::pull(&mut page, expected, &progress, cancel)?;
            self.outln(&item_header(
                expected,
                snapshot.status_text.as_deref(),
                &snapshot.available_locales,
            ));
            if let Some(err) = &snapshot.error {
                failed = true;
                self.outln(&format!("  エラー: {err}"));
                continue;
            }
            for locale in &expected.locales {
                let current = snapshot.find(&locale.code);
                let current = match current {
                    Some(c) if c.error.is_none() => c,
                    other => {
                        failed = true;
                        let msg = other
                            .and_then(|c| c.error.clone())
                            .unwrap_or_else(|| "読み出せませんでした".to_string());
                        self.outln(&format!("-- [{}] エラー: {msg}", locale.code));
                        continue;
                    }
                };
                for field in locale.fields() {
                    let value = current.values.get(&field).cloned().unwrap_or_default();
                    if field == ListingField::Description {
                        self.outln(&format!(
                            "-- [{}] {}（{} 文字）",
                            locale.code,
                            field.display_name(),
                            format_n0(utf16_len(&value))
                        ));
                    } else {
                        self.outln(&format!("-- [{}] {}", locale.code, field.display_name()));
                    }
                    self.outln(&value);
                }
            }
            self.outln("");
        }
        Ok(if failed {
            exit_codes::FAILURE
        } else {
            exit_codes::SUCCESS
        })
    }

    fn diff(
        &self,
        config: &EdictoraConfig,
        targets: &[ExpectedItem],
        cancel: &CancelToken,
        push_hint: bool,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = self.item_page(&browser)?;
        let progress = self.progress();
        let mut any_diff = false;
        let mut any_error = false;

        for expected in targets {
            let (snapshot, rows) = services::diff(&mut page, expected, &progress, cancel)?;
            self.outln(&item_header(
                expected,
                snapshot.status_text.as_deref(),
                &snapshot.available_locales,
            ));
            for row in &rows {
                self.out_raw(&diff_formatter::format(row));
                let status = row.status();
                any_diff |= status == ComparisonStatus::Different;
                any_error |= matches!(
                    status,
                    ComparisonStatus::Error | ComparisonStatus::NotFetched
                );
            }
            self.outln("");
        }

        if push_hint && any_diff {
            self.outln("差分のある欄を書き換えるには、--yes を付けて push を再実行してください（下書き保存まで行い、審査には送信しません）。");
        }
        Ok(if any_error {
            exit_codes::FAILURE
        } else if any_diff {
            exit_codes::DIFFERENCES
        } else {
            exit_codes::SUCCESS
        })
    }

    fn push(
        &self,
        config: &EdictoraConfig,
        targets: &[ExpectedItem],
        cancel: &CancelToken,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = self.item_page(&browser)?;
        let progress = self.progress();

        // 1 件でも失敗したら残りは行わない（エラーがそのまま run へ上がる）
        for expected in targets {
            let result = services::push(&mut page, expected, &progress, cancel)?;
            self.outln(&format!("== {} ({})", result.item_name, result.item_id));
            for locale in &result.locales {
                let line = if locale.outcome == LocalePushOutcome::Saved {
                    let names: Vec<&str> = locale
                        .updated_fields
                        .iter()
                        .map(|f| f.display_name())
                        .collect();
                    format!(
                        "  [{}] 下書き保存・照合済み: {}",
                        locale.locale,
                        names.join("・")
                    )
                } else {
                    format!("  [{}] 変更なし（正本と一致）", locale.locale)
                };
                self.outln(&line);
            }
        }
        self.outln("完了しました。審査への送信は行っていません（必要なら submit を 1 件ずつ実行してください）。");
        Ok(exit_codes::SUCCESS)
    }

    fn submit(
        &self,
        config: &EdictoraConfig,
        target: &ExpectedItem,
        cancel: &CancelToken,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = self.item_page(&browser)?;
        let progress = self.progress();

        let mut confirm = |dialog: &crate::dashboard::SubmitDialogInfo| -> Result<bool> {
            // --yes で同意済み。確認ダイアログの内容は記録として表示する
            self.outln("確認ダイアログ:");
            self.outln(&indent(&dialog.text));
            self.outln(&format!(
                "  自動公開: {}（変更しません）",
                auto_publish_text(dialog.auto_publish)
            ));
            Ok(true)
        };
        let result = services::submit(&mut page, &target.item_id, &mut confirm, &progress, cancel)?;

        self.outln(&format!("== {} ({})", target.display_name, target.item_id));
        self.outln(&format!(
            "  結果: {}",
            result.message.as_deref().unwrap_or("（メッセージなし）")
        ));
        self.outln(&format!(
            "  ステータス: {}",
            result.status_text.as_deref().unwrap_or("不明")
        ));
        self.outln(&format!(
            "  自動公開: {}",
            auto_publish_text(result.dialog.auto_publish)
        ));
        Ok(exit_codes::SUCCESS)
    }
}

/// 対象アイテムを選び、正本を組み立てる（説明文をファイルから取り出す）。
/// --item はすべて設定に存在する必要がある（打ち間違いで別のアイテムを触らないため）
pub fn select_targets(config: &EdictoraConfig, command: &CliCommand) -> Result<Vec<ExpectedItem>> {
    if config.items.is_empty() {
        return Err(Error::Config(format!(
            "設定ファイルに items がありません: {}",
            config.source_path.display()
        )));
    }

    let items: Vec<&config::ItemConfig> = if command.item_ids.is_empty() {
        config.items.iter().collect()
    } else {
        command
            .item_ids
            .iter()
            .map(|id| {
                config.find_item(id).ok_or_else(|| {
                    Error::Config(format!("設定ファイルにアイテム {id} がありません"))
                })
            })
            .collect::<Result<_>>()?
    };

    let mut result = Vec::new();
    for item in items {
        let mut expected = build_expected(item)?;
        if !command.locale_codes.is_empty() {
            if !command.item_ids.is_empty() {
                // アイテムを明示したなら、指定ロケールが無いのは誤り
                expected = expected.filter_locales(&command.locale_codes)?;
            } else {
                // 全アイテムが対象なら、指定ロケールを持つアイテムだけを残す
                let codes: Vec<String> = command
                    .locale_codes
                    .iter()
                    .filter(|c| expected.locales.iter().any(|l| eq_ic(&l.code, c)))
                    .cloned()
                    .collect();
                if codes.is_empty() {
                    continue;
                }
                expected = expected.filter_locales(&codes)?;
            }
        }
        result.push(expected);
    }

    if result.is_empty() {
        return Err(Error::Config(format!(
            "指定したロケール（{}）を持つアイテムが設定にありません",
            command.locale_codes.join(", ")
        )));
    }
    Ok(result)
}

fn item_header(item: &ExpectedItem, status: Option<&str>, locales: &[String]) -> String {
    let status = status
        .map(|s| format!("  ステータス: {s}"))
        .unwrap_or_default();
    let locales = if locales.is_empty() {
        String::new()
    } else {
        format!("  言語: {}", locales.join(", "))
    };
    format!(
        "== {} ({}){status}{locales}",
        item.display_name, item.item_id
    )
}

/// 自動公開の表示
pub fn auto_publish_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "する",
        Some(false) => "しない",
        None => "不明",
    }
}

fn indent(text: &str) -> String {
    text.split('\n')
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
