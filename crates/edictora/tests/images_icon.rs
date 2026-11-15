//! ストア アイコン（`images.icon`、128x128）の対応: アルファ込みの画素ダイジェスト・正本の検証・設定の読込と対象の絞り込み。
//!
//! アイコンは全言語共通の 1 枚で、他の画像と違って透過（RGBA）の PNG を許す。
//! そのため画素のダイジェストは RGB だけでなくアルファ込みで求める（`png::pixel_digest_rgba`）。
//! Chrome は開かない（ダッシュボード側の選び方は element_selectors.rs、書き込みの手順は image_service.rs）。

mod common;

use std::path::PathBuf;

use common::{ITEM_A, config_err, png_bytes, rgba_png, test_paths, work_dir, write_png};
use edictora::cli::args::parse;
use edictora::cli::select_targets;
use edictora::config::{self, EdictoraConfig};
use edictora::images::png::{pixel_digest, pixel_digest_rgba};
use edictora::images::{
    ImageScope, ImageSlot, build_expected_slot, digest_for_slot, validate_image_file,
};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "images",
        name,
    ]
    .iter()
    .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// 角丸のアイコンを真似た画素: 四隅が完全に透明、縁が半透明、中は不透明
fn icon_pixels(size: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let corner = (x < 3 || x >= size - 3) && (y < 3 || y >= size - 3);
            let edge = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            let a = if corner {
                0
            } else if edge {
                253
            } else {
                255
            };
            px.extend_from_slice(&[(x * 2) as u8, (y * 2) as u8, 90, a]);
        }
    }
    px
}

// ───────── アルファ込みの画素ダイジェスト ─────────

#[test]
fn アルファ込みのダイジェストは透過のあるpngでも求まる() {
    let png = rgba_png(128, 128, &icon_pixels(128));
    // RGB のダイジェストは透過があると比較不能のまま（小タイル・マーキー・スクリーンショットの扱いは変えない）
    assert!(pixel_digest(&png).unwrap_err().contains("transparent"));
    let digest = pixel_digest_rgba(&png).unwrap();
    assert_eq!(digest.len(), 64);
    assert_eq!(pixel_digest_rgba(&png).unwrap(), digest);
}

#[test]
fn 完全に透明な画素の色は無視し_見える画素の違いは検出する() {
    let base = icon_pixels(8);
    let digest = |px: &[u8]| pixel_digest_rgba(&rgba_png(8, 8, px)).unwrap();
    let want = digest(&base);
    let at = |x: usize, y: usize| (y * 8 + x) * 4;

    // 完全に透明（A=0）の画素の RGB は、再エンコードで変わりうるので比べない
    let mut hidden = base.clone();
    hidden[at(0, 0)..at(0, 0) + 3].copy_from_slice(&[200, 100, 50]);
    assert_eq!(digest(&hidden), want);

    // 半透明の画素の A・RGB、不透明の画素の RGB は比べる
    let mut alpha = base.clone();
    alpha[at(4, 0) + 3] = 252;
    assert_ne!(digest(&alpha), want);
    let mut semi_color = base.clone();
    semi_color[at(4, 0)] ^= 1;
    assert_ne!(digest(&semi_color), want);
    let mut visible = base.clone();
    visible[at(4, 4) + 1] ^= 1;
    assert_ne!(digest(&visible), want);

    // 透明だった画素が見えるようになれば別の画像
    let mut revealed = base.clone();
    revealed[at(0, 0) + 3] = 255;
    assert_ne!(digest(&revealed), want);
}

#[test]
fn 不透明なrgbとrgbaは同じダイジェスト_グレーやインターレースは比較不能() {
    assert_eq!(
        pixel_digest_rgba(&fixture("rgb-avg.png")),
        pixel_digest_rgba(&fixture("rgba-opaque.png"))
    );
    assert!(pixel_digest_rgba(&fixture("rgba-transparent.png")).is_ok());
    assert!(
        pixel_digest_rgba(&fixture("gray.png"))
            .unwrap_err()
            .contains("color type")
    );
    assert!(
        pixel_digest_rgba(&fixture("rgb-interlaced.png"))
            .unwrap_err()
            .contains("interlace")
    );
    assert!(pixel_digest_rgba(b"").is_err());
    assert!(pixel_digest_rgba(b"not a png").is_err());
}

