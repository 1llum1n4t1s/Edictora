//! PNG の画素ダイジェスト（`images::png`）。ファイルのバイト列が違っても、画素が同じなら同じダイジェストになることを確かめる。
//!
//! 背景（2026-11-15 の実画面）: ダッシュボードにアップロードした PNG は、サーバーが再エンコードして保存する。
//! `=s0` の原本はサイズもバイト列も変わるが、画素は同じ。そのため、ファイルのバイト列でなく画素で比べる。
//! フィクスチャ（tests/fixtures/images/）は同じ 40x24 の画素を、フィルター・圧縮レベル・IDAT の分割を変えて書いたもの。

use std::path::PathBuf;

use edictora::images::png::{decode, pixel_digest};

/// 40x24 の基準の画素のダイジェスト（Python で `sha256(幅 BE4 + 高さ BE4 + RGB)` を計算した値）
const BASE: &str = "0E58349775C274AE4CCE624CB8809D5CF4BDCD3D7679408661B3EFCBC854DF28";
const TWEAKED: &str = "911D88A647859DD707ECAB0C3CAD0D2E604979D63588CE9A6C09B92C27A388E8";

mod common;

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

#[test]
fn フィルターと圧縮が違っても画素が同じなら同じダイジェスト() {
    for name in [
        "rgb-none-l9.png",
        "rgb-mixed-filters-l1.png",
        "rgb-sub-split.png",
        "rgb-stored.png",
        "rgb-paeth-l9.png",
        "rgb-avg.png",
    ] {
        assert_eq!(pixel_digest(&fixture(name)).as_deref(), Ok(BASE), "{name}");
    }
}

#[test]
fn 画素が1つ違えば別のダイジェスト() {
    assert_eq!(
        pixel_digest(&fixture("rgb-tweaked.png")).as_deref(),
        Ok(TWEAKED)
    );
    assert_ne!(BASE, TWEAKED);
}

#[test]
fn すべて不透明なrgbaはrgbと同じダイジェスト() {
    assert_eq!(
        pixel_digest(&fixture("rgba-opaque.png")).as_deref(),
        Ok(BASE)
    );
}

#[test]
fn 透過のあるrgbaやグレー_インターレースは比較不能() {
    assert!(
        pixel_digest(&fixture("rgba-transparent.png"))
            .unwrap_err()
            .contains("transparent")
    );
    assert!(
        pixel_digest(&fixture("gray.png"))
            .unwrap_err()
            .contains("color type")
    );
    assert!(
        pixel_digest(&fixture("rgb-interlaced.png"))
            .unwrap_err()
            .contains("interlace")
    );
}

#[test]
fn 寸法と画素数を返す() {
    let image = decode(&fixture("rgb-avg.png")).unwrap();
    assert_eq!((image.width, image.height), (40, 24));
    assert_eq!(image.pixels.len(), 40 * 24 * 3);
}

#[test]
fn 壊れた入力はエラーで落ちない() {
    let good = fixture("rgb-avg.png");
    assert!(pixel_digest(b"").is_err());
    assert!(pixel_digest(b"not a png").is_err());
    // 途中で切れている（IEND が無い）
    assert!(pixel_digest(&good[..good.len() / 2]).is_err());
    // 圧縮データを壊す
    let mut broken = good.clone();
    for b in broken.iter_mut().skip(60).take(40) {
        *b ^= 0xFF;
    }
    assert!(pixel_digest(&broken).is_err() || pixel_digest(&broken).as_deref() != Ok(BASE));
}

#[test]
fn 自前で組んだ非圧縮のpngも読める() {
    let a = common::png_bytes(64, 32, 2, 8, 1);
    let b = common::png_bytes(64, 32, 2, 8, 2);
    let (da, db) = (pixel_digest(&a).unwrap(), pixel_digest(&b).unwrap());
    assert_ne!(da, db);
    assert_eq!(pixel_digest(&a).unwrap(), da);
}
