//! 掲載画像の手順（`services::pull_images` / `diff_images` / `push_images`）を、メモリ上の偽ページで確かめる。
//!
//! 画面の代わりに、画像の置き場ごとに「画像（alt と画素ダイジェスト）の並び」を持つ偽ページを使い、
//! 手順（順番・中止判断・保存と照合）だけを見る。偽ページは `ListingPage::submit_for_review` が呼ばれたら
//! panic する（画像の手順が審査送信に届かないことの確認。`ImagePage` には審査送信が無いので型の上でも届かない）。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use edictora::cancel::CancelToken;
use edictora::dashboard::write_gate::ItemPageState;
use edictora::dashboard::{
    EditorPage, ImagePage, ListingPage, PageImage, SubmitConfirm, SubmitResult,
};
use edictora::images::fetch::ImageFetcher;
use edictora::images::{ExpectedImageFile, ExpectedImages, ExpectedSlot, ImageSlot};
use edictora::listing::{
    ComparisonStatus, ExpectedItem, ExpectedLocale, FieldValues, ListingField,
};
use edictora::services;
use edictora::{Error, Result};

mod common;
use common::ITEM_A as ITEM;

/// 画面の画像 1 枚（alt と、画素のダイジェストの代わりの文字列）
#[derive(Debug, Clone, PartialEq, Eq)]
struct Img {
    alt: String,
    digest: String,
}

/// 置き場の鍵（ローカライズ版はロケール付き）
type Key = (ImageSlot, Option<String>);

struct FakeDashboard {
    status: String,
    locales: Vec<String>,
    current_locale: String,
    saved: BTreeMap<Key, Vec<Img>>,
    editing: BTreeMap<Key, Vec<Img>>,
    /// アップロードするファイル → 画素のダイジェスト
    files: HashMap<PathBuf, String>,
    log: Vec<String>,
    /// n 回目（0 始まり）のアップロードを失敗させる
    fail_upload_at: Option<usize>,
    uploads: usize,
    /// 保存したとき、アップロードした画像の中身を変えてしまう（保存後の照合失敗の再現）
    corrupt_on_save: bool,
    /// 書き込みの前後で状態が変わらないことの確認用（テキスト）
    text: HashMap<String, String>,
}

impl FakeDashboard {
    fn new(locales: &[&str]) -> FakeDashboard {
        FakeDashboard {
            status: "公開済み".to_string(),
            locales: locales.iter().map(|s| s.to_string()).collect(),
            current_locale: locales[0].to_string(),
            saved: BTreeMap::new(),
            editing: BTreeMap::new(),
            files: HashMap::new(),
            log: Vec::new(),
            fail_upload_at: None,
            uploads: 0,
            corrupt_on_save: false,
            text: HashMap::new(),
        }
    }

    fn key(&self, slot: ImageSlot) -> Key {
        match slot {
            ImageSlot::LocalizedScreenshots => (slot, Some(self.current_locale.clone())),
            _ => (slot, None),
        }
    }

    fn set_saved(&mut self, slot: ImageSlot, locale: Option<&str>, digests: &[&str]) {
        let imgs = digests
            .iter()
            .enumerate()
            .map(|(i, d)| Img {
                alt: alt_of(slot, i),
                digest: d.to_string(),
            })
            .collect();
        self.saved.insert((slot, locale.map(str::to_string)), imgs);
    }

    fn saved_digests(&self, slot: ImageSlot, locale: Option<&str>) -> Vec<String> {
        self.saved
            .get(&(slot, locale.map(str::to_string)))
            .map(|v| v.iter().map(|i| i.digest.clone()).collect())
            .unwrap_or_default()
    }

    fn count(&self, prefix: &str) -> usize {
        self.log.iter().filter(|l| l.starts_with(prefix)).count()
    }
}

