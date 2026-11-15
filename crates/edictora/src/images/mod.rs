//! 掲載画像（ストア アイコン・全言語向けスクリーンショット・ローカライズ版スクリーンショット・小タイル・マーキー）のモデル・検証・比較。
//!
//! 画面操作は持たない（`dashboard` の `ImagePage` が担う）。ここは「正本の画像は何か」「現在の画像と一致するか」を
//! 決める純粋な部分で、ブラウザなしでテストできる。
//!
//! 設計の要点:
//! - 比較は「画素の SHA-256」の順序付き完全一致（`png::pixel_digest`）。ファイルのバイト列では比べない。
//!   ダッシュボードはアップロードした PNG を再エンコードして保存する（`=s0` の原本もバイト列もサイズも変わる）が、
//!   画素は同じなので、デコードした画素で比べる（2026-11-15 に SearchClock の実画面で確認）。
//!   現在値はダッシュボードの画像 URL に `=s0` を付けて取得する（取得は `fetch`）。
//!   取得・デコードできなければ「比較不能」で、一致扱いにも常に置換にもしない。
//! - 正本は Chrome を開く前に検証する（存在・PNG・8bit/RGB（アルファなし）・寸法・同一セット内の重複なし）。
//!   v1 は PNG だけ（ダッシュボードは JPEG も受け付けるが、検証を PNG に絞って取り違えを減らす）。
//! - ストア アイコン（128x128）も 1 枚のスロットとして扱う（全言語共通）。アイコンだけは透過（RGBA）を許し、
//!   比較はアルファ込みの画素ダイジェスト（`png::pixel_digest_rgba`）で行う。並べ替え・プロモーション動画は対象外。

pub mod fetch;
pub mod png;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::listing::ComparisonStatus;
use crate::{tr, trs};

/// スクリーンショットの最大枚数
pub const MAX_SCREENSHOTS: usize = 5;

/// 画像の置き場（スロット）。並び順（Ord）は処理・表示の順番と同じ（全言語向け → ローカライズ版）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImageSlot {
    /// ストア アイコン 128x128、1 枚（全言語共通。透過 PNG を許す唯一のスロット）
    Icon,
    /// プロモーション タイル（小）440x280、1 枚
    SmallTile,
    /// マーキー プロモーション タイル 1400x560、1 枚
    Marquee,
    /// 全言語向けのスクリーンショット（1〜5 枚）
    GlobalScreenshots,
    /// ローカライズ版のスクリーンショット（言語ごと、1〜5 枚）
    LocalizedScreenshots,
}

impl ImageSlot {
    /// 設定ファイル上のキー名
    pub fn config_key(self) -> &'static str {
        match self {
            ImageSlot::Icon => "icon",
            ImageSlot::SmallTile => "smallTile",
            ImageSlot::Marquee => "marquee",
            ImageSlot::GlobalScreenshots => "globalScreenshots",
            ImageSlot::LocalizedScreenshots => "localizedScreenshots",
        }
    }

    /// 画面・CLI に出す名前（表示言語に合わせる）
    pub fn display_name(self) -> &'static str {
        match self {
            ImageSlot::Icon => trs!("images.icon"),
            ImageSlot::SmallTile => trs!("images.small_tile"),
            ImageSlot::Marquee => trs!("images.marquee"),
            ImageSlot::GlobalScreenshots => trs!("images.global_screenshots"),
            ImageSlot::LocalizedScreenshots => trs!("images.localized_screenshots"),
        }
    }

    /// 許される寸法（幅 x 高さ）
    pub fn allowed_sizes(self) -> &'static [(u32, u32)] {
        match self {
            ImageSlot::Icon => &[(128, 128)],
            ImageSlot::SmallTile => &[(440, 280)],
            ImageSlot::Marquee => &[(1400, 560)],
            ImageSlot::GlobalScreenshots | ImageSlot::LocalizedScreenshots => {
                &[(1280, 800), (640, 400)]
            }
        }
    }

    /// 置ける最大枚数
    pub fn max_count(self) -> usize {
        match self {
            ImageSlot::Icon | ImageSlot::SmallTile | ImageSlot::Marquee => 1,
            ImageSlot::GlobalScreenshots | ImageSlot::LocalizedScreenshots => MAX_SCREENSHOTS,
        }
    }

    /// 透過（アルファ）を持つ PNG を許すか。ストア アイコンだけが許す
    /// （角丸・余白を透過で表すのが普通で、ダッシュボードの規格も 128x128 の透過 PNG を受け付ける。
    /// 小タイル・マーキー・スクリーンショットは 24 ビット（アルファなし）の規格）。
    /// 許すスロットはアルファ込みの画素ダイジェスト（`png::pixel_digest_rgba`）で比べる
    pub fn allows_alpha(self) -> bool {
        self == ImageSlot::Icon
    }

    /// ローカライズ版か（言語の切替が要る）
    pub fn is_localized(self) -> bool {
        self == ImageSlot::LocalizedScreenshots
    }
}

