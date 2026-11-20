//! アイテムの編集画面（ストアの掲載情報・画像・「プライバシー」タブ）を CDP で操作する [`ListingPage`] / [`ImagePage`] /
//! [`PrivacyPage`] 実装（C# の `DashboardItemPage`）。
//!
//! 2026-11-13 の検証（DESIGN.md）で確かめた操作手順をそのまま使う。
//! - 操作の前に毎回 Emulation.setFocusEmulationEnabled を行う（ウィンドウを前面には出さない）
//! - クリックは scrollIntoView → 矩形の中心へ実際のマウス入力（他要素に覆われていたら止める）
//! - 書き換えはフォーカス → 全選択 → Input.insertText。DOM の value への直接代入はしない
//! - 待ちは固定時間の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）
//! - 画面の表示言語（日本語 / 英語）は、編集画面を開く・読み込み直すたびに `<html lang>` から 1 回だけ判定し、
//!   以降はその言語の文言セットだけで要素を探す（`text.rs`）。日本語・英語以外なら止める

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::cdp::CdpPage;
use crate::dashboard::page_scripts::{self as scripts, kinds};
use crate::dashboard::selectors::{self as sel, ElementInfo};
use crate::dashboard::text::{CategoryText, DashboardLang, DashboardText, PrivacyText};
use crate::dashboard::urls;
use crate::dashboard::write_gate::ItemPageState;
use crate::dashboard::{
    EditorPage, ImagePage, ListingPage, PageImage, PrivacyPage, SubmitConfirm, SubmitDialogInfo,
    SubmitResult,
};
use crate::error::{Error, Result};
use crate::images::ImageSlot;
use crate::listing::text_diff::first_difference_position;
use crate::listing::{FieldValues, ListingField};
use crate::logging::{LogLevel, log, redact_diagnostics};
use crate::privacy::{DataKind, ExpectedPrivacy, PrivacyField, PrivacyReading};
use crate::tr;
use crate::util::{LocalTime, contains_ic, diagnostics_file_name, eq_ic, utf16_len};

/// 条件確認の間隔
const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// 今開いているタブ（保存・読み込み直し・書き込み可否の判断が、タブごとに見る欄を変える）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    /// 「ストアの掲載情報」（`open_item` で開く）
    Listing,
    /// 「プライバシー」（`open_privacy` で開く）
    Privacy,
}

/// 編集画面の操作
pub struct DashboardItemPage<'a> {
    page: &'a CdpPage,
    publisher_id: String,
    item_id: Option<String>,
    diagnostics_dir: PathBuf,
    /// 今の画面の表示言語の文言セット（open_item / reload で判定する。判定前は None）
    text: Option<&'static DashboardText>,
    /// 今開いているタブ
    tab: Tab,
}