fn alt_of(slot: ImageSlot, i: usize) -> String {
    match slot {
        ImageSlot::Icon => "ショップ アイコン".to_string(),
        ImageSlot::SmallTile => "プロモーション タイル（小）".to_string(),
        ImageSlot::Marquee => "マーキー プロモーション タイル".to_string(),
        _ => format!("スクリーンショット {}", i + 1),
    }
}

impl EditorPage for FakeDashboard {
    fn open_item(&mut self, item_id: &str, _: &CancelToken) -> Result<()> {
        self.log.push(format!("open {item_id}"));
        self.editing = self.saved.clone();
        self.current_locale = self.locales[0].clone();
        Ok(())
    }

    fn reload(&mut self, _: &CancelToken) -> Result<()> {
        self.log.push("reload".to_string());
        self.editing = self.saved.clone();
        self.current_locale = self.locales[0].clone();
        Ok(())
    }

    fn get_state(&mut self, _: &CancelToken) -> Result<ItemPageState> {
        Ok(ItemPageState {
            status_text: Some(self.status.clone()),
            description_found: true,
            description_editable: true,
            save_button_found: true,
            save_button_enabled: true,
        })
    }

    fn get_locales(&mut self, _: &CancelToken) -> Result<Vec<String>> {
        Ok(self.locales.clone())
    }

    fn switch_locale(&mut self, code: &str, _: &CancelToken) -> Result<()> {
        self.log.push(format!("switch {code}"));
        self.current_locale = code.to_string();
        Ok(())
    }

    fn save_draft(&mut self, _: &CancelToken) -> Result<()> {
        self.log.push("save".to_string());
        if self.corrupt_on_save {
            for imgs in self.editing.values_mut() {
                for img in imgs.iter_mut() {
                    img.digest = format!("{}-reencoded", img.digest);
                }
            }
        }
        self.saved = self.editing.clone();
        Ok(())
    }

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        self.log.push(format!("capture {label}"));
        None
    }
}

