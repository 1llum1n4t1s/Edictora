//! ダッシュボードの「プライバシー」タブ（Chrome ウェブストアの Privacy practices）のモデル・正本の組み立て・比較。
//!
//! 画面操作は持たない（`dashboard` の `PrivacyPage` が担う）。ここは「正本は何か」「現在の画面と一致するか」を
//! 決める純粋な部分で、ブラウザなしでテストできる。
//!
//! 設計の要点:
//! - 拡張機能に権限を足すと、このタブで権限の正当性などを入力しないと審査に送信できない。
//!   手作業の入力をなくすため、掲載文と同じく各リポジトリの正本ファイルから読み取り・比較・下書き保存する。
//! - 正本の本文は説明文と同じ「ファイル + sectionStart / sectionEnd」で指す（`config::section`）。
//!   書式が固定でないので解析はしない。字下げされた本文は、前後の空行を除いて共通の字下げを取り除く（`dedent`）。
//! - 対象は設定に書いた欄だけ（単一用途の説明・権限ごとの正当性・データ使用の 2 欄）。書いていない欄は読み出し・比較・
//!   書き換えのどれにも使わない。ただし `permissions` を書いたときは「権限の一覧そのもの」も照合する:
//!   設定にある権限の欄が画面に無い、または画面にある権限の欄が設定に無いなら、比較結果で分かるようにして push を止める
//!   （欄の無い権限を黙って飛ばすと、正当性の入力漏れに気づけないため）。
//! - 比較は文字列の完全一致（前後の空白の違いも差分）。チェックボックスは設定キーの並びに直して文字列で比べる。
//! - リモートコードの欄（`remoteCode`）は、設定と比較の枠だけを持つ。実機（2026-11-18、15 アイテム）のどのアイテムにも
//!   その欄が出ておらず DOM を確かめられなかったので、画面側は「未対応」として読めない扱いにし、設定に書くと push を止める。

use std::collections::{BTreeMap, BTreeSet};

use crate::config::ItemConfig;
use crate::config::section::{extract_source, is_js_whitespace};
use crate::error::{Error, Result};
use crate::listing::{ComparisonStatus, compare_value};
use crate::util::{format_n0, utf16_len};
use crate::{tr, trs};

/// ダッシュボードのテキスト欄（単一用途の説明・権限の正当性）の上限（文字数カウンタ「n/1,000」。UTF-16 単位）
pub const TEXT_MAX_LENGTH: usize = 1_000;

/// 権限のうち、ホスト権限を表す特別なキー（設定の `permissions` のキー）
pub const HOST_PERMISSION_KEY: &str = "host";

/// データ使用の開示で選べるデータの種類（ダッシュボードの 9 種類。並びは画面の並びと同じ）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataKind {
    /// 個人を特定できる情報
    PersonallyIdentifiable,
    /// 健康に関する情報
    Health,
    /// 財務状況や支払いに関する情報
    Financial,
    /// 認証に関する情報
    Authentication,
    /// 個人的コミュニケーション
    PersonalCommunications,
    /// 位置情報
    Location,
    /// ウェブ履歴
    WebHistory,
    /// ユーザーのアクティビティ
    UserActivity,
    /// ウェブサイトのコンテンツ
    WebsiteContent,
}

impl DataKind {
    /// 画面の並び順（チェックボックスの value 1〜9 と同じ）
    pub const ALL: [DataKind; 9] = [
        DataKind::PersonallyIdentifiable,
        DataKind::Health,
        DataKind::Financial,
        DataKind::Authentication,
        DataKind::PersonalCommunications,
        DataKind::Location,
        DataKind::WebHistory,
        DataKind::UserActivity,
        DataKind::WebsiteContent,
    ];

