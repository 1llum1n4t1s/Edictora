//! アイテムの編集画面（ストアの掲載情報）を CDP で操作する [`ListingPage`] 実装（C# の `DashboardItemPage`）。
//!
//! 2026-11-13 の検証（DESIGN.md）で確かめた操作手順をそのまま使う。
//! - 操作の前に毎回 Emulation.setFocusEmulationEnabled を行う（ウィンドウを前面には出さない）
//! - クリックは scrollIntoView → 矩形の中心へ実際のマウス入力（他要素に覆われていたら止める）
//! - 書き換えはフォーカス → 全選択 → Input.insertText。DOM の value への直接代入はしない
//! - 待ちは固定時間の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::cdp::CdpPage;
use crate::dashboard::page_scripts::{self as scripts, kinds};
use crate::dashboard::selectors::{self as sel, ElementInfo};
use crate::dashboard::text::*;
use crate::dashboard::urls;
use crate::dashboard::write_gate::ItemPageState;
use crate::dashboard::{ListingPage, SubmitConfirm, SubmitDialogInfo, SubmitResult};
use crate::error::{Error, Result};
use crate::listing::text_diff::first_difference_position;
use crate::listing::{FieldValues, ListingField};
use crate::logging::{LogLevel, log};
use crate::util::{LocalTime, diagnostics_file_name, eq_ic, utf16_len};

