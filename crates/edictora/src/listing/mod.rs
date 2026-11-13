//! 掲載情報の欄・正本と現在値のモデル・比較（C# の `Edictora.Core.Listing`）。

pub mod text_diff;

use std::collections::BTreeMap;

use crate::config::section::extract_source;
use crate::config::{ItemConfig, normalize_locale_code};
use crate::error::{Error, Result};
use crate::util::{eq_ic, format_n0, utf16_len};

/// Edictora が読み書きする掲載情報の欄。
///
/// ダッシュボードの「ストアの掲載情報」で編集できる欄のうち、テキストで正本を持てるものだけを扱う。
/// カテゴリ・画像・プロモーション動画は対象外。設定ファイルに書かれていない欄は、
/// 読み出し・比較・書き換えのどれにも使わない。並び順（Ord）は処理・表示の順番と同じ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListingField {
    /// 説明（textarea、最大 16,000 文字）
    Description,
    /// 公式 URL
    OfficialUrl,
    /// ホームページ URL
    HomepageUrl,
    /// サポート URL
    SupportUrl,
}

impl ListingField {
    /// 処理・表示の順番（説明 → 公式 → ホームページ → サポート）
    pub const ALL: [ListingField; 4] = [
        ListingField::Description,
        ListingField::OfficialUrl,
        ListingField::HomepageUrl,
        ListingField::SupportUrl,
    ];

    /// 画面・CLI に出す日本語名
    pub fn display_name(self) -> &'static str {
        match self {
            ListingField::Description => "説明",
            ListingField::OfficialUrl => "公式 URL",
            ListingField::HomepageUrl => "ホームページ URL",
            ListingField::SupportUrl => "サポート URL",
        }
    }

    /// 設定ファイル上のキー名
    pub fn config_key(self) -> &'static str {
        match self {
            ListingField::Description => "description",
            ListingField::OfficialUrl => "officialUrl",
            ListingField::HomepageUrl => "homepageUrl",
            ListingField::SupportUrl => "supportUrl",
        }
    }

    /// C# の enum 名（ログ・テストの表記用）
    pub fn name(self) -> &'static str {
        match self {
            ListingField::Description => "Description",
            ListingField::OfficialUrl => "OfficialUrl",
            ListingField::HomepageUrl => "HomepageUrl",
            ListingField::SupportUrl => "SupportUrl",
        }
    }
}

/// 欄 → 値（並びは欄の表示順）
pub type FieldValues = BTreeMap<ListingField, String>;

/// ダッシュボードの説明欄の上限（文字数カウンタ「n/16,000」。UTF-16 単位）
pub const DESCRIPTION_MAX_LENGTH: usize = 16_000;

/// ロケール 1 件の正本の値（ファイルから取り出し済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedLocale {
    /// ロケールコード
    pub code: String,
    /// 対象欄 → 正本の値（設定に書いた欄だけを持つ）
    pub values: FieldValues,
}

impl ExpectedLocale {
    /// 作る
    pub fn new(code: &str, values: FieldValues) -> ExpectedLocale {
        ExpectedLocale {
            code: code.to_string(),
            values,
        }
    }

    /// 対象欄（表示順）
    pub fn fields(&self) -> Vec<ListingField> {
        ListingField::ALL
            .into_iter()
            .filter(|f| self.values.contains_key(f))
            .collect()
    }
}

/// アイテム 1 件の正本
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedItem {
    /// アイテム ID
    pub item_id: String,
    /// 表示名
    pub display_name: String,
    /// ロケールごとの正本
    pub locales: Vec<ExpectedLocale>,
}