    /// 設定ファイル上のキー名（識別子なので訳さない）
    pub fn config_key(self) -> &'static str {
        match self {
            DataKind::PersonallyIdentifiable => "personallyIdentifiable",
            DataKind::Health => "health",
            DataKind::Financial => "financial",
            DataKind::Authentication => "authentication",
            DataKind::PersonalCommunications => "personalCommunications",
            DataKind::Location => "location",
            DataKind::WebHistory => "webHistory",
            DataKind::UserActivity => "userActivity",
            DataKind::WebsiteContent => "websiteContent",
        }
    }

    /// 設定のキー名から作る（大文字小文字は区別する）
    pub fn parse(key: &str) -> Option<DataKind> {
        DataKind::ALL.into_iter().find(|k| k.config_key() == key)
    }

    /// 設定に書ける名前の一覧（エラー文用）
    pub fn config_keys() -> String {
        DataKind::ALL
            .iter()
            .map(|k| k.config_key())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// データの種類の集合を、設定キーの並び（画面の順）の文字列にする。比較と表示の両方で同じ形を使う
pub fn format_kinds(kinds: &BTreeSet<DataKind>) -> String {
    kinds
        .iter()
        .map(|k| k.config_key())
        .collect::<Vec<_>>()
        .join(", ")
}

// ───────── 正本 ─────────

/// リモートコードの正本
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRemoteCode {
    /// リモートコードを使うか
    pub uses: bool,
    /// 使うときの正当性（使わないなら None）
    pub justification: Option<String>,
}

/// データ使用の開示の正本
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedDataUsage {
    /// チェックするデータの種類（それ以外はチェックを外す）
    pub collected: BTreeSet<DataKind>,
    /// 3 つの宣誓チェックを入れるか（設定では true だけを許すので、常に true）
    pub certify: bool,
}

/// アイテム 1 件の「プライバシー」タブの正本（設定に書いた欄だけを持つ）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPrivacy {
    /// 単一用途の説明
    pub single_purpose: Option<String>,
    /// 権限の正当性（キーは manifest の権限名か `host`。設定に書いた順）。None なら権限は対象外
    pub permissions: Option<Vec<(String, String)>>,
    /// リモートコード（画面側は未対応。書くと push を止める）
    pub remote_code: Option<ExpectedRemoteCode>,
    /// データ使用の開示
    pub data_usage: Option<ExpectedDataUsage>,
}

impl ExpectedPrivacy {
    /// 対象の欄が 1 つ以上あるか
    pub fn is_empty(&self) -> bool {
        self.single_purpose.is_none()
            && self.permissions.is_none()
            && self.remote_code.is_none()
            && self.data_usage.is_none()
    }

    /// 対象の欄（表示順）
    pub fn fields(&self) -> Vec<PrivacyField> {
        let mut fields = Vec::new();
        if self.single_purpose.is_some() {
            fields.push(PrivacyField::SinglePurpose);
        }
        for (key, _) in self.permissions.iter().flatten() {
            fields.push(PrivacyField::Permission(key.clone()));
        }
        if let Some(remote) = &self.remote_code {
            fields.push(PrivacyField::RemoteCodeUses);
            if remote.justification.is_some() {
                fields.push(PrivacyField::RemoteCodeJustification);
            }
        }
        if self.data_usage.is_some() {
            fields.push(PrivacyField::DataCollected);
            fields.push(PrivacyField::Certify);
        }
        fields
    }

    /// 欄の正本の値（比較用の文字列）。対象外の欄は None
    pub fn value_of(&self, field: &PrivacyField) -> Option<String> {
        match field {
            PrivacyField::SinglePurpose => self.single_purpose.clone(),
            PrivacyField::Permission(key) => self
                .permissions
                .iter()
                .flatten()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone()),
            PrivacyField::RemoteCodeUses => self.remote_code.as_ref().map(|r| r.uses.to_string()),
            PrivacyField::RemoteCodeJustification => self
                .remote_code
                .as_ref()
                .and_then(|r| r.justification.clone()),
            PrivacyField::DataCollected => {
                self.data_usage.as_ref().map(|d| format_kinds(&d.collected))
            }
            PrivacyField::Certify => self.data_usage.as_ref().map(|d| d.certify.to_string()),
        }
    }
}

/// 「プライバシー」タブの欄 1 つ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyField {
    /// 単一用途の説明（textarea）
    SinglePurpose,
    /// 権限の正当性（textarea。キーは manifest の権限名か `host`）
    Permission(String),
    /// リモートコードを使うか
    RemoteCodeUses,
    /// リモートコードの正当性
    RemoteCodeJustification,
    /// 収集するデータの種類（9 つのチェックボックス）
    DataCollected,
    /// 3 つの宣誓チェック
    Certify,
}