/// どのスロットを対象にするか（`--image-scope`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageScope {
    /// 全言語向けとローカライズ版の両方（既定）
    #[default]
    All,
    /// 全言語向け（アイコン・小タイル・マーキー・全言語向けスクリーンショット）だけ
    Global,
    /// ローカライズ版スクリーンショットだけ
    Localized,
}

impl ImageScope {
    /// 引数の値から作る（大文字小文字は区別しない）
    pub fn parse(value: &str) -> Option<ImageScope> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Some(ImageScope::All),
            "global" => Some(ImageScope::Global),
            "localized" => Some(ImageScope::Localized),
            _ => None,
        }
    }

    /// 引数での名前
    pub fn name(self) -> &'static str {
        match self {
            ImageScope::All => "all",
            ImageScope::Global => "global",
            ImageScope::Localized => "localized",
        }
    }

    /// このスロットを含むか
    pub fn includes(self, slot: ImageSlot) -> bool {
        match self {
            ImageScope::All => true,
            ImageScope::Global => !slot.is_localized(),
            ImageScope::Localized => slot.is_localized(),
        }
    }
}

/// 正本の画像ファイル 1 枚
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedImageFile {
    /// ファイルの絶対パス
    pub path: PathBuf,
    /// 画素のダイジェスト（`png::pixel_digest`。大文字の 16 進 SHA-256）
    pub pixel_sha256: String,
}

/// 正本のスロット 1 つ（並びが画面での並び）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedSlot {
    pub slot: ImageSlot,
    /// ローカライズ版のときのロケールコード（全言語向けは None）
    pub locale: Option<String>,
    pub files: Vec<ExpectedImageFile>,
}

impl ExpectedSlot {
    /// 表示用の名前（ローカライズ版は「[ja] …」）
    pub fn label(&self) -> String {
        slot_label(self.slot, self.locale.as_deref())
    }

    /// 正本のハッシュ列
    pub fn hashes(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.pixel_sha256.as_str()).collect()
    }
}

/// スロットの表示用の名前
pub fn slot_label(slot: ImageSlot, locale: Option<&str>) -> String {
    match locale {
        Some(code) => format!("[{code}] {}", slot.display_name()),
        None => slot.display_name().to_string(),
    }
}

/// アイテム 1 件の画像の正本（設定に書いたスロットだけ。処理順に並ぶ）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpectedImages {
    pub slots: Vec<ExpectedSlot>,
}

impl ExpectedImages {
    /// 範囲とロケールで絞る。`locales` が空ならロケールでは絞らない。
    /// ロケールの指定は、ローカライズ版スクリーンショットだけに効く（全言語向けはロケールを持たない）
    pub fn filtered(&self, scope: ImageScope, locales: &[String]) -> ExpectedImages {
        ExpectedImages {
            slots: self
                .slots
                .iter()
                .filter(|s| scope.includes(s.slot))
                .filter(|s| match (&s.locale, locales.is_empty()) {
                    (Some(code), false) => locales.iter().any(|l| crate::util::eq_ic(l, code)),
                    _ => true,
                })
                .cloned()
                .collect(),
        }
    }

    /// 対象のスロットが無いか
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// 全言語向けのスロット（処理順）
    pub fn global_slots(&self) -> Vec<&ExpectedSlot> {
        self.slots.iter().filter(|s| s.locale.is_none()).collect()
    }

    /// ローカライズ版のロケール（設定に書いた順・重複なし）
    pub fn localized_locales(&self) -> Vec<&str> {
        let mut codes: Vec<&str> = Vec::new();
        for s in &self.slots {
            if let Some(code) = &s.locale
                && !codes.iter().any(|c| crate::util::eq_ic(c, code))
            {
                codes.push(code);
            }
        }
        codes
    }
}