impl ExpectedItem {
    /// ロケールで絞り込む（空なら全ロケール）。未設定のロケールを指定したら `Config` エラー
    pub fn filter_locales(&self, codes: &[String]) -> Result<ExpectedItem> {
        if codes.is_empty() {
            return Ok(self.clone());
        }
        let mut selected: Vec<ExpectedLocale> = Vec::new();
        for code in codes {
            let normalized = normalize_locale_code(code);
            let found = self
                .locales
                .iter()
                .find(|l| eq_ic(&l.code, &normalized))
                .ok_or_else(|| {
                    Error::Config(format!(
                        "{} ({}) の設定にロケール {} がありません",
                        self.display_name, self.item_id, code
                    ))
                })?;
            if !selected.contains(found) {
                selected.push(found.clone());
            }
        }
        Ok(ExpectedItem {
            locales: selected,
            ..self.clone()
        })
    }
}

/// 設定のアイテムから正本を作る。説明文はここでファイルから取り出す（失敗は `Config` エラー）
pub fn build_expected(item: &ItemConfig) -> Result<ExpectedItem> {
    let mut locales = Vec::new();
    for locale in &item.locales {
        let mut values = FieldValues::new();
        if let Some(description) = &locale.description {
            let text = extract_source(description)?;
            let len = utf16_len(&text);
            if len > DESCRIPTION_MAX_LENGTH {
                return Err(Error::Config(format!(
                    "{} ({}) [{}] の説明が {} 文字あり、上限 {} 文字を超えています",
                    item.display_name,
                    item.item_id,
                    locale.code,
                    format_n0(len),
                    format_n0(DESCRIPTION_MAX_LENGTH)
                )));
            }
            values.insert(ListingField::Description, text);
        }
        for (field, url) in &locale.urls {
            values.insert(*field, url.clone());
        }
        locales.push(ExpectedLocale::new(&locale.code, values));
    }
    Ok(ExpectedItem {
        item_id: item.item_id.clone(),
        display_name: item.display_name.clone(),
        locales,
    })
}

/// ダッシュボードから読み出したロケール 1 件の現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleSnapshot {
    /// ロケールコード
    pub code: String,
    /// 欄 → 現在値
    pub values: FieldValues,
    /// 読み出しに失敗した場合の理由（成功なら None）
    pub error: Option<String>,
}

impl LocaleSnapshot {
    /// 成功した読み出し
    pub fn ok(code: &str, values: FieldValues) -> LocaleSnapshot {
        LocaleSnapshot {
            code: code.to_string(),
            values,
            error: None,
        }
    }

    /// 失敗した読み出し
    pub fn failed(code: &str, error: impl Into<String>) -> LocaleSnapshot {
        LocaleSnapshot {
            code: code.to_string(),
            values: FieldValues::new(),
            error: Some(error.into()),
        }
    }
}

/// ダッシュボードから読み出したアイテム 1 件の現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSnapshot {
    /// アイテム ID
    pub item_id: String,
    /// ステータス表示（「公開済み」など。読めなければ None）
    pub status_text: Option<String>,
    /// アイテムが持つロケール（言語の選択肢から取得）
    pub available_locales: Vec<String>,
    /// 読み出したロケール
    pub locales: Vec<LocaleSnapshot>,
    /// アイテム全体の読み出し失敗の理由（成功なら None）
    pub error: Option<String>,
}

impl ItemSnapshot {
    /// ロケールの現在値を探す
    pub fn find(&self, code: &str) -> Option<&LocaleSnapshot> {
        self.locales.iter().find(|l| eq_ic(&l.code, code))
    }
}

/// 比較結果の状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonStatus {
    /// まだダッシュボードから読み出していない
    NotFetched,
    /// 正本と一致
    Match,
    /// 正本と異なる
    Different,
    /// 読み出しに失敗した
    Error,
}

impl ComparisonStatus {
    /// 画面・CLI に出す日本語名
    pub fn display_name(self) -> &'static str {
        match self {
            ComparisonStatus::NotFetched => "未取得",
            ComparisonStatus::Match => "一致",
            ComparisonStatus::Different => "差分あり",
            ComparisonStatus::Error => "エラー",
        }
    }
}

/// 欄 1 つの比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldComparison {
    pub field: ListingField,
    pub expected: String,
    pub current: Option<String>,
    pub status: ComparisonStatus,
}

