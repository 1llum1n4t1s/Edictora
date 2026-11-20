//! 設定の `privacy`（ダッシュボードの「プライバシー」タブの正本）の読込。
//!
//! 書いた欄だけが対象（書いていない欄は読み出し・比較・書き換えのどれにも使わない）。
//! ここでは「パスが実在するか・正規表現が正しいか・値が許される形か」までを見る（本文の取り出しと文字数の検証は、
//! 対象にしたときの `privacy::build_expected`＝Chrome を開く前で行う。対象にしないアイテムのファイルまで開かないため）。
//!
//! ```json
//! "privacy": {
//!   "singlePurpose": { "file": "webstore/store-listing.txt", "sectionStart": "^■ 単一の目的の説明$", "sectionEnd": "^■ データの使用に関する開示$" },
//!   "permissions": {
//!     "activeTab": { "file": "webstore/store-listing.txt", "sectionStart": "^  activeTab:$", "sectionEnd": "^  scripting:$" },
//!     "host": { "file": "...", "sectionStart": "^  ホスト権限:$", "sectionEnd": "^■ プライバシーポリシー URL$" }
//!   },
//!   "remoteCode": { "uses": false },
//!   "dataUsage": { "collected": ["personallyIdentifiable", "authentication"], "certify": true }
//! }
//! ```
//! `permissions` のキーは manifest の権限名で、ホスト権限だけは特別なキー `host`。
//! `remoteCode.uses` が true のときは `justification`（区間の指定）が必須、false のときは書けない。
//! `dataUsage.certify` は true だけを許す（「販売しない」など 3 つの宣誓チェックを入れる意味。false・省略は設定エラー）。
//! `policyUrl`（プライバシー ポリシー URL）は、公式 URL などと同じ契約: キーなし＝対象外、`""`＝空欄が正本、
//! それ以外は http(s) の絶対 URL だけ（2,048 文字以内）。

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use crate::config::{TextSource, TextSourceDto, check_http_url, resolve_under, section};
use crate::error::{Error, Result};
use crate::privacy::DataKind;
use crate::util::{format_n0, is_blank, utf16_len};
use crate::{tr, trs};

/// リモートコードの設定（検証済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCodeConfig {
    /// リモートコードを使うか
    pub uses: bool,
    /// 使うときの正当性の取り出し元
    pub justification: Option<TextSource>,
}

/// データ使用の開示の設定（検証済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataUsageConfig {
    /// チェックするデータの種類（書いた順。重複なし）
    pub collected: Vec<DataKind>,
    /// 宣誓チェックを入れるか（true だけを許すので常に true）
    pub certify: bool,
}

/// アイテム 1 件の「プライバシー」タブの設定（検証済み。パスは絶対パス）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrivacyConfig {
    /// 単一用途の説明の取り出し元
    pub single_purpose: Option<TextSource>,
    /// 権限ごとの正当性の取り出し元（キーは権限名か `host`。書いた順）。None なら権限は対象外
    pub permissions: Option<Vec<(String, TextSource)>>,
    pub remote_code: Option<RemoteCodeConfig>,
    pub data_usage: Option<DataUsageConfig>,
    /// プライバシー ポリシー URL（前後の空白は除去済み。空文字は「空欄が正本」。None なら対象外）
    pub policy_url: Option<String>,
}

/// プライバシー ポリシー URL の上限（ダッシュボードの文字数カウンタ「n/2,048」。UTF-16 単位）
pub const POLICY_URL_MAX_LENGTH: usize = 2_048;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PrivacyDto {
    single_purpose: Option<TextSourceDto>,
    permissions: Option<OrderedPermissions>,
    remote_code: Option<RemoteCodeDto>,
    data_usage: Option<DataUsageDto>,
    policy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct RemoteCodeDto {
    uses: Option<bool>,
    justification: Option<TextSourceDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct DataUsageDto {
    collected: Option<Vec<String>>,
    certify: Option<bool>,
}

/// 書いた順を保つ「権限名 → 本文の指定」
#[derive(Debug)]
pub(super) struct OrderedPermissions(Vec<(String, Option<TextSourceDto>)>);

impl<'de> Deserialize<'de> for OrderedPermissions {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OrderedPermissions;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(trs!("config.privacy_an_object_keyed_by_permission_name"))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Option<TextSourceDto>>()? {
                    entries.push((k, v));
                }
                Ok(OrderedPermissions(entries))
            }
        }
        deserializer.deserialize_map(V)
    }
}

/// `privacy` を検証して作る。`place` は「items[1] (…)」の形のエラー表示用。
/// 検査は設定キーごとの関数に分け、順序（singlePurpose → permissions → remoteCode → dataUsage → policyUrl →
/// 1 欄も無ければエラー）と「最初のエラーで止まる」動きは変えない。
pub(super) fn parse_privacy(
    dto: PrivacyDto,
    place: &str,
    base_dir: &Path,
) -> Result<PrivacyConfig> {
    let place = format!("{place}.privacy");
    let mut config = PrivacyConfig::default();

    if let Some(source) = dto.single_purpose {
        config.single_purpose = Some(parse_source(
            source,
            &format!("{place}.singlePurpose"),
            base_dir,
        )?);
    }
    if let Some(OrderedPermissions(entries)) = dto.permissions {
        config.permissions = Some(parse_permissions(entries, &place, base_dir)?);
    }
    if let Some(remote) = dto.remote_code {
        config.remote_code = Some(parse_remote_code(remote, &place, base_dir)?);
    }
    if let Some(usage) = dto.data_usage {
        config.data_usage = Some(parse_data_usage(usage, &place)?);
    }
    if let Some(value) = dto.policy_url {
        config.policy_url = Some(parse_policy_url(&value, &place)?);
    }

    if config.single_purpose.is_none()
        && config.permissions.is_none()
        && config.remote_code.is_none()
        && config.data_usage.is_none()
        && config.policy_url.is_none()
    {
        return Err(Error::Config(tr!(
            "config.privacy_specify_at_least_one",
            place = place
        )));
    }
    Ok(config)
}

