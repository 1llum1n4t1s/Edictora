//! C# の ListingService.test.cs の移植。画面の代わりにメモリ上の値で動く ListingPage で、手順（順番・中止判断）だけを確かめる。

use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::dashboard::write_gate::ItemPageState;
use edictora::dashboard::{ListingPage, SubmitConfirm, SubmitDialogInfo, SubmitResult};
use edictora::listing::*;
use edictora::services::{self, LocalePushOutcome};
use edictora::util::eq_ic;

const ITEM: &str = "abcdefghijklmnopabcdefghijklmnop";

/// 画面の代わりにメモリ上の値で動く ListingPage
struct FakeListingPage {
    /// 保存済み（読み込み直すと見える）値。登録順を保つ
    saved: Vec<(String, FieldValues)>,
    /// 編集中（未保存）の値
    editing: Vec<(String, FieldValues)>,
    current_locale: Option<String>,
    status: String,
    log: Vec<String>,
    /// 書き込み時に値を壊す（照合失敗の再現用）
    corrupt_writes: bool,
    /// 送信の確認で渡すダイアログ
    dialog: SubmitDialogInfo,
}

impl FakeListingPage {
    fn find<'a>(map: &'a mut [(String, FieldValues)], code: &str) -> &'a mut FieldValues {
        &mut map
            .iter_mut()
            .find(|(c, _)| eq_ic(c, code))
            .expect("ロケール")
            .1
    }

    fn saved_value(&mut self, code: &str, field: ListingField) -> String {
        Self::find(&mut self.saved, code)[&field].clone()
    }
}

impl ListingPage for FakeListingPage {
    fn open_item(&mut self, item_id: &str, _: &CancelToken) -> edictora::Result<()> {
        self.log.push(format!("open {item_id}"));
        self.editing = self.saved.clone();
        self.current_locale = self.saved.first().map(|(c, _)| c.clone());
        Ok(())
    }

    fn reload(&mut self, _: &CancelToken) -> edictora::Result<()> {
        self.log.push("reload".to_string());
        self.editing = self.saved.clone();
        self.current_locale = self.saved.first().map(|(c, _)| c.clone());
        Ok(())
    }

    fn get_state(&mut self, _: &CancelToken) -> edictora::Result<ItemPageState> {
        Ok(ItemPageState {
            status_text: Some(self.status.clone()),
            description_found: true,
            description_editable: true,
            save_button_found: true,
            save_button_enabled: true,
        })
    }

    fn get_locales(&mut self, _: &CancelToken) -> edictora::Result<Vec<String>> {
        Ok(self.saved.iter().map(|(c, _)| c.clone()).collect())
    }

    fn switch_locale(&mut self, code: &str, _: &CancelToken) -> edictora::Result<()> {
        self.log.push(format!("switch {code}"));
        self.current_locale = Some(code.to_string());
        Ok(())
    }

    fn read_fields(
        &mut self,
        fields: &[ListingField],
        _: &CancelToken,
    ) -> edictora::Result<FieldValues> {
        let code = self.current_locale.clone().unwrap();
        let values = Self::find(&mut self.editing, &code);
        Ok(fields
            .iter()
            .map(|f| (*f, values.get(f).cloned().unwrap_or_default()))
            .collect())
    }

    fn write_field(
        &mut self,
        field: ListingField,
        value: &str,
        _: &CancelToken,
    ) -> edictora::Result<()> {
        let code = self.current_locale.clone().unwrap();
        self.log.push(format!("write {code} {}", field.name()));
        let v = if self.corrupt_writes {
            format!("{value}!")
        } else {
            value.to_string()
        };
        Self::find(&mut self.editing, &code).insert(field, v);
        Ok(())
    }

    fn save_draft(&mut self, _: &CancelToken) -> edictora::Result<()> {
        self.log.push("save".to_string());
        let code = self.current_locale.clone().unwrap();
        let values = Self::find(&mut self.editing, &code).clone();
        *Self::find(&mut self.saved, &code) = values;
        Ok(())
    }

    fn submit_for_review(
        &mut self,
        confirm: &mut SubmitConfirm<'_>,
        _: &CancelToken,
    ) -> edictora::Result<SubmitResult> {
        self.log.push("submit-dialog".to_string());
        if !confirm(&self.dialog)? {
            return Ok(SubmitResult {
                submitted: false,
                dialog: self.dialog.clone(),
                message: None,
                status_text: Some(self.status.clone()),
            });
        }
        self.status = "審査待ち".to_string();
        self.log.push("submit-confirm".to_string());
        Ok(SubmitResult {
            submitted: true,
            dialog: self.dialog.clone(),
            message: Some("アイテムを送信しました。".to_string()),
            status_text: Some(self.status.clone()),
        })
    }

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        self.log.push(format!("capture {label}"));
        None
    }
}

fn values(pairs: &[(ListingField, &str)]) -> FieldValues {
    pairs.iter().map(|(f, v)| (*f, v.to_string())).collect()
}

fn expected() -> ExpectedItem {
    ExpectedItem {
        item_id: ITEM.to_string(),
        display_name: "テスト".to_string(),
        locales: vec![
            ExpectedLocale::new("en", values(&[(ListingField::Description, "New EN")])),
            ExpectedLocale::new(
                "ja",
                values(&[
                    (ListingField::Description, "日本語"),
                    (ListingField::SupportUrl, "https://example.com/support"),
                ]),
            ),
        ],
    }
}