impl<'a> DashboardItemPage<'a> {
    /// 作る（publisherId はログイン確認で得たもの。外へは出さない）
    pub fn new(
        page: &'a CdpPage,
        publisher_id: String,
        diagnostics_dir: PathBuf,
    ) -> DashboardItemPage<'a> {
        DashboardItemPage {
            page,
            publisher_id,
            item_id: None,
            diagnostics_dir,
            text: None,
            tab: Tab::Listing,
        }
    }

    /// 判定済みの文言セット（判定前に要素を探そうとしたらエラー。推測で日本語を使わない）
    fn t(&self) -> Result<&'static DashboardText> {
        self.text.ok_or_else(|| {
            Error::Dashboard(tr!("dashboard.the_dashboard_display_language_has_not_been"))
        })
    }

    /// 画面の表示言語を `<html lang>` から判定し、文言セットを決める（ページごとに 1 回）。
    ///
    /// 導入部（`window.__edictora`）を含む式を評価する前に呼ぶ。導入部は最初の評価で作られ、
    /// 案内通知の語はそのときの言語のもので固定されるため。
    /// 読み込み直後は lang がまだ空のことがあるので、空でなくなるまで上限つきで待つ。
    /// 空でない値が日本語・英語以外なら、待たずに止める（推測しない）。
    fn detect_language(&mut self, cancel: &CancelToken) -> Result<()> {
        self.text = None;
        let mut last = String::new();
        // 空のままなら、下の判定が日本語の文言セットにする（英語対応の前と同じ）。
        // 日本語の経路で lang が無い場合の待ちを短くするため、上限は 5 秒にする
        try_poll(
            || {
                let v = self.evaluate(scripts::DOCUMENT_LANG, cancel)?;
                last = v.as_str().unwrap_or("").trim().to_string();
                Ok((!last.is_empty()).then_some(()))
            },
            Duration::from_secs(5),
            cancel,
        )?;
        let text = sel::dashboard_text_for_lang(Some(&last))?;
        log(
            &format!("ダッシュボードの表示言語: {last} → {:?}", text.lang),
            LogLevel::Info,
        );
        self.text = Some(text);
        Ok(())
    }

    fn item_label(&self) -> String {
        self.item_id.clone().unwrap_or_default()
    }

    /// SPA なので load 後も中身が遅れて出る。言語コンボボックスと説明欄が揃うまで待つ
    fn wait_for_editor_ready(&self, cancel: &CancelToken) -> Result<()> {
        let t = self.t()?;
        let item = self.item_label();
        let what = tr!(
            "dashboard.the_edit_page_of_item_language_selector",
            item = item
        );
        poll(
            || {
                let combos = self.collect(kinds::COMBOBOX, cancel)?;
                let textareas = self.collect(kinds::TEXTAREA, cancel)?;
                // 待ち条件なので緩い部分一致で足りる
                let has_language = combos.iter().any(|c| sel::mentions_language_label(t, c));
                Ok((has_language && !textareas.is_empty()).then_some(()))
            },
            Duration::from_secs(45),
            &what,
            cancel,
        )
    }

    /// 言語の一覧が開くまで待ち、表示中の選択肢を返す。
    /// パブリッシャー名の選択肢は常に表示されているので、「言語 – code」形式の選択肢が現れるまで待つ
    fn wait_for_options(&self, cancel: &CancelToken) -> Result<Vec<ElementInfo>> {
        let t = self.t()?;
        poll(
            || {
                let options = self.collect(kinds::OPTION, cancel)?;
                let any = options
                    .iter()
                    .any(|o| sel::parse_locale_option(t, Some(&o.text)).is_some());
                Ok(any.then_some(options))
            },
            Duration::from_secs(10),
            &tr!("dashboard.the_language_options_to_appear"),
            cancel,
        )
    }

    /// 言語欄の aria-expanded が "false" か（一覧が閉じているか）
    fn is_language_list_closed(&self, cancel: &CancelToken) -> Result<bool> {
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(self.t()?, &combos)?;
        Ok(combo.expanded.as_deref() == Some("false"))
    }

    /// 欄の要素を特定する（説明は textarea、URL はラベル）
    fn locate(
        &self,
        field: ListingField,
        cancel: &CancelToken,
    ) -> Result<(&'static str, ElementInfo)> {
        if field == ListingField::Description {
            let textareas = self.collect(kinds::TEXTAREA, cancel)?;
            return Ok((kinds::TEXTAREA, sel::pick_description(&textareas)?.clone()));
        }
        let inputs = self.collect(kinds::TEXT_INPUT, cancel)?;
        Ok((
            kinds::TEXT_INPUT,
            sel::pick_field_by_label(self.t()?, &inputs, field)?.clone(),
        ))
    }

    /// 要素を画面内へスクロールし、中心を実際のマウス入力でクリックする
    fn click(&self, kind: &str, index: i32, what: &str, cancel: &CancelToken) -> Result<()> {
        let t = self.t()?;
        self.page.prepare_for_input(cancel)?;

        // スクロール後の矩形を読み直す。中心が別の要素に覆われていたら押さない。
        // 一覧（メニュー）は開く・スクロールするアニメーションの途中だと、中心に別の要素が来ることがある
        // （例: 言語が多いアイテムで、一覧の下の方の選択肢）。覆われている間は押さずに、
        // スクロールし直して少し待ち、上限まで測り直す。上限を過ぎても覆われていれば従来どおり止める。
        const ATTEMPTS: u32 = 8;
        let mut point = Value::Null;
        for attempt in 0..ATTEMPTS {
            self.evaluate(&scripts::scroll_to(t, kind, index), cancel)?;
            cancel.sleep(Duration::from_millis(if attempt == 0 { 150 } else { 250 }))?;
            point = self.evaluate(&scripts::point(t, kind, index), cancel)?;
            if !point.is_object() {
                return Err(Error::Dashboard(tr!(
                    "dashboard.could_not_get_the_position_of",
                    what = what
                )));
            }
            if point.get("hit").and_then(Value::as_bool) == Some(true) {
                break;
            }
        }
        if point.get("hit").and_then(Value::as_bool) != Some(true) {
            // 何が重なっていたか・画面の大きさをエラー文に含める（表示して確かめなくても原因を絞れるように）
            return Err(Error::Dashboard(sel::covered_error_message(what, &point)));
        }
        let x = point.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = point.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        self.page.click_at(x, y, cancel)
    }

    fn collect(&self, kind: &str, cancel: &CancelToken) -> Result<Vec<ElementInfo>> {
        Ok(sel::parse_elements(
            &self.evaluate(&scripts::collect(self.t()?, kind), cancel)?,
        ))
    }

    fn value(&self, kind: &str, index: i32, cancel: &CancelToken) -> Result<String> {
        let v = self.evaluate(&scripts::value(self.t()?, kind, index), cancel)?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    fn messages(&self, cancel: &CancelToken) -> Result<Vec<String>> {
        Ok(sel::parse_strings(
            &self.evaluate(&scripts::messages(self.t()?), cancel)?,
        ))
    }

    fn dialogs(&self, cancel: &CancelToken) -> Result<Vec<String>> {
        Ok(sel::parse_strings(
            &self.evaluate(&scripts::dialogs(self.t()?), cancel)?,
        ))
    }

    fn body_text(&self, cancel: &CancelToken) -> Result<String> {
        let v = self.evaluate(&scripts::body_text(self.t()?), cancel)?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    /// 画像の書き込みを許す画面か。日本語の画面だけ許す（英語の画面の文言は実画面で未確認。
    /// 削除ボタンの aria-label・見出しを推測で押さないため）
    fn ensure_image_write_supported(&self) -> Result<()> {
        if self.t()?.lang != DashboardLang::Ja {
            return Err(Error::Dashboard(tr!(
                "dashboard.writing_images_is_only_supported_on_a_japanese_dashboard"
            )));
        }
        Ok(())
    }

    /// 画像のスロット（入力）を特定して返す。画面の描画が遅れることがあるので、見つかるまで少し待つ
    fn locate_slot(&self, slot: ImageSlot, cancel: &CancelToken) -> Result<ElementInfo> {
        let t = self.t()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            cancel.check()?;
            let inputs = self.collect(kinds::FILE_INPUT, cancel)?;
            match sel::pick_image_slot(t, &inputs, slot) {
                Ok(found) => return Ok(found.clone()),
                Err(e) if Instant::now() >= deadline => return Err(e),
                Err(_) => cancel.sleep(POLL_INTERVAL)?,
            }
        }
    }

    /// ページ側の例外はダッシュボード操作の失敗として扱う。
    /// ページ内スクリプトが固定の印で投げた例外（要素が消えた）は、ここで利用者向けの文に直す
    fn evaluate(&self, expression: &str, cancel: &CancelToken) -> Result<Value> {
        self.page.evaluate(expression, cancel).map_err(page_error)
    }
}

/// ページ内スクリプトの例外（`JavaScript`）をダッシュボード操作の失敗（利用者向けの文）へ直す。他のエラーはそのまま
fn page_error(e: Error) -> Error {
    match e {
        Error::JavaScript(m) => Error::Dashboard(sel::page_script_error_message(&m)),
        other => other,
    }
}