impl ListingPage for FakeDashboard {
    fn read_fields(&mut self, fields: &[ListingField], _: &CancelToken) -> Result<FieldValues> {
        Ok(fields
            .iter()
            .map(|f| {
                (
                    *f,
                    self.text
                        .get(&self.current_locale)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect())
    }

    fn write_field(&mut self, _: ListingField, value: &str, _: &CancelToken) -> Result<()> {
        self.log.push("write-text".to_string());
        self.text
            .insert(self.current_locale.clone(), value.to_string());
        Ok(())
    }

    fn submit_for_review(
        &mut self,
        _: &mut SubmitConfirm<'_>,
        _: &CancelToken,
    ) -> Result<SubmitResult> {
        panic!("画像・テキストの push から審査送信が呼ばれました");
    }
}

impl ImagePage for FakeDashboard {
    fn read_images(&mut self, slot: ImageSlot, _: &CancelToken) -> Result<Vec<PageImage>> {
        let key = self.key(slot);
        Ok(self
            .editing
            .get(&key)
            .map(|v| {
                v.iter()
                    .map(|i| PageImage {
                        alt: i.alt.clone(),
                        src: format!("fake://{}", i.digest),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn delete_image(&mut self, slot: ImageSlot, position: usize, _: &CancelToken) -> Result<()> {
        self.log.push(format!("delete {slot} {position}"));
        let key = self.key(slot);
        let imgs = self.editing.entry(key).or_default();
        if position >= imgs.len() {
            return Err(Error::Dashboard("範囲外の削除".to_string()));
        }
        imgs.remove(position);
        Ok(())
    }

    fn upload_image(&mut self, slot: ImageSlot, path: &Path, _: &CancelToken) -> Result<()> {
        let n = self.uploads;
        self.uploads += 1;
        self.log.push(format!("upload {slot} {}", path.display()));
        if self.fail_upload_at == Some(n) {
            return Err(Error::Dashboard("アップロードが終わりません".to_string()));
        }
        let digest = self.files.get(path).cloned().expect("登録済みのファイル");
        let key = self.key(slot);
        let imgs = self.editing.entry(key).or_default();
        let i = imgs.len();
        imgs.push(Img {
            alt: alt_of(slot, i),
            digest,
        });
        Ok(())
    }
}

/// 偽の原本取得（`fake://<ダイジェスト>`）。`unfetchable` のダイジェストの画像は取得に失敗する（比較不能の再現）
struct FakeFetcher {
    unfetchable: Option<String>,
}

impl ImageFetcher for FakeFetcher {
    fn pixel_digest_of(&self, src: &str, _: ImageSlot, _: &CancelToken) -> Result<String> {
        let digest = src.strip_prefix("fake://").expect("偽の URL");
        if self.unfetchable.as_deref() == Some(digest) {
            return Err(Error::Dashboard("HTTP 403".to_string()));
        }
        Ok(digest.to_string())
    }
}

fn file(name: &str, digest: &str) -> ExpectedImageFile {
    ExpectedImageFile {
        path: PathBuf::from(name),
        pixel_sha256: digest.to_string(),
    }
}

fn slot(slot: ImageSlot, locale: Option<&str>, files: &[(&str, &str)]) -> ExpectedSlot {
    ExpectedSlot {
        slot,
        locale: locale.map(str::to_string),
        files: files.iter().map(|(n, d)| file(n, d)).collect(),
    }
}

fn item(slots: Vec<ExpectedSlot>) -> ExpectedItem {
    ExpectedItem {
        item_id: ITEM.to_string(),
        display_name: "テスト".to_string(),
        locales: Vec::new(),
        images: Some(ExpectedImages { slots }),
        privacy: None,
    }
}

/// 偽ページにアップロード元のファイルを覚えさせる
fn register(page: &mut FakeDashboard, expected: &ExpectedItem) {
    for s in &expected.images.as_ref().unwrap().slots {
        for f in &s.files {
            page.files.insert(f.path.clone(), f.pixel_sha256.clone());
        }
    }
}

fn no_progress(_: &str) {}

fn expected_two_global() -> ExpectedItem {
    item(vec![
        slot(ImageSlot::SmallTile, None, &[("tile.png", "T1")]),
        slot(ImageSlot::Marquee, None, &[("marquee.png", "M1")]),
        slot(
            ImageSlot::GlobalScreenshots,
            None,
            &[("s1.png", "S1"), ("s2.png", "S2")],
        ),
    ])
}

#[test]
fn diff_images_一致と差分と枚数違いを返す() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::SmallTile, None, &["T1"]);
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    page.set_saved(ImageSlot::GlobalScreenshots, None, &["S1", "S2", "S3"]);
    let expected = expected_two_global();

    let (_, rows) = services::diff_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].status, ComparisonStatus::Match);
    assert_eq!(rows[1].status, ComparisonStatus::Different);
    assert_eq!(rows[1].differing_positions, vec![1]);
    assert_eq!(rows[2].status, ComparisonStatus::Different);
    assert_eq!((rows[2].current_count, rows[2].expected_count), (3, 2));
    // 読み取りだけ
    assert_eq!(page.count("delete"), 0);
    assert_eq!(page.count("upload"), 0);
    assert_eq!(page.count("save"), 0);
}

#[test]
fn diff_images_並びが違えば差分() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::GlobalScreenshots, None, &["S2", "S1"]);
    let expected = item(vec![slot(
        ImageSlot::GlobalScreenshots,
        None,
        &[("s1.png", "S1"), ("s2.png", "S2")],
    )]);

    let (_, rows) = services::diff_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(rows[0].status, ComparisonStatus::Different);
    assert_eq!(rows[0].differing_positions, vec![1, 2]);
}

#[test]
fn diff_images_原本を取得できなければ比較不能で一致扱いにしない() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::SmallTile, None, &["T1"]);
    let fetch = FakeFetcher {
        unfetchable: Some("T1".to_string()),
    };
    let expected = item(vec![slot(
        ImageSlot::SmallTile,
        None,
        &[("tile.png", "T1")],
    )]);

    let (_, rows) = services::diff_images(
        &mut page,
        &fetch,
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(rows[0].status, ComparisonStatus::Error);
    assert!(rows[0].error.as_deref().unwrap().contains("403"));
}

#[test]
fn push_images_差分のある置き場だけ全入れ替えして保存し読み直して照合する() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::SmallTile, None, &["T1"]); // 一致
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    page.set_saved(ImageSlot::GlobalScreenshots, None, &["X1", "X2", "X3"]);
    let expected = expected_two_global();
    register(&mut page, &expected);