#[test]
fn スロットに合わせたダイジェストを選ぶ() {
    let opaque = png_bytes(128, 128, 2, 8, 4);
    // アイコンはアルファ込み、それ以外は RGB のみ
    assert_eq!(
        digest_for_slot(ImageSlot::Icon, &opaque).unwrap(),
        pixel_digest_rgba(&opaque).unwrap()
    );
    assert_eq!(
        digest_for_slot(ImageSlot::SmallTile, &png_bytes(440, 280, 2, 8, 4)).unwrap(),
        pixel_digest(&png_bytes(440, 280, 2, 8, 4)).unwrap()
    );
    // 透過のある画像は、アイコン以外では比較不能
    let transparent = rgba_png(128, 128, &icon_pixels(128));
    assert!(digest_for_slot(ImageSlot::Icon, &transparent).is_ok());
    assert!(digest_for_slot(ImageSlot::Marquee, &transparent).is_err());
}

// ───────── スロットのモデルと正本の検証 ─────────

#[test]
fn アイコンのスロットは128x128の1枚で全言語共通() {
    let slot = ImageSlot::Icon;
    assert_eq!(slot.config_key(), "icon");
    assert_eq!(slot.allowed_sizes(), &[(128, 128)]);
    assert_eq!(slot.max_count(), 1);
    assert!(slot.allows_alpha() && !slot.is_localized());
    for other in [
        ImageSlot::SmallTile,
        ImageSlot::Marquee,
        ImageSlot::GlobalScreenshots,
        ImageSlot::LocalizedScreenshots,
    ] {
        assert!(!other.allows_alpha(), "{other}");
    }
    // 処理順は先頭（全言語向け → ローカライズ版）
    assert!(ImageSlot::Icon < ImageSlot::SmallTile);
    // 範囲: 全言語向けに含め、ローカライズ版だけの指定からは外す
    assert!(ImageScope::All.includes(slot) && ImageScope::Global.includes(slot));
    assert!(!ImageScope::Localized.includes(slot));
}

#[test]
fn アイコンは128x128のrgbかrgbaを通し_ほかの寸法や色種別は通さない() {
    let dir = work_dir("icon-validate");
    // RGB（色種別 2）・RGBA（色種別 6、透過あり）の 128x128
    let rgb = write_png(&dir, "rgb.png", 128, 128, 2, 8, 1);
    let rgba_path = dir.join("rgba.png");
    std::fs::write(&rgba_path, rgba_png(128, 128, &icon_pixels(128))).unwrap();
    let rgb_file = validate_image_file(ImageSlot::Icon, &rgb, "place").unwrap();
    let rgba_file = validate_image_file(ImageSlot::Icon, &rgba_path, "place").unwrap();
    assert_eq!(rgb_file.pixel_sha256.len(), 64);
    assert_eq!(
        rgba_file.pixel_sha256,
        pixel_digest_rgba(&rgba_png(128, 128, &icon_pixels(128))).unwrap()
    );
    assert_ne!(rgb_file.pixel_sha256, rgba_file.pixel_sha256);

    // 寸法違い（小タイルの寸法・1 画素違い）
    let tile = write_png(&dir, "tile.png", 440, 280, 2, 8, 1);
    let msg = config_err(validate_image_file(ImageSlot::Icon, &tile, "place"));
    assert!(msg.contains("440x280") && msg.contains("128x128"), "{msg}");
    let odd = write_png(&dir, "odd.png", 129, 128, 2, 8, 1);
    assert!(config_err(validate_image_file(ImageSlot::Icon, &odd, "p")).contains("129x128"));

    // 16 ビット・インターレースは通らない
    let deep = write_png(&dir, "deep.png", 128, 128, 2, 16, 1);
    let msg = config_err(validate_image_file(ImageSlot::Icon, &deep, "p"));
    assert!(msg.contains("RGB") && msg.contains("16"), "{msg}");
    let mut bytes = png_bytes(128, 128, 2, 8, 1);
    bytes[28] = 1;
    let interlaced = dir.join("interlaced.png");
    std::fs::write(&interlaced, bytes).unwrap();
    assert!(
        config_err(validate_image_file(ImageSlot::Icon, &interlaced, "p")).contains("デコード")
    );

    // 色種別 0（グレー）は通らない
    let mut gray = png_bytes(128, 128, 2, 8, 1);
    gray[25] = 0;
    let gray_path = dir.join("gray.png");
    std::fs::write(&gray_path, gray).unwrap();
    assert!(config_err(validate_image_file(ImageSlot::Icon, &gray_path, "p")).contains("RGB"));

    // 透過を許すのはアイコンだけ（小タイルの RGBA は従来どおり弾く）
    let rgba_tile = write_png(&dir, "rgba-tile.png", 440, 280, 6, 8, 1);
    assert!(
        config_err(validate_image_file(ImageSlot::SmallTile, &rgba_tile, "p"))
            .contains("24 ビット")
    );
}