/// 条件確認の間隔
const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// 編集画面の操作
pub struct DashboardItemPage<'a> {
    page: &'a CdpPage,
    publisher_id: String,
    item_id: Option<String>,
    diagnostics_dir: PathBuf,
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
        }
    }

    fn item_label(&self) -> String {
        self.item_id.clone().unwrap_or_default()
    }

    /// SPA なので load 後も中身が遅れて出る。言語コンボボックスと説明欄が揃うまで待つ
    fn wait_for_editor_ready(&self, cancel: &CancelToken) -> Result<()> {
        let what = format!(
            "アイテム {} の編集画面（言語の選択欄と説明欄）の表示",
            self.item_label()
        );
        poll(
            || {
                let combos = self.collect(kinds::COMBOBOX, cancel)?;
                let textareas = self.collect(kinds::TEXTAREA, cancel)?;
                // 待ち条件なので緩い部分一致で足りる
                let has_language = combos.iter().any(sel::mentions_language_label);
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
        poll(
            || {
                let options = self.collect(kinds::OPTION, cancel)?;
                let any = options
                    .iter()
                    .any(|o| sel::parse_locale_option(Some(&o.text)).is_some());
                Ok(any.then_some(options))
            },
            Duration::from_secs(10),
            "言語の選択肢の表示",
            cancel,
        )
    }

    /// 言語欄の aria-expanded が "false" か（一覧が閉じているか）
    fn is_language_list_closed(&self, cancel: &CancelToken) -> Result<bool> {
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(&combos)?;
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
            sel::pick_field_by_label(&inputs, field)?.clone(),
        ))
    }

    /// 要素を画面内へスクロールし、中心を実際のマウス入力でクリックする
    fn click(&self, kind: &str, index: i32, what: &str, cancel: &CancelToken) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        self.evaluate(&scripts::scroll_to(kind, index), cancel)?;
        cancel.sleep(Duration::from_millis(150))?;

        // スクロール後の矩形を読み直す。中心が別の要素に覆われていたら押さない
        let point = self.evaluate(&scripts::point(kind, index), cancel)?;
        if !point.is_object() {
            return Err(Error::Dashboard(format!(
                "{what}の位置を取得できませんでした"
            )));
        }
        if point.get("hit").and_then(Value::as_bool) != Some(true) {
            return Err(Error::Dashboard(format!(
                "{what}が他の要素に覆われていて押せません"
            )));
        }
        let x = point.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = point.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        self.page.click_at(x, y, cancel)
    }

    fn collect(&self, kind: &str, cancel: &CancelToken) -> Result<Vec<ElementInfo>> {
        Ok(sel::parse_elements(
            &self.evaluate(&scripts::collect(kind), cancel)?,
        ))
    }

    fn value(&self, kind: &str, index: i32, cancel: &CancelToken) -> Result<String> {
        let v = self.evaluate(&scripts::value(kind, index), cancel)?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    fn messages(&self, cancel: &CancelToken) -> Result<Vec<String>> {
        Ok(sel::parse_strings(
            &self.evaluate(&scripts::messages(), cancel)?,
        ))
    }

    fn dialogs(&self, cancel: &CancelToken) -> Result<Vec<String>> {
        Ok(sel::parse_strings(
            &self.evaluate(&scripts::dialogs(), cancel)?,
        ))
    }

    fn body_text(&self, cancel: &CancelToken) -> Result<String> {
        let v = self.evaluate(&scripts::body_text(), cancel)?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    /// ページ側の例外はダッシュボード操作の失敗として扱う
    fn evaluate(&self, expression: &str, cancel: &CancelToken) -> Result<Value> {
        self.page.evaluate(expression, cancel).map_err(|e| match e {
            Error::JavaScript(m) => Error::Dashboard(m),
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
                Error::Cdp(m) => Error::Dashboard(format!(
                    "アイテム {item_id} の編集画面を開けませんでした: {m}"
                )),
                other => other,
            })?;

        let url = self.page.get_url(cancel)?;
        if urls::is_login_page(url.as_deref()) {
            return Err(Error::LoginRequired);
        }
        self.wait_for_editor_ready(cancel)?;

        // 読み込み後に別画面（一覧など）へ移されていないか確認する。アイテム ID の誤りで起きる
        let url = self.page.get_url(cancel)?;
        if !urls::is_edit_page_of(url.as_deref(), item_id) {
            return Err(Error::Dashboard(format!(
                "アイテム {item_id} の編集画面を開けませんでした（別の画面へ移りました）。アイテム ID とパブリッシャーを確認してください"
            )));
        }
        Ok(())
    }

    fn reload(&mut self, cancel: &CancelToken) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        self.page.reload(cancel).map_err(|e| match e {
            Error::Cdp(m) => Error::Dashboard(format!("編集画面を読み込み直せませんでした: {m}")),
            other => other,
        })?;
        self.wait_for_editor_ready(cancel)
    }

    fn get_state(&mut self, cancel: &CancelToken) -> Result<ItemPageState> {
        // CDP での収集はこの順（本文 → textarea → ボタン）のまま。判断は selectors の純粋関数で行う
        let status = sel::parse_status(Some(&self.body_text(cancel)?));
        let textareas = self.collect(kinds::TEXTAREA, cancel)?;
        let buttons = self.collect(kinds::BUTTON, cancel)?;
        Ok(sel::item_page_state(status, &textareas, &buttons))
    }

    fn get_locales(&mut self, cancel: &CancelToken) -> Result<Vec<String>> {
        self.page.prepare_for_input(cancel)?;
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(&combos)?;
        let current = sel::current_locale_code(combo);
        if current.is_none() {
            // 言語が 1 つだけのアイテム。選択欄はコードなしの言語名だけで、開くと既定の言語を選ぶ
            // 全言語の一覧になるので開かない（誤って既定の言語を変えないため）。表示中の言語名から決める
            return match sel::single_locale_code(combo) {
                Some(code) => Ok(vec![code]),
                None => Err(Error::Dashboard(
                    "言語の選択欄の表示から言語を読み取れませんでした（言語が 1 つだけのアイテムで、対応していない言語名の可能性があります）".to_string(),
                )),
            };
        }
        self.click(kinds::COMBOBOX, combo.index, "言語の選択欄", cancel)?;

        let options = self.wait_for_options(cancel)?;
        let codes = sel::locale_codes(&options);

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
            let option = sel::pick_locale_option(&again, &current)?;
            self.click(kinds::OPTION, option.index, "言語の選択肢", cancel)?;
        }
        poll(
            || Ok(self.is_language_list_closed(cancel)?.then_some(())),
            Duration::from_secs(5),
            "言語の選択肢を閉じる",
            cancel,
        )?;

        if codes.is_empty() {
            return Err(Error::Dashboard(
                "言語の選択肢からロケールを読み取れませんでした".to_string(),
            ));
        }
        Ok(codes)
    }

    fn switch_locale(&mut self, code: &str, cancel: &CancelToken) -> Result<()> {
        self.page.prepare_for_input(cancel)?;
        let combos = self.collect(kinds::COMBOBOX, cancel)?;
        let combo = sel::pick_language_combobox(&combos)?;
        if sel::current_locale_code(combo).is_some_and(|c| eq_ic(&c, code)) {
            return Ok(());
        }
        // 言語が 1 つだけのアイテムは切り替えない（選択欄を開くと既定の言語の一覧になるため触らない）
        if let Some(single) = sel::single_locale_code(combo) {
            return if eq_ic(&single, code) {
                Ok(())
            } else {
                Err(Error::Dashboard(format!(
                    "このアイテムの言語は {single} だけで、{code} はありません"
                )))
            };
        }

        self.click(kinds::COMBOBOX, combo.index, "言語の選択欄", cancel)?;
        let options = self.wait_for_options(cancel)?;
        let option = sel::pick_locale_option(&options, code)?;
        self.click(
            kinds::OPTION,
            option.index,
            &format!("言語の選択肢（{code}）"),
            cancel,
        )?;

        // 選択欄の表示が切り替わるまで待つ
        poll(
            || {
                let combos = self.collect(kinds::COMBOBOX, cancel)?;
                let c = sel::pick_language_combobox(&combos)?;
                Ok(sel::current_locale_code(c)
                    .is_some_and(|c| eq_ic(&c, code))
                    .then_some(()))
            },
            Duration::from_secs(15),
            &format!("言語 {code} への切り替え"),
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
            &format!("言語 {code} の内容の表示"),
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

        self.evaluate(&scripts::scroll_to(kind, element.index), cancel)?;
        let selected = self.evaluate(&scripts::focus_select_all(kind, element.index), cancel)?;
        if selected != Value::Bool(true) {
            return Err(Error::Dashboard(format!(
                "{}の欄にフォーカスして全選択できませんでした",
                field.display_name()
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
            return Err(Error::Dashboard(format!(
                "{}を書き換えましたが、値が正本と一致しません（{at} 文字目から相違、現在 {} 文字 / 正本 {} 文字）。保存せずに止めます",
                field.display_name(),
                utf16_len(&last),
                utf16_len(value)
            )));
        }
        Ok(())
    }

    fn save_draft(&mut self, cancel: &CancelToken) -> Result<()> {
        self.page.prepare_for_input(cancel)?;

        // 前回の「保存されました」が残っていると成功を誤認するので、消えるまで待つ
        let clean = try_poll(
            || Ok((!sel::has_message(&self.messages(cancel)?, SAVED_MESSAGE)).then_some(())),
            Duration::from_secs(10),
            cancel,
        )?;
        if !clean {
            return Err(Error::Dashboard(
                "前回の保存メッセージが消えないため、保存結果を確かめられません。保存を止めます"
                    .to_string(),
            ));
        }

        let buttons = self.collect(kinds::BUTTON, cancel)?;
        let button = sel::pick_button(&buttons, SAVE_DRAFT_BUTTON)?;
        if button.disabled {
            return Err(Error::WriteBlocked(format!(
                "「{SAVE_DRAFT_BUTTON}」ボタンが押せない状態です"
            )));
        }
        self.click(
            kinds::BUTTON,
            button.index,
            &format!("「{SAVE_DRAFT_BUTTON}」ボタン"),
            cancel,
        )?;

        let mut messages: Vec<String> = Vec::new();
        let saved = try_poll(
            || {
                messages = self.messages(cancel)?;
                Ok(sel::has_message(&messages, SAVED_MESSAGE).then_some(()))
            },
            Duration::from_secs(30),
            cancel,
        )?;
        if !saved {
            let shown = sel::result_messages(&messages);
            let tail = if shown.is_empty() {
                String::new()
            } else {
                format!("。表示: {}", shown.join(" / "))
            };
            return Err(Error::Dashboard(format!(
                "下書き保存の完了メッセージ（{SAVED_MESSAGE}）を確認できませんでした{tail}"
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
        self.page.prepare_for_input(cancel)?;
        if !self.dialogs(cancel)?.is_empty() {
            return Err(Error::Dashboard(
                "別のダイアログが開いているため、審査送信を始めません".to_string(),
            ));
        }

        let buttons = self.collect(kinds::BUTTON, cancel)?;
        let button = sel::pick_button(&buttons, SUBMIT_BUTTON)?;
        if button.disabled {
            return Err(Error::WriteBlocked(format!(
                "「{SUBMIT_BUTTON}」ボタンが押せない状態です"
            )));
        }
        self.click(
            kinds::BUTTON,
            button.index,
            &format!("「{SUBMIT_BUTTON}」ボタン"),
            cancel,
        )?;

        // 確認ダイアログがちょうど 1 つ出るまで待つ
        let dialog_text = poll(
            || {
                let dialogs = self.dialogs(cancel)?;
                if dialogs.len() > 1 {
                    return Err(Error::Dashboard(format!(
                        "ダイアログが {} 個開いており、確認ダイアログを特定できません",
                        dialogs.len()
                    )));
                }
                Ok(dialogs.into_iter().next())
            },
            Duration::from_secs(15),
            "審査送信の確認ダイアログの表示",
            cancel,
        )?;

        // 自動公開チェックは読むだけで変更しない
        let checks = self.collect(kinds::DIALOG_CHECKBOX, cancel)?;
        let auto_publish = sel::auto_publish_state(&checks);
        let info = SubmitDialogInfo {
            text: dialog_text,
            auto_publish,
        };

        let dialog_buttons = self.collect(kinds::DIALOG_BUTTON, cancel)?;
        if !confirm(&info)? {
            let cancel_button = sel::pick_button(&dialog_buttons, CANCEL_BUTTON)?;
            self.click(
                kinds::DIALOG_BUTTON,
                cancel_button.index,
                "確認ダイアログの「キャンセル」",
                cancel,
            )?;
            let status = sel::parse_status(Some(&self.body_text(cancel)?));
            return Ok(SubmitResult {
                submitted: false,
                dialog: info,
                message: None,
                status_text: status,
            });
        }

        let ok = sel::pick_button(&dialog_buttons, SUBMIT_BUTTON)?;
        if ok.disabled {
            return Err(Error::WriteBlocked(
                "確認ダイアログの送信ボタンが押せない状態です".to_string(),
            ));
        }
        self.click(
            kinds::DIALOG_BUTTON,
            ok.index,
            &format!("確認ダイアログの「{SUBMIT_BUTTON}」"),
            cancel,
        )?;

        let mut messages: Vec<String> = Vec::new();
        let sent = try_poll(
            || {
                messages = self.messages(cancel)?;
                Ok(sel::has_message(&messages, SUBMITTED_MESSAGE).then_some(()))
            },
            Duration::from_secs(45),
            cancel,
        )?;

        let result_messages = sel::result_messages(&messages);
        let status = sel::parse_status(Some(&self.body_text(cancel)?));
        if !sent {
            let tail = if result_messages.is_empty() {
                String::new()
            } else {
                format!("。表示: {}", result_messages.join(" / "))
            };
            return Err(Error::Dashboard(format!(
                "送信完了のメッセージ（{SUBMITTED_MESSAGE}）を確認できませんでした。ステータス: {}{tail}",
                status.as_deref().unwrap_or("不明")
            )));
        }
        let message = result_messages
            .iter()
            .find(|m| m.contains(SUBMITTED_MESSAGE))
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
            return Err(Error::Dashboard(format!(
                "{what}を {} 秒待ちましたが確認できませんでした",
                timeout.as_secs()
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