impl EditorPage for DashboardItemPage<'_> {
    fn open_item(&mut self, item_id: &str, cancel: &CancelToken) -> Result<()> {
        self.item_id = Some(item_id.to_string());
        self.tab = Tab::Listing;
        self.page.prepare_for_input(cancel)?;
        self.page
            .navigate(&urls::edit_url(&self.publisher_id, item_id), cancel)
            .map_err(|e| match e {
                Error::Cdp(m) => Error::Dashboard(tr!(
                    "dashboard.could_not_open_the_edit_page_of",
                    item_id = item_id,
                    detail = m
                )),
                other => other,
            })?;

        let url = self.page.get_url(cancel)?;
        if urls::is_login_page(url.as_deref()) {
            return Err(Error::LoginRequired);
        }
        // 新しいページなので表示言語を判定し直す（導入部を含む式より前に）
        self.detect_language(cancel)?;
        self.wait_for_editor_ready(cancel)?;

        // 読み込み後に別画面（一覧など）へ移されていないか確認する。アイテム ID の誤りで起きる
        let url = self.page.get_url(cancel)?;
        if !urls::is_edit_page_of(url.as_deref(), item_id) {
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_open_the_edit_page_of_2",
                item_id = item_id
            )));
        }
        Ok(())
    }

    fn reload(&mut self, cancel: &CancelToken) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        self.page.reload(cancel).map_err(|e| match e {
            Error::Cdp(m) => {
                Error::Dashboard(tr!("dashboard.could_not_reload_the_edit_page", detail = m))
            }
            other => other,
        })?;
        // 読み込み直しで window は作り直されるので、表示言語も判定し直す
        self.detect_language(cancel)?;
        match self.tab {
            Tab::Listing => self.wait_for_editor_ready(cancel),
            Tab::Privacy => self.wait_for_privacy_ready(cancel),
        }
    }

    fn get_state(&mut self, cancel: &CancelToken) -> Result<ItemPageState> {
        // CDP での収集はこの順（本文 → textarea → ボタン）のまま。判断は selectors の純粋関数で行う
        let t = self.t()?;
        let status = sel::parse_status(t, Some(&self.body_text(cancel)?));
        let textareas = self.collect(kinds::TEXTAREA, cancel)?;
        let buttons = self.collect(kinds::BUTTON, cancel)?;
        match (self.tab, t.privacy.as_ref()) {
            // 「プライバシー」タブでは、主の入力欄を単一用途の説明として見る
            (Tab::Privacy, Some(p)) => {
                Ok(sel::privacy_page_state(t, p, status, &textareas, &buttons))
            }
            _ => Ok(sel::item_page_state(t, status, &textareas, &buttons)),
        }
    }

    fn get_locales(&mut self, cancel: &CancelToken) -> Result<Vec<String>> {
        let t = self.t()?;
        self.page.prepare_for_input(cancel)?;
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(t, &combos)?;
        let current = sel::current_locale_code(t, combo);
        if current.is_none() {
            // 言語が 1 つだけのアイテム。選択欄はコードなしの言語名だけで、開くと既定の言語を選ぶ
            // 全言語の一覧になるので開かない（誤って既定の言語を変えないため）。表示中の言語名から決める
            return match sel::single_locale_code(t, combo) {
                Some(code) => Ok(vec![code]),
                None => Err(Error::Dashboard(tr!(
                    "dashboard.could_not_read_the_language_from_the"
                ))),
            };
        }
        let selector_name = tr!("dashboard.the_language_selector");
        self.click(kinds::COMBOBOX, combo.index, &selector_name, cancel)?;

        let options = self.wait_for_options(cancel)?;
        let codes = sel::locale_codes(t, &options);

        // 一覧を閉じる。Escape で閉じなければ、今の言語の選択肢を押し直して閉じる（言語は変わらない）。
        // 開閉は言語欄の aria-expanded で判定する。選択肢の要素は閉じてもページに残り、
        // パブリッシャー名の選択肢も常に表示されているため、「選択肢が 0 件になったか」では判定できない。
        self.page.press_key("Escape", 27, cancel)?;
        let closed = try_poll(
            || Ok(self.is_language_list_closed(cancel)?.then_some(())),
            Duration::from_secs(3),
            cancel,
        )?;
        if !closed && let Some(current) = current {
            let again = self.collect(kinds::OPTION, cancel)?;
            let option = sel::pick_locale_option(t, &again, &current)?;
            self.click(
                kinds::OPTION,
                option.index,
                &tr!("dashboard.the_language_option"),
                cancel,
            )?;
        }
        poll(
            || Ok(self.is_language_list_closed(cancel)?.then_some(())),
            Duration::from_secs(5),
            &tr!("dashboard.the_language_options_to_close"),
            cancel,
        )?;

        if codes.is_empty() {
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_read_any_locale_from_the"
            )));
        }
        Ok(codes)
    }

    fn switch_locale(&mut self, code: &str, cancel: &CancelToken) -> Result<()> {
        let t = self.t()?;
        self.page.prepare_for_input(cancel)?;
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(t, &combos)?;
        if sel::current_locale_code(t, combo).is_some_and(|c| eq_ic(&c, code)) {
            return Ok(());
        }
        // 言語が 1 つだけのアイテムは切り替えない（選択欄を開くと既定の言語の一覧になるため触らない）
        if let Some(single) = sel::single_locale_code(t, combo) {
            return if eq_ic(&single, code) {
                Ok(())
            } else {
                Err(Error::Dashboard(tr!(
                    "dashboard.this_item_has_only_the_language_there",
                    single = single,
                    code = code
                )))
            };
        }

        let selector_name = tr!("dashboard.the_language_selector");
        self.click(kinds::COMBOBOX, combo.index, &selector_name, cancel)?;
        let options = self.wait_for_options(cancel)?;
        let option = sel::pick_locale_option(t, &options, code)?;
        self.click(
            kinds::OPTION,
            option.index,
            &tr!("dashboard.the_language_option_2", code = code),
            cancel,
        )?;

        // 選択欄の表示が切り替わるまで待つ
        poll(
            || {
                let combos = self.collect(kinds::COMBOBOX, cancel)?;
                let c = sel::pick_language_combobox(t, &combos)?;
                Ok(sel::current_locale_code(t, c)
                    .is_some_and(|c| eq_ic(&c, code))
                    .then_some(()))
            },
            Duration::from_secs(15),
            &tr!("dashboard.the_switch_to_language", code = code),
            cancel,
        )?;

        // 欄の中身が新しい言語のものに入れ替わり、落ち着くまで待つ（説明欄の値が 2 回続けて同じになるまで）
        let mut previous: Option<String> = None;
        poll(
            || {
                cancel.sleep(Duration::from_millis(400))?;
                let textareas = self.collect(kinds::TEXTAREA, cancel)?;
                let textarea = sel::pick_description(&textareas)?;
                let value = self.value(kinds::TEXTAREA, textarea.index, cancel)?;
                let stable = previous.as_deref() == Some(value.as_str());
                previous = Some(value);
                Ok(stable.then_some(()))
            },
            Duration::from_secs(15),
            &tr!("dashboard.the_content_of_language_to_appear", code = code),
            cancel,
        )
    }

    fn save_draft(&mut self, cancel: &CancelToken) -> Result<()> {
        let t = self.t()?;
        let (save, saved_message) = (t.save_draft_button, t.saved_message);
        self.page.prepare_for_input(cancel)?;

        // 前回の「保存されました」が残っていると成功を誤認するので、消えるまで待つ
        let clean = try_poll(
            || Ok((!sel::has_message(t, &self.messages(cancel)?, saved_message)).then_some(())),
            Duration::from_secs(10),
            cancel,
        )?;
        if !clean {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_previous_save_message_does_not_go"
            )));
        }

        let buttons = self.collect(kinds::BUTTON, cancel)?;
        let button = sel::pick_button(&buttons, save)?;
        if button.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_button_cannot_be_clicked",
                save = save
            )));
        }
        self.click(
            kinds::BUTTON,
            button.index,
            &tr!("dashboard.the_button", save = save),
            cancel,
        )?;

        let mut messages: Vec<String> = Vec::new();
        let saved = try_poll(
            || {
                messages = self.messages(cancel)?;
                Ok(sel::has_message(t, &messages, saved_message).then_some(()))
            },
            Duration::from_secs(30),
            cancel,
        )?;
        if !saved {
            let shown = sel::result_messages(t, &messages);
            let tail = if shown.is_empty() {
                String::new()
            } else {
                let joined = shown.join(" / ");
                tr!("dashboard.shown", joined = joined)
            };
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_confirm_the_draft_saved_message",
                saved_message = saved_message,
                suffix = tail
            )));
        }
        log(
            &format!("下書き保存を確認: {}", self.item_label()),
            LogLevel::Info,
        );
        Ok(())
    }

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        // 失敗時の記録なので、中断の後でも保存を試みる（新しい合図を使う）
        let run = || -> Result<String> {
            std::fs::create_dir_all(&self.diagnostics_dir)?;
            // ファイル名の規則（yyyyMMdd-HHmmss-<安全化したラベル>.txt）は純粋関数にまとめ、util のテストで固めている
            let name = diagnostics_file_name(&LocalTime::now(), label);
            let path = self.diagnostics_dir.join(name);
            // 言語の判定前の失敗でも取れるよう、未判定なら日本語の文言セットで導入部を作る
            // （スナップショットは文言を使わない。失敗した項目の残りは行わないので、導入部が固定されても影響しない）
            let t = match self.text {
                Some(t) => t,
                None => sel::dashboard_text_for_lang(Some(""))?,
            };
            let snapshot = self
                .page
                .evaluate(&scripts::snapshot(t), &CancelToken::new())?;
            // 文字列でない・空なら空ファイルを成功として残さず、失敗（警告ログと None）にする
            let body = snapshot
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    Error::Dashboard("the page snapshot was not a non-empty string".to_string())
                })?;
            // 保存前に必ず伏せ字を通す（URL の publisherId に加え、本文・属性に現れる分と、アカウントの氏名・メールアドレスも）
            std::fs::write(&path, redact_diagnostics(body, &self.publisher_id))?;
            Ok(path.display().to_string())
        };
        match run() {
            Ok(p) => Some(p),
            Err(e) => {
                log(
                    &format!("診断用のページのスナップショット保存に失敗: {e}"),
                    LogLevel::Warning,
                );
                None
            }
        }
    }
}

