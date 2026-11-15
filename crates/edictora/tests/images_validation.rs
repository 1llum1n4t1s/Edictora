//! 画像の正本の検証（`images::validate_image_file` / `build_expected_slot`）と比較（`compare_slot`）。
//! Chrome を開く前に、設定の誤りを終了コード 2 になるエラーで止めるための検査を固める。

mod common;

use common::{config_err, png_bytes, work_dir, write_png};
use edictora::images::png::pixel_digest;
use edictora::images::{
    CurrentImage, ExpectedImageFile, ExpectedSlot, ImageSlot, build_expected_slot, compare_slot,
    describe, parse_png_header, validate_image_file,
};
use edictora::listing::ComparisonStatus;

#[test]
fn ヘッダーから寸法と色種別を読む() {
    let info = parse_png_header(&png_bytes(440, 280, 2, 8, 0)).unwrap();
    assert_eq!(
        (
            info.width,
            info.height,
            info.bit_depth,
            info.color_type,
            info.interlace
        ),
        (440, 280, 8, 2, 0)
    );
    assert!(parse_png_header(b"GIF89a").is_none());
    assert!(parse_png_header(&[0u8; 40]).is_none());
}

#[test]
fn 規格どおりの画像は画素のダイジェストつきで通る() {
    let dir = work_dir("images-valid");
    let path = write_png(&dir, "tile.png", 440, 280, 2, 8, 3);
    let file = validate_image_file(ImageSlot::SmallTile, &path, "place").unwrap();
    assert_eq!(file.path, path);
    assert_eq!(
        file.pixel_sha256,
        pixel_digest(&png_bytes(440, 280, 2, 8, 3)).unwrap()
    );
}

#[test]
fn スロットごとの寸法を確かめる() {
    let dir = work_dir("images-sizes");
    for (slot, w, h) in [
        (ImageSlot::SmallTile, 440, 280),
        (ImageSlot::Marquee, 1400, 560),
        (ImageSlot::GlobalScreenshots, 1280, 800),
        (ImageSlot::GlobalScreenshots, 640, 400),
        (ImageSlot::LocalizedScreenshots, 1280, 800),
    ] {
        let path = write_png(&dir, &format!("{slot}-{w}.png"), w, h, 2, 8, 1);
        validate_image_file(slot, &path, "place").unwrap_or_else(|e| panic!("{slot} {w}x{h}: {e}"));
    }
    // 別のスロットの寸法は通らない
    let tile = write_png(&dir, "tile-as-marquee.png", 440, 280, 2, 8, 1);
    let msg = config_err(validate_image_file(ImageSlot::Marquee, &tile, "place"));
    assert!(msg.contains("440x280") && msg.contains("1400x560"), "{msg}");
    let shot = write_png(&dir, "shot-odd.png", 1281, 800, 2, 8, 1);
    assert!(
        config_err(validate_image_file(
            ImageSlot::GlobalScreenshots,
            &shot,
            "p"
        ))
        .contains("1281x800")
    );
}