// ───────── 正本の検証 ─────────

/// PNG のシグネチャ
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// PNG のヘッダーから読み取った情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    /// インターレース方式（0 = なし）
    pub interlace: u8,
}

/// バイト列の先頭から PNG のヘッダー（IHDR）を読む。PNG でなければ None
pub fn parse_png_header(bytes: &[u8]) -> Option<PngInfo> {
    // シグネチャ 8 + チャンク長 4 + "IHDR" 4 + 幅 4 + 高さ 4 + 深さ 1 + 色種別 1 + 圧縮 1 + フィルター 1 + インターレース 1
    if bytes.len() < 29 || bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    if u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) != 13 {
        return None;
    }
    Some(PngInfo {
        width: u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        height: u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
        bit_depth: bytes[24],
        color_type: bytes[25],
        interlace: bytes[28],
    })
}

/// スロットに合わせた画素ダイジェスト（正本のファイルとダッシュボードの原本の両方で同じ関数を使う）。
/// アイコンはアルファ込み（`png::pixel_digest_rgba`）、それ以外は RGB（`png::pixel_digest`）
pub fn digest_for_slot(slot: ImageSlot, bytes: &[u8]) -> std::result::Result<String, String> {
    if slot.allows_alpha() {
        png::pixel_digest_rgba(bytes)
    } else {
        png::pixel_digest(bytes)
    }
}

