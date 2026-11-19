//! 「ストアの掲載情報」タブのカテゴリ（全言語共通の欄）の対応表と比較。
//!
//! 設定の `category` は言語に依らない小文字のキー（例 `productivity`・`developer_tools`）で書く。
//! ダッシュボードの一覧（`ul[role=listbox][aria-label="カテゴリ"]`）の各選択肢は `data-value` 属性に
//! `CATEGORY_PRODUCTIVITY` のような識別子を持つので、キーはそこから `CATEGORY_` と `EXTENSIONS_` を除いて
//! 小文字にしたもの。表示名（「仕事効率化」など）は画面の言語で変わるため、正本には使わない。
//! キーと `data-value` の対応は、この表の 1 か所だけに持つ（2026-11-19 に SearchClock の日本語画面で読み取った）。

use crate::listing::{ComparisonStatus, ExpectedItem, ItemSnapshot};
use crate::{tr, trs};

/// キーと `data-value` の対応（画面の並び順）
pub const CATEGORIES: [(&str, &str); 21] = [
    ("productivity", "CATEGORY_PRODUCTIVITY"),
    ("communication", "CATEGORY_COMMUNICATION"),
    ("tools", "CATEGORY_TOOLS"),
    ("developer_tools", "CATEGORY_DEVELOPER_TOOLS"),
    ("workflow_and_planning", "CATEGORY_WORKFLOW_AND_PLANNING"),
    ("education", "CATEGORY_EDUCATION"),
    ("lifestyle", "CATEGORY_LIFESTYLE"),
    ("art_and_design", "CATEGORY_EXTENSIONS_ART_AND_DESIGN"),
    ("entertainment", "CATEGORY_EXTENSIONS_ENTERTAINMENT"),
    ("games", "CATEGORY_GAMES"),
    ("shopping", "CATEGORY_SHOPPING"),
    ("social_networking", "CATEGORY_SOCIAL_NETWORKING"),
    ("news_and_weather", "CATEGORY_NEWS_AND_WEATHER"),
    ("well_being", "CATEGORY_WELL_BEING"),
    ("household", "CATEGORY_HOUSEHOLD"),
    ("travel", "CATEGORY_TRAVEL"),
    ("just_for_fun", "CATEGORY_JUST_FOR_FUN"),
    ("make_chrome_yours", "CATEGORY_MAKE_CHROME_YOURS"),
    ("privacy_and_security", "CATEGORY_PRIVACY_AND_SECURITY"),
    ("accessibility", "CATEGORY_ACCESSIBILITY"),
    ("functionality_and_ui", "CATEGORY_FUNCTIONALITY_AND_UI"),
];

/// 未選択を表すプレースホルダーの選択肢（「カテゴリを選択してください」）の `data-value`。
/// 正本にはできない（選ぶ操作もしない）。現在値として読めたときだけ「未選択」と表示する
pub const PLACEHOLDER_VALUE: &str = "Select a category";

/// キーに対応する `data-value`（未知のキーは None。キーは完全一致）
pub fn data_value_of(key: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// `data-value` に対応するキー（表に無い値は None）
pub fn key_of(data_value: &str) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(_, v)| *v == data_value)
        .map(|(k, _)| *k)
}

/// 設定に書けるキーの一覧（エラー文用）
pub fn config_keys() -> String {
    CATEGORIES
        .iter()
        .map(|(k, _)| *k)
        .collect::<Vec<_>>()
        .join(", ")
}

/// 現在の選択（`data-value`）の表示。キー、未選択、表に無い値はその値のまま
pub fn describe_value(data_value: &str) -> String {
    if let Some(key) = key_of(data_value) {
        key.to_string()
    } else if data_value == PLACEHOLDER_VALUE {
        trs!("listing.category_unselected").to_string()
    } else {
        data_value.to_string()
    }
}

/// カテゴリの比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryComparison {
    /// 正本のキー
    pub expected: String,
    /// 現在の選択の表示（読めなければ None）
    pub current: Option<String>,
    pub status: ComparisonStatus,
    /// 読み出しに失敗した理由
    pub error: Option<String>,
}

/// 正本のカテゴリと現在値を比べる。正本にカテゴリが無ければ None。
/// `snapshot` が None または読み出しが無ければ「未取得」、読み出し失敗は「エラー」
pub fn compare(
    expected: &ExpectedItem,
    snapshot: Option<&ItemSnapshot>,
) -> Option<CategoryComparison> {
    let key = expected.category.as_deref()?;
    let want = data_value_of(key);
    let row = |current: Option<String>, status, error: Option<String>| CategoryComparison {
        expected: key.to_string(),
        current,
        status,
        error,
    };
    let Some(snapshot) = snapshot else {
        return Some(row(None, ComparisonStatus::NotFetched, None));
    };
    if let Some(err) = &snapshot.error {
        return Some(row(None, ComparisonStatus::NotFetched, Some(err.clone())));
    }
    Some(match &snapshot.category {
        None => row(None, ComparisonStatus::NotFetched, None),
        Some(Err(message)) => row(None, ComparisonStatus::Error, Some(message.clone())),
        Some(Ok(value)) => {
            // 表に無い正本は設定の検証で弾かれているので来ない（来たら一致扱いにせず差分にする）
            let status = if want == Some(value.as_str()) {
                ComparisonStatus::Match
            } else {
                ComparisonStatus::Different
            };
            row(Some(describe_value(value)), status, None)
        }
    })
}

/// 比較結果 1 行の説明（表示言語に合わせる。末尾に改行）
pub fn describe(row: &CategoryComparison) -> String {
    match row.status {
        ComparisonStatus::Match => format!(
            "{}\n",
            tr!("listing.category_row_match", value = row.expected)
        ),
        ComparisonStatus::NotFetched => format!(
            "{}\n",
            match &row.error {
                Some(e) => tr!("listing.category_row_error", error = e),
                None => trs!("listing.category_row_not_fetched").to_string(),
            }
        ),
        ComparisonStatus::Error => format!(
            "{}\n",
            tr!(
                "listing.category_row_error",
                error = row.error.as_deref().unwrap_or("")
            )
        ),
        ComparisonStatus::Different => format!(
            "{}\n- {}\n+ {}\n",
            trs!("listing.category_row_different"),
            row.current.as_deref().unwrap_or(""),
            row.expected
        ),
    }
}