impl ListingPage for DashboardItemPage<'_> {
    fn read_fields(
        &mut self,
        fields: &[ListingField],
        cancel: &CancelToken,
    ) -> Result<FieldValues> {
        let mut result = FieldValues::new();
        for field in fields {
            let (kind, element) = self.locate(*field, cancel)?;
            result.insert(*field, self.value(kind, element.index, cancel)?);
        }
        Ok(result)
    }

    fn write_field(
        &mut self,
        field: ListingField,
        value: &str,
        cancel: &CancelToken,
    ) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        let (kind, element) = self.locate(field, cancel)?;
        sel::ensure_writable(&element, field)?;
        self.rewrite_text(kind, &element, field.display_name(), value, cancel)
    }

    fn read_category(&mut self, cancel: &CancelToken) -> Result<String> {
        let c = self.category_text()?;
        self.page.prepare_for_input(cancel)?;
        // 一覧は閉じたまま読む（閉じた一覧にも選択肢は全部ある）。reload 直後は選択肢が揃うまで少し待ち、
        // 待ち切れなければ最後の失敗（見つからない・複数など）をそのまま返す
        // 選択中がプレースホルダーのままだったり、コンボボックスの表示値と食い違ったりする間（再読込の直後の
        // 一時的な状態）も上限つき（8 秒）で待ち、上限に達したらその時点の値で判定する
        let deadline = Instant::now() + Duration::from_secs(15);
        let settle_deadline = Instant::now() + Duration::from_secs(8);
        loop {
            cancel.check()?;
            let options = self.collect(kinds::LISTBOX_OPTION, cancel)?;
            match sel::selected_category(c, &options) {
                Ok(value) => {
                    let combos = self.collect(kinds::COMBOBOX, cancel)?;
                    let combo = sel::pick_category_combobox(c, &combos).ok();
                    if sel::category_reading_is_stable(c, combo, &options)
                        || Instant::now() >= settle_deadline
                    {
                        return Ok(value);
                    }
                    cancel.sleep(POLL_INTERVAL)?;
                }
                Err(e) if Instant::now() >= deadline => return Err(e),
                Err(_) => cancel.sleep(POLL_INTERVAL)?,
            }
        }
    }

    fn write_category(&mut self, data_value: &str, cancel: &CancelToken) -> Result<()> {
        let c = self.category_text()?;
        self.page.prepare_for_input(cancel)?;
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_category_combobox(c, &combos)?;
        if combo.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_field_cannot_be_edited",
                name = c.label
            )));
        }
        let what = tr!("dashboard.the_category_combo_box");
        self.click(kinds::COMBOBOX, combo.index, &what, cancel)?;

        // 一覧が開いて見えるまで待ち、選ぶ選択肢をちょうど 1 つに絞る（言語切替と同じ流れ）
        let option_index = poll(
            || {
                let options = self.collect(kinds::LISTBOX_OPTION, cancel)?;
                if !sel::category_list_visible(c, &options) {
                    return Ok(None);
                }
                Ok(Some(
                    sel::pick_category_option(c, &options, data_value)?.index,
                ))
            },
            Duration::from_secs(10),
            &tr!("dashboard.the_category_options_to_appear"),
            cancel,
        )?;
        self.click(
            kinds::LISTBOX_OPTION,
            option_index,
            &tr!("dashboard.the_category_option", value = data_value),
            cancel,
        )?;

        // 一覧が閉じ、選んだ選択肢が選択状態になるまで待つ
        let settled = try_poll(
            || Ok(self.category_settled(c, data_value, cancel)?.then_some(())),
            Duration::from_secs(5),
            cancel,
        )?;
        if !settled {
            // 閉じなければ言語切替と同じく Escape で閉じる。選択が違っていれば（閉じても）止める
            self.page.press_key("Escape", 27, cancel)?;
        }
        poll(
            || Ok(self.category_settled(c, data_value, cancel)?.then_some(())),
            Duration::from_secs(5),
            &tr!("dashboard.the_category_to_be_selected", value = data_value),
            cancel,
        )
    }

    fn submit_for_review(
        &mut self,
        confirm: &mut SubmitConfirm<'_>,
        cancel: &CancelToken,
    ) -> Result<SubmitResult> {
        let t = self.t()?;
        let (submit, submitted_message) = (t.submit_button, t.submitted_message);
        self.page.prepare_for_input(cancel)?;
        if !self.dialogs(cancel)?.is_empty() {
            return Err(Error::Dashboard(tr!(
                "dashboard.not_starting_the_review_submission_because_another"
            )));
        }
        // 送信前から送信完了の語を含むメッセージが出ていると、送信後の成功を確かめられない。
        // 英語の暫定の語（"submitted"）は短く、常設の通知に含まれる恐れもあるので、押す前に確かめて止める
        // 日本語の画面は 2026-11-15 に実画面で確認した送信の流れを変えないため、英語の画面だけで確かめる
        if t.lang == DashboardLang::En
            && sel::has_message(t, &self.messages(cancel)?, submitted_message)
        {
            return Err(Error::Dashboard(tr!(
                "dashboard.a_submission_message_is_already_shown_before",
                submitted_message = submitted_message
            )));
        }

        let buttons = self.collect(kinds::BUTTON, cancel)?;
        let button = sel::pick_button(&buttons, submit)?;
        if button.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_button_cannot_be_clicked_2",
                submit = submit
            )));
        }
        self.click(
            kinds::BUTTON,
            button.index,
            &tr!("dashboard.the_button_2", submit = submit),
            cancel,
        )?;

        // 確認ダイアログがちょうど 1 つ出るまで待つ
        let dialog_text = poll(
            || {
                let dialogs = self.dialogs(cancel)?;
                if dialogs.len() > 1 {
                    let n = dialogs.len();
                    return Err(Error::Dashboard(tr!(
                        "dashboard.dialogs_are_open_cannot_identify_the_confirmation",
                        count = n
                    )));
                }
                Ok(dialogs.into_iter().next())
            },
            Duration::from_secs(15),
            &tr!("dashboard.the_review_submission_confirmation_dialog_to_appear"),
            cancel,
        )?;

        // 自動公開チェックは読むだけで変更しない
        let checks = self.collect(kinds::DIALOG_CHECKBOX, cancel)?;
        let auto_publish = sel::auto_publish_state(t, &checks);
        let info = SubmitDialogInfo {
            text: dialog_text,
            auto_publish,
        };

        let dialog_buttons = self.collect(kinds::DIALOG_BUTTON, cancel)?;
        if !confirm(&info)? {
            let cancel_label = t.cancel_button;
            let cancel_button = sel::pick_button(&dialog_buttons, cancel_label)?;
            self.click(
                kinds::DIALOG_BUTTON,
                cancel_button.index,
                &tr!(
                    "dashboard.in_the_confirmation_dialog",
                    cancel_label = cancel_label
                ),
                cancel,
            )?;
            let status = sel::parse_status(t, Some(&self.body_text(cancel)?));
            return Ok(SubmitResult {
                submitted: false,
                dialog: info,
                message: None,
                status_text: status,
            });
        }

        let ok = sel::pick_button(&dialog_buttons, submit)?;
        if ok.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_submit_button_in_the_confirmation_dialog"
            )));
        }
        self.click(
            kinds::DIALOG_BUTTON,
            ok.index,
            &tr!("dashboard.in_the_confirmation_dialog_2", submit = submit),
            cancel,
        )?;

        let mut messages: Vec<String> = Vec::new();
        let sent = try_poll(
            || {
                messages = self.messages(cancel)?;
                Ok(sel::has_message(t, &messages, submitted_message).then_some(()))
            },
            Duration::from_secs(45),
            cancel,
        )?;

        let result_messages = sel::result_messages(t, &messages);
        let status = sel::parse_status(t, Some(&self.body_text(cancel)?));
        if !sent {
            let tail = if result_messages.is_empty() {
                String::new()
            } else {
                let joined = result_messages.join(" / ");
                tr!("dashboard.shown", joined = joined)
            };
            let shown_status = status.clone().unwrap_or_else(|| tr!("cli.unknown"));
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_confirm_the_submission_message_status",
                submitted_message = submitted_message,
                shown_status = shown_status,
                suffix = tail
            )));
        }
        let message = result_messages
            .iter()
            .find(|m| sel::message_matches(t, m, submitted_message))
            .cloned();
        log(
            &format!(
                "審査送信を確認: {} ステータス={}",
                self.item_label(),
                status.as_deref().unwrap_or("")
            ),
            LogLevel::Info,
        );
        Ok(SubmitResult {
            submitted: true,
            dialog: info,
            message,
            status_text: status,
        })
    }
}

