//! 掲載情報の読み出し・比較・書き換え・審査送信の手順（C# の `ListingService`）。
//!
//! 画面操作そのものは [`ListingPage`] に任せ、ここでは「何をどの順でやり、どこで止めるか」だけを決める。
//! 安全策:
//! - 書き込みの前に `write_gate` で状態を確かめ、確認できない状態なら止める（審査の取り消しはしない）
//! - ロケールごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行う（複数言語をまとめて保存する手順は未検証のため）
//! - 途中で 1 つでも失敗したら、そのアイテムの残りは行わない（未保存の変更を次の保存に巻き込まないため）
//! - 最後にもう一度読み込み直して、全対象欄が正本と一致するか確かめる
//!
//! 掲載画像（`pull_images` / `diff_images` / `push_images`）は別の手順で、画面操作は [`ImagePage`] だけを受け取る。
//! `ImagePage` に審査送信は無いので、画像の手順は型の上で審査送信を呼べない。

use crate::cancel::CancelToken;
use crate::dashboard::write_gate::ensure_allowed;
use crate::dashboard::{EditorPage, ImagePage, ListingPage, SubmitConfirm, SubmitResult};
use crate::error::{Error, Result};
use crate::images::fetch::ImageFetcher;
use crate::images::{
    CurrentImage, ExpectedImages, ExpectedSlot, ImageComparison, ImageSlot, compare_slot,
};
use crate::listing::{
    ComparisonStatus, ExpectedItem, ExpectedLocale, FieldValues, ItemSnapshot, ListingField,
    LocaleComparison, LocaleSnapshot, compare, compare_value,
};
use crate::logging::{LogLevel, log, redact};
use crate::util::eq_ic;
use crate::{tr, trs};

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
    progress(&tr!(
        "services.opening_the_edit_page",
        display_name = expected.display_name
    ));
    match pull_core(page, expected, progress, cancel) {
        Ok(s) => Ok(s),
        Err(Error::LoginRequired) => Err(Error::LoginRequired),
        Err(e) if e.is_dashboard() => {
            let shot = capture_read_failure(page, &expected.item_id, None, progress);
            log(
                &format!(
                    "読み出し失敗 {}: {}{}",
                    expected.item_id,
                    e,
                    log_shot_suffix(&shot)
                ),
                LogLevel::Warning,
            );
            Ok(ItemSnapshot {
                item_id: expected.item_id.clone(),
                status_text: None,
                available_locales: Vec::new(),
                locales: Vec::new(),
                // 結果は標準出力へ出るので、エラー文中のダッシュボード URL の publisherId を伏せる
                // （ログと同じ redact。欄の値や差分本文には掛けない）
                error: Some(redact(&format!("{}{}", e.message(), shot_suffix(&shot)))),
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
                tr!(
                    "listing.locale_is_not_on_the_dashboard_available",
                    locale = locale.code,
                    available_locales = available.join(", ")
                ),
            ));
            continue;
        }

        progress(&tr!(
            "services.reading",
            display_name = expected.display_name,
            locale = locale.code
        ));
        let read = page
            .switch_locale(&locale.code, cancel)
            .and_then(|_| page.read_fields(&locale.fields(), cancel));
        match read {
            Ok(values) => locales.push(LocaleSnapshot::ok(&locale.code, values)),
            // C# 版と同じく、ロケール単位では DashboardException 系（ログイン切れも含む）をそのロケールのエラーにする
            Err(e) if e.is_dashboard() => {
                // ログイン切れは画面を見ても原因が分からないので保存しない（push / submit と同じ）
                let shot = if e == Error::LoginRequired {
                    None
                } else {
                    capture_read_failure(page, &expected.item_id, Some(&locale.code), progress)
                };
                log(
                    &format!(
                        "読み出し失敗 {} [{}]: {}{}",
                        expected.item_id,
                        locale.code,
                        e,
                        log_shot_suffix(&shot)
                    ),
                    LogLevel::Warning,
                );
                // 結果（標準出力）へ入るエラー文は publisherId を伏せる
                locales.push(LocaleSnapshot::failed(
                    &locale.code,
                    redact(&format!("{}{}", e.message(), shot_suffix(&shot))),
                ));
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

/// 読み出しに失敗したときの画面を診断用に保存し、保存先を返す（保存できなければ None）。
///
/// pull / diff は読み取りだけで続行するため、以前は画面を残していなかった。そのため原因を調べる側
/// （AI エージェントを含む）が `--show-browser` で表示して確かめようとし、利用者の画面にウィンドウが出ていた。
/// 保存先はエラー文（標準出力の結果）にも含める。標準エラーを捨てて実行されても場所が分かるように。
/// - 画面のラベル: `pull-<item_id>` または `pull-<item_id>-<locale>`
fn capture_read_failure(
    page: &mut dyn EditorPage,
    item_id: &str,
    locale: Option<&str>,
    progress: &Progress<'_>,
) -> Option<String> {
    let label = match locale {
        Some(code) => format!("pull-{item_id}-{code}"),
        None => format!("pull-{item_id}"),
    };
    let shot = page.capture_diagnostics(&label);
    if let Some(s) = &shot {
        progress(&tr!("services.saved_a_screenshot_of_the_failure", path = s));
    }
    shot
}

/// エラー文の後ろに付ける「（失敗時の画面: <パス>）」（保存できなかったときは空）。
/// 結果（標準出力）に入るので表示言語に合わせる。ログには [`log_shot_suffix`] を使う
fn shot_suffix(shot: &Option<String>) -> String {
    shot.as_ref()
        .map(|s| tr!("services.failure_screenshot", path = s))
        .unwrap_or_default()
}

/// ログ用の「（失敗時の画面: <パス>）」。ログは作者の調査用なので表示言語によらず日本語
fn log_shot_suffix(shot: &Option<String>) -> String {
    shot.as_ref()
        .map(|s| format!("（失敗時の画面: {s}）"))
        .unwrap_or_default()
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
    page: &mut dyn EditorPage,
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
        progress(&tr!("services.saved_a_screenshot_of_the_failure", path = s));
    }
}

fn push_core(
    page: &mut dyn ListingPage,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<PushResult> {
    let name = &expected.display_name;
    progress(&tr!("services.opening_the_edit_page_2", name = name));
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
        return Err(Error::Dashboard(tr!(
            "services.locale_is_not_on_the_dashboard_available",
            missing = missing.join(", "),
            available = available.join(", ")
        )));
    }

    let mut results = Vec::new();
    for locale in &expected.locales {
        cancel.check()?;
        let code = &locale.code;
        progress(&tr!(
            "services.checking_the_current_values_of",
            name = name,
            code = code
        ));
        page.switch_locale(code, cancel)?;
        let fields = locale.fields();
        let current = page.read_fields(&fields, cancel)?;
        let targets = mismatched_fields(locale, &fields, &current);
        if targets.is_empty() {
            progress(&tr!(
                "services.matches_the_source_files",
                name = name,
                code = code
            ));
            results.push(LocalePushResult {
                locale: code.clone(),
                outcome: LocalePushOutcome::Unchanged,
                updated_fields: Vec::new(),
            });
            continue;
        }

        for field in &targets {
            progress(&tr!(
                "services.updating",
                display_name = field.display_name(),
                name = name,
                code = code
            ));
            page.write_field(*field, &locale.values[field], cancel)?;
        }

        // 保存前に、このロケールの全対象欄が正本どおりか確かめる（書き換えていない欄が動いていないかも含む）
        ensure_matches(
            locale,
            &fields,
            &page.read_fields(&fields, cancel)?,
            trs!("services.pre_save_check"),
        )?;

        // 保存の直前にも状態を確かめる
        ensure_allowed(&page.get_state(cancel)?)?;
        progress(&tr!("services.saving_as_draft", name = name, code = code));
        page.save_draft(cancel)?;

        // 読み込み直して、保存された値を確かめる
        page.reload(cancel)?;
        page.switch_locale(code, cancel)?;
        ensure_matches(
            locale,
            &fields,
            &page.read_fields(&fields, cancel)?,
            trs!("services.post_save_check"),
        )?;
        progress(&tr!(
            "services.saved_reloaded_and_confirmed_it_matches",
            name = name,
            code = code
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
        progress(&tr!(
            "services.reloading_and_checking_all_languages",
            name = name
        ));
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
                    format!("[{}] {}", b.locale, names.join(trs!("cli.name_separator")))
                })
                .collect();
            return Err(Error::Dashboard(tr!(
                "services.some_fields_do_not_match_the_source",
                text = detail.join(trs!("services.list_separator"))
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
            progress(&tr!("services.opening_the_edit_page_3", item_id = item_id));
            page.open_item(item_id, cancel)?;
            ensure_allowed(&page.get_state(cancel)?)?;
            progress(&tr!(
                "services.opening_the_submit_for_review_confirmation_dialog",
                item_id = item_id
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

/// 対象欄がすべて正本と一致するか確かめる。一致しない欄があれば、その欄名を並べた `Dashboard` エラー。
/// `phase` は表示言語に合わせた照合の名前（呼び出し側で `trs!` で渡す）
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
        Err(Error::Dashboard(tr!(
            "services.some_fields_do_not_match_the_source_2",
            locale = locale.code,
            text = bad.join(trs!("cli.name_separator")),
            phase = phase
        )))
    }
}

// ───────── 掲載画像 ─────────

/// ダッシュボードから読んだスロット 1 つの現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotReading {
    pub slot: ImageSlot,
    /// ローカライズ版のときのロケール
    pub locale: Option<String>,
    /// 画像（原本のハッシュつき）。読めなかったら理由
    pub images: std::result::Result<Vec<CurrentImage>, String>,
}

/// アイテム 1 件の画像の現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSnapshot {
    pub item_id: String,
    /// ステータス表示（読めなければ None）
    pub status_text: Option<String>,
    pub available_locales: Vec<String>,
    pub slots: Vec<SlotReading>,
    /// アイテム全体の読み出し失敗の理由（成功なら None）
    pub error: Option<String>,
}

/// 画像の書き換え結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePushResult {
    pub item_id: String,
    pub item_name: String,
    /// 置き換えて下書き保存し、読み直して一致を確認したスロット（表示名）
    pub saved: Vec<String>,
    /// 差分がなく、何もしなかったスロット（表示名）
    pub unchanged: Vec<String>,
    /// 最後に読み直して照合した結果
    pub final_comparison: Vec<ImageComparison>,
}

/// 画像の現在値を読む（読み取りのみ）。失敗はアイテム単位のエラーとして返し、他のアイテムは続ける
pub fn pull_images(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ImageSnapshot> {
    progress(&tr!(
        "services.opening_the_edit_page",
        display_name = expected.display_name
    ));
    match pull_images_core(page, fetcher, expected, progress, cancel) {
        Ok(s) => Ok(s),
        Err(Error::LoginRequired) => Err(Error::LoginRequired),
        Err(e) if e.is_dashboard() => {
            let shot = capture_read_failure(page, &expected.item_id, None, progress);
            log(
                &format!(
                    "画像の読み出し失敗 {}: {}{}",
                    expected.item_id,
                    e,
                    log_shot_suffix(&shot)
                ),
                LogLevel::Warning,
            );
            Ok(ImageSnapshot {
                item_id: expected.item_id.clone(),
                status_text: None,
                available_locales: Vec::new(),
                slots: Vec::new(),
                error: Some(redact(&format!("{}{}", e.message(), shot_suffix(&shot)))),
            })
        }
        Err(e) => Err(e),
    }
}

/// 読み出して正本と比べる（読み取りのみ）
pub fn diff_images(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<(ImageSnapshot, Vec<ImageComparison>)> {
    let snapshot = pull_images(page, fetcher, expected, progress, cancel)?;
    let rows = compare_images(expected, &snapshot);
    Ok((snapshot, rows))
}

/// 画像の現在値と正本を比べる（正本の並び順）。読めなかったスロットは `Error`（比較不能）
pub fn compare_images(expected: &ExpectedItem, snapshot: &ImageSnapshot) -> Vec<ImageComparison> {
    let Some(images) = &expected.images else {
        return Vec::new();
    };
    images
        .slots
        .iter()
        .map(|slot| {
            let reading = snapshot
                .slots
                .iter()
                .find(|r| r.slot == slot.slot && locale_eq(&r.locale, &slot.locale));
            let mut row = match reading.map(|r| &r.images) {
                Some(Ok(current)) => {
                    compare_slot(&expected.item_id, &expected.display_name, slot, current)
                }
                other => {
                    let mut row =
                        compare_slot(&expected.item_id, &expected.display_name, slot, &[]);
                    row.status = ComparisonStatus::Error;
                    row.error = Some(match other {
                        Some(Err(m)) => m.clone(),
                        _ => snapshot
                            .error
                            .clone()
                            .unwrap_or_else(|| trs!("services.images_not_read").to_string()),
                    });
                    row
                }
            };
            // アイテム全体の読み出し失敗は、どのスロットも同じ理由で比較不能
            if let Some(e) = &snapshot.error {
                row.status = ComparisonStatus::Error;
                row.error = Some(e.clone());
            }
            row
        })
        .collect()
}

fn locale_eq(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => eq_ic(x, y),
        _ => false,
    }
}

fn pull_images_core(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ImageSnapshot> {
    let images = expected.images.clone().unwrap_or_default();
    page.open_item(&expected.item_id, cancel)?;
    let state = page.get_state(cancel)?;
    let available = page.get_locales(cancel)?;
    ensure_image_targets(&images, &available)?;

    let mut slots = Vec::new();
    for slot in images.global_slots() {
        slots.push(read_slot(
            page,
            fetcher,
            slot,
            &expected.display_name,
            progress,
            cancel,
        )?);
    }
    for code in images.localized_locales() {
        cancel.check()?;
        page.switch_locale(code, cancel)?;
        for slot in images
            .slots
            .iter()
            .filter(|s| s.locale.as_deref().is_some_and(|l| eq_ic(l, code)))
        {
            slots.push(read_slot(
                page,
                fetcher,
                slot,
                &expected.display_name,
                progress,
                cancel,
            )?);
        }
    }
    Ok(ImageSnapshot {
        item_id: expected.item_id.clone(),
        status_text: state.status_text,
        available_locales: available,
        slots,
        error: None,
    })
}

/// 画像の対象にするロケールがダッシュボードにあるか確かめる。
/// 無ければ（言語が 1 つだけのアイテムを含む）全体を止め、`--image-scope global` か `--locale` を案内する
/// （一部のロケールだけ黙って飛ばすと、書き換え漏れに気づけないため。テキストの `push` と同じ方針）
fn ensure_image_targets(images: &ExpectedImages, available: &[String]) -> Result<()> {
    let wanted = images.localized_locales();
    if wanted.is_empty() {
        return Ok(());
    }
    if available.len() <= 1 {
        return Err(Error::Dashboard(tr!(
            "services.this_item_has_only_one_language_so_localized_screenshots",
            available = available.join(", ")
        )));
    }
    let missing: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|c| !available.iter().any(|a| eq_ic(a, c)))
        .collect();
    if !missing.is_empty() {
        return Err(Error::Dashboard(tr!(
            "services.image_locale_is_not_on_the_dashboard",
            missing = missing.join(", "),
            available = available.join(", ")
        )));
    }
    Ok(())
}

/// スロット 1 つを読む（画像ごとに原本を取得してハッシュにする）
fn read_slot(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    slot: &ExpectedSlot,
    name: &str,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<SlotReading> {
    progress(&tr!(
        "services.reading_images",
        name = name,
        label = slot.label()
    ));
    let images = match read_current(page, fetcher, slot.slot, cancel) {
        Ok(v) => Ok(v),
        // 取得できないのは「比較不能」としてそのスロットに記録する（他は続ける）
        Err(Error::Dashboard(m)) => Err(m),
        Err(e) => return Err(e),
    };
    Ok(SlotReading {
        slot: slot.slot,
        locale: slot.locale.clone(),
        images,
    })
}

/// 画面の画像と、その原本のハッシュ。取得に失敗した画像は `sha256` が Err（比較不能）
fn read_current(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    slot: ImageSlot,
    cancel: &CancelToken,
) -> Result<Vec<CurrentImage>> {
    let mut current = Vec::new();
    for image in page.read_images(slot, cancel)? {
        cancel.check()?;
        let pixel_sha256 = match fetcher.pixel_digest_of(&image.src, slot, cancel) {
            Ok(h) => Ok(h),
            Err(Error::Dashboard(m)) => Err(m),
            Err(e) => return Err(e),
        };
        current.push(CurrentImage {
            alt: image.alt,
            pixel_sha256,
        });
    }
    Ok(current)
}

/// 現在の画像が正本と一致するか（比較不能なら `Dashboard` エラー。一致扱いにも常に置換にもしない）
fn slot_matches(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    slot: &ExpectedSlot,
    cancel: &CancelToken,
) -> Result<bool> {
    let current = read_current(page, fetcher, slot.slot, cancel)?;
    let row = compare_slot("", "", slot, &current);
    match row.status {
        ComparisonStatus::Match => Ok(true),
        ComparisonStatus::Different => Ok(false),
        _ => Err(Error::Dashboard(tr!(
            "services.cannot_compare_images",
            label = slot.label(),
            error = row.error.unwrap_or_default()
        ))),
    }
}

/// 差分のある画像を置き換えて下書き保存し、読み直して照合する。
///
/// 順序: テキストの push は呼び出し側が済ませる → 編集画面を開く → 書き込み可否 → 全言語向け（アイコン → 小タイル → マーキー →
/// スクリーンショット。差分のあるスロットだけ全入れ替え）→ 保存 → 読み直して照合 → ロケールごとに
/// （切替 → 置換 → 保存 → 読み直し → 切替 → 照合）→ 最後に読み直して全対象スロットを再照合。
/// どの段の失敗でも保存せず、そのアイテムの残りは行わない。失敗時は画面を診断用に保存する
pub fn push_images(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ImagePushResult> {
    match push_images_core(page, fetcher, expected, progress, cancel) {
        Ok(r) => Ok(r),
        Err(e) if e.is_dashboard() && e != Error::LoginRequired => {
            report_failure(page, "push-images", &expected.item_id, &e, progress);
            Err(e)
        }
        Err(e) => Err(e),
    }
}

fn push_images_core(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    expected: &ExpectedItem,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<ImagePushResult> {
    let name = &expected.display_name;
    let images = expected.images.clone().unwrap_or_default();
    progress(&tr!("services.opening_the_edit_page_2", name = name));
    page.open_item(&expected.item_id, cancel)?;
    // 最初の削除の前に書き込み可否を確かめる（審査待ちなどでは何も触らない）
    ensure_allowed(&page.get_state(cancel)?)?;
    let available = page.get_locales(cancel)?;
    ensure_image_targets(&images, &available)?;

    let mut saved: Vec<String> = Vec::new();
    let mut unchanged: Vec<String> = Vec::new();

    // ── 全言語向け ──
    let global = images.global_slots();
    let mut changed = false;
    for slot in &global {
        cancel.check()?;
        progress(&tr!(
            "services.checking_images",
            name = name,
            label = slot.label()
        ));
        if slot_matches(page, fetcher, slot, cancel)? {
            progress(&tr!(
                "services.images_match_the_source_files",
                name = name,
                label = slot.label()
            ));
            unchanged.push(slot.label());
            continue;
        }
        replace_slot(page, slot, name, progress, cancel)?;
        changed = true;
    }
    if changed {
        ensure_allowed(&page.get_state(cancel)?)?;
        progress(&tr!("services.saving_images_as_draft", name = name));
        page.save_draft(cancel)?;
        page.reload(cancel)?;
        for slot in &global {
            verify_slot(
                page,
                fetcher,
                slot,
                trs!("services.post_save_check"),
                cancel,
            )?;
        }
        for slot in &global {
            if !unchanged.contains(&slot.label()) {
                saved.push(slot.label());
            }
        }
        progress(&tr!(
            "services.images_saved_reloaded_and_confirmed",
            name = name
        ));
    }

    // ── ローカライズ版（ロケールごと）──
    for code in images.localized_locales() {
        cancel.check()?;
        page.switch_locale(code, cancel)?;
        for slot in images
            .slots
            .iter()
            .filter(|s| s.locale.as_deref().is_some_and(|l| eq_ic(l, code)))
        {
            progress(&tr!(
                "services.checking_images",
                name = name,
                label = slot.label()
            ));
            if slot_matches(page, fetcher, slot, cancel)? {
                progress(&tr!(
                    "services.images_match_the_source_files",
                    name = name,
                    label = slot.label()
                ));
                unchanged.push(slot.label());
                continue;
            }
            replace_slot(page, slot, name, progress, cancel)?;
            ensure_allowed(&page.get_state(cancel)?)?;
            progress(&tr!("services.saving_images_as_draft", name = name));
            page.save_draft(cancel)?;
            page.reload(cancel)?;
            page.switch_locale(code, cancel)?;
            verify_slot(
                page,
                fetcher,
                slot,
                trs!("services.post_save_check"),
                cancel,
            )?;
            saved.push(slot.label());
            progress(&tr!(
                "services.images_saved_reloaded_and_confirmed",
                name = name
            ));
        }
    }

    // ── 最後に読み込み直して、全対象スロットを照合する（保存が他のスロットに影響していないか）──
    let mut final_rows = Vec::new();
    if !saved.is_empty() {
        progress(&tr!(
            "services.reloading_and_checking_all_languages",
            name = name
        ));
        page.reload(cancel)?;
    }
    let mut current_locale: Option<&str> = None;
    for slot in &images.slots {
        cancel.check()?;
        if let Some(code) = slot.locale.as_deref()
            && current_locale != Some(code)
        {
            page.switch_locale(code, cancel)?;
            current_locale = Some(code);
        }
        let current = read_current(page, fetcher, slot.slot, cancel)?;
        let row = compare_slot(&expected.item_id, name, slot, &current);
        if !saved.is_empty() && row.status != ComparisonStatus::Match {
            return Err(Error::Dashboard(tr!(
                "services.images_do_not_match_the_source",
                text = crate::images::describe(&row)
            )));
        }
        final_rows.push(row);
    }

    Ok(ImagePushResult {
        item_id: expected.item_id.clone(),
        item_name: expected.display_name.clone(),
        saved,
        unchanged,
        final_comparison: final_rows,
    })
}

/// スロットを正本どおりに置き換える（未保存）。
/// 既存を末尾から 1 枚ずつ削除（毎回「枚数が 1 枚減る」を待つ）→ 正本を先頭から 1 枚ずつ追加
/// （毎回「枚数が 1 枚増える・alt の番号・エラー表示なし」を待つ）→ 保存前に枚数を照合する
fn replace_slot(
    page: &mut dyn ImagePage,
    slot: &ExpectedSlot,
    name: &str,
    progress: &Progress<'_>,
    cancel: &CancelToken,
) -> Result<()> {
    let current = page.read_images(slot.slot, cancel)?;
    progress(&tr!(
        "services.replacing_images",
        name = name,
        label = slot.label(),
        current = current.len(),
        expected = slot.files.len()
    ));
    for position in (0..current.len()).rev() {
        cancel.check()?;
        page.delete_image(slot.slot, position, cancel)?;
    }
    for file in &slot.files {
        cancel.check()?;
        page.upload_image(slot.slot, &file.path, cancel)?;
    }
    let now = page.read_images(slot.slot, cancel)?;
    if now.len() != slot.files.len() {
        return Err(Error::Dashboard(tr!(
            "services.the_image_count_does_not_match_before_saving",
            label = slot.label(),
            now = now.len(),
            expected = slot.files.len()
        )));
    }
    Ok(())
}

/// 保存後の照合（原本のハッシュが正本と順序まで一致すること）
fn verify_slot(
    page: &mut dyn ImagePage,
    fetcher: &dyn ImageFetcher,
    slot: &ExpectedSlot,
    phase: &str,
    cancel: &CancelToken,
) -> Result<()> {
    let current = read_current(page, fetcher, slot.slot, cancel)?;
    let row = compare_slot("", "", slot, &current);
    match row.status {
        ComparisonStatus::Match => Ok(()),
        _ => Err(Error::Dashboard(tr!(
            "services.the_images_do_not_match_the_source",
            text = crate::images::describe(&row),
            phase = phase
        ))),
    }
}