impl PrivacyField {
    /// 画面・CLI に出す欄の名前（表示言語に合わせる）
    pub fn display_name(&self) -> String {
        match self {
            PrivacyField::SinglePurpose => trs!("privacy.single_purpose").to_string(),
            PrivacyField::Permission(key) => tr!("privacy.permission_justification", name = key),
            PrivacyField::RemoteCodeUses => trs!("privacy.remote_code_uses").to_string(),
            PrivacyField::RemoteCodeJustification => {
                trs!("privacy.remote_code_justification").to_string()
            }
            PrivacyField::DataCollected => trs!("privacy.data_collected").to_string(),
            PrivacyField::Certify => trs!("privacy.certify").to_string(),
        }
    }

    /// 本文が長い文章の欄か（差分を行単位で出す）
    pub fn is_long_text(&self) -> bool {
        matches!(
            self,
            PrivacyField::SinglePurpose
                | PrivacyField::Permission(_)
                | PrivacyField::RemoteCodeJustification
        )
    }
}

/// 正本の本文を整える: 前後の空行を除き、全行に共通する字下げ（行頭の空白）を取り除く。
///
/// 正本ファイルでは、権限の正当性が「  activeTab:」の下に字下げされて書かれている。区間の切り出しは
/// 先頭の空行と末尾の空白を落とすだけなので、字下げはここで外す（行頭の空白だけの行は、共通の字下げの判定に使わない）。
/// 行末の空白は落とさない（本文の一部として完全一致で比べるため。末尾の空白は切り出しの時点で落ちている）。
pub fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let indent_of = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let common = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| indent_of(l))
        .min()
        .unwrap_or(0);
    let stripped: Vec<&str> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                // 空白だけの行は空行に寄せる（字下げの長さが行ごとに違っても完全一致で比べられるように）
                ""
            } else {
                &l[common..]
            }
        })
        .collect();
    // 前後の空行を除く
    let joined = stripped.join("\n");
    joined
        .trim_start_matches('\n')
        .trim_end_matches(is_js_whitespace)
        .to_string()
}

/// 設定のアイテムから「プライバシー」タブの正本を作る。本文はここでファイルから取り出す（失敗は `Config` エラー）。
/// `privacy` を書いていないアイテムは None
pub fn build_expected(item: &ItemConfig) -> Result<Option<ExpectedPrivacy>> {
    let Some(config) = &item.privacy else {
        return Ok(None);
    };
    let place = format!("{} ({})", item.display_name, item.item_id);
    let text_of = |source: &crate::config::TextSource, what: &str| -> Result<String> {
        let place = format!("{place} privacy.{what}");
        let text = dedent(&extract_source(source)?);
        if text.is_empty() {
            return Err(Error::Config(tr!(
                "privacy.the_source_text_is_empty",
                place = place
            )));
        }
        let len = utf16_len(&text);
        if len > TEXT_MAX_LENGTH {
            return Err(Error::Config(tr!(
                "privacy.the_source_text_is_over_the_limit",
                place = place,
                length = format_n0(len),
                max = format_n0(TEXT_MAX_LENGTH)
            )));
        }
        Ok(text)
    };

    let single_purpose = match &config.single_purpose {
        Some(source) => Some(text_of(source, "singlePurpose")?),
        None => None,
    };
    let permissions = match &config.permissions {
        Some(list) => {
            let mut result = Vec::new();
            for (key, source) in list {
                result.push((key.clone(), text_of(source, &format!("permissions.{key}"))?));
            }
            Some(result)
        }
        None => None,
    };
    let remote_code = match &config.remote_code {
        Some(remote) => Some(ExpectedRemoteCode {
            uses: remote.uses,
            justification: match &remote.justification {
                Some(source) => Some(text_of(source, "remoteCode.justification")?),
                None => None,
            },
        }),
        None => None,
    };
    let data_usage = config.data_usage.as_ref().map(|d| ExpectedDataUsage {
        collected: d.collected.iter().copied().collect(),
        certify: d.certify,
    });
    Ok(Some(ExpectedPrivacy {
        single_purpose,
        permissions,
        remote_code,
        data_usage,
    }))
}