impl DashboardItemPage<'_> {
    /// 入力欄（textarea・input）を `value` に書き換え、値が完全一致するまで待つ（フォーカス → 全選択 → insertText）。
    /// 掲載情報の欄とプライバシー タブの欄で共通。一致しなければ保存せずに止める
    fn rewrite_text(
        &self,
        kind: &'static str,
        element: &ElementInfo,
        name: &str,
        value: &str,
        cancel: &CancelToken,
    ) -> Result<()> {
        let t = self.t()?;
        self.evaluate(&scripts::scroll_to(t, kind, element.index), cancel)?;
        let selected = self.evaluate(&scripts::focus_select_all(t, kind, element.index), cancel)?;
        if selected != Value::Bool(true) {
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_focus_the_field_and_select",
                name = name
            )));
        }

        if value.is_empty() {
            // Input.insertText("") では選択範囲が消えないため、Delete キーで消す
            self.page.press_key("Delete", 46, cancel)?;
        } else {
            self.page.insert_text(value, cancel)?;
        }

        // 書き換え後の値が正本と完全一致するまで待つ。一致しなければ保存せずに止める
        let mut last: Option<String> = None;
        let ok = try_poll(
            || {
                let v = self.value(kind, element.index, cancel)?;
                let same = v == value;
                last = Some(v);
                Ok(same.then_some(()))
            },
            Duration::from_secs(10),
            cancel,
        )?;
        if !ok {
            let last = last.unwrap_or_default();
            // 表示は 1 始まり。一致しなかった経路なので位置は必ずあるが、無ければ 0 を出さず「?」にする。
            let at = first_difference_position(&last, value)
                .map_or_else(|| "?".to_string(), |p| p.to_string());
            let (now, want) = (utf16_len(&last), utf16_len(value));
            return Err(Error::Dashboard(tr!(
                "dashboard.rewrote_but_the_value_does_not_match",
                name = name,
                position = at,
                now = now,
                want = want
            )));
        }
        Ok(())
    }

    /// カテゴリ欄の文言（日本語の画面だけ。英語の画面は文言が未確認なので、書き込みも読み出しも止める）
    fn category_text(&self) -> Result<&'static CategoryText> {
        self.t()?.category.as_ref().ok_or_else(|| {
            Error::Dashboard(tr!(
                "dashboard.the_category_is_only_supported_on_a_japanese_dashboard"
            ))
        })
    }

    /// カテゴリ欄の一覧が閉じ（コンボボックスの aria-expanded が "false"）、`data_value` の選択肢だけが
    /// 選択状態になっているか
    fn category_settled(
        &self,
        c: &CategoryText,
        data_value: &str,
        cancel: &CancelToken,
    ) -> Result<bool> {
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_category_combobox(c, &combos)?;
        if combo.expanded.as_deref() != Some("false") {
            return Ok(false);
        }
        let options = self.collect(kinds::LISTBOX_OPTION, cancel)?;
        Ok(sel::selected_category(c, &options).is_ok_and(|v| v == data_value))
    }

    /// 「プライバシー」タブの文言（日本語の画面だけ。英語の画面は文言が未確認なので止める）
    fn privacy_text(&self) -> Result<&'static PrivacyText> {
        self.t()?.privacy.as_ref().ok_or_else(|| {
            Error::Dashboard(tr!(
                "dashboard.the_privacy_tab_is_only_supported_on_a_japanese_dashboard"
            ))
        })
    }

    /// 「プライバシー」タブの入力欄（単一用途の説明の textarea とデータ使用のチェックボックス）が揃うまで待つ。
    /// SPA なので読み込み後に中身が遅れて出る
    fn wait_for_privacy_ready(&self, cancel: &CancelToken) -> Result<()> {
        let p = self.privacy_text()?;
        let what = tr!("dashboard.the_privacy_tab_fields", item = self.item_label());
        poll(
            || {
                let textareas = self.collect(kinds::TEXTAREA, cancel)?;
                let checks = self.collect(kinds::CHECKBOX, cancel)?;
                let areas = sel::classify_privacy_textareas(p, &textareas)?;
                let ready =
                    areas.single_purpose.is_some() && sel::collected_kinds(p, &checks)?.is_some();
                Ok(ready.then_some(()))
            },
            Duration::from_secs(45),
            &what,
            cancel,
        )
    }

    /// プライバシー ポリシー URL 欄が出るまで上限つき（15 秒）で待って返す（出なければ None＝欄が無い扱い。複数なら止める）。
    /// 設定に policyUrl があるときだけ呼ぶ。SPA なので、他の欄が出てもこの欄は遅れて出ることがある
    fn wait_for_policy_url_input(
        &self,
        p: &PrivacyText,
        cancel: &CancelToken,
    ) -> Result<Option<ElementInfo>> {
        let mut found: Option<ElementInfo> = None;
        try_poll(
            || {
                let inputs = self.collect(kinds::TEXT_INPUT, cancel)?;
                found = sel::find_policy_url_input(p, &inputs)?.cloned();
                Ok(found.is_some().then_some(()))
            },
            Duration::from_secs(15),
            cancel,
        )?;
        Ok(found)
    }

    /// チェックボックスを `want` の状態にする（すでにその状態なら何もしない）。
    /// 実際のマウス入力で押し、状態が変わるまで待つ。`locate` は集め直したチェックボックスから対象を 1 つ選ぶ
    fn set_checkbox(
        &self,
        locate: &dyn Fn(&[ElementInfo]) -> Result<ElementInfo>,
        label: &str,
        want: bool,
        cancel: &CancelToken,
    ) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        let current = locate(&self.collect(kinds::CHECKBOX, cancel)?)?;
        if current.checked == Some(want) {
            return Ok(());
        }
        let what = tr!("dashboard.the_privacy_checkbox", label = label);
        if current.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_image_button_cannot_be_clicked",
                what = what
            )));
        }
        self.click(kinds::CHECKBOX, current.index, &what, cancel)?;
        let changed = try_poll(
            || {
                let now = locate(&self.collect(kinds::CHECKBOX, cancel)?)?;
                Ok((now.checked == Some(want)).then_some(()))
            },
            Duration::from_secs(5),
            cancel,
        )?;
        if !changed {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_privacy_checkbox_did_not_change",
                label = label
            )));
        }
        Ok(())
    }
}