/// アイテム × ロケール 1 件の比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleComparison {
    pub item_id: String,
    pub item_name: String,
    pub locale: String,
    pub fields: Vec<FieldComparison>,
    pub error: Option<String>,
}

impl LocaleComparison {
    /// 行全体の状態。エラー > 未取得 > 差分あり > 一致 の順に強い方を採る
    pub fn status(&self) -> ComparisonStatus {
        if self.error.is_some()
            || self
                .fields
                .iter()
                .any(|f| f.status == ComparisonStatus::Error)
        {
            return ComparisonStatus::Error;
        }
        if self.fields.is_empty()
            || self
                .fields
                .iter()
                .any(|f| f.status == ComparisonStatus::NotFetched)
        {
            return ComparisonStatus::NotFetched;
        }
        if self
            .fields
            .iter()
            .any(|f| f.status == ComparisonStatus::Different)
        {
            return ComparisonStatus::Different;
        }
        ComparisonStatus::Match
    }

    /// 差分のある欄
    pub fn different_fields(&self) -> Vec<ListingField> {
        self.fields
            .iter()
            .filter(|f| f.status == ComparisonStatus::Different)
            .map(|f| f.field)
            .collect()
    }
}

/// 値 1 つを比べる（完全一致。前後の空白や改行の違いも差分として扱う）
///
/// 正本側は改行を \n に正規化済み、ダッシュボードの textarea の value も \n なので、
/// 正規化の差で誤検出することはない。曖昧に「ほぼ同じ」とみなすと書き換え漏れに気づけなくなるため厳密に比べる。
pub fn compare_value(expected: &str, current: Option<&str>) -> ComparisonStatus {
    match current {
        None => ComparisonStatus::NotFetched,
        Some(c) if c == expected => ComparisonStatus::Match,
        Some(_) => ComparisonStatus::Different,
    }
}

/// アイテム 1 件を比べる。`snapshot` が None なら全行「未取得」
pub fn compare(expected: &ExpectedItem, snapshot: Option<&ItemSnapshot>) -> Vec<LocaleComparison> {
    let mut result = Vec::new();
    for locale in &expected.locales {
        let Some(snapshot) = snapshot else {
            result.push(not_fetched(expected, locale, None));
            continue;
        };
        if let Some(err) = &snapshot.error {
            result.push(not_fetched(expected, locale, Some(err.clone())));
            continue;
        }
        let Some(current) = snapshot.find(&locale.code) else {
            let error = (!snapshot.available_locales.is_empty()
                && !snapshot
                    .available_locales
                    .iter()
                    .any(|c| eq_ic(c, &locale.code)))
            .then(|| {
                format!(
                    "ダッシュボードにロケール {} がありません（ある言語: {}）",
                    locale.code,
                    snapshot.available_locales.join(", ")
                )
            });
            result.push(not_fetched(expected, locale, error));
            continue;
        };
        if let Some(err) = &current.error {
            result.push(not_fetched(expected, locale, Some(err.clone())));
            continue;
        }
        let fields = locale
            .fields()
            .into_iter()
            .map(|f| {
                let expected_value = locale.values[&f].clone();
                let value = current.values.get(&f).cloned();
                let status = compare_value(&expected_value, value.as_deref());
                FieldComparison {
                    field: f,
                    expected: expected_value,
                    current: value,
                    status,
                }
            })
            .collect();
        result.push(LocaleComparison {
            item_id: expected.item_id.clone(),
            item_name: expected.display_name.clone(),
            locale: locale.code.clone(),
            fields,
            error: None,
        });
    }
    result
}

fn not_fetched(
    item: &ExpectedItem,
    locale: &ExpectedLocale,
    error: Option<String>,
) -> LocaleComparison {
    LocaleComparison {
        item_id: item.item_id.clone(),
        item_name: item.display_name.clone(),
        locale: locale.code.clone(),
        fields: locale
            .fields()
            .into_iter()
            .map(|f| FieldComparison {
                field: f,
                expected: locale.values[&f].clone(),
                current: None,
                status: ComparisonStatus::NotFetched,
            })
            .collect(),
        error,
    }
}
