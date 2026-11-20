//! C# の ListingService.test.cs の移植。画面の代わりにメモリ上の値で動く ListingPage で、手順（順番・中止判断）だけを確かめる。

use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::dashboard::write_gate::ItemPageState;
use edictora::dashboard::{EditorPage, ListingPage, SubmitConfirm, SubmitDialogInfo, SubmitResult};
use edictora::listing::*;
use edictora::services::{self, LocalePushOutcome};
use edictora::util::eq_ic;

mod common;

/// 架空のアイテム ID。他のテストと同じ値を使うよう、共通補助の定数を別名で使う
use common::ITEM_A as ITEM;

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
    /// 編集画面を開くときに返す Dashboard エラー（アイテム単位の失敗の再現用）
    open_error: Option<String>,
    /// このロケールへ切り替えるときに返す Dashboard エラー（ロケール単位の失敗の再現用）
    switch_error: Option<(String, String)>,
    /// capture_diagnostics が返す保存先（None なら保存できなかった扱い）
    shot: Option<String>,
    /// カテゴリ（全言語共通）の保存済みの `data-value` と、編集中（未保存）の値
    saved_category: String,
    editing_category: String,
    /// カテゴリの読み出しが返す Dashboard エラー
    category_read_error: Option<String>,
    /// カテゴリの書き込み時に別の選択肢にしてしまう（照合失敗の再現用）
    corrupt_category: bool,
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

impl EditorPage for FakeListingPage {
    fn open_item(&mut self, item_id: &str, _: &CancelToken) -> edictora::Result<()> {
        self.log.push(format!("open {item_id}"));
        if let Some(m) = &self.open_error {
            return Err(Error::Dashboard(m.clone()));
        }
        self.editing = self.saved.clone();
        self.editing_category = self.saved_category.clone();
        self.current_locale = self.saved.first().map(|(c, _)| c.clone());
        Ok(())
    }

    fn reload(&mut self, _: &CancelToken) -> edictora::Result<()> {
        self.log.push("reload".to_string());
        self.editing = self.saved.clone();
        self.editing_category = self.saved_category.clone();
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
        if let Some((c, m)) = &self.switch_error
            && eq_ic(c, code)
        {
            return Err(Error::Dashboard(m.clone()));
        }
        self.current_locale = Some(code.to_string());
        Ok(())
    }

    fn save_draft(&mut self, _: &CancelToken) -> edictora::Result<()> {
        self.log.push("save".to_string());
        let code = self.current_locale.clone().unwrap();
        let values = Self::find(&mut self.editing, &code).clone();
        *Self::find(&mut self.saved, &code) = values;
        self.saved_category = self.editing_category.clone();
        Ok(())
    }

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        self.log.push(format!("capture {label}"));
        self.shot.clone()
    }
}

impl ListingPage for FakeListingPage {
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

    fn read_category(&mut self, _: &CancelToken) -> edictora::Result<String> {
        self.log.push("read-category".to_string());
        if let Some(m) = &self.category_read_error {
            return Err(Error::Dashboard(m.clone()));
        }
        Ok(self.editing_category.clone())
    }