fn page() -> FakeListingPage {
    FakeListingPage {
        saved: vec![
            (
                "en".to_string(),
                values(&[(ListingField::Description, "Old EN")]),
            ),
            (
                "ja".to_string(),
                values(&[
                    (ListingField::Description, "日本語"),
                    (ListingField::SupportUrl, "https://example.com/support"),
                ]),
            ),
        ],
        editing: Vec::new(),
        current_locale: None,
        status: "公開済み".to_string(),
        log: Vec::new(),
        corrupt_writes: false,
        dialog: SubmitDialogInfo {
            text: "「テスト」を審査のために送信しますか？".to_string(),
            auto_publish: Some(true),
        },
    }
}

fn no_progress(_: &str) {}

fn wrote_or_saved(page: &FakeListingPage) -> bool {
    page.log
        .iter()
        .any(|l| l.starts_with("write") || l == "save")
}

#[test]
fn diff_読み取りだけで差分を返す() {
    let mut page = page();

    let (_, rows) =
        services::diff(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    assert_eq!(
        rows.iter().find(|r| r.locale == "en").unwrap().status(),
        ComparisonStatus::Different
    );
    assert_eq!(
        rows.iter().find(|r| r.locale == "ja").unwrap().status(),
        ComparisonStatus::Match
    );
    assert!(!wrote_or_saved(&page));
}

#[test]
fn push_差分のあるロケールだけを書き換えて保存し読み直して照合する() {
    let mut page = page();

    let result = services::push(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    let outcome = |code: &str| {
        result
            .locales
            .iter()
            .find(|l| l.locale == code)
            .unwrap()
            .outcome
    };
    assert_eq!(outcome("en"), LocalePushOutcome::Saved);
    assert_eq!(outcome("ja"), LocalePushOutcome::Unchanged);
    assert_eq!(page.saved_value("en", ListingField::Description), "New EN");
    assert!(
        result
            .final_comparison
            .iter()
            .all(|c| c.status() == ComparisonStatus::Match)
    );
    // 書き換えたのは en の説明だけ。保存は 1 回で、保存後に読み込み直している。
    let writes: Vec<&String> = page.log.iter().filter(|l| l.starts_with("write")).collect();
    assert_eq!(writes, vec!["write en Description"]);
    assert_eq!(page.log.iter().filter(|l| *l == "save").count(), 1);
    let pos = |s: &str| page.log.iter().position(|l| l == s).unwrap();
    assert!(pos("reload") > pos("save"));
}

#[test]
fn push_審査待ちなら何も書き換えない() {
    let mut page = page();
    page.status = "審査待ち".to_string();

    let r = services::push(&mut page, &expected(), &no_progress, &CancelToken::new());

    assert!(matches!(r, Err(Error::WriteBlocked(_))));
    assert!(!wrote_or_saved(&page));
}

#[test]
fn push_書き換え後の値が一致しなければ保存しない() {
    let mut page = page();
    page.corrupt_writes = true;

    let r = services::push(&mut page, &expected(), &no_progress, &CancelToken::new());

    assert!(matches!(r, Err(Error::Dashboard(_))));
    assert!(!page.log.iter().any(|l| l == "save"));
    assert_eq!(page.saved_value("en", ListingField::Description), "Old EN");
    assert!(page.log.iter().any(|l| l.starts_with("capture")));
}

#[test]
fn push_ダッシュボードに無いロケールがあれば何も書き換えない() {
    let mut page = page();
    page.saved.retain(|(c, _)| c != "ja");

    let r = services::push(&mut page, &expected(), &no_progress, &CancelToken::new());

    assert!(matches!(r, Err(Error::Dashboard(_))));
    assert!(!page.log.iter().any(|l| l.starts_with("write")));
}

#[test]
fn submit_確認で取りやめたら送信しない() {
    let mut page = page();

    let result = services::submit(
        &mut page,
        ITEM,
        &mut |_| Ok(false),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert!(!result.submitted);
    assert!(!page.log.iter().any(|l| l == "submit-confirm"));
}

#[test]
fn submit_確認後に送信し結果を返す() {
    let mut page = page();
    let mut seen: Option<SubmitDialogInfo> = None;

    let result = services::submit(
        &mut page,
        ITEM,
        &mut |d| {
            seen = Some(d.clone());
            Ok(true)
        },
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert!(result.submitted);
    assert_eq!(result.status_text.as_deref(), Some("審査待ち"));
    assert_eq!(seen.and_then(|d| d.auto_publish), Some(true));
}

#[test]
fn submit_審査待ちのアイテムは送信を始めない() {
    let mut page = page();
    page.status = "審査待ち".to_string();

    let r = services::submit(
        &mut page,
        ITEM,
        &mut |_| Ok(true),
        &no_progress,
        &CancelToken::new(),
    );

    assert!(matches!(r, Err(Error::WriteBlocked(_))));
    assert!(!page.log.iter().any(|l| l == "submit-dialog"));
}

// ───────── Rust 版で足したもの ─────────

#[test]
fn push_中断されたら次のロケールへ進まない() {
    let mut page = page();
    let cancel = CancelToken::new();
    cancel.cancel();

    let r = services::push(&mut page, &expected(), &no_progress, &cancel);

    assert!(matches!(r, Err(Error::Cancelled)));
    assert!(!wrote_or_saved(&page));
}