// ───────── 現在値 ─────────

/// ダッシュボードの「プライバシー」タブから読み出した現在値。
///
/// 読むのは正本に書いた欄だけ（権限は、画面にある権限の名前の一覧だけを全部読み、本文は設定にある権限の分だけ読む）。
/// `None` は「画面にその欄が無い」を表す。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrivacyReading {
    /// 単一用途の説明（欄が無ければ None）
    pub single_purpose: Option<String>,
    /// 画面にある権限の欄のキー（manifest の権限名か `host`。画面の並び）
    pub screen_permissions: Vec<String>,
    /// 設定にある権限の本文（キー → 本文。画面にあるものだけ）
    pub permissions: BTreeMap<String, String>,
    /// チェックされているデータの種類（9 つのチェックボックスが揃っていなければ None）
    pub collected: Option<BTreeSet<DataKind>>,
    /// 3 つの宣誓チェックがすべて入っているか（3 つが揃っていなければ None）
    pub certified: Option<bool>,
}

/// アイテム 1 件のプライバシー タブの現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacySnapshot {
    /// アイテム ID
    pub item_id: String,
    /// ステータス表示（読めなければ None）
    pub status_text: Option<String>,
    /// 読み出した値（失敗なら None）
    pub reading: Option<PrivacyReading>,
    /// 読み出し失敗の理由（成功なら None）
    pub error: Option<String>,
}

// ───────── 比較 ─────────

/// 欄 1 つの比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRow {
    pub field: PrivacyField,
    /// 正本の値（画面にあって設定に無い権限は空）
    pub expected: String,
    /// 現在値（画面に欄が無い・読めないときは None）
    pub current: Option<String>,
    pub status: ComparisonStatus,
    /// 状態が Error のときの理由（欄が画面に無い・設定に無い・未対応など）
    pub note: Option<String>,
}

/// アイテム 1 件の比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyComparison {
    pub item_id: String,
    pub item_name: String,
    pub rows: Vec<PrivacyRow>,
    /// アイテム全体の読み出し失敗の理由
    pub error: Option<String>,
}

impl PrivacyComparison {
    /// 全体の状態。エラー > 未取得 > 差分あり > 一致 の順に強い方を採る（`LocaleComparison` と同じ）
    pub fn status(&self) -> ComparisonStatus {
        if self.error.is_some()
            || self
                .rows
                .iter()
                .any(|r| r.status == ComparisonStatus::Error)
        {
            return ComparisonStatus::Error;
        }
        if self.rows.is_empty()
            || self
                .rows
                .iter()
                .any(|r| r.status == ComparisonStatus::NotFetched)
        {
            return ComparisonStatus::NotFetched;
        }
        if self
            .rows
            .iter()
            .any(|r| r.status == ComparisonStatus::Different)
        {
            return ComparisonStatus::Different;
        }
        ComparisonStatus::Match
    }

    /// 差分のある（書き換えで直せる）欄
    pub fn different_fields(&self) -> Vec<&PrivacyField> {
        self.rows
            .iter()
            .filter(|r| r.status == ComparisonStatus::Different)
            .map(|r| &r.field)
            .collect()
    }

    /// 書き換えでは直せない不整合（画面に欄が無い・設定に無い権限・未対応）の説明
    pub fn blockers(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.status == ComparisonStatus::Error)
            .map(|r| {
                format!(
                    "{}: {}",
                    r.field.display_name(),
                    r.note.as_deref().unwrap_or("")
                )
            })
            .collect()
    }
}

