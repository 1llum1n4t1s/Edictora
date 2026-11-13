//! 設定ファイル（config.json）の読込と検証（C# の `ConfigLoader` / `EdictoraConfig` / `ConfigFileModels`）。
//!
//! ブラウザを開く前に、設定の誤りをすべてここで検出して止める（途中まで書き換えてから設定ミスに気づく事態を避ける）。
//! 未知のキーはエラーにする。綴り間違いの欄が黙って「対象外」になり、更新漏れに気づけなくなるのを防ぐため。
//!
//! JSON の読み方は C# 版（System.Text.Json）に合わせる:
//! - キー名は camelCase・大文字小文字を区別する
//! - コメント（`//`・`/* */`）と末尾カンマを許す（人が手で書く設定なので）
//! - `locales` は書いた順を保つ（push の処理順と出力の順番が変わらないように）
//! - 「キーがない（null）」と「空文字」を区別する。URL 欄はキーなし＝対象外、`""`＝空欄が正本

pub mod section;

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use crate::app_paths::AppPaths;
use crate::error::{Error, Result};
use crate::listing::ListingField;
use crate::util::{
    expand_env, is_blank, parse_absolute_url, read_text_file,
    strip_json_comments_and_trailing_commas,
};

// ───────── 検証済みの設定 ─────────

/// 検証済みの設定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdictoraConfig {
    /// 読み込んだ設定ファイルの絶対パス（存在しない場合は想定パス）
    pub source_path: PathBuf,
    /// Chrome プロファイルの絶対パス
    pub profile_directory: PathBuf,
    /// chrome.exe の指定（None なら自動検出）
    pub chrome_path: Option<PathBuf>,
    /// 対象アイテム
    pub items: Vec<ItemConfig>,
}

impl EdictoraConfig {
    /// アイテム ID で検索する（大文字小文字は区別しない）
    pub fn find_item(&self, item_id: &str) -> Option<&ItemConfig> {
        self.items
            .iter()
            .find(|i| i.item_id.eq_ignore_ascii_case(item_id))
    }
}

/// アイテム 1 件の設定（検証済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemConfig {
    /// 32 文字のアイテム ID（小文字）
    pub item_id: String,
    /// 表示名（未指定ならアイテム ID）
    pub display_name: String,
    /// 相対パスの基準（絶対パス）
    pub base_directory: PathBuf,
    /// ロケールごとの正本（設定ファイルに書いた順）
    pub locales: Vec<LocaleConfig>,
}

/// ロケール 1 件の正本（検証済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleConfig {
    /// ロケールコード（例: ja, en, pt-BR）。区切りは「-」に正規化済み
    pub code: String,
    /// 説明欄の取り出し元（None なら説明欄は対象外）
    pub description: Option<TextSource>,
    /// URL 欄の正本（キーがない欄は対象外。空文字は「空欄が正本」）
    pub urls: BTreeMap<ListingField, String>,
}

impl LocaleConfig {
    /// このロケールで対象にする欄（表示順）
    pub fn fields(&self) -> Vec<ListingField> {
        ListingField::ALL
            .into_iter()
            .filter(|f| {
                if *f == ListingField::Description {
                    self.description.is_some()
                } else {
                    self.urls.contains_key(f)
                }
            })
            .collect()
    }
}

/// 説明文の取り出し元（検証済み）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSource {
    /// ファイルの絶対パス
    pub file_path: PathBuf,
    /// 開始見出し行の正規表現（None ならファイル全体）
    pub section_start: Option<String>,
    /// 終端見出し行の正規表現（None ならファイル全体）
    pub section_end: Option<String>,
}

// ───────── 設定ファイルをそのまま写した入力用の型 ─────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ConfigFileDto {
    /// Chrome プロファイルの場所（任意。相対パスは設定ファイルの場所が基準）
    profile_dir: Option<String>,
    /// chrome.exe の場所（任意。省略時は自動検出）
    chrome_path: Option<String>,
    /// 対象アイテムの一覧
    items: Option<Vec<ItemConfigDto>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ItemConfigDto {
    /// CWS のアイテム ID（32 文字）
    item_id: Option<String>,
    /// 表示名（任意）
    name: Option<String>,
    /// 相対パスの基準（任意。相対なら設定ファイルの場所が基準）
    base_dir: Option<String>,
    /// ロケールコード → そのロケールの正本（書いた順を保つ）
    locales: Option<OrderedLocales>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LocaleConfigDto {
    description: Option<TextSourceDto>,
    homepage_url: Option<String>,
    support_url: Option<String>,
    official_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TextSourceDto {
    /// ファイルのパス（相対なら baseDir が基準）
    file: Option<String>,
    /// 区間の開始見出し行の正規表現（任意。sectionEnd と組で指定）
    section_start: Option<String>,
    /// 区間の終端見出し行の正規表現（任意。sectionStart と組で指定）
    section_end: Option<String>,
}

/// 書いた順を保つ `locales`（serde_json の Map は既定でキー順に並べ替えるため、自前で読む）
#[derive(Debug)]
struct OrderedLocales(Vec<(String, Option<LocaleConfigDto>)>);

impl<'de> Deserialize<'de> for OrderedLocales {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OrderedLocales;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("ロケールコードをキーとするオブジェクト")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Option<LocaleConfigDto>>()? {
                    entries.push((k, v));
                }
                Ok(OrderedLocales(entries))
            }
        }
        deserializer.deserialize_map(V)
    }
}

