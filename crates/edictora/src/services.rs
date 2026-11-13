//! 掲載情報の読み出し・比較・書き換え・審査送信の手順（C# の `ListingService`）。
//!
//! 画面操作そのものは [`ListingPage`] に任せ、ここでは「何をどの順でやり、どこで止めるか」だけを決める。
//! 安全策:
//! - 書き込みの前に `write_gate` で状態を確かめ、確認できない状態なら止める（審査の取り消しはしない）
//! - ロケールごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行う（複数言語をまとめて保存する手順は未検証のため）
//! - 途中で 1 つでも失敗したら、そのアイテムの残りは行わない（未保存の変更を次の保存に巻き込まないため）
//! - 最後にもう一度読み込み直して、全対象欄が正本と一致するか確かめる

use crate::cancel::CancelToken;
use crate::dashboard::write_gate::ensure_allowed;
use crate::dashboard::{ListingPage, SubmitConfirm, SubmitResult};
use crate::error::{Error, Result};
use crate::listing::{
    ComparisonStatus, ExpectedItem, ExpectedLocale, FieldValues, ItemSnapshot, ListingField,
    LocaleComparison, LocaleSnapshot, compare, compare_value,
};
use crate::logging::{LogLevel, log, redact};
use crate::util::eq_ic;

/// 進捗の出力先
pub type Progress<'a> = dyn Fn(&str) + 'a;

/// ロケール 1 件の書き換え結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalePushOutcome {
    /// 差分がなく、何もしなかった
    Unchanged,
    /// 書き換えて下書き保存し、読み直して一致を確認した
    Saved,
}

/// ロケール 1 件の書き換え結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalePushResult {
    pub locale: String,
    pub outcome: LocalePushOutcome,
    pub updated_fields: Vec<ListingField>,
}

/// アイテム 1 件の書き換え結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushResult {
    pub item_id: String,
    pub item_name: String,
    /// 完了したロケール
    pub locales: Vec<LocalePushResult>,
    /// 最後に読み直して照合した結果
    pub final_comparison: Vec<LocaleComparison>,
}