impl PrivacyPage for DashboardItemPage<'_> {
    fn open_privacy(&mut self, item_id: &str, cancel: &CancelToken) -> Result<()> {
        self.item_id = Some(item_id.to_string());
        self.tab = Tab::Privacy;
        self.page.prepare_for_input(cancel)?;
        self.page
            .navigate(&urls::privacy_url(&self.publisher_id, item_id), cancel)
            .map_err(|e| match e {
                Error::Cdp(m) => Error::Dashboard(tr!(
                    "dashboard.could_not_open_the_privacy_tab",
                    item_id = item_id,
                    detail = m
                )),
                other => other,
            })?;

        let url = self.page.get_url(cancel)?;
        if urls::is_login_page(url.as_deref()) {
            return Err(Error::LoginRequired);
        }
        // 新しいページなので表示言語を判定し直す（導入部を含む式より前に）。日本語以外はここで止める
        self.detect_language(cancel)?;
        self.privacy_text()?;
        self.wait_for_privacy_ready(cancel)?;

        // 読み込み後に別画面へ移されていないか確認する（アイテム ID の誤りで起きる）
        let url = self.page.get_url(cancel)?;
        if !urls::is_privacy_page_of(url.as_deref(), item_id) {
            return Err(Error::Dashboard(tr!(
                "dashboard.could_not_open_the_privacy_tab_2",
                item_id = item_id
            )));
        }
        Ok(())
    }

    fn read_privacy(
        &mut self,
        targets: &ExpectedPrivacy,
        cancel: &CancelToken,
    ) -> Result<PrivacyReading> {
        let p = self.privacy_text()?;
        self.page.prepare_for_input(cancel)?;
        let textareas = self.collect(kinds::TEXTAREA, cancel)?;
        let areas = sel::classify_privacy_textareas(p, &textareas)?;
        let mut reading = PrivacyReading::default();
        if targets.single_purpose.is_some()
            && let Some(element) = areas.single_purpose
        {
            reading.single_purpose = Some(self.value(kinds::TEXTAREA, element.index, cancel)?);
        }
        if let Some(wanted) = &targets.permissions {
            // 画面にある権限の名前は全部読み、本文は設定にある権限の分だけ読む
            reading.screen_permissions = areas.permissions.iter().map(|(k, _)| k.clone()).collect();
            for (key, element) in &areas.permissions {
                if wanted.iter().any(|(k, _)| k == key) {
                    reading.permissions.insert(
                        key.clone(),
                        self.value(kinds::TEXTAREA, element.index, cancel)?,
                    );
                }
            }
        }
        if targets.data_usage.is_some() {
            let checks = self.collect(kinds::CHECKBOX, cancel)?;
            reading.collected = sel::collected_kinds(p, &checks)?;
            reading.certified = sel::certified(p, &checks)?;
        }
        if targets.policy_url.is_some()
            && let Some(element) = self.wait_for_policy_url_input(p, cancel)?
        {
            reading.policy_url = Some(self.value(kinds::TEXT_INPUT, element.index, cancel)?);
        }
        Ok(reading)
    }

    fn write_privacy_text(
        &mut self,
        field: &PrivacyField,
        value: &str,
        cancel: &CancelToken,
    ) -> Result<()> {
        let p = self.privacy_text()?;
        self.page.prepare_for_input(cancel)?;
        let textareas = self.collect(kinds::TEXTAREA, cancel)?;
        let areas = sel::classify_privacy_textareas(p, &textareas)?;
        let found = match field {
            PrivacyField::SinglePurpose => areas.single_purpose,
            PrivacyField::Permission(key) => areas
                .permissions
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, e)| *e),
            // テキスト欄ではない欄（チェックボックス・リモートコード）は、ここでは書かない
            _ => None,
        };
        let name = field.display_name();
        let Some(element) = found else {
            return Err(Error::Dashboard(tr!(
                "privacy.the_field_is_not_on_the_dashboard",
                label = name
            )));
        };
        sel::ensure_privacy_text_writable(element, &name)?;
        let element = element.clone();
        self.rewrite_text(kinds::TEXTAREA, &element, &name, value, cancel)
    }

    fn set_data_kinds(&mut self, wanted: &BTreeSet<DataKind>, cancel: &CancelToken) -> Result<()> {
        let p = self.privacy_text()?;
        for kind in DataKind::ALL {
            let label = p.data_kind_labels[DataKind::ALL
                .iter()
                .position(|k| *k == kind)
                .expect("ALL に全種類がある")];
            self.set_checkbox(
                &|checks| sel::pick_data_kind_checkbox(p, checks, kind).cloned(),
                label,
                wanted.contains(&kind),
                cancel,
            )?;
        }
        Ok(())
    }

    fn write_policy_url(&mut self, value: &str, cancel: &CancelToken) -> Result<()> {
        let p = self.privacy_text()?;
        self.page.prepare_for_input(cancel)?;
        let name = PrivacyField::PolicyUrl.display_name();
        let Some(element) = self.wait_for_policy_url_input(p, cancel)? else {
            return Err(Error::Dashboard(tr!(
                "privacy.the_field_is_not_on_the_dashboard",
                label = name
            )));
        };
        sel::ensure_privacy_url_writable(&element, &name)?;
        self.rewrite_text(kinds::TEXT_INPUT, &element, &name, value, cancel)
    }

    fn check_certifications(&mut self, cancel: &CancelToken) -> Result<()> {
        let p = self.privacy_text()?;
        for (index, label) in p.certify_labels.iter().enumerate() {
            self.set_checkbox(
                &|checks| sel::pick_certify_checkbox(p, checks, index).cloned(),
                label,
                true,
                cancel,
            )?;
        }
        Ok(())
    }
}