// ───────── 読込 ─────────

/// 設定を読み込む。
///
/// - `config_path`: 設定ファイルのパス（None なら既定の `%APPDATA%\Edictora\config.json`）
/// - `profile_override`: CLI の `--profile` 指定（設定ファイルより優先。相対パスはカレントディレクトリ基準）
/// - `chrome_override`: CLI の `--chrome` 指定（設定ファイルより優先）
/// - `require_file`: true なら設定ファイルが無いときにエラー。false なら空の設定で続行（login 用）
pub fn load(
    config_path: Option<&str>,
    profile_override: Option<&str>,
    chrome_override: Option<&str>,
    require_file: bool,
    paths: &AppPaths,
) -> Result<EdictoraConfig> {
    let raw = expand(config_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.default_config_path());
    let path = full_path(&raw);
    if !path.is_file() {
        if require_file {
            return Err(Error::Config(format!(
                "設定ファイルがありません: {}（README の設定例を参考に作成してください）",
                path.display()
            )));
        }
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        return Ok(EdictoraConfig {
            profile_directory: resolve_profile(profile_override, None, &dir, paths),
            chrome_path: expand(chrome_override).map(PathBuf::from),
            source_path: path,
            items: Vec::new(),
        });
    }

    let json = read_text_file(&path).map_err(|e| {
        Error::Config(format!(
            "設定ファイルを読めません: {}（{e}）",
            path.display()
        ))
    })?;
    parse(&json, &path, profile_override, chrome_override, paths)
}

/// JSON 文字列から設定を作る（テスト用に公開）
pub fn parse(
    json: &str,
    config_path: &Path,
    profile_override: Option<&str>,
    chrome_override: Option<&str>,
    paths: &AppPaths,
) -> Result<EdictoraConfig> {
    let cleaned = strip_json_comments_and_trailing_commas(json);
    let dto: Option<ConfigFileDto> = serde_json::from_str(&cleaned).map_err(|e| {
        Error::Config(format!(
            "設定ファイルの形式が正しくありません: {}（{e}）",
            config_path.display()
        ))
    })?;
    let Some(dto) = dto else {
        return Err(Error::Config(format!(
            "設定ファイルが空です: {}",
            config_path.display()
        )));
    };

    let full = full_path(config_path);
    let config_dir = full.parent().map(Path::to_path_buf).unwrap_or_default();
    let profile = resolve_profile(
        profile_override,
        dto.profile_dir.as_deref(),
        &config_dir,
        paths,
    );
    let chrome = expand(chrome_override)
        .or_else(|| expand(dto.chrome_path.as_deref()))
        .map(PathBuf::from);

    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for (i, item_dto) in dto.items.unwrap_or_default().into_iter().enumerate() {
        let index = i + 1;
        let item = parse_item(item_dto, index, &config_dir)?;
        if !seen.insert(item.item_id.clone()) {
            return Err(Error::Config(format!(
                "items[{index}]: アイテム ID が重複しています: {}",
                item.item_id
            )));
        }
        items.push(item);
    }

    Ok(EdictoraConfig {
        source_path: full,
        profile_directory: profile,
        chrome_path: chrome,
        items,
    })
}

/// アイテム ID の形式か（CWS の ID は a〜p の 32 文字）
pub fn is_valid_item_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// ロケールコードを比較用に正規化する（「_」を「-」に寄せる。大文字小文字はそのまま）
pub fn normalize_locale_code(code: &str) -> String {
    code.trim().replace('_', "-")
}

/// ロケールコードの形式か（例: ja, en, pt-BR, es-419, zh-CN）
pub fn is_valid_locale_code(value: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$").expect("固定の正規表現"))
        .is_match(value)
}