    fn write_category(&mut self, data_value: &str, _: &CancelToken) -> edictora::Result<()> {
        self.log.push(format!("write-category {data_value}"));
        self.editing_category = if self.corrupt_category {
            "CATEGORY_GAMES".to_string()
        } else {
            data_value.to_string()
        };
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
}

fn values(pairs: &[(ListingField, &str)]) -> FieldValues {
    pairs.iter().map(|(f, v)| (*f, v.to_string())).collect()
}

fn expected() -> ExpectedItem {
    ExpectedItem {
        item_id: ITEM.to_string(),
        display_name: "テスト".to_string(),
        category: None,
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
        images: None,
        privacy: None,
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
        open_error: None,
        switch_error: None,
        shot: None,
        saved_category: "CATEGORY_TOOLS".to_string(),
        editing_category: String::new(),
        category_read_error: None,
        corrupt_category: false,
    }
}

/// カテゴリ `productivity` を正本に持つアイテム（ロケールは 2 つ）
fn expected_with_category() -> ExpectedItem {
    ExpectedItem {
        category: Some("productivity".to_string()),
        ..expected()
    }
}

fn count(page: &FakeListingPage, prefix: &str) -> usize {
    page.log.iter().filter(|l| l.starts_with(prefix)).count()
}

fn position(page: &FakeListingPage, entry: &str) -> usize {
    page.log
        .iter()
        .position(|l| l.starts_with(entry))
        .unwrap_or_else(|| panic!("{entry} が記録に無い: {:?}", page.log))
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

// ───────── 結果（標準出力）へ入るエラー文の伏せ字 ─────────

/// 架空の publisherId（結果に残ってはいけない値）
const PUBLISHER: &str = "0123456789abcdef-fake-publisher";

fn error_with_publisher_url() -> String {
    format!(
        "画面を開けませんでした: https://chrome.google.com/webstore/devconsole/{PUBLISHER}/{ITEM}/edit"
    )
}

#[test]
fn pull_アイテム単位のエラー文はpublisher_idを伏せる() {
    let mut page = page();
    page.open_error = Some(error_with_publisher_url());

    let snapshot =
        services::pull(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    let error = snapshot.error.expect("アイテム単位のエラー");
    assert!(!error.contains(PUBLISHER), "{error}");
    assert_eq!(
        error,
        format!(
            "画面を開けませんでした: https://chrome.google.com/webstore/devconsole/<publisher>/{ITEM}/edit"
        )
    );
}

#[test]
fn pull_ロケール単位のエラー文はpublisher_idを伏せる() {
    let mut page = page();
    page.switch_error = Some(("ja".to_string(), error_with_publisher_url()));

    let snapshot =
        services::pull(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    assert_eq!(snapshot.error, None);
    let ja = snapshot.find("ja").expect("ja の結果");
    let error = ja.error.clone().expect("ロケール単位のエラー");
    assert!(!error.contains(PUBLISHER), "{error}");
    assert!(error.contains("devconsole/<publisher>/"), "{error}");
    // 他のロケールは続けて読めている
    assert_eq!(snapshot.find("en").and_then(|l| l.error.clone()), None);
}

#[test]
fn pull_ロケール単位の失敗ではスナップショットを保存し結果のエラー文に保存先を含める() {
    let mut page = page();
    page.switch_error = Some((
        "ja".to_string(),
        "言語の選択肢（ja）が他の要素に覆われていて押せません".to_string(),
    ));
    page.shot = Some(r"C:\diag\20261115-103133-pull-x-ja.txt".to_string());

    let snapshot =
        services::pull(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    assert!(
        page.log.contains(&format!("capture pull-{ITEM}-ja")),
        "{:?}",
        page.log
    );
    let error = snapshot
        .find("ja")
        .and_then(|l| l.error.clone())
        .expect("ja のエラー");
    assert_eq!(
        error,
        r"言語の選択肢（ja）が他の要素に覆われていて押せません（失敗時のページのスナップショット: C:\diag\20261115-103133-pull-x-ja.txt）"
    );
    // 成功したロケールでは保存しない
    assert_eq!(
        page.log.iter().filter(|l| l.starts_with("capture")).count(),
        1
    );
}

#[test]
fn pull_アイテム単位の失敗でもスナップショットを保存する() {
    let mut page = page();
    page.open_error = Some("画面を開けませんでした".to_string());

    let snapshot =
        services::pull(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    assert!(
        page.log.contains(&format!("capture pull-{ITEM}")),
        "{:?}",
        page.log
    );
    // 保存できなかったときはエラー文を変えない
    assert_eq!(snapshot.error.as_deref(), Some("画面を開けませんでした"));
}

#[test]
fn pull_成功したときはスナップショットを保存しない() {
    let mut page = page();

    services::pull(&mut page, &expected(), &no_progress, &CancelToken::new()).unwrap();

    assert!(
        !page.log.iter().any(|l| l.starts_with("capture")),
        "{:?}",
        page.log
    );
}

// ───────── カテゴリ（全言語共通の欄）─────────

#[test]
fn push_カテゴリの差分はロケールのループの外で1回だけ書き保存し読み直して照合する() {
    let mut page = page();

    let result = services::push(
        &mut page,
        &expected_with_category(),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert!(result.category_updated);
    assert_eq!(page.saved_category, "CATEGORY_PRODUCTIVITY");
    // 書き込みは正本の data-value で 1 回だけ（ロケールごとに繰り返さない）
    assert_eq!(count(&page, "write-category"), 1);
    assert_eq!(
        page.log
            .iter()
            .filter(|l| *l == "write-category CATEGORY_PRODUCTIVITY")
            .count(),
        1
    );
    // カテゴリの保存・読み直し・照合が、最初のロケールの切替より前に済む
    let first_switch = position(&page, "switch");
    let write = position(&page, "write-category");
    let first_save = position(&page, "save");
    let first_reload = position(&page, "reload");
    assert!(write < first_save && first_save < first_reload && first_reload < first_switch);
    // 読み直した後にカテゴリを照合している
    assert!(
        page.log[first_reload..first_switch]
            .iter()
            .any(|l| l == "read-category")
    );
}

#[test]
fn push_カテゴリが一致していれば書かず保存もしない() {
    let mut page = page();
    page.saved_category = "CATEGORY_PRODUCTIVITY".to_string();
    // ロケールも一致させ、カテゴリだけが対象のときの無駄な保存が無いことを見る
    let item = ExpectedItem {
        locales: Vec::new(),
        ..expected_with_category()
    };

    let result = services::push(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();

    assert!(!result.category_updated);
    assert!(!wrote_or_saved(&page));
    assert_eq!(count(&page, "write-category"), 0);
}

#[test]
fn push_カテゴリだけのアイテムでも書いて保存する() {
    let mut page = page();
    let item = ExpectedItem {
        locales: Vec::new(),
        ..expected_with_category()
    };

    let result = services::push(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();

    assert!(result.category_updated);
    assert!(result.locales.is_empty());
    assert_eq!(page.saved_category, "CATEGORY_PRODUCTIVITY");
    assert_eq!(count(&page, "switch"), 0);
}

#[test]
fn push_カテゴリの書き込み後の値が違えば保存しない() {
    let mut page = page();
    page.corrupt_category = true;

    let err = services::push(
        &mut page,
        &expected_with_category(),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(err, Error::Dashboard(_)));
    assert_eq!(count(&page, "save"), 0);
    // ロケールには進まない
    assert_eq!(count(&page, "write en"), 0);
}

#[test]
fn push_ダッシュボードに無いロケールがあればカテゴリも書かない() {
    let mut page = page();
    let mut item = expected_with_category();
    item.locales.push(ExpectedLocale::new(
        "fr",
        values(&[(ListingField::Description, "x")]),
    ));

    let err = services::push(&mut page, &item, &no_progress, &CancelToken::new()).unwrap_err();

    assert!(matches!(err, Error::Dashboard(_)));
    assert_eq!(count(&page, "write-category"), 0);
    assert!(!wrote_or_saved(&page));
}

#[test]
fn push_審査待ちならカテゴリも書かない() {
    let mut page = page();
    page.status = "審査待ち".to_string();

    let err = services::push(
        &mut page,
        &expected_with_category(),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(err, Error::WriteBlocked(_)));
    assert_eq!(count(&page, "write-category"), 0);
}

#[test]
fn diff_カテゴリの差分行を返し読み取りだけで済む() {
    let mut page = page();
    let item = expected_with_category();

    let (snapshot, _) =
        services::diff(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();
    let row = category::compare(&item, Some(&snapshot)).expect("カテゴリの行");

    assert_eq!(row.status, ComparisonStatus::Different);
    assert_eq!(row.current.as_deref(), Some("tools"));
    assert_eq!(row.expected, "productivity");
    assert!(!wrote_or_saved(&page));
    assert_eq!(count(&page, "read-category"), 1);
}

#[test]
fn diff_カテゴリが一致なら一致の行() {
    let mut page = page();
    page.saved_category = "CATEGORY_PRODUCTIVITY".to_string();
    let item = expected_with_category();

    let (snapshot, _) =
        services::diff(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();

    let row = category::compare(&item, Some(&snapshot)).unwrap();
    assert_eq!(row.status, ComparisonStatus::Match);
}

#[test]
fn diff_カテゴリの読み出し失敗はこの欄だけのエラーでロケールは読み続ける() {
    let mut page = page();
    page.category_read_error = Some("選択肢が見つかりません".to_string());
    let item = expected_with_category();

    let (snapshot, rows) =
        services::diff(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();

    let row = category::compare(&item, Some(&snapshot)).unwrap();
    assert_eq!(row.status, ComparisonStatus::Error);
    assert!(row.error.unwrap().contains("選択肢が見つかりません"));
    // ロケールは読めている
    assert_eq!(
        rows.iter().find(|r| r.locale == "ja").unwrap().status(),
        ComparisonStatus::Match
    );
}

#[test]
fn カテゴリを書いていなければ読まず比較行も無い() {
    let mut page = page();
    let item = expected();

    let (snapshot, _) =
        services::diff(&mut page, &item, &no_progress, &CancelToken::new()).unwrap();

    assert_eq!(count(&page, "read-category"), 0);
    assert!(snapshot.category.is_none());
    assert!(category::compare(&item, Some(&snapshot)).is_none());
}

#[test]
fn カテゴリの対応表は21件で往復でき_プレースホルダーは正本にできない() {
    assert_eq!(category::CATEGORIES.len(), 21);
    for (key, value) in category::CATEGORIES {
        assert_eq!(category::data_value_of(key), Some(value));
        assert_eq!(category::key_of(value), Some(key));
        assert!(!key.contains(char::is_uppercase));
        assert!(value.starts_with("CATEGORY_"));
    }
    assert_eq!(
        category::data_value_of("art_and_design"),
        Some("CATEGORY_EXTENSIONS_ART_AND_DESIGN")
    );
    assert_eq!(
        category::data_value_of("entertainment"),
        Some("CATEGORY_EXTENSIONS_ENTERTAINMENT")
    );
    assert_eq!(category::data_value_of(category::PLACEHOLDER_VALUE), None);
    assert_eq!(category::data_value_of("Productivity"), None);
    assert_eq!(category::key_of(category::PLACEHOLDER_VALUE), None);
    // 未知の data-value はそのまま表示する
    assert_eq!(
        category::describe_value("CATEGORY_NEW_ONE"),
        "CATEGORY_NEW_ONE"
    );
}