impl ImagePage for DashboardItemPage<'_> {
    fn read_images(&mut self, slot: ImageSlot, cancel: &CancelToken) -> Result<Vec<PageImage>> {
        Ok(self.locate_slot(slot, cancel)?.images)
    }

    fn delete_image(
        &mut self,
        slot: ImageSlot,
        position: usize,
        cancel: &CancelToken,
    ) -> Result<()> {
        self.ensure_image_write_supported()?;
        let t = self.t()?;
        self.page.prepare_for_input(cancel)?;
        let info = self.locate_slot(slot, cancel)?;
        let before = info.images.len();
        let Some(target) = info.images.get(position) else {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_to_delete_does_not_exist",
                name = slot.display_name(),
                position = position + 1,
                count = before
            )));
        };
        let alt = target.alt.clone();

        // 削除ボタンはホバーで現れる（普段は 0x0・透明）ので、先に画像の上へマウスを動かす
        let thumbs = sel::parse_elements(&self.evaluate(
            &scripts::collect_slot(t, kinds::SLOT_IMAGE, info.index),
            cancel,
        )?);
        let thumb = thumbs.get(position).ok_or_else(|| {
            Error::Dashboard(tr!(
                "dashboard.the_image_to_delete_does_not_exist",
                name = slot.display_name(),
                position = position + 1,
                count = thumbs.len()
            ))
        })?;
        self.evaluate(
            &scripts::scroll_to(t, kinds::SLOT_IMAGE, thumb.index),
            cancel,
        )?;
        cancel.sleep(Duration::from_millis(150))?;
        let point = self.evaluate(&scripts::point(t, kinds::SLOT_IMAGE, thumb.index), cancel)?;
        let x = point.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = point.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        self.page.hover_at(x, y, cancel)?;

        // 削除ボタンが表示される（大きさを持つ）まで待つ。aria-label が「画像を削除 <alt>」に完全一致するものだけを選ぶ
        let what = tr!(
            "dashboard.the_delete_button_of_image",
            name = slot.display_name(),
            alt = alt
        );
        let button = poll(
            || {
                let buttons = sel::parse_elements(&self.evaluate(
                    &scripts::collect_slot(t, kinds::IMAGE_BUTTON, info.index),
                    cancel,
                )?);
                let b = sel::pick_delete_button(t, &buttons, &alt)?;
                Ok((b.width > 0 && b.height > 0).then(|| b.clone()))
            },
            Duration::from_secs(10),
            &what,
            cancel,
        )?;
        if button.disabled {
            return Err(Error::WriteBlocked(tr!(
                "dashboard.the_image_button_cannot_be_clicked",
                what = what
            )));
        }
        // 押してよいのは削除ボタンだけ（並べ替えなどは押さない）
        sel::ensure_image_click_allowed(t, &button.label)?;
        self.click(kinds::IMAGE_BUTTON, button.index, &what, cancel)?;

        // 削除の確認ダイアログ（「この画像を削除してもよろしいですか？」）が出たら、文面を確かめて確定ボタンだけを押す。
        // 文面が違うダイアログ・複数のダイアログなら、押さずに止める。その後、枚数が 1 枚減るまで待つ
        let mut dialog: Option<String> = None;
        let mut confirmed = false;
        let done = try_poll(
            || {
                if !confirmed {
                    let dialogs = self.dialogs(cancel)?;
                    if let Some(first) = dialogs.first() {
                        let known =
                            dialogs.len() == 1 && contains_ic(first, t.delete_image_confirm_marker);
                        if !known {
                            dialog = Some(first.clone());
                            return Ok(Some(()));
                        }
                        let buttons = self.collect(kinds::DIALOG_BUTTON, cancel)?;
                        let ok = sel::pick_button(&buttons, t.delete_image_confirm_button)?;
                        if ok.disabled {
                            return Err(Error::WriteBlocked(tr!(
                                "dashboard.the_image_button_cannot_be_clicked",
                                what = t.delete_image_confirm_button
                            )));
                        }
                        self.click(
                            kinds::DIALOG_BUTTON,
                            ok.index,
                            &tr!(
                                "dashboard.the_button_in_the_delete_image_dialog",
                                label = t.delete_image_confirm_button
                            ),
                            cancel,
                        )?;
                        confirmed = true;
                        return Ok(None);
                    }
                }
                let now = self.locate_slot(slot, cancel)?.images.len();
                Ok((now + 1 == before).then_some(()))
            },
            Duration::from_secs(20),
            cancel,
        )?;
        if let Some(d) = dialog {
            return Err(Error::Dashboard(tr!(
                "dashboard.a_dialog_appeared_after_deleting_the_image",
                text = d
            )));
        }
        if !done {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_count_did_not_decrease_after_deleting",
                name = slot.display_name()
            )));
        }
        Ok(())
    }

    fn upload_image(&mut self, slot: ImageSlot, path: &Path, cancel: &CancelToken) -> Result<()> {
        self.ensure_image_write_supported()?;
        let t = self.t()?;
        if !path.is_file() {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_file_does_not_exist",
                path = path.display()
            )));
        }
        self.page.prepare_for_input(cancel)?;
        let info = self.locate_slot(slot, cancel)?;
        let before = info.images.len();
        if before >= slot.max_count() {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_slot_is_already_full",
                name = slot.display_name(),
                max = slot.max_count()
            )));
        }
        // パスは CDP の引数で渡す（ページ内スクリプトには埋め込まない）
        let expression = scripts::element(t, kinds::FILE_INPUT, info.index);
        self.page
            .set_file_input_files(&expression, &[path.to_path_buf()], cancel)
            .map_err(page_error)?;

        // 枚数が 1 枚増え、alt の番号が揃い、エラー表示が無くなるまで待つ（1 枚 60 秒まで）
        let mut last_error: Option<String> = None;
        let mut last_count = before;
        let done = try_poll(
            || {
                let now = self.locate_slot(slot, cancel)?;
                last_count = now.images.len();
                let errors = sel::image_error_messages(t, &self.messages(cancel)?);
                if !errors.is_empty() {
                    last_error = Some(errors.join(" / "));
                    return Ok(Some(()));
                }
                if now.images.len() == before + 1
                    && sel::ensure_image_alts(t, slot, &now.images).is_ok()
                {
                    return Ok(Some(()));
                }
                Ok(None)
            },
            Duration::from_secs(60),
            cancel,
        )?;
        if let Some(e) = last_error {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_upload_showed_an_error",
                name = slot.display_name(),
                path = path.display(),
                error = e
            )));
        }
        if !done {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_upload_did_not_finish",
                name = slot.display_name(),
                path = path.display(),
                before = before,
                now = last_count
            )));
        }
        Ok(())
    }
}

/// 条件が満たされる（Some を返す）まで繰り返し確かめる。上限を超えたら `Dashboard` エラー
fn poll<T>(
    mut probe: impl FnMut() -> Result<Option<T>>,
    timeout: Duration,
    what: &str,
    cancel: &CancelToken,
) -> Result<T> {
    let deadline = Instant::now() + timeout;
    loop {
        cancel.check()?;
        if let Some(v) = probe()? {
            return Ok(v);
        }
        if Instant::now() >= deadline {
            let secs = timeout.as_secs();
            return Err(Error::Dashboard(tr!(
                "dashboard.waited_seconds_for_but_could_not_confirm",
                secs = secs,
                what = what
            )));
        }
        cancel.sleep(POLL_INTERVAL)?;
    }
}

/// 条件が満たされるまで確かめる。上限を超えたら false
fn try_poll(
    mut probe: impl FnMut() -> Result<Option<()>>,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    loop {
        cancel.check()?;
        if probe()?.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        cancel.sleep(POLL_INTERVAL)?;
    }
}