fn parse_item(dto: ItemConfigDto, index: usize, config_dir: &Path) -> Result<ItemConfig> {
    let mut place = format!("items[{index}]");
    let item_id = dto.item_id.as_deref().map(|s| s.trim().to_lowercase());
    let item_id = match item_id {
        Some(id) if is_valid_item_id(&id) => id,
        _ => {
            return Err(Error::Config(format!(
                "{place}: itemId は 32 文字の CWS アイテム ID（a〜p）で指定してください: {}",
                dto.item_id.as_deref().unwrap_or("(未指定)")
            )));
        }
    };
    place = format!("{place} ({item_id})");

    let base_dir = if is_blank(dto.base_dir.as_deref()) {
        config_dir.to_path_buf()
    } else {
        full_path(&config_dir.join(expand(dto.base_dir.as_deref()).unwrap_or_default()))
    };
    if !base_dir.is_dir() {
        return Err(Error::Config(format!(
            "{place}: baseDir がありません: {}",
            base_dir.display()
        )));
    }

    let entries = match dto.locales {
        Some(OrderedLocales(entries)) if !entries.is_empty() => entries,
        _ => {
            return Err(Error::Config(format!(
                "{place}: locales を 1 つ以上指定してください"
            )));
        }
    };

    let mut locales = Vec::new();
    let mut seen_codes: HashSet<String> = HashSet::new();
    for (raw_code, locale_dto) in entries {
        let code = normalize_locale_code(&raw_code);
        if !is_valid_locale_code(&code) {
            return Err(Error::Config(format!(
                "{place}: ロケールコードが正しくありません: {raw_code}"
            )));
        }
        if !seen_codes.insert(code.to_uppercase()) {
            return Err(Error::Config(format!(
                "{place}: ロケールコードが重複しています: {raw_code}"
            )));
        }
        let Some(locale_dto) = locale_dto else {
            return Err(Error::Config(format!("{place}.{code}: 内容が空です")));
        };
        locales.push(parse_locale(
            locale_dto,
            &code,
            &format!("{place}.{code}"),
            &base_dir,
        )?);
    }

    let display_name = match dto.name.as_deref() {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => item_id.clone(),
    };
    Ok(ItemConfig {
        item_id,
        display_name,
        base_directory: base_dir,
        locales,
    })
}

fn parse_locale(
    dto: LocaleConfigDto,
    code: &str,
    place: &str,
    base_dir: &Path,
) -> Result<LocaleConfig> {
    let mut description = None;
    if let Some(d) = dto.description {
        if is_blank(d.file.as_deref()) {
            return Err(Error::Config(format!(
                "{place}.description: file を指定してください"
            )));
        }
        if d.section_start.is_none() != d.section_end.is_none() {
            return Err(Error::Config(format!(
                "{place}.description: sectionStart と sectionEnd は両方指定してください"
            )));
        }
        if let Some(p) = &d.section_start {
            section::validate_pattern(p, &format!("{place}.description.sectionStart"))?;
        }
        if let Some(p) = &d.section_end {
            section::validate_pattern(p, &format!("{place}.description.sectionEnd"))?;
        }
        let file = full_path(&base_dir.join(expand(d.file.as_deref()).unwrap_or_default()));
        if !file.is_file() {
            return Err(Error::Config(format!(
                "{place}.description: 正本ファイルがありません: {}",
                file.display()
            )));
        }
        description = Some(TextSource {
            file_path: file,
            section_start: d.section_start,
            section_end: d.section_end,
        });
    }

    let mut urls = BTreeMap::new();
    add_url(
        &mut urls,
        ListingField::OfficialUrl,
        dto.official_url.as_deref(),
        place,
    )?;
    add_url(
        &mut urls,
        ListingField::HomepageUrl,
        dto.homepage_url.as_deref(),
        place,
    )?;
    add_url(
        &mut urls,
        ListingField::SupportUrl,
        dto.support_url.as_deref(),
        place,
    )?;

    if description.is_none() && urls.is_empty() {
        return Err(Error::Config(format!(
            "{place}: description・homepageUrl・supportUrl・officialUrl のいずれかを指定してください"
        )));
    }

    Ok(LocaleConfig {
        code: code.to_string(),
        description,
        urls,
    })
}

fn add_url(
    urls: &mut BTreeMap<ListingField, String>,
    field: ListingField,
    value: Option<&str>,
    place: &str,
) -> Result<()> {
    // キーなし（null）は対象外。空文字は「空欄が正本」として扱う。
    let Some(value) = value else { return Ok(()) };
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        let ok = parse_absolute_url(trimmed)
            .is_some_and(|u| (u.scheme == "https" || u.scheme == "http") && !u.host.is_empty());
        if !ok {
            return Err(Error::Config(format!(
                "{place}.{}: http(s) の絶対 URL を指定してください: {value}",
                field.config_key()
            )));
        }
    }
    urls.insert(field, trimmed.to_string());
    Ok(())
}

fn resolve_profile(
    profile_override: Option<&str>,
    configured: Option<&str>,
    config_dir: &Path,
    paths: &AppPaths,
) -> PathBuf {
    // 優先順位: --profile（カレント基準） > 設定ファイル（設定ファイルの場所基準） > 既定
    if let Some(p) = expand(profile_override) {
        return full_path(Path::new(&p));
    }
    if let Some(c) = expand(configured) {
        return full_path(&config_dir.join(c));
    }
    paths.default_profile_directory()
}

/// %LOCALAPPDATA% などの環境変数を展開する（空白だけなら None）
fn expand(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) if !v.trim().is_empty() => Some(expand_env(v.trim())),
        _ => None,
    }
}

/// 絶対パスにする（C# の `Path.GetFullPath`。`..` と `.` も畳む）
pub fn full_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
