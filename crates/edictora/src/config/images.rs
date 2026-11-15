//! 設定の `images`（掲載画像の正本）の読込。
//!
//! 書いたキーだけが対象（書いていないスロットは読み出し・比較・書き換えのどれにも使わない）。
//! ここでは「パスが実在するか・枚数が範囲か」までを見る（設定全体の読込で、対象にしないアイテムの
//! 画像の中身まで開かないため）。PNG かどうか・寸法・重複の検証は、対象にしたときの
//! `listing::build_expected`（Chrome を開く前）で行う。
//!
//! ```json
//! "images": {
//!   "icon": "icons/icon-128.png",
//!   "smallTile": "webstore/images/en/tile-small.png",
//!   "marquee": "webstore/images/en/tile-marquee.png",
//!   "globalScreenshots": { "dir": "webstore/images/en" },
//!   "localizedScreenshots": {
//!     "ja": { "dir": "webstore/images/ja" },
//!     "zh_CN": { "files": ["webstore/images/zh_CN/screenshot-01.png"] }
//!   }
//! }
//! ```
//! `dir` は直下の `screenshot-*.png` を名前の昇順で並べる（1〜5 枚）。`files` は書いた順（1〜5 枚）。
//! ロケールのキーは `locales` と同じ規則で正規化する（`zh_CN` と `zh-CN` は同じ）。

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use crate::config::{expand, full_path, is_valid_locale_code, normalize_locale_code};
use crate::error::{Error, Result};
use crate::images::MAX_SCREENSHOTS;
use crate::util::is_blank;
use crate::{tr, trs};

/// アイテム 1 件の画像の設定（検証済み。パスは絶対パス）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImagesConfig {
    /// ストア アイコン（128x128 の PNG。透過可）
    pub icon: Option<PathBuf>,
    pub small_tile: Option<PathBuf>,
    pub marquee: Option<PathBuf>,
    pub global_screenshots: Option<Vec<PathBuf>>,
    /// ロケールコード（「-」に正規化済み）→ ファイル（書いた順）
    pub localized_screenshots: Vec<(String, Vec<PathBuf>)>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ImagesDto {
    icon: Option<String>,
    small_tile: Option<String>,
    marquee: Option<String>,
    global_screenshots: Option<ScreenshotsDto>,
    localized_screenshots: Option<OrderedScreenshots>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ScreenshotsDto {
    /// 直下の screenshot-*.png を使うフォルダー
    dir: Option<String>,
    /// ファイルを 1 枚ずつ書く
    files: Option<Vec<String>>,
}

/// 書いた順を保つ「ロケール → スクリーンショットの指定」
#[derive(Debug)]
pub(super) struct OrderedScreenshots(Vec<(String, Option<ScreenshotsDto>)>);

impl<'de> Deserialize<'de> for OrderedScreenshots {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OrderedScreenshots;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(trs!("config.an_object_keyed_by_locale_code"))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Option<ScreenshotsDto>>()? {
                    entries.push((k, v));
                }
                Ok(OrderedScreenshots(entries))
            }
        }
        deserializer.deserialize_map(V)
    }
}

/// `images` を検証して作る。`place` は「items[1] (…)」の形のエラー表示用
pub(super) fn parse_images(dto: ImagesDto, place: &str, base_dir: &Path) -> Result<ImagesConfig> {
    let place = format!("{place}.images");
    let mut config = ImagesConfig::default();

    if let Some(p) = dto.icon.as_deref() {
        config.icon = Some(single_file(p, base_dir, &format!("{place}.icon"))?);
    }
    if let Some(p) = dto.small_tile.as_deref() {
        config.small_tile = Some(single_file(p, base_dir, &format!("{place}.smallTile"))?);
    }
    if let Some(p) = dto.marquee.as_deref() {
        config.marquee = Some(single_file(p, base_dir, &format!("{place}.marquee"))?);
    }
    if let Some(s) = dto.global_screenshots {
        config.global_screenshots = Some(screenshot_files(
            s,
            base_dir,
            &format!("{place}.globalScreenshots"),
        )?);
    }
    if let Some(OrderedScreenshots(entries)) = dto.localized_screenshots {
        let mut seen: HashSet<String> = HashSet::new();
        for (raw_code, s) in entries {
            let code = normalize_locale_code(&raw_code);
            let here = format!("{place}.localizedScreenshots.{code}");
            if !is_valid_locale_code(&code) {
                return Err(Error::Config(tr!(
                    "config.invalid_locale_code",
                    place = place,
                    raw_code = raw_code
                )));
            }
            if !seen.insert(code.to_uppercase()) {
                return Err(Error::Config(tr!(
                    "config.duplicate_locale_code",
                    place = place,
                    raw_code = raw_code
                )));
            }
            let Some(s) = s else {
                return Err(Error::Config(tr!(
                    "config.the_entry_is_empty",
                    place = place,
                    code = code
                )));
            };
            let files = screenshot_files(s, base_dir, &here)?;
            config.localized_screenshots.push((code, files));
        }
    }

    if config.icon.is_none()
        && config.small_tile.is_none()
        && config.marquee.is_none()
        && config.global_screenshots.is_none()
        && config.localized_screenshots.is_empty()
    {
        return Err(Error::Config(tr!(
            "config.images_specify_at_least_one",
            place = place
        )));
    }
    Ok(config)
}

/// 1 枚のファイル（アイコン・小タイル・マーキー）
fn single_file(value: &str, base_dir: &Path, place: &str) -> Result<PathBuf> {
    if is_blank(Some(value)) {
        return Err(Error::Config(tr!(
            "config.images_path_is_empty",
            place = place
        )));
    }
    let file = full_path(&base_dir.join(expand(Some(value)).unwrap_or_default()));
    if !file.is_file() {
        return Err(Error::Config(tr!(
            "config.images_file_not_found",
            file = file.display(),
            place = place
        )));
    }
    Ok(file)
}

/// スクリーンショットのファイル一覧（`dir` か `files`。どちらか 1 つ）
fn screenshot_files(dto: ScreenshotsDto, base_dir: &Path, place: &str) -> Result<Vec<PathBuf>> {
    let files = match (dto.dir, dto.files) {
        (Some(dir), None) => {
            if is_blank(Some(&dir)) {
                return Err(Error::Config(tr!(
                    "config.images_path_is_empty",
                    place = place
                )));
            }
            let dir = full_path(&base_dir.join(expand(Some(&dir)).unwrap_or_default()));
            list_screenshots(&dir, place)?
        }
        (None, Some(files)) => files
            .iter()
            .map(|f| single_file(f, base_dir, place))
            .collect::<Result<Vec<_>>>()?,
        _ => {
            return Err(Error::Config(tr!(
                "config.images_specify_dir_or_files",
                place = place
            )));
        }
    };
    if files.is_empty() || files.len() > MAX_SCREENSHOTS {
        return Err(Error::Config(tr!(
            "config.images_screenshot_count",
            place = place,
            count = files.len(),
            max = MAX_SCREENSHOTS
        )));
    }
    Ok(files)
}

/// フォルダー直下の `screenshot-*.png` を名前の昇順で並べる（大文字小文字は区別しない）
fn list_screenshots(dir: &Path, place: &str) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(dir).map_err(|_| {
        Error::Config(tr!(
            "config.images_dir_not_found",
            dir = dir.display(),
            place = place
        ))
    })?;
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::Config(e.to_string()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if lower.starts_with("screenshot-")
            && lower.ends_with(".png")
            && entry.file_type().is_ok_and(|t| t.is_file())
        {
            names.push(name);
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names.into_iter().map(|n| dir.join(n)).collect())
}