    let result = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(
        result.unchanged,
        vec!["プロモーション タイル（小）".to_string()]
    );
    assert_eq!(result.saved.len(), 2);
    assert_eq!(page.saved_digests(ImageSlot::Marquee, None), vec!["M1"]);
    assert_eq!(
        page.saved_digests(ImageSlot::GlobalScreenshots, None),
        vec!["S1", "S2"]
    );
    // 既存は末尾から 1 枚ずつ削除、正本は先頭から 1 枚ずつ追加
    let ops: Vec<&str> = page
        .log
        .iter()
        .map(String::as_str)
        .filter(|l| l.starts_with("delete") || l.starts_with("upload"))
        .collect();
    assert_eq!(
        ops,
        vec![
            "delete marquee 0",
            "upload marquee marquee.png",
            "delete globalScreenshots 2",
            "delete globalScreenshots 1",
            "delete globalScreenshots 0",
            "upload globalScreenshots s1.png",
            "upload globalScreenshots s2.png",
        ]
    );
    // 全言語向けは 1 回だけ保存し、読み直す
    assert_eq!(page.count("save"), 1);
    assert!(
        result
            .final_comparison
            .iter()
            .all(|r| r.status == ComparisonStatus::Match)
    );
}

#[test]
fn push_images_すべて一致なら何も触らない() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::SmallTile, None, &["T1"]);
    page.set_saved(ImageSlot::Marquee, None, &["M1"]);
    page.set_saved(ImageSlot::GlobalScreenshots, None, &["S1", "S2"]);
    let expected = expected_two_global();
    register(&mut page, &expected);

    let result = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert!(result.saved.is_empty());
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn push_images_審査待ちなら最初の削除の前に止める() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.status = "審査待ち".to_string();
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    let expected = item(vec![slot(
        ImageSlot::Marquee,
        None,
        &[("marquee.png", "M1")],
    )]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(err, Error::WriteBlocked(_)));
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn push_images_アップロードの失敗では保存せず残りも行わない() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::SmallTile, None, &["OLD-T"]);
    page.set_saved(ImageSlot::Marquee, None, &["OLD-M"]);
    page.fail_upload_at = Some(0);
    let expected = item(vec![
        slot(ImageSlot::SmallTile, None, &[("tile.png", "T1")]),
        slot(ImageSlot::Marquee, None, &[("marquee.png", "M1")]),
    ]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(err, Error::Dashboard(_)));
    // 保存しない・次の置き場へ進まない・失敗時の画面を残す
    assert_eq!(page.count("save"), 0);
    assert!(!page.log.iter().any(|l| l.contains("marquee.png")));
    assert!(
        page.log
            .iter()
            .any(|l| l.starts_with("capture push-images-"))
    );
}

#[test]
fn push_images_保存後の照合が合わなければエラー() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    page.corrupt_on_save = true;
    let expected = item(vec![slot(
        ImageSlot::Marquee,
        None,
        &[("marquee.png", "M1")],
    )]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(&err, Error::Dashboard(m) if m.contains("保存後の照合")));
}

