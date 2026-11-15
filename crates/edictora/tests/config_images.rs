//! 設定の `images`（掲載画像の正本）の読込と、`--only` / `--image-scope` / `--locale` による対象の絞り込み。
//! Chrome を開く前の検証なので、誤りは `Config` エラー（終了コード 2）になる。

mod common;

use std::path::PathBuf;

use common::{ITEM_A, ITEM_B, config_err, test_paths, work_dir, write_png};
use edictora::cli::args::parse;
use edictora::cli::select_targets;
use edictora::config::{self, EdictoraConfig};
use edictora::images::ImageSlot;
use edictora::listing::ExpectedItem;

struct Ctx {
    dir: PathBuf,
}

impl Ctx {
    /// repo/webstore/images/{en,ja,zh_CN}/screenshot-01..03.png（640x400）と tile / marquee を置く
    fn new(name: &str) -> Ctx {
        let dir = work_dir(name);
        let repo = dir.join("repo");
        let images = repo.join("webstore").join("images");
        std::fs::create_dir_all(repo.join("webstore")).unwrap();
        std::fs::write(
            repo.join("webstore").join("listing.txt"),
            "■ 説明\n本文です\n■ カテゴリ\n",
        )
        .unwrap();
        write_png(&images, "tile-small.png", 440, 280, 2, 8, 1);
        write_png(&images, "tile-marquee.png", 1400, 560, 2, 8, 2);
        for (i, locale) in ["en", "ja", "zh_CN"].iter().enumerate() {
            // 順序を確かめるため、ファイル名の昇順と作成順を変える（03 → 01 → 02 の順に作る）
            for (n, number) in [3u8, 1, 2].into_iter().enumerate() {
                let name = format!("screenshot-{number:02}.png");
                write_png(
                    &images.join(locale),
                    &name,
                    640,
                    400,
                    2,
                    8,
                    (i as u8) * 10 + n as u8 + 1,
                );
            }
        }
        // dir の対象外（拾わない）
        std::fs::write(images.join("en").join("notes.txt"), "x").unwrap();
        std::fs::write(images.join("en").join("hero.png"), "x").unwrap();
        Ctx { dir }
    }

    fn parse(&self, json: &str) -> edictora::Result<EdictoraConfig> {
        config::parse(
            json,
            &self.dir.join("config.json"),
            None,
            None,
            &test_paths(&self.dir),
        )
    }
}

fn config_json(images: &str) -> String {
    format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "name": "テスト", "baseDir": "repo",
              "locales": {{ "ja": {{ "description": {{ "file": "webstore/listing.txt" }} }} }},
              "images": {images} }} ] }}"#
    )
}

const FULL: &str = r#"{
    "smallTile": "webstore/images/tile-small.png",
    "marquee": "webstore/images/tile-marquee.png",
    "globalScreenshots": { "dir": "webstore/images/en" },
    "localizedScreenshots": {
      "ja": { "dir": "webstore/images/ja" },
      "zh_CN": { "dir": "webstore/images/zh_CN" }
    }
  }"#;

fn names(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
        .collect()
}

#[test]
fn imagesを読む_dirは直下のscreenshotpngを名前の昇順で並べる() {
    let ctx = Ctx::new("config-images-full");
    let config = ctx.parse(&config_json(FULL)).unwrap();
    let images = config.items[0].images.as_ref().unwrap();

    assert!(
        images
            .small_tile
            .as_ref()
            .unwrap()
            .ends_with("tile-small.png")
    );
    assert!(
        images
            .marquee
            .as_ref()
            .unwrap()
            .ends_with("tile-marquee.png")
    );
    // notes.txt・hero.png は拾わず、昇順（作成順ではない）
    assert_eq!(
        names(images.global_screenshots.as_ref().unwrap()),
        vec![
            "screenshot-01.png",
            "screenshot-02.png",
            "screenshot-03.png"
        ]
    );
    // ロケールのキーは「-」区切りに正規化し、書いた順を保つ
    let codes: Vec<&str> = images
        .localized_screenshots
        .iter()
        .map(|(c, _)| c.as_str())
        .collect();
    assert_eq!(codes, vec!["ja", "zh-CN"]);
}

#[test]
fn filesは書いた順で読む() {
    let ctx = Ctx::new("config-images-files");
    let config = ctx
        .parse(&config_json(
            r#"{ "globalScreenshots": { "files": ["webstore/images/en/screenshot-03.png", "webstore/images/en/screenshot-01.png"] } }"#,
        ))
        .unwrap();
    let images = config.items[0].images.as_ref().unwrap();
    assert_eq!(
        names(images.global_screenshots.as_ref().unwrap()),
        vec!["screenshot-03.png", "screenshot-01.png"]
    );
    assert!(images.small_tile.is_none() && images.localized_screenshots.is_empty());
}