/// 画像ファイル 1 枚を検証し、ハッシュ付きで返す（誤りは `Config` エラー）。
/// `place` はエラー文に出す設定上の場所
pub fn validate_image_file(slot: ImageSlot, path: &Path, place: &str) -> Result<ExpectedImageFile> {
    let shown = path.display();
    let read_error = |e: &dyn fmt::Display| {
        Error::Config(tr!(
            "images.cannot_read_the_image_file",
            place = place,
            path = shown,
            error = e
        ))
    };
    let size = std::fs::metadata(path).map_err(|e| read_error(&e))?.len();
    if size > fetch::MAX_IMAGE_BYTES as u64 {
        return Err(Error::Config(tr!(
            "images.the_image_file_is_too_large",
            place = place,
            path = shown,
            max_mb = fetch::MAX_IMAGE_BYTES / (1024 * 1024)
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| read_error(&e))?;
    let Some(info) = parse_png_header(&bytes) else {
        return Err(Error::Config(tr!(
            "images.the_image_is_not_a_png",
            place = place,
            path = shown
        )));
    };
    // 8 ビットの PNG だけ。ダッシュボードの規格と同じく、小タイル・マーキー・スクリーンショットは
    // 24 ビット（アルファなし。色種別 2 = RGB）。アイコンだけは透過を許す（色種別 6 = RGBA も可）
    if slot.allows_alpha() {
        if info.bit_depth != 8 || !matches!(info.color_type, 2 | 6) {
            return Err(Error::Config(tr!(
                "images.the_icon_png_must_be_8_bit_rgb_or_rgba",
                place = place,
                path = shown,
                bit_depth = info.bit_depth,
                color_type = info.color_type
            )));
        }
    } else if info.bit_depth != 8 || info.color_type != 2 {
        return Err(Error::Config(tr!(
            "images.the_png_must_be_24_bit_without_alpha",
            place = place,
            path = shown,
            bit_depth = info.bit_depth,
            color_type = info.color_type
        )));
    }
    let sizes = slot.allowed_sizes();
    if !sizes.contains(&(info.width, info.height)) {
        let allowed: Vec<String> = sizes.iter().map(|(w, h)| format!("{w}x{h}")).collect();
        return Err(Error::Config(tr!(
            "images.the_image_size_is_not_allowed",
            place = place,
            path = shown,
            width = info.width,
            height = info.height,
            allowed = allowed.join(" / ")
        )));
    }
    // 画素のダイジェスト。インターレースありなど、デコードできない PNG はここで弾く（比較できないため）
    let pixel_sha256 = digest_for_slot(slot, &bytes).map_err(|e| {
        Error::Config(tr!(
            "images.the_image_cannot_be_decoded",
            place = place,
            path = shown,
            error = e
        ))
    })?;
    Ok(ExpectedImageFile {
        path: path.to_path_buf(),
        pixel_sha256,
    })
}

/// スロット 1 つ分のファイルを検証する。1 枚以上・最大枚数以下、同一セット内で内容が重複しないこと
pub fn build_expected_slot(
    slot: ImageSlot,
    locale: Option<&str>,
    paths: &[PathBuf],
    place: &str,
) -> Result<ExpectedSlot> {
    if paths.is_empty() || paths.len() > slot.max_count() {
        return Err(Error::Config(tr!(
            "images.the_number_of_images_is_out_of_range",
            place = place,
            count = paths.len(),
            max = slot.max_count()
        )));
    }
    let mut files: Vec<ExpectedImageFile> = Vec::new();
    for path in paths {
        let file = validate_image_file(slot, path, place)?;
        // 同じ内容の画像を 1 セットに 2 回入れる誤り（コピーミス）を止める。
        // 比較は順序付きのハッシュ列なので、重複があっても動くが、意図した並びではない可能性が高い
        if let Some(dup) = files.iter().find(|f| f.pixel_sha256 == file.pixel_sha256) {
            return Err(Error::Config(tr!(
                "images.duplicate_image_content_in_one_set",
                place = place,
                first = dup.path.display(),
                second = file.path.display()
            )));
        }
        files.push(file);
    }
    Ok(ExpectedSlot {
        slot,
        locale: locale.map(str::to_string),
        files,
    })
}

// ───────── 現在値と比較 ─────────

/// ダッシュボードの画像 1 枚の現在値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentImage {
    /// 画像の alt（「スクリーンショット 1」など）
    pub alt: String,
    /// 原本の画素のダイジェスト（大文字の 16 進 SHA-256）。取得・デコードできなければ理由（比較不能）
    pub pixel_sha256: std::result::Result<String, String>,
}

/// スロット 1 つの比較結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageComparison {
    pub item_id: String,
    pub item_name: String,
    pub slot: ImageSlot,
    pub locale: Option<String>,
    pub status: ComparisonStatus,
    pub expected_count: usize,
    pub current_count: usize,
    /// 内容が異なる位置（1 始まり。枚数が違うときは共通部分だけ）
    pub differing_positions: Vec<usize>,
    /// 失敗の理由（`status` が Error のとき）
    pub error: Option<String>,
}

impl ImageComparison {
    /// 表示用の名前
    pub fn label(&self) -> String {
        slot_label(self.slot, self.locale.as_deref())
    }
}

/// 現在値と正本を比べる。ハッシュを 1 つでも取得できなければ、判定せず `Error`（比較不能）にする
pub fn compare_slot(
    item_id: &str,
    item_name: &str,
    expected: &ExpectedSlot,
    current: &[CurrentImage],
) -> ImageComparison {
    let mut result = ImageComparison {
        item_id: item_id.to_string(),
        item_name: item_name.to_string(),
        slot: expected.slot,
        locale: expected.locale.clone(),
        status: ComparisonStatus::Match,
        expected_count: expected.files.len(),
        current_count: current.len(),
        differing_positions: Vec::new(),
        error: None,
    };
    if let Some(reason) = current.iter().find_map(|c| c.pixel_sha256.as_ref().err()) {
        result.status = ComparisonStatus::Error;
        result.error = Some(reason.clone());
        return result;
    }
    for (i, (want, got)) in expected.files.iter().zip(current).enumerate() {
        if got.pixel_sha256.as_deref() != Ok(want.pixel_sha256.as_str()) {
            result.differing_positions.push(i + 1);
        }
    }
    if expected.files.len() != current.len() || !result.differing_positions.is_empty() {
        result.status = ComparisonStatus::Different;
    }
    result
}

/// 比較結果の差分の説明（1 行。表示言語に合わせる）
pub fn describe(row: &ImageComparison) -> String {
    let label = row.label();
    match row.status {
        ComparisonStatus::Match => tr!("images.row_match", label = label),
        ComparisonStatus::Error => tr!(
            "images.row_error",
            label = label,
            error = row.error.as_deref().unwrap_or("")
        ),
        ComparisonStatus::NotFetched => tr!("images.row_not_fetched", label = label),
        ComparisonStatus::Different => {
            let positions: Vec<String> = row
                .differing_positions
                .iter()
                .map(ToString::to_string)
                .collect();
            tr!(
                "images.row_different",
                label = label,
                current = row.current_count,
                expected = row.expected_count,
                positions = if positions.is_empty() {
                    "-".to_string()
                } else {
                    positions.join(",")
                }
            )
        }
    }
}

impl fmt::Display for ImageSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.config_key())
    }
}