#[test]
fn アイコンは1枚だけ() {
    let dir = work_dir("icon-count");
    let a = write_png(&dir, "a.png", 128, 128, 2, 8, 1);
    let b = write_png(&dir, "b.png", 128, 128, 2, 8, 2);
    let slot = build_expected_slot(ImageSlot::Icon, None, std::slice::from_ref(&a), "p").unwrap();
    assert_eq!(
        (slot.slot, slot.locale.clone(), slot.files.len()),
        (ImageSlot::Icon, None, 1)
    );
    assert_eq!(slot.label(), "ショップ アイコン");
    assert!(build_expected_slot(ImageSlot::Icon, None, &[a, b], "p").is_err());
    assert!(build_expected_slot(ImageSlot::Icon, None, &[], "p").is_err());
}

// ───────── 設定（images.icon）と対象の絞り込み ─────────

struct Ctx {
    dir: PathBuf,
}

impl Ctx {
    /// repo/icons/icon-128.png（透過あり RGBA）と、小タイル・スクリーンショット
    fn new(name: &str) -> Ctx {
        let dir = work_dir(name);
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("icons")).unwrap();
        std::fs::write(
            repo.join("icons").join("icon-128.png"),
            rgba_png(128, 128, &icon_pixels(128)),
        )
        .unwrap();
        std::fs::write(
            repo.join("icons").join("icon-48.png"),
            rgba_png(48, 48, &icon_pixels(48)),
        )
        .unwrap();
        std::fs::write(repo.join("listing.txt"), "■ 説明\n本文です\n■ カテゴリ\n").unwrap();
        let images = repo.join("images");
        write_png(&images, "tile-small.png", 440, 280, 2, 8, 1);
        write_png(&images.join("en"), "screenshot-01.png", 640, 400, 2, 8, 2);
        write_png(&images.join("ja"), "screenshot-01.png", 640, 400, 2, 8, 3);
        Ctx { dir }
    }

    fn parse(&self, images: &str) -> edictora::Result<EdictoraConfig> {
        let json = format!(
            r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "name": "テスト", "baseDir": "repo",
                  "locales": {{ "ja": {{ "description": {{ "file": "listing.txt" }} }} }},
                  "images": {images} }} ] }}"#
        );
        config::parse(
            &json,
            &self.dir.join("config.json"),
            None,
            None,
            &test_paths(&self.dir),
        )
    }
}

const WITH_ICON: &str = r#"{
    "icon": "icons/icon-128.png",
    "smallTile": "images/tile-small.png",
    "globalScreenshots": { "dir": "images/en" },
    "localizedScreenshots": { "ja": { "dir": "images/ja" } }
}"#;

fn slots_of(ctx: &Ctx, args: &[&str]) -> Vec<(ImageSlot, Option<String>)> {
    let config = ctx.parse(WITH_ICON).unwrap();
    let items = select_targets(&config, &parse(args).unwrap()).unwrap();
    items[0]
        .images
        .as_ref()
        .map(|i| i.slots.iter().map(|s| (s.slot, s.locale.clone())).collect())
        .unwrap_or_default()
}