#[test]
fn 画像だけのアイテムも読める_localesもimagesも無ければエラー() {
    let ctx = Ctx::new("config-images-only");
    let json = format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "baseDir": "repo", "images": {{ "marquee": "webstore/images/tile-marquee.png" }} }} ] }}"#
    );
    let config = ctx.parse(&json).unwrap();
    assert!(config.items[0].locales.is_empty());
    assert!(config.items[0].images.is_some());

    let json = format!(r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "baseDir": "repo" }} ] }}"#);
    assert!(config_err(ctx.parse(&json)).contains("locales"));
}

#[test]
fn imagesの誤りは設定エラー() {
    let ctx = Ctx::new("config-images-errors");
    let err = |images: &str| config_err(ctx.parse(&config_json(images)));

    // 空・未知のキー
    assert!(err("{}").contains("images"));
    assert!(err(r#"{ "small": "x.png" }"#).contains("small"));
    // 存在しないファイル・フォルダー
    assert!(err(r#"{ "marquee": "webstore/images/none.png" }"#).contains("none.png"));
    assert!(err(r#"{ "globalScreenshots": { "dir": "webstore/images/none" } }"#).contains("none"));
    // dir と files の両方・どちらも無い
    assert!(
        err(r#"{ "globalScreenshots": { "dir": "webstore/images/en", "files": ["a.png"] } }"#)
            .contains("dir")
    );
    assert!(err(r#"{ "globalScreenshots": {} }"#).contains("dir"));
    // 枚数（0 枚・6 枚）
    let empty = ctx
        .dir
        .join("repo")
        .join("webstore")
        .join("images")
        .join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(err(r#"{ "globalScreenshots": { "dir": "webstore/images/empty" } }"#).contains("1"));
    let six: Vec<String> = (1..=6)
        .map(|i| format!("\"webstore/images/en/screenshot-0{}.png\"", i.min(3)))
        .collect();
    assert!(
        err(&format!(
            r#"{{ "globalScreenshots": {{ "files": [{}] }} }}"#,
            six.join(",")
        ))
        .contains("5")
    );
    // ロケール: 形式の誤り・重複（zh_CN と zh-CN）
    assert!(
        err(r#"{ "localizedScreenshots": { "日本語": { "dir": "webstore/images/ja" } } }"#)
            .contains("日本語")
    );
    assert!(
        err(r#"{ "localizedScreenshots": { "zh_CN": { "dir": "webstore/images/zh_CN" }, "zh-CN": { "dir": "webstore/images/zh_CN" } } }"#)
            .contains("zh-CN")
    );
}

fn targets(ctx: &Ctx, images: &str, args: &[&str]) -> edictora::Result<Vec<ExpectedItem>> {
    let config = ctx.parse(&config_json(images))?;
    select_targets(&config, &parse(args).unwrap())
}

fn slots(item: &ExpectedItem) -> Vec<(ImageSlot, Option<String>)> {
    item.images
        .as_ref()
        .map(|i| i.slots.iter().map(|s| (s.slot, s.locale.clone())).collect())
        .unwrap_or_default()
}

#[test]
fn 既定はテキストと画像の両方_処理順は小タイル_マーキー_全言語向け_ローカライズ版() {
    let ctx = Ctx::new("config-images-targets");
    let result = targets(&ctx, FULL, &["diff"]).unwrap();
    let item = &result[0];
    assert_eq!(item.locales.len(), 1);
    assert_eq!(
        slots(item),
        vec![
            (ImageSlot::SmallTile, None),
            (ImageSlot::Marquee, None),
            (ImageSlot::GlobalScreenshots, None),
            (ImageSlot::LocalizedScreenshots, Some("ja".to_string())),
            (ImageSlot::LocalizedScreenshots, Some("zh-CN".to_string())),
        ]
    );
    // 画素のダイジェストは Chrome を開く前に計算済み
    let first = &item.images.as_ref().unwrap().slots[2].files;
    assert_eq!(first.len(), 3);
    assert_eq!(first[0].pixel_sha256.len(), 64);
}

#[test]
fn only_imagesはテキストを外し_only_textは画像を外す() {
    let ctx = Ctx::new("config-images-only-flag");
    let images_only = targets(&ctx, FULL, &["diff", "--only", "images"]).unwrap();
    assert!(images_only[0].locales.is_empty());
    assert_eq!(slots(&images_only[0]).len(), 5);

    let text_only = targets(&ctx, FULL, &["diff", "--only", "text"]).unwrap();
    assert_eq!(text_only[0].locales.len(), 1);
    assert!(text_only[0].images.is_none());
}

#[test]
fn image_scopeで全言語向けとローカライズ版を分ける() {
    let ctx = Ctx::new("config-images-scope");
    let global = targets(
        &ctx,
        FULL,
        &["diff", "--only", "images", "--image-scope", "global"],
    )
    .unwrap();
    assert_eq!(
        slots(&global[0]),
        vec![
            (ImageSlot::SmallTile, None),
            (ImageSlot::Marquee, None),
            (ImageSlot::GlobalScreenshots, None),
        ]
    );
    let localized = targets(
        &ctx,
        FULL,
        &["diff", "--only", "images", "--image-scope", "localized"],
    )
    .unwrap();
    assert_eq!(slots(&localized[0]).len(), 2);
    assert!(
        slots(&localized[0])
            .iter()
            .all(|(s, _)| *s == ImageSlot::LocalizedScreenshots)
    );
}

#[test]
fn localeはローカライズ版の言語を絞る_全言語向けは絞らない() {
    let ctx = Ctx::new("config-images-locale");
    // zh_CN と zh-CN は同じ
    let result = targets(
        &ctx,
        FULL,
        &[
            "diff", "--only", "images", "--item", ITEM_A, "--locale", "zh_CN",
        ],
    )
    .unwrap();
    assert_eq!(
        slots(&result[0]),
        vec![
            (ImageSlot::SmallTile, None),
            (ImageSlot::Marquee, None),
            (ImageSlot::GlobalScreenshots, None),
            (ImageSlot::LocalizedScreenshots, Some("zh-CN".to_string())),
        ]
    );
    // テキストの言語（ja）にもローカライズ版（ja・zh-CN）にも無い言語を明示したら誤り
    let msg = config_err(targets(
        &ctx,
        FULL,
        &["diff", "--item", ITEM_A, "--locale", "fr"],
    ));
    assert!(msg.contains("fr"));
    // ローカライズ版だけに当たる言語なら、テキストは今回は対象外
    let only_img_locale =
        targets(&ctx, FULL, &["diff", "--item", ITEM_A, "--locale", "zh-CN"]).unwrap();
    assert!(only_img_locale[0].locales.is_empty());
}

#[test]
fn 対象が無いアイテムは明示なら誤り_全件なら飛ばす() {
    let ctx = Ctx::new("config-images-nothing");
    let json = format!(
        r#"{{ "items": [ {{ "itemId": "{ITEM_A}", "baseDir": "repo", "locales": {{ "ja": {{ "description": {{ "file": "webstore/listing.txt" }} }} }} }},
                         {{ "itemId": "{ITEM_B}", "baseDir": "repo", "images": {{ "marquee": "webstore/images/tile-marquee.png" }} }} ] }}"#
    );
    let config = ctx.parse(&json).unwrap();

    // 全件: テキストだけのアイテムは飛ばす
    let all = select_targets(&config, &parse(&["diff", "--only", "images"]).unwrap()).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].item_id, ITEM_B);
    // 明示: 対象が無いのは誤り
    let msg = config_err(select_targets(
        &config,
        &parse(&["diff", "--only", "images", "--item", ITEM_A]).unwrap(),
    ));
    assert!(msg.contains(ITEM_A));
    // どのアイテムにも対象が無い
    assert!(
        config_err(select_targets(
            &config,
            &parse(&["diff", "--only", "images", "--image-scope", "localized"]).unwrap(),
        ))
        .contains("対象")
    );
}

#[test]
fn 画像の検証エラーは対象にしたときだけ出る() {
    let ctx = Ctx::new("config-images-validate-on-target");
    // ja のスクリーンショットに規格外（1281x800）を混ぜる
    write_png(
        &ctx.dir
            .join("repo")
            .join("webstore")
            .join("images")
            .join("ja"),
        "screenshot-02.png",
        1281,
        800,
        2,
        8,
        9,
    );
    // 設定の読込は通る（中身までは開かない）
    let config = ctx.parse(&config_json(FULL)).unwrap();
    // 全言語向けだけなら通る
    assert!(
        select_targets(
            &config,
            &parse(&["diff", "--only", "images", "--image-scope", "global"]).unwrap()
        )
        .is_ok()
    );
    // ローカライズ版を対象にすると、Chrome を開く前に止まる
    let msg = config_err(select_targets(
        &config,
        &parse(&["diff", "--only", "images", "--image-scope", "localized"]).unwrap(),
    ));
    assert!(msg.contains("1281x800"), "{msg}");
}