#[test]
fn push_images_原本を取得できなければ削除の前に止める() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    let fetch = FakeFetcher {
        unfetchable: Some("OLD".to_string()),
    };
    let expected = item(vec![slot(
        ImageSlot::Marquee,
        None,
        &[("marquee.png", "M1")],
    )]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &fetch,
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    // 比較不能は「常に置換」にしない
    assert!(matches!(&err, Error::Dashboard(m) if m.contains("比較できない")));
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn push_images_ローカライズ版はロケールごとに切替_置換_保存_読み直し_切替_照合() {
    let mut page = FakeDashboard::new(&["en", "ja", "ko"]);
    page.set_saved(ImageSlot::LocalizedScreenshots, Some("ja"), &["J-OLD"]);
    page.set_saved(ImageSlot::LocalizedScreenshots, Some("ko"), &["K1"]);
    let expected = item(vec![
        slot(
            ImageSlot::LocalizedScreenshots,
            Some("ja"),
            &[("ja1.png", "J1"), ("ja2.png", "J2")],
        ),
        slot(
            ImageSlot::LocalizedScreenshots,
            Some("ko"),
            &[("ko1.png", "K1")],
        ),
    ]);
    register(&mut page, &expected);

    let result = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(
        result.saved,
        vec!["[ja] ローカライズ版スクリーンショット".to_string()]
    );
    assert_eq!(
        result.unchanged,
        vec!["[ko] ローカライズ版スクリーンショット".to_string()]
    );
    assert_eq!(
        page.saved_digests(ImageSlot::LocalizedScreenshots, Some("ja")),
        vec!["J1", "J2"]
    );
    // ja の保存より前に ko へは切り替えない（ロケールごとの手順）
    let save = page.log.iter().position(|l| l == "save").unwrap();
    let ko = page.log.iter().position(|l| l == "switch ko").unwrap();
    assert!(save < ko);
}

#[test]
fn push_images_言語が1つのアイテムでローカライズ版が対象ならエラーで何も触らない() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::Marquee, None, &["OLD"]);
    let expected = item(vec![
        slot(ImageSlot::Marquee, None, &[("marquee.png", "M1")]),
        slot(
            ImageSlot::LocalizedScreenshots,
            Some("en"),
            &[("en1.png", "E1")],
        ),
    ]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(&err, Error::Dashboard(m) if m.contains("--image-scope global")));
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn push_images_ダッシュボードに無いロケールがあれば全体を止める() {
    let mut page = FakeDashboard::new(&["en", "ja"]);
    let expected = item(vec![slot(
        ImageSlot::LocalizedScreenshots,
        Some("ko"),
        &[("ko1.png", "K1")],
    )]);
    register(&mut page, &expected);

    let err = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(matches!(&err, Error::Dashboard(m) if m.contains("ko")));
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn テキストと画像のpushを通しても審査送信は呼ばれない() {
    // submit_for_review は panic する偽ページ。テキスト → 画像の順で push 全体を通す
    let mut page = FakeDashboard::new(&["en", "ja"]);
    page.text.insert("en".to_string(), "旧".to_string());
    page.text.insert("ja".to_string(), "旧".to_string());
    page.set_saved(ImageSlot::GlobalScreenshots, None, &["OLD"]);
    let mut expected = item(vec![slot(
        ImageSlot::GlobalScreenshots,
        None,
        &[("s1.png", "S1")],
    )]);
    expected.locales = vec![
        ExpectedLocale::new(
            "en",
            FieldValues::from([(ListingField::Description, "新".into())]),
        ),
        ExpectedLocale::new(
            "ja",
            FieldValues::from([(ListingField::Description, "新".into())]),
        ),
    ];
    register(&mut page, &expected);
    let cancel = CancelToken::new();

    let text = services::push(&mut page, &expected, &no_progress, &cancel).unwrap();
    let images = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &cancel,
    )
    .unwrap();

    assert_eq!(text.locales.len(), 2);
    assert_eq!(images.saved.len(), 1);
}

