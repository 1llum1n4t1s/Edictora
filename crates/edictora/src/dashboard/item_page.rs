//! アイテムの編集画面（ストアの掲載情報）を CDP で操作する [`ListingPage`] 実装（C# の `DashboardItemPage`）。
//!
//! 2026-11-13 の検証（DESIGN.md）で確かめた操作手順をそのまま使う。
//! - 操作の前に毎回 Emulation.setFocusEmulationEnabled を行う（ウィンドウを前面には出さない）
//! - クリックは scrollIntoView → 矩形の中心へ実際のマウス入力（他要素に覆われていたら止める）
//! - 書き換えはフォーカス → 全選択 → Input.insertText。DOM の value への直接代入はしない
//! - 待ちは固定時間の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）
//! - 画面の表示言語（日本語 / 英語）は、編集画面を開く・読み込み直すたびに `<html lang>` から 1 回だけ判定し、
//!   以降はその言語の文言セットだけで要素を探す（`text.rs`）。日本語・英語以外なら止める

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::cdp::CdpPage;
use crate::dashboard::page_scripts::{self as scripts, kinds};
use crate::dashboard::selectors::{self as sel, ElementInfo};
use crate::dashboard::text::{DashboardLang, DashboardText};
use crate::dashboard::urls;
use crate::dashboard::write_gate::ItemPageState;
use crate::dashboard::{ListingPage, SubmitConfirm, SubmitDialogInfo, SubmitResult};
use crate::error::{Error, Result};
use crate::listing::text_diff::first_difference_position;
use crate::listing::{FieldValues, ListingField};
use crate::logging::{LogLevel, log};
use crate::tr;
use crate::util::{LocalTime, diagnostics_file_name, eq_ic, utf16_len};

/// 条件確認の間隔
const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// 編集画面の操作
pub struct DashboardItemPage<'a> {
    page: &'a CdpPage,
    publisher_id: String,
    item_id: Option<String>,
    diagnostics_dir: PathBuf,
    /// 今の画面の表示言語の文言セット（open_item / reload で判定する。判定前は None）
    text: Option<&'static DashboardText>,
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

    /// ページ側の例外はダッシュボード操作の失敗として扱う。
    /// ページ内スクリプトが固定の印で投げた例外（要素が消えた）は、ここで利用者向けの文に直す
    fn evaluate(&self, expression: &str, cancel: &CancelToken) -> Result<Value> {
        self.page.evaluate(expression, cancel).map_err(|e| match e {
            Error::JavaScript(m) => Error::Dashboard(sel::page_script_error_message(&m)),
            other => other,
        })
    }
}

impl ListingPage for DashboardItemPage<'_> {
    fn open_item(&mut self, item_id: &str, cancel: &CancelToken) -> Result<()> {
        self.item_id = Some(item_id.to_string());
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
        self.wait_for_editor_ready(cancel)
    }

    fn get_state(&mut self, cancel: &CancelToken) -> Result<ItemPageState> {
        // CDP での収集はこの順（本文 → textarea → ボタン）のまま。判断は selectors の純粋関数で行う
        let t = self.t()?;
        let status = sel::parse_status(t, Some(&self.body_text(cancel)?));
        let textareas = self.collect(kinds::TEXTAREA, cancel)?;
        let buttons = self.collect(kinds::BUTTON, cancel)?;
        Ok(sel::item_page_state(t, status, &textareas, &buttons))
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

        let t = self.t()?;
        self.evaluate(&scripts::scroll_to(t, kind, element.index), cancel)?;
        let selected = self.evaluate(&scripts::focus_select_all(t, kind, element.index), cancel)?;
        let name = field.display_name();
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

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        // 失敗時の記録なので、中断の後でも保存を試みる（新しい合図を使う）
        let run = || -> Result<String> {
            std::fs::create_dir_all(&self.diagnostics_dir)?;
            // ファイル名の規則（yyyyMMdd-HHmmss-<安全化したラベル>.png）は純粋関数にまとめ、util のテストで固めている
            let name = diagnostics_file_name(&LocalTime::now(), label);
            let path = self.diagnostics_dir.join(name);
            let png = self.page.capture_screenshot(&CancelToken::new())?;
            std::fs::write(&path, png)?;
            Ok(path.display().to_string())
        };
        match run() {
            Ok(p) => Some(p),
            Err(e) => {
                log(&format!("診断用の画面保存に失敗: {e}"), LogLevel::Warning);
                None
            }
        }
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