#[test]
fn pngでない_アルファあり_16ビット_インターレースは通らない() {
    let dir = work_dir("images-invalid");
    let jpeg = dir.join("a.png");
    std::fs::write(
        &jpeg,
        [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    )
    .unwrap();
    assert!(config_err(validate_image_file(ImageSlot::SmallTile, &jpeg, "p")).contains("PNG"));

    let rgba = write_png(&dir, "rgba.png", 440, 280, 6, 8, 1);
    assert!(
        config_err(validate_image_file(ImageSlot::SmallTile, &rgba, "p")).contains("24 ビット")
    );

    let deep = write_png(&dir, "deep.png", 440, 280, 2, 16, 1);
    assert!(
        config_err(validate_image_file(ImageSlot::SmallTile, &deep, "p")).contains("24 ビット")
    );

    // インターレースあり（IHDR の最後の 1 バイト）。画素をデコードできないので使えない
    let mut bytes = png_bytes(440, 280, 2, 8, 1);
    bytes[28] = 1;
    let interlaced = dir.join("interlaced.png");
    std::fs::write(&interlaced, bytes).unwrap();
    assert!(
        config_err(validate_image_file(ImageSlot::SmallTile, &interlaced, "p"))
            .contains("デコード")
    );

    let missing = dir.join("none.png");
    assert!(
        config_err(validate_image_file(ImageSlot::SmallTile, &missing, "p")).contains("none.png")
    );
}

#[test]
fn スクリーンショットは1から5枚で_同じ内容の重複は通らない() {
    let dir = work_dir("images-set");
    let shot = |name: &str, seed: u8| write_png(&dir, name, 640, 400, 2, 8, seed);
    let paths: Vec<_> = (1..=5).map(|i| shot(&format!("s{i}.png"), i)).collect();

    let slot = build_expected_slot(ImageSlot::GlobalScreenshots, None, &paths, "p").unwrap();
    assert_eq!(slot.files.len(), 5);
    assert_eq!(slot.locale, None);

    let six: Vec<_> = paths.iter().cloned().chain([shot("s6.png", 6)]).collect();
    assert!(
        config_err(build_expected_slot(
            ImageSlot::GlobalScreenshots,
            None,
            &six,
            "p"
        ))
        .contains("6")
    );
    assert!(build_expected_slot(ImageSlot::GlobalScreenshots, None, &six, "p").is_err());
    assert!(build_expected_slot(ImageSlot::GlobalScreenshots, None, &[], "p").is_err());

    // 同じ内容（ファイル名は別）
    let dup = shot("dup.png", 1);
    let msg = config_err(build_expected_slot(
        ImageSlot::LocalizedScreenshots,
        Some("ja"),
        &[paths[0].clone(), dup.clone()],
        "p",
    ));
    assert!(msg.contains("dup.png") && msg.contains("s1.png"), "{msg}");

    // 小タイルは 1 枚だけ
    let tile_a = write_png(&dir, "ta.png", 440, 280, 2, 8, 1);
    let tile_b = write_png(&dir, "tb.png", 440, 280, 2, 8, 2);
    assert!(build_expected_slot(ImageSlot::SmallTile, None, &[tile_a, tile_b], "p").is_err());
}

fn slot_of(digests: &[&str]) -> ExpectedSlot {
    ExpectedSlot {
        slot: ImageSlot::GlobalScreenshots,
        locale: None,
        files: digests
            .iter()
            .enumerate()
            .map(|(i, d)| ExpectedImageFile {
                path: format!("s{i}.png").into(),
                pixel_sha256: d.to_string(),
            })
            .collect(),
    }
}

fn current(digests: &[&str]) -> Vec<CurrentImage> {
    digests
        .iter()
        .enumerate()
        .map(|(i, d)| CurrentImage {
            alt: format!("スクリーンショット {}", i + 1),
            pixel_sha256: Ok(d.to_string()),
        })
        .collect()
}

#[test]
fn 比較は順序付きの完全一致() {
    let expected = slot_of(&["A", "B", "C"]);
    let status = |cur: &[&str]| compare_slot("i", "n", &expected, &current(cur)).status;
    assert_eq!(status(&["A", "B", "C"]), ComparisonStatus::Match);
    assert_eq!(status(&["B", "A", "C"]), ComparisonStatus::Different);
    assert_eq!(status(&["A", "B"]), ComparisonStatus::Different);
    assert_eq!(status(&["A", "B", "C", "D"]), ComparisonStatus::Different);
    assert_eq!(status(&[]), ComparisonStatus::Different);

    let row = compare_slot("i", "n", &expected, &current(&["A", "X", "C"]));
    assert_eq!(row.differing_positions, vec![2]);
    assert!(describe(&row).contains("2"));
}

#[test]
fn 取得できない画像があれば比較不能でエラー() {
    let expected = slot_of(&["A", "B"]);
    let mut cur = current(&["A", "B"]);
    cur[1].pixel_sha256 = Err("HTTP 403".to_string());
    let row = compare_slot("i", "n", &expected, &cur);
    assert_eq!(row.status, ComparisonStatus::Error);
    assert_eq!(row.error.as_deref(), Some("HTTP 403"));
    assert!(describe(&row).contains("403"));
}