/// `permissions`（権限名ごとの正当性の取り出し元）
fn parse_permissions(
    entries: Vec<(String, Option<TextSourceDto>)>,
    place: &str,
    base_dir: &Path,
) -> Result<Vec<(String, TextSource)>> {
    if entries.is_empty() {
        return Err(Error::Config(tr!(
            "config.privacy_permissions_is_empty",
            place = place
        )));
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut list = Vec::new();
    for (key, source) in entries {
        let here = format!("{place}.permissions.{key}");
        if key.trim().is_empty() || key.trim() != key {
            return Err(Error::Config(tr!(
                "config.privacy_invalid_permission_name",
                place = place,
                key = key
            )));
        }
        if !seen.insert(key.clone()) {
            return Err(Error::Config(tr!(
                "config.privacy_duplicate_permission_name",
                place = place,
                key = key
            )));
        }
        let Some(source) = source else {
            return Err(Error::Config(tr!(
                "config.privacy_entry_is_empty",
                place = here
            )));
        };
        list.push((key, parse_source(source, &here, base_dir)?));
    }
    Ok(list)
}

/// `remoteCode`（`uses` と、true のときだけ必須の `justification`）
fn parse_remote_code(
    remote: RemoteCodeDto,
    place: &str,
    base_dir: &Path,
) -> Result<RemoteCodeConfig> {
    let here = format!("{place}.remoteCode");
    let Some(uses) = remote.uses else {
        return Err(Error::Config(tr!(
            "config.privacy_remotecode_specify_uses",
            place = here
        )));
    };
    let justification = match (uses, remote.justification) {
        (true, Some(source)) => Some(parse_source(
            source,
            &format!("{here}.justification"),
            base_dir,
        )?),
        (true, None) => {
            return Err(Error::Config(tr!(
                "config.privacy_remotecode_justification_required",
                place = here
            )));
        }
        (false, Some(_)) => {
            return Err(Error::Config(tr!(
                "config.privacy_remotecode_justification_not_allowed",
                place = here
            )));
        }
        (false, None) => None,
    };
    Ok(RemoteCodeConfig {
        uses,
        justification,
    })
}

/// `dataUsage`（収集するデータの種類と、true だけを許す宣誓）
fn parse_data_usage(usage: DataUsageDto, place: &str) -> Result<DataUsageConfig> {
    let here = format!("{place}.dataUsage");
    let Some(names) = usage.collected else {
        return Err(Error::Config(tr!(
            "config.privacy_datausage_specify_collected",
            place = here
        )));
    };
    let mut collected: Vec<DataKind> = Vec::new();
    for name in names {
        let Some(kind) = DataKind::parse(&name) else {
            return Err(Error::Config(tr!(
                "config.privacy_unknown_data_kind",
                place = here,
                name = name,
                allowed = DataKind::config_keys()
            )));
        };
        if collected.contains(&kind) {
            return Err(Error::Config(tr!(
                "config.privacy_duplicate_data_kind",
                place = here,
                name = name
            )));
        }
        collected.push(kind);
    }
    // 「宣誓」は true だけを許す。false や省略を黙って「入れない」とすると、
    // 審査に必要な 3 つの宣誓を外す書き換えになりうるため、設定の誤りとして止める
    if usage.certify != Some(true) {
        return Err(Error::Config(tr!(
            "config.privacy_datausage_certify_must_be_true",
            place = here
        )));
    }
    Ok(DataUsageConfig {
        collected,
        certify: true,
    })
}

/// `policyUrl`。キーなしは対象外（呼び出し側）。空文字は「空欄が正本」。
/// それ以外は http(s) の絶対 URL だけ（公式 URL などと共通の検査）で、長さの上限は ここで見る
fn parse_policy_url(value: &str, place: &str) -> Result<String> {
    let trimmed = check_http_url(value, "policyUrl", place)?;
    let length = utf16_len(&trimmed);
    if length > POLICY_URL_MAX_LENGTH {
        return Err(Error::Config(tr!(
            "config.privacy_policy_url_too_long",
            place = place,
            length = format_n0(length),
            max = format_n0(POLICY_URL_MAX_LENGTH)
        )));
    }
    Ok(trimmed)
}

/// 本文の取り出し元 1 つ（`description` と同じ形: file と、sectionStart / sectionEnd の組）
fn parse_source(dto: TextSourceDto, place: &str, base_dir: &Path) -> Result<TextSource> {
    if is_blank(dto.file.as_deref()) {
        return Err(Error::Config(tr!(
            "config.privacy_specify_file",
            place = place
        )));
    }
    if dto.section_start.is_none() != dto.section_end.is_none() {
        return Err(Error::Config(tr!(
            "config.privacy_specify_both_sectionstart_and_sectionend",
            place = place
        )));
    }
    if let Some(p) = &dto.section_start {
        section::validate_pattern(p, &format!("{place}.sectionStart"))?;
    }
    if let Some(p) = &dto.section_end {
        section::validate_pattern(p, &format!("{place}.sectionEnd"))?;
    }
    let file = resolve_under(base_dir, dto.file.as_deref());
    if !file.is_file() {
        return Err(Error::Config(tr!(
            "config.privacy_source_file_not_found",
            file = file.display(),
            place = place
        )));
    }
    Ok(TextSource {
        file_path: file,
        section_start: dto.section_start,
        section_end: dto.section_end,
    })
}
