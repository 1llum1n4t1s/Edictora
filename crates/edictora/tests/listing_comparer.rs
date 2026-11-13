//! C# の ListingComparer.test.cs の移植

use edictora::listing::text_diff::{compute, first_difference_index, format};
use edictora::listing::*;

const ITEM: &str = "abcdefghijklmnopabcdefghijklmnop";

fn values(pairs: &[(ListingField, &str)]) -> FieldValues {
    pairs.iter().map(|(f, v)| (*f, v.to_string())).collect()
}

fn expected() -> ExpectedItem {
    ExpectedItem {
        item_id: ITEM.to_string(),
        display_name: "テスト".to_string(),
        locales: vec![
            ExpectedLocale::new(
                "ja",
                values(&[
                    (ListingField::Description, "説明1\n説明2"),
                    (ListingField::SupportUrl, "https://example.com/support"),
                ]),
            ),
            ExpectedLocale::new("en", values(&[(ListingField::Description, "Desc")])),
        ],
    }
}

fn snapshot(
    status: Option<&str>,
    available: &[&str],
    locales: Vec<LocaleSnapshot>,
    error: Option<&str>,
) -> ItemSnapshot {
    ItemSnapshot {
        item_id: ITEM.to_string(),
        status_text: status.map(str::to_string),
        available_locales: available.iter().map(|s| s.to_string()).collect(),
        locales,
        error: error.map(str::to_string),
    }
}

#[test]
fn compare_スナップショットなしは全行未取得() {
    let rows = compare(&expected(), None);
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r.status() == ComparisonStatus::NotFetched)
    );
}

#[test]
fn compare_完全一致と差分を欄ごとに判定する() {
    let snap = snapshot(
        Some("公開済み"),
        &["en", "ja"],
        vec![
            LocaleSnapshot::ok(
                "ja",
                values(&[
                    (ListingField::Description, "説明1\n説明2"),
                    (ListingField::SupportUrl, "https://example.com/support/"),
                ]),
            ),
            LocaleSnapshot::ok("en", values(&[(ListingField::Description, "Desc")])),
        ],
        None,
    );

    let rows = compare(&expected(), Some(&snap));
    let ja = rows.iter().find(|r| r.locale == "ja").unwrap();
    let en = rows.iter().find(|r| r.locale == "en").unwrap();

    assert_eq!(ja.status(), ComparisonStatus::Different);
    assert_eq!(ja.different_fields(), vec![ListingField::SupportUrl]);
    assert_eq!(en.status(), ComparisonStatus::Match);
}

#[test]
fn compare_value_末尾の空白や改行の違いも差分にする() {
    assert_eq!(
        compare_value("abc", Some("abc\n")),
        ComparisonStatus::Different
    );
    assert_eq!(
        compare_value("abc", Some("abc ")),
        ComparisonStatus::Different
    );
    assert_eq!(compare_value("abc", Some("abc")), ComparisonStatus::Match);
    assert_eq!(compare_value("abc", None), ComparisonStatus::NotFetched);
}

#[test]
fn compare_ダッシュボードに無いロケールはエラー() {
    let snap = snapshot(
        Some("公開済み"),
        &["en"],
        vec![LocaleSnapshot::ok(
            "en",
            values(&[(ListingField::Description, "Desc")]),
        )],
        None,
    );

    let rows = compare(&expected(), Some(&snap));
    let ja = rows.iter().find(|r| r.locale == "ja").unwrap();

    assert_eq!(ja.status(), ComparisonStatus::Error);
    assert!(ja.error.as_deref().unwrap().contains("ja"));
}

#[test]
fn compare_読み出し失敗はエラーとして伝える() {
    let snap = snapshot(None, &[], vec![], Some("編集画面を開けませんでした"));

    let rows = compare(&expected(), Some(&snap));

    assert!(rows.iter().all(|r| r.status() == ComparisonStatus::Error
        && r.error.as_deref() == Some("編集画面を開けませんでした")));
}

#[test]
fn text_diff_変更行だけを前後の文脈付きで示す() {
    let before = "a\nb\nc\nd\ne\nf\ng";
    let after = "a\nb\nc\nD\ne\nf\ng";

    let text = format(&compute(before, after), 1);

    assert_eq!(text, "  …\n  c\n- d\n+ D\n  e\n  …\n");
}

#[test]
fn text_diff_最初の相違位置を返す() {
    assert_eq!(first_difference_index("abc", "abc"), -1);
    assert_eq!(first_difference_index("abc", "abX"), 2);
    assert_eq!(first_difference_index("abc", "abc "), 3);
}
