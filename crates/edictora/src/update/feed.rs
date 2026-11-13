//! フィード（`releases.{channel}.json`）の解釈と、更新先の選択。
//!
//! vendored の SDK と同じ約束にする:
//! - 形式（manager.rs の `VelopackAssetFeed` / `VelopackAsset`）: `{"Assets":[{"PackageId","Version","Type",
//!   "FileName","SHA1","SHA256","Size",...}]}`。キーは大文字小文字を区別しない（macros.rs の
//!   `define_struct_case_insensitive`。キーを小文字にしてから照合する）。無いキーは既定値（空文字・0）、
//!   知らないキーは無視、型が違えばエラー。
//! - 選び方（manager.rs の `check_for_updates`）: Type が `Full`（大文字小文字を区別しない）で版を解釈できる
//!   ものの中から最大の版を選ぶ（同じ版なら先に出たもの）。それが現在の版より新しければ更新あり。
//!   ダウングレードは許さない（`AllowVersionDowngrade` の既定は false）。差分（Delta）パッケージは使わない
//!   （差分の適用には Update.exe の `patch` と手元の完全パッケージが要るため。SDK も差分が無ければ完全版を取る）。

use serde_json::{Map, Value};

use super::version::Version;

/// フィードの 1 項目
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeedAsset {
    pub package_id: String,
    pub version: String,
    /// `Full` / `Delta`
    pub kind: String,
    pub file_name: String,
    pub sha1: String,
    pub sha256: String,
    pub size: u64,
}

/// 更新の確認結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheck {
    /// フィードに完全パッケージが 1 つも無い（SDK の `RemoteIsEmpty`）
    RemoteIsEmpty,
    /// 最新の完全パッケージが現在の版以下（SDK の `NoUpdateAvailable`）
    NoUpdateAvailable { latest: Version },
    /// 新しい版がある
    UpdateAvailable { asset: FeedAsset, version: Version },
}

/// フィードの JSON を解釈する
pub fn parse_feed(json: &str) -> Result<Vec<FeedAsset>, String> {
    let json = json.strip_prefix('\u{feff}').unwrap_or(json);
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("フィードが JSON として読めません: {e}"))?;
    let root = lower_keys(&root, "フィード")?;
    let Some(assets) = root.get("assets") else {
        return Ok(Vec::new());
    };
    let assets = assets
        .as_array()
        .ok_or_else(|| "フィードの Assets が配列ではありません".to_string())?;
    assets
        .iter()
        .enumerate()
        .map(|(i, v)| parse_asset(v).map_err(|m| format!("フィードの Assets[{i}]: {m}")))
        .collect()
}

/// オブジェクトのキーを小文字にした写しを作る（オブジェクトでなければエラー）
fn lower_keys(value: &Value, what: &str) -> Result<Map<String, Value>, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{what}が JSON のオブジェクトではありません"))?;
    Ok(obj
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.clone()))
        .collect())
}

fn parse_asset(value: &Value) -> Result<FeedAsset, String> {
    let map = lower_keys(value, "項目")?;
    let text = |key: &str| -> Result<String, String> {
        match map.get(key) {
            None => Ok(String::new()),
            Some(Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(format!("{key} が文字列ではありません")),
        }
    };
    let size = match map.get("size") {
        None => 0,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| "size が 0 以上の整数ではありません".to_string())?,
    };
    Ok(FeedAsset {
        package_id: text("packageid")?,
        version: text("version")?,
        kind: text("type")?,
        file_name: text("filename")?,
        sha1: text("sha1")?,
        sha256: text("sha256")?,
        size,
    })
}

/// 現在の版と比べて、更新するかを決める
pub fn check_for_updates(assets: &[FeedAsset], current: &Version) -> UpdateCheck {
    let mut latest: Option<(&FeedAsset, Version)> = None;
    for asset in assets {
        if !asset.kind.eq_ignore_ascii_case("Full") {
            continue;
        }
        let Some(v) = Version::parse(&asset.version) else {
            continue;
        };
        // 厳密に大きいときだけ置き換える（同じ版なら先に出たものを残す。SDK と同じ）
        if latest.as_ref().is_none_or(|(_, lv)| v > *lv) {
            latest = Some((asset, v));
        }
    }
    match latest {
        None => UpdateCheck::RemoteIsEmpty,
        Some((asset, version)) if version > *current => UpdateCheck::UpdateAvailable {
            asset: asset.clone(),
            version,
        },
        Some((_, version)) => UpdateCheck::NoUpdateAvailable { latest: version },
    }
}

/// フィードの FileName を、パッケージ置き場のファイル名・URL の最後の要素として使ってよいか。
/// パスの区切り・`..`・URL で意味を持つ文字を拒み、`.nupkg` で終わる素のファイル名だけを許す
pub fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.contains("..")
        && name.to_ascii_lowercase().ends_with(".nupkg")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}
