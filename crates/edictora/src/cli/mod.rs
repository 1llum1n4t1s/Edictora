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
use crate::error::{Error, Result};
use crate::images::fetch::WinHttpImageFetcher;
use crate::images::{self, ImageSlot, slot_label};
use crate::listing::{
    ComparisonStatus, ExpectedItem, ItemSnapshot, ListingField, build_expected,
    build_expected_images,
};
use crate::logging::{LogLevel, log, log_error_detail, redact};
use crate::redirect;
use crate::services::{self, ImageSnapshot, LocalePushOutcome};
use crate::update::UpdateOutcome;
use crate::update::lock::{LockFile, is_held_by_another};
use crate::update::source::UpdateSource;
use crate::util::{eq_ic, format_n0, utf16_len};
use crate::{tr, trs};
use args::{CliCommand, CliCommandKind, help_text};

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
        trs!("cli.updates_are_not_available_in_this_environment").to_string(),
    ))
}

/// CLI のコマンド実行
pub struct CliApp<'a> {
    out: RefCell<&'a mut dyn Write>,
    err: RefCell<&'a mut dyn Write>,
    paths: AppPaths,
    open_browser: &'a BrowserOpener<'a>,
    show_browser: bool,
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
                self.errln(&error_line(&e.message()));
                return exit_codes::USAGE;
            }
        };

        match command.kind {
            CliCommandKind::Help => {
                self.outln(help_text());
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
                self.errln(&error_line(&e.message()));
                return exit_codes::USAGE;
            }
            Err(e) => {
                // 設定の段階で起きるのは Config / Usage だけのはずだが、念のため実行時エラーとして扱う
                self.errln(&error_line(&redact(&e.message())));
                return exit_codes::FAILURE;
            }
        };

        // ── 既定の設定ファイルがアプリ専用領域（MSIX パッケージ内のアプリの仮想化先）から読まれていれば警告 ──
        // --config で明示された場合は利用者の指定なので対象にしない。標準エラーなので MCP の JSON-RPC も壊さない
        if command.config_path.is_none()
            && let Some(warning) = redirect::config_warning(&config.source_path)
        {
            self.errln(&warning);
        }

        // ── 実行中の印（更新の適用側に、Chrome を開くコマンドが動いていることを知らせる）──
        // Update.exe の apply がインストール先配下のプロセスを止めて push の途中で落とすのを防ぐため、
        // ブラウザ操作の間だけ共有ロックを持つ（CLI 同士は同時に動ける）。更新の適用中（排他で持たれている）
        // なら Chrome を開かずに止める。ロックファイル自体を開けない（フォルダーを作れないなど）ときは
        // 使えなくしないよう、ログに残して続ける（インストールされていない実行でも無害）
        let _running = match LockFile::try_acquire_shared(&self.paths.running_lock_path()) {
            Ok(lock) => Some(lock),
            Err(e) if is_held_by_another(&e) => {
                self.errln(trs!("cli.error_an_edictora_update_is_being_applied"));
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
                self.errln(&Error::Cancelled.message());
                exit_codes::CANCELLED
            }
            Err(Error::LoginRequired) => {
                self.errln(&error_line(&Error::LoginRequired.message()));
                exit_codes::LOGIN_REQUIRED
            }
            Err(Error::WriteBlocked(m)) => {
                self.errln(&tr!("cli.aborted", detail = redact(&m)));
                exit_codes::BLOCKED
            }
            Err(e) => {
                log_error_detail(
                    &format!("{} の実行に失敗", command.kind.name()),
                    &format!("{}: {}", e.kind_name(), e.message()),
                );
                self.errln(&error_line(&redact(&e.message())));
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
                self.errln(&error_line(&m));
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
                self.errln(&Error::Cancelled.message());
                exit_codes::CANCELLED
            }
            Err(e) => {
                log_error_detail(
                    "update の実行に失敗",
                    &format!("{}: {}", e.kind_name(), e.message()),
                );
                self.errln(&error_line(&e.message()));
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
        self.outln(&tr!(
            "cli.opening_chrome_profile",
            profile_directory = config.profile_directory.display()
        ));
        // ログインは利用者が画面で操作するので、常にウィンドウを表示する
        let options = self.browser_options(config, false).visible();
        let mut browser = (self.open_browser)(&options, cancel)?;
        // プロファイルはここで新規に作られうるので、作成後に仮想化先へ入っていないか確かめる
        self.warn_profile_redirect(config);
        let progress = self.progress();
        let result = browser.wait_for_login(
            command.login_wait.unwrap_or(DEFAULT_LOGIN_WAIT),
            &progress,
            cancel,
        );
        browser.close();
        result?;
        self.outln(trs!("cli.saved_the_sign_in_to_the_profile"));
        Ok(exit_codes::SUCCESS)
    }

    /// Chrome プロファイルがアプリ専用領域へリダイレクトされていれば、標準エラーへ警告を 1 回出す
    fn warn_profile_redirect(&self, config: &EdictoraConfig) {
        if let Some(warning) = redirect::profile_warning(&config.profile_directory) {
            self.errln(&warning);
        }
    }

    /// ログイン済みのブラウザを開く。既定はヘッドレス（作業中に前面へ出てこない）。--show-browser のときだけウィンドウを表示する
    fn open_logged_in(
        &self,
        config: &EdictoraConfig,
        cancel: &CancelToken,
    ) -> Result<DashboardBrowser> {
        let mut browser =
            (self.open_browser)(&self.browser_options(config, self.show_browser), cancel)?;
        self.warn_profile_redirect(config);
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
        let mut page = browser.create_item_page()?;
        let progress = self.progress();
        let fetcher = WinHttpImageFetcher;
        let mut failed = false;

        for expected in targets {
            let mut header_done = false;
            if !expected.locales.is_empty() {
                let snapshot = services::pull(&mut page, expected, &progress, cancel)?;
                self.outln(&item_header(
                    expected,
                    snapshot.status_text.as_deref(),
                    &snapshot.available_locales,
                ));
                header_done = true;
                failed |= self.print_pulled_text(expected, &snapshot);
            }
            if expected.images.as_ref().is_some_and(|i| !i.is_empty()) {
                let snapshot =
                    services::pull_images(&mut page, &fetcher, expected, &progress, cancel)?;
                if !header_done {
                    self.outln(&item_header(
                        expected,
                        snapshot.status_text.as_deref(),
                        &snapshot.available_locales,
                    ));
                }
                failed |= self.print_pulled_images(&snapshot);
            }
            self.outln("");
        }
        Ok(if failed {
            exit_codes::FAILURE
        } else {
            exit_codes::SUCCESS
        })
    }

    /// pull のテキスト部分を表示する。失敗があれば true
    fn print_pulled_text(&self, expected: &ExpectedItem, snapshot: &ItemSnapshot) -> bool {
        let mut failed = false;
        if let Some(err) = &snapshot.error {
            self.outln(&format!("  {}", error_line(err)));
            return true;
        }
        for locale in &expected.locales {
            let current = snapshot.find(&locale.code);
            let current = match current {
                Some(c) if c.error.is_none() => c,
                other => {
                    failed = true;
                    let msg = other
                        .and_then(|c| c.error.clone())
                        .unwrap_or_else(|| trs!("cli.could_not_read_the_values").to_string());
                    self.outln(&format!("-- [{}] {}", locale.code, error_line(&msg)));
                    continue;
                }
            };
            for field in locale.fields() {
                let value = current.values.get(&field).cloned().unwrap_or_default();
                if field == ListingField::Description {
                    self.outln(&tr!(
                        "cli.characters",
                        locale = locale.code,
                        display_name = field.display_name(),
                        length = format_n0(utf16_len(&value))
                    ));
                } else {
                    self.outln(&format!("-- [{}] {}", locale.code, field.display_name()));
                }
                self.outln(&value);
            }
        }
        failed
    }

    /// pull の画像部分を表示する（枚数・alt・原本のハッシュの先頭）。失敗があれば true
    fn print_pulled_images(&self, snapshot: &ImageSnapshot) -> bool {
        if let Some(err) = &snapshot.error {
            self.outln(&format!("  {}", error_line(err)));
            return true;
        }
        let mut failed = false;
        for reading in &snapshot.slots {
            let label = slot_label(reading.slot, reading.locale.as_deref());
            match &reading.images {
                Err(m) => {
                    failed = true;
                    self.outln(&format!("-- {label} {}", error_line(m)));
                }
                Ok(images) => {
                    self.outln(&tr!(
                        "cli.image_slot_header",
                        label = label,
                        count = images.len()
                    ));
                    for (i, image) in images.iter().enumerate() {
                        let hash = match &image.pixel_sha256 {
                            Ok(h) => h.chars().take(12).collect::<String>(),
                            Err(m) => {
                                failed = true;
                                error_line(m)
                            }
                        };
                        self.outln(&format!("  {}. {}  {hash}", i + 1, image.alt));
                    }
                }
            }
        }
        failed
    }

    fn diff(
        &self,
        config: &EdictoraConfig,
        targets: &[ExpectedItem],
        cancel: &CancelToken,
        push_hint: bool,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = browser.create_item_page()?;
        let progress = self.progress();
        let fetcher = WinHttpImageFetcher;
        let mut any_diff = false;
        let mut any_error = false;

        for expected in targets {
            let mut header_done = false;
            if !expected.locales.is_empty() {
                let (snapshot, rows) = services::diff(&mut page, expected, &progress, cancel)?;
                self.outln(&item_header(
                    expected,
                    snapshot.status_text.as_deref(),
                    &snapshot.available_locales,
                ));
                header_done = true;
                for row in &rows {
                    self.out_raw(&diff_formatter::format(row));
                    let status = row.status();
                    any_diff |= status == ComparisonStatus::Different;
                    any_error |= matches!(
                        status,
                        ComparisonStatus::Error | ComparisonStatus::NotFetched
                    );
                }
            }
            if expected.images.as_ref().is_some_and(|i| !i.is_empty()) {
                let (snapshot, rows) =
                    services::diff_images(&mut page, &fetcher, expected, &progress, cancel)?;
                if !header_done {
                    self.outln(&item_header(
                        expected,
                        snapshot.status_text.as_deref(),
                        &snapshot.available_locales,
                    ));
                }
                if let Some(err) = &snapshot.error {
                    self.outln(&format!("  {}", error_line(err)));
                    any_error = true;
                }
                for row in &rows {
                    if snapshot.error.is_none() {
                        self.outln(&images::describe(row));
                    }
                    any_diff |= row.status == ComparisonStatus::Different;
                    any_error |= matches!(
                        row.status,
                        ComparisonStatus::Error | ComparisonStatus::NotFetched
                    );
                }
            }
            self.outln("");
        }

        if push_hint && any_diff {
            self.outln(trs!("cli.to_update_the_fields_that_differ_run"));
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
        let mut page = browser.create_item_page()?;
        let progress = self.progress();
        let fetcher = WinHttpImageFetcher;

        // 1 件でも失敗したら残りは行わない（エラーがそのまま run へ上がる）
        for expected in targets {
            if !expected.locales.is_empty() {
                let result = services::push(&mut page, expected, &progress, cancel)?;
                self.outln(&format!("== {} ({})", result.item_name, result.item_id));
                for locale in &result.locales {
                    let line = if locale.outcome == LocalePushOutcome::Saved {
                        let names: Vec<&str> = locale
                            .updated_fields
                            .iter()
                            .map(|f| f.display_name())
                            .collect();
                        tr!(
                            "cli.saved_as_draft_and_verified",
                            locale = locale.locale,
                            text = names.join(trs!("cli.name_separator"))
                        )
                    } else {
                        tr!(
                            "cli.no_changes_matches_the_source_files",
                            locale = locale.locale
                        )
                    };
                    self.outln(&line);
                }
            }
            // 画像は、テキストの保存・照合が終わってから（同じアイテムの別の保存として行う）
            if expected.images.as_ref().is_some_and(|i| !i.is_empty()) {
                let result =
                    services::push_images(&mut page, &fetcher, expected, &progress, cancel)?;
                if expected.locales.is_empty() {
                    self.outln(&format!("== {} ({})", result.item_name, result.item_id));
                }
                for label in &result.saved {
                    self.outln(&tr!(
                        "cli.images_saved_as_draft_and_verified",
                        label = label
                    ));
                }
                for label in &result.unchanged {
                    self.outln(&tr!("cli.images_no_changes", label = label));
                }
            }
        }
        self.outln(trs!("cli.done_nothing_was_submitted_for_review_run"));
        Ok(exit_codes::SUCCESS)
    }

    fn submit(
        &self,
        config: &EdictoraConfig,
        target: &ExpectedItem,
        cancel: &CancelToken,
    ) -> Result<i32> {
        let browser = self.open_logged_in(config, cancel)?;
        let mut page = browser.create_item_page()?;
        let progress = self.progress();

        let mut confirm = |dialog: &crate::dashboard::SubmitDialogInfo| -> Result<bool> {
            // --yes で同意済み。確認ダイアログの内容は記録として表示する
            self.outln(trs!("cli.confirmation_dialog"));
            self.outln(&indent(&dialog.text));
            self.outln(&tr!(
                "cli.publish_automatically_unchanged",
                auto_publish = auto_publish_text(dialog.auto_publish)
            ));
            Ok(true)
        };
        let result = services::submit(&mut page, &target.item_id, &mut confirm, &progress, cancel)?;

        self.outln(&format!("== {} ({})", target.display_name, target.item_id));
        self.outln(&tr!(
            "cli.result",
            message = result.message.as_deref().unwrap_or(trs!("cli.no_message"))
        ));
        self.outln(&tr!(
            "cli.status",
            status = result.status_text.as_deref().unwrap_or(trs!("cli.unknown"))
        ));
        self.outln(&tr!(
            "cli.publish_automatically",
            auto_publish = auto_publish_text(result.dialog.auto_publish)
        ));
        Ok(exit_codes::SUCCESS)
    }
}

/// 対象アイテムを選び、正本を組み立てる（説明文をファイルから取り出し、画像を検証する）。
/// --item はすべて設定に存在する必要がある（打ち間違いで別のアイテムを触らないため）。
///
/// `--only` で対象の種類を、`--image-scope` で画像の範囲を絞る。`--locale` はテキストのロケールと
/// ローカライズ版スクリーンショットのロケールに効く（全言語向けの画像はロケールを持たないので絞らない）
pub fn select_targets(config: &EdictoraConfig, command: &CliCommand) -> Result<Vec<ExpectedItem>> {
    if config.items.is_empty() {
        return Err(Error::Config(tr!(
            "cli.the_config_file_has_no_items",
            source_path = config.source_path.display()
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
                    Error::Config(tr!("cli.item_is_not_in_the_config_file", id = id))
                })
            })
            .collect::<Result<_>>()?
    };

    let explicit = !command.item_ids.is_empty();
    let codes = &command.locale_codes;
    let mut result = Vec::new();
    for item in items {
        let want_text = command.only.text() && !item.locales.is_empty();
        let want_images = command.only.images() && item.images.is_some();

        // ロケールの指定が、このアイテムのどこに当たるか（テキストの言語・ローカライズ版スクリーンショットの言語）
        let in_text = |c: &str| want_text && item.locales.iter().any(|l| eq_ic(&l.code, c));
        let in_images = |c: &str| {
            want_images
                && command
                    .image_scope
                    .includes(ImageSlot::LocalizedScreenshots)
                && item
                    .images
                    .as_ref()
                    .is_some_and(|i| i.localized_screenshots.iter().any(|(l, _)| eq_ic(l, c)))
        };
        let matched: Vec<String> = codes
            .iter()
            .filter(|c| in_text(c) || in_images(c))
            .cloned()
            .collect();
        if !codes.is_empty() {
            if explicit {
                // アイテムを明示したなら、指定ロケールが無いのは誤り
                if let Some(missing) = codes.iter().find(|c| !in_text(c) && !in_images(c)) {
                    return Err(Error::Config(tr!(
                        "listing.has_no_locale_in_the_config_file",
                        display_name = item.display_name,
                        item_id = item.item_id,
                        code = missing
                    )));
                }
            } else if matched.is_empty() {
                // 全アイテムが対象なら、指定ロケールを持つアイテムだけを残す
                continue;
            }
        }

        let mut expected = if want_text {
            let mut e = build_expected(item)?;
            if !codes.is_empty() {
                let text_codes: Vec<String> =
                    codes.iter().filter(|c| in_text(c)).cloned().collect();
                if text_codes.is_empty() {
                    // ロケールの指定がローカライズ版スクリーンショットだけに当たる。テキストは今回は対象外
                    e.locales.clear();
                } else {
                    e = e.filter_locales(&text_codes)?;
                }
            }
            e
        } else {
            ExpectedItem {
                item_id: item.item_id.clone(),
                display_name: item.display_name.clone(),
                locales: Vec::new(),
                images: None,
            }
        };
        if want_images {
            expected.images = build_expected_images(item, command.image_scope, codes)?;
        }

        let has_work =
            !expected.locales.is_empty() || expected.images.as_ref().is_some_and(|i| !i.is_empty());
        if !has_work {
            if explicit {
                return Err(Error::Config(tr!(
                    "cli.the_item_has_nothing_to_do_with_these_options",
                    id = item.item_id
                )));
            }
            continue;
        }
        result.push(expected);
    }

    if result.is_empty() {
        return Err(Error::Config(if codes.is_empty() {
            trs!("cli.no_item_in_the_config_file_has_targets").to_string()
        } else {
            tr!(
                "cli.no_item_in_the_config_file_has",
                locale_codes = codes.join(", ")
            )
        }));
    }
    Ok(result)
}

fn item_header(item: &ExpectedItem, status: Option<&str>, locales: &[String]) -> String {
    let status = status
        .map(|s| tr!("cli.status_2", status = s))
        .unwrap_or_default();
    let locales = if locales.is_empty() {
        String::new()
    } else {
        tr!("cli.languages", locales = locales.join(", "))
    };
    format!(
        "== {} ({}){status}{locales}",
        item.display_name, item.item_id
    )
}

/// 自動公開の表示
pub fn auto_publish_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => trs!("cli.on"),
        Some(false) => trs!("cli.off"),
        None => trs!("cli.unknown"),
    }
}

/// 「エラー: <本文>」の 1 行（接頭辞を表示言語に合わせる。本文は作った時点の言語のまま）
fn error_line(message: &str) -> String {
    tr!("cli.error_2", message = message)
}

fn indent(text: &str) -> String {
    text.split('\n')
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