/// 正本と現在値を比べる。`reading` が None なら全欄「未取得」
pub fn compare(
    item_id: &str,
    item_name: &str,
    expected: &ExpectedPrivacy,
    reading: Option<&PrivacyReading>,
) -> PrivacyComparison {
    let mut rows = Vec::new();
    for field in expected.fields() {
        let want = expected.value_of(&field).unwrap_or_default();
        let Some(reading) = reading else {
            rows.push(PrivacyRow {
                field,
                expected: want,
                current: None,
                status: ComparisonStatus::NotFetched,
                note: None,
            });
            continue;
        };
        rows.push(compare_field(expected, reading, field, want));
    }

    // `permissions` を書いたときは、画面にあって設定に無い権限も不整合として出す（読み取った名前だけ。本文は読まない）
    if let (Some(permissions), Some(reading)) = (&expected.permissions, reading) {
        for key in &reading.screen_permissions {
            if !permissions.iter().any(|(k, _)| k == key) {
                rows.push(PrivacyRow {
                    field: PrivacyField::Permission(key.clone()),
                    expected: String::new(),
                    current: None,
                    status: ComparisonStatus::Error,
                    note: Some(tr!(
                        "privacy.the_permission_is_not_in_the_config",
                        name = key
                    )),
                });
            }
        }
    }
    PrivacyComparison {
        item_id: item_id.to_string(),
        item_name: item_name.to_string(),
        rows,
        error: None,
    }
}

fn compare_field(
    expected: &ExpectedPrivacy,
    reading: &PrivacyReading,
    field: PrivacyField,
    want: String,
) -> PrivacyRow {
    let not_on_dashboard = |field: PrivacyField, want: String| PrivacyRow {
        note: Some(tr!(
            "privacy.the_field_is_not_on_the_dashboard",
            label = field.display_name()
        )),
        field,
        expected: want,
        current: None,
        status: ComparisonStatus::Error,
    };
    let current: Option<String> = match &field {
        PrivacyField::SinglePurpose => reading.single_purpose.clone(),
        PrivacyField::Permission(key) => {
            if !reading.screen_permissions.contains(key) {
                return not_on_dashboard(field, want);
            }
            reading.permissions.get(key).cloned()
        }
        // 画面側が未対応（実機でこの欄の DOM を確認できていない）。設定に書くと push を止める
        PrivacyField::RemoteCodeUses | PrivacyField::RemoteCodeJustification => {
            return PrivacyRow {
                note: Some(trs!("privacy.remote_code_is_not_supported").to_string()),
                field,
                expected: want,
                current: None,
                status: ComparisonStatus::Error,
            };
        }
        PrivacyField::DataCollected => reading.collected.as_ref().map(format_kinds),
        PrivacyField::Certify => reading.certified.map(|c| c.to_string()),
    };
    let _ = expected;
    match current {
        None => not_on_dashboard(field, want),
        Some(c) => {
            let status = compare_value(&want, Some(&c));
            PrivacyRow {
                field,
                expected: want,
                current: Some(c),
                status,
                note: None,
            }
        }
    }
}

/// 比較結果 1 行の説明（短い欄は 1 行、長い本文の欄は行単位の差分つき。表示言語に合わせる）
pub fn describe(row: &PrivacyRow) -> String {
    use crate::listing::text_diff::{self, DiffKind};
    let label = row.field.display_name();
    match row.status {
        ComparisonStatus::Match => format!("{}\n", tr!("privacy.row_match", label = label)),
        ComparisonStatus::NotFetched => {
            format!("{}\n", tr!("privacy.row_not_fetched", label = label))
        }
        ComparisonStatus::Error => format!(
            "{}\n",
            tr!(
                "privacy.row_error",
                label = label,
                error = row.note.as_deref().unwrap_or("")
            )
        ),
        ComparisonStatus::Different => {
            let current = row.current.as_deref().unwrap_or("");
            let mut out = format!("{}\n", tr!("privacy.row_different", label = label));
            if row.field.is_long_text() {
                let lines = text_diff::compute(current, &row.expected);
                if lines.iter().all(|l| l.kind == DiffKind::Same) {
                    // 行単位では同じ（不可視文字・行末の空白などの違い）
                    match text_diff::first_difference_position(current, &row.expected) {
                        Some(at) => out.push_str(&tr!(
                            "cli.lines_are_identical_there_is_an_invisible",
                            position = at
                        )),
                        None => {
                            out.push_str(trs!("cli.lines_are_identical_there_is_an_invisible_2"))
                        }
                    }
                } else {
                    out.push_str(&text_diff::format(&lines, 2));
                }
            } else {
                out.push_str(&format!("- {current}\n+ {}\n", row.expected));
            }
            out
        }
    }
}