#[test]
fn iconを読む_パスは絶対パスになり_書いていなければ対象外() {
    let ctx = Ctx::new("icon-config");
    let config = ctx.parse(WITH_ICON).unwrap();
    let images = config.items[0].images.as_ref().unwrap();
    let icon = images.icon.as_ref().unwrap();
    assert!(icon.is_absolute() && icon.ends_with("icon-128.png"));

    // icon だけでも images として成立する（画像だけのアイテム）
    let only = ctx.parse(r#"{ "icon": "icons/icon-128.png" }"#).unwrap();
    let images = only.items[0].images.as_ref().unwrap();
    assert!(images.icon.is_some() && images.small_tile.is_none());

    // 書いていなければ対象外（アイコンを勝手に足さない）
    let none = ctx
        .parse(r#"{ "smallTile": "images/tile-small.png" }"#)
        .unwrap();
    assert!(none.items[0].images.as_ref().unwrap().icon.is_none());
}

#[test]
fn iconの誤りは設定エラー() {
    let ctx = Ctx::new("icon-config-errors");
    let err = |images: &str| config_err(ctx.parse(images));
    assert!(err(r#"{ "icon": "icons/none.png" }"#).contains("none.png"));
    assert!(err(r#"{ "icon": "" }"#).contains("icon"));
    assert!(err(r#"{ "icon": ["icons/icon-128.png"] }"#).contains("icon"));
    // 空の images は、icon も含めて 1 つ以上を求める
    assert!(err("{}").contains("icon"));
    // 置いたあとの検証（寸法）は対象にしたときに行う。エラー文に設定上の場所（images.icon）が出る
    let config = ctx.parse(r#"{ "icon": "icons/icon-48.png" }"#).unwrap();
    let msg = config_err(select_targets(
        &config,
        &parse(&["diff", "--only", "images"]).unwrap(),
    ));
    assert!(
        msg.contains("images.icon") && msg.contains("48x48") && msg.contains("128x128"),
        "{msg}"
    );
}

#[test]
fn アイコンは処理順の先頭で_image_scopeはglobalに含め_localizedには含めない() {
    let ctx = Ctx::new("icon-scope");
    // 既定（all）: アイコン → 小タイル → 全言語向け → ローカライズ版
    assert_eq!(
        slots_of(&ctx, &["diff", "--only", "images"]),
        vec![
            (ImageSlot::Icon, None),
            (ImageSlot::SmallTile, None),
            (ImageSlot::GlobalScreenshots, None),
            (ImageSlot::LocalizedScreenshots, Some("ja".to_string())),
        ]
    );
    // global: アイコンを含む（全言語共通の欄）
    assert_eq!(
        slots_of(
            &ctx,
            &["diff", "--only", "images", "--image-scope", "global"]
        ),
        vec![
            (ImageSlot::Icon, None),
            (ImageSlot::SmallTile, None),
            (ImageSlot::GlobalScreenshots, None),
        ]
    );
    // localized: アイコンを含まない
    assert_eq!(
        slots_of(
            &ctx,
            &["diff", "--only", "images", "--image-scope", "localized"]
        ),
        vec![(ImageSlot::LocalizedScreenshots, Some("ja".to_string()))]
    );
    // --locale はローカライズ版だけを絞る（アイコンは言語に関係なく対象のまま）
    let by_locale = slots_of(&ctx, &["diff", "--only", "images", "--locale", "ja"]);
    assert_eq!(by_locale[0], (ImageSlot::Icon, None));
    assert_eq!(by_locale.len(), 4);
}

#[test]
fn アイコンの画素のダイジェストはchromeを開く前に計算済み() {
    let ctx = Ctx::new("icon-digest");
    let config = ctx.parse(WITH_ICON).unwrap();
    let items = select_targets(&config, &parse(&["diff", "--only", "images"]).unwrap()).unwrap();
    let icon = &items[0].images.as_ref().unwrap().slots[0];
    assert_eq!(icon.slot, ImageSlot::Icon);
    assert_eq!(
        icon.files[0].pixel_sha256,
        pixel_digest_rgba(&rgba_png(128, 128, &icon_pixels(128))).unwrap()
    );
}