#[test]
fn ロケール名の大文字小文字を区別しない() {
    // 設定のロケールが zh-CN、ダッシュボードの一覧が zh-cn のように違っても同じロケールとして扱う
    let mut page = FakeDashboard::new(&["en", "zh-cn"]);
    page.set_saved(ImageSlot::LocalizedScreenshots, Some("zh-CN"), &["Z-OLD"]);
    let expected = item(vec![slot(
        ImageSlot::LocalizedScreenshots,
        Some("zh-CN"),
        &[("z1.png", "Z1")],
    )]);
    register(&mut page, &expected);

    let (_, rows) = services::diff_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
}

// ───────── ストア アイコン ─────────

#[test]
fn アイコンは全言語向けの置き場として差分を読み取り専用で返す() {
    // 言語が複数でも、アイコンは全言語共通なのでロケールを切り替えずに読む
    let mut page = FakeDashboard::new(&["ja", "en"]);
    page.set_saved(ImageSlot::Icon, None, &["ICON-OLD"]);
    let expected = item(vec![slot(ImageSlot::Icon, None, &[("icon.png", "ICON1")])]);

    let (_, rows) = services::diff_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(rows[0].status, ComparisonStatus::Different);
    assert_eq!(rows[0].label(), "ショップ アイコン");
    assert_eq!(page.count("switch"), 0);
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );
}

#[test]
fn push_images_アイコンは小タイルと同じ経路で削除_追加_保存_照合する() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::Icon, None, &["ICON-OLD"]);
    page.set_saved(ImageSlot::SmallTile, None, &["T1"]); // 一致
    let expected = item(vec![
        slot(ImageSlot::Icon, None, &[("icon.png", "ICON1")]),
        slot(ImageSlot::SmallTile, None, &[("tile.png", "T1")]),
    ]);
    register(&mut page, &expected);

    let result = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(result.saved, vec!["ショップ アイコン".to_string()]);
    assert_eq!(
        result.unchanged,
        vec!["プロモーション タイル（小）".to_string()]
    );
    assert_eq!(page.saved_digests(ImageSlot::Icon, None), vec!["ICON1"]);
    let ops: Vec<&str> = page
        .log
        .iter()
        .map(String::as_str)
        .filter(|l| l.starts_with("delete") || l.starts_with("upload"))
        .collect();
    assert_eq!(ops, vec!["delete icon 0", "upload icon icon.png"]);
    // 全言語向けなので保存は 1 回、ロケールの切替もしない
    assert_eq!(page.count("save"), 1);
    assert_eq!(page.count("switch"), 0);
    assert!(
        result
            .final_comparison
            .iter()
            .all(|r| r.status == ComparisonStatus::Match)
    );
}

#[test]
fn push_images_アイコンが一致なら何も触らず_審査待ちなら削除の前に止める() {
    let mut page = FakeDashboard::new(&["ja"]);
    page.set_saved(ImageSlot::Icon, None, &["ICON1"]);
    let expected = item(vec![slot(ImageSlot::Icon, None, &[("icon.png", "ICON1")])]);
    register(&mut page, &expected);
    let result = services::push_images(
        &mut page,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(result.saved.is_empty());
    assert_eq!(
        page.count("delete") + page.count("upload") + page.count("save"),
        0
    );

    // 審査待ちのアイテムのアイコンは、差分があっても何も触らない
    let mut pending = FakeDashboard::new(&["ja"]);
    pending.status = "審査待ち".to_string();
    pending.set_saved(ImageSlot::Icon, None, &["ICON-OLD"]);
    register(&mut pending, &expected);
    let err = services::push_images(
        &mut pending,
        &FakeFetcher { unfetchable: None },
        &expected,
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::WriteBlocked(_)));
    assert_eq!(
        pending.count("delete") + pending.count("upload") + pending.count("save"),
        0
    );
}