/// 現在値を読み出す（読み取りのみ）。ロケール単位の失敗はそのロケールのエラーとして返し、他は続ける
pub fn pull(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ItemSnapshot> {
    progress(&format!(
        "{}: 編集画面を開いています…",
        expected.display_name
    ));
    match pull_core(page, expected, progress, cancel) {
        Ok(s) => Ok(s),
        Err(Error::LoginRequired) => Err(Error::LoginRequired),
        Err(e) if e.is_dashboard() => {
            log(
                &format!("読み出し失敗 {}: {}", expected.item_id, e),
                LogLevel::Warning,
            );
            Ok(ItemSnapshot {
                item_id: expected.item_id.clone(),
                status_text: None,
                available_locales: Vec::new(),
                locales: Vec::new(),
                // 結果は標準出力へ出るので、エラー文中のダッシュボード URL の publisherId を伏せる
                // （ログと同じ redact。欄の値や差分本文には掛けない）
                error: Some(redact(&e.message())),
            })
        }
        Err(e) => Err(e),
    }
}

fn pull_core(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ItemSnapshot> {
    page.open_item(&expected.item_id, cancel)?;
    let state = page.get_state(cancel)?;
    let available = page.get_locales(cancel)?;

    let mut locales = Vec::new();
    for locale in &expected.locales {
        cancel.check()?;
        if !available.iter().any(|c| eq_ic(c, &locale.code)) {
            locales.push(LocaleSnapshot::failed(
                &locale.code,
                format!(
                    "ダッシュボードにロケール {} がありません（ある言語: {}）",
                    locale.code,
                    available.join(", ")
                ),
            ));
            continue;
        }

        progress(&format!(
            "{}: [{}] を読み出しています…",
            expected.display_name, locale.code
        ));
        let read = page
            .switch_locale(&locale.code, cancel)
            .and_then(|_| page.read_fields(&locale.fields(), cancel));
        match read {
            Ok(values) => locales.push(LocaleSnapshot::ok(&locale.code, values)),
            // C# 版と同じく、ロケール単位では DashboardException 系（ログイン切れも含む）をそのロケールのエラーにする
            Err(e) if e.is_dashboard() => {
                log(
                    &format!("読み出し失敗 {} [{}]: {}", expected.item_id, locale.code, e),
                    LogLevel::Warning,
                );
                // 結果（標準出力）へ入るエラー文は publisherId を伏せる
                locales.push(LocaleSnapshot::failed(&locale.code, redact(&e.message())));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(ItemSnapshot {
        item_id: expected.item_id.clone(),
        status_text: state.status_text,
        available_locales: available,
        locales,
        error: None,
    })
}

/// 読み出して比較する（読み取りのみ）
pub fn diff(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<(ItemSnapshot, Vec<LocaleComparison>)> {
    let snapshot = pull(page, expected, progress, cancel)?;
    let rows = compare(expected, Some(&snapshot));
    Ok((snapshot, rows))
}

/// 差分のある欄だけを書き換えて下書き保存し、読み直して照合する。
/// 失敗したらエラー（`WriteBlocked` / `Dashboard` など）。失敗時は画面を診断用に保存する
pub fn push(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<PushResult> {
    match push_core(page, expected, progress, cancel) {
        Ok(r) => Ok(r),
        // 画面を保存する条件: ダッシュボード系の失敗のうちログイン切れ以外。書き込み不可（WriteBlocked）でも保存する。
        // submit は WriteBlocked を除くので条件が違う。この差を保つため、条件は report_failure に入れず呼び出し側に置く。
        Err(e) if e.is_dashboard() && e != Error::LoginRequired => {
            report_failure(page, "push", &expected.item_id, &e, progress);
            Err(e)
        }
        Err(e) => Err(e),
    }
}

/// 失敗時の記録: 画面を診断用に保存し、エラーログを書き、保存できたら進捗にその場所を出す。
///
/// push と submit で同じ形だった処理をまとめたもの。どの失敗で呼ぶか（保存の条件）は
/// 呼び出し側ごとに違うため、ここでは判断せず、呼ばれたら必ず記録する。
/// - 画面のラベル: `<operation>-<item_id>`（診断ファイル名の一部になる）
/// - ログ: `<operation> 失敗 <item_id>: <エラー>（画面: <パス>）`（保存できなければ括弧部分なし）
/// - 進捗: `失敗時の画面を保存しました: <パス>`（保存できたときだけ）
fn report_failure(
    page: &mut dyn ListingPage,
    operation: &str,
    item_id: &str,
    error: &Error,
    progress: &Progress<'_>,
) {
    let shot = page.capture_diagnostics(&format!("{operation}-{item_id}"));
    log(
        &format!(
            "{operation} 失敗 {item_id}: {error}{}",
            shot.as_ref()
                .map(|s| format!("（画面: {s}）"))
                .unwrap_or_default()
        ),
        LogLevel::Error,
    );
    if let Some(s) = shot {
        progress(&format!("失敗時の画面を保存しました: {s}"));
    }
}

fn push_core(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<PushResult> {
    let name = &expected.display_name;
    progress(&format!("{name}: 編集画面を開いています…"));
    page.open_item(&expected.item_id, cancel)?;
    ensure_allowed(&page.get_state(cancel)?)?;

    let available = page.get_locales(cancel)?;
    let missing: Vec<&str> = expected
        .locales
        .iter()
        .filter(|l| !available.iter().any(|c| eq_ic(c, &l.code)))
        .map(|l| l.code.as_str())
        .collect();
    if !missing.is_empty() {
        return Err(Error::Dashboard(format!(
            "ダッシュボードにロケール {} がありません（ある言語: {}）。何も書き換えずに止めます",
            missing.join(", "),
            available.join(", ")
        )));
    }

    let mut results = Vec::new();
    for locale in &expected.locales {
        cancel.check()?;
        let code = &locale.code;
        progress(&format!("{name}: [{code}] の現在値を確認しています…"));
        page.switch_locale(code, cancel)?;
        let fields = locale.fields();
        let current = page.read_fields(&fields, cancel)?;
        let targets = mismatched_fields(locale, &fields, &current);
        if targets.is_empty() {
            progress(&format!("{name}: [{code}] は正本と一致しています"));
            results.push(LocalePushResult {
                locale: code.clone(),
                outcome: LocalePushOutcome::Unchanged,
                updated_fields: Vec::new(),
            });
            continue;
        }

        for field in &targets {
            progress(&format!(
                "{name}: [{code}] {}を書き換えています…",
                field.display_name()
            ));
            page.write_field(*field, &locale.values[field], cancel)?;
        }

        // 保存前に、このロケールの全対象欄が正本どおりか確かめる（書き換えていない欄が動いていないかも含む）
        ensure_matches(
            locale,
            &fields,
            &page.read_fields(&fields, cancel)?,
            "保存前の照合",
        )?;

        // 保存の直前にも状態を確かめる
        ensure_allowed(&page.get_state(cancel)?)?;
        progress(&format!("{name}: [{code}] を下書き保存しています…"));
        page.save_draft(cancel)?;

        // 読み込み直して、保存された値を確かめる
        page.reload(cancel)?;
        page.switch_locale(code, cancel)?;
        ensure_matches(
            locale,
            &fields,
            &page.read_fields(&fields, cancel)?,
            "保存後の照合",
        )?;
        progress(&format!(
            "{name}: [{code}] を保存し、読み直して一致を確認しました"
        ));
        results.push(LocalePushResult {
            locale: code.clone(),
            outcome: LocalePushOutcome::Saved,
            updated_fields: targets,
        });
    }

    // 最後にもう一度読み込み直し、全対象ロケールを照合する（あるロケールの保存が他に影響していないか）
    let final_rows = if results
        .iter()
        .any(|r| r.outcome == LocalePushOutcome::Saved)
    {
        progress(&format!("{name}: 全言語を読み直して照合しています…"));
        page.reload(cancel)?;
        let mut snapshots = Vec::new();
        for locale in &expected.locales {
            page.switch_locale(&locale.code, cancel)?;
            snapshots.push(LocaleSnapshot::ok(
                &locale.code,
                page.read_fields(&locale.fields(), cancel)?,
            ));
        }
        let rows = compare(
            expected,
            Some(&snapshot_of(expected, &available, snapshots)),
        );
        let bad: Vec<&LocaleComparison> = rows
            .iter()
            .filter(|c| c.status() != ComparisonStatus::Match)
            .collect();
        if !bad.is_empty() {
            let detail: Vec<String> = bad
                .iter()
                .map(|b| {
                    let names: Vec<&str> = b
                        .different_fields()
                        .iter()
                        .map(|f| f.display_name())
                        .collect();
                    format!("[{}] {}", b.locale, names.join("・"))
                })
                .collect();
            return Err(Error::Dashboard(format!(
                "最終照合で正本と一致しない欄があります: {}",
                detail.join(", ")
            )));
        }
        rows
    } else {
        let snapshots = expected
            .locales
            .iter()
            .map(|l| LocaleSnapshot::ok(&l.code, l.values.clone()))
            .collect();
        compare(
            expected,
            Some(&snapshot_of(expected, &available, snapshots)),
        )
    };

    Ok(PushResult {
        item_id: expected.item_id.clone(),
        item_name: expected.display_name.clone(),
        locales: results,
        final_comparison: final_rows,
    })
}

fn snapshot_of(
    expected: &ExpectedItem,
    available: &[String],
    locales: Vec<LocaleSnapshot>,
) -> ItemSnapshot {
    ItemSnapshot {
        item_id: expected.item_id.clone(),
        status_text: None,
        available_locales: available.to_vec(),
        locales,
        error: None,
    }
}

/// 審査に送信する（1 アイテムずつ）。送信前に書き込み可否を確かめ、確認ダイアログの内容を `confirm` に渡す
pub fn submit(
    page: &mut dyn ListingPage,
    item_id: &str,
    confirm: &mut SubmitConfirm<'_>,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<SubmitResult> {
    let run =
        |page: &mut dyn ListingPage, confirm: &mut SubmitConfirm<'_>| -> Result<SubmitResult> {
            progress(&format!("{item_id}: 編集画面を開いています…"));
            page.open_item(item_id, cancel)?;
            ensure_allowed(&page.get_state(cancel)?)?;
            progress(&format!(
                "{item_id}: 審査送信の確認ダイアログを開いています…"
            ));
            page.submit_for_review(confirm, cancel)
        };
    match run(page, confirm) {
        Ok(r) => Ok(r),
        // 画面を保存する条件: ダッシュボード系の失敗のうち、ログイン切れと書き込み不可（WriteBlocked）以外。
        // push（WriteBlocked でも保存する）と条件が違うので、条件は report_failure に入れず呼び出し側に置く。
        Err(e)
            if e.is_dashboard() && !matches!(e, Error::LoginRequired | Error::WriteBlocked(_)) =>
        {
            report_failure(page, "submit", item_id, &e, progress);
            Err(e)
        }
        Err(e) => Err(e),
    }
}

/// 対象欄のうち、読み出した値が正本と一致しない欄を `fields` の並びのまま返す。
///
/// push の「書き換える欄を決める」と「保存前後の照合」の両方で同じ判定を使うためにまとめている。
/// 一致の判定は比較（`compare_value`）と同じ規則で、読み出せなかった欄（`values` に無い欄）は不一致として扱う。
/// `fields` は呼び出し側が `locale.fields()` で一度だけ作ったものを渡す（同じ一覧を何度も作り直さない）。
fn mismatched_fields(
    locale: &ExpectedLocale,
    fields: &[ListingField],
    values: &FieldValues,
) -> Vec<ListingField> {
    fields
        .iter()
        .copied()
        .filter(|f| {
            compare_value(&locale.values[f], values.get(f).map(String::as_str))
                != ComparisonStatus::Match
        })
        .collect()
}

/// 対象欄がすべて正本と一致するか確かめる。一致しない欄があれば、その欄名を並べた `Dashboard` エラー
fn ensure_matches(
    locale: &ExpectedLocale,
    fields: &[ListingField],
    values: &FieldValues,
    phase: &str,
) -> Result<()> {
    let bad: Vec<&str> = mismatched_fields(locale, fields, values)
        .into_iter()
        .map(|f| f.display_name())
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(Error::Dashboard(format!(
            "[{}] {phase}で正本と一致しない欄があります: {}",
            locale.code,
            bad.join("・")
        )))
    }
}
