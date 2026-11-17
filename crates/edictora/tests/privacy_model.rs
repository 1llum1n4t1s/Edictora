//! 「プライバシー」タブのモデル・正本の整形・比較（`privacy`）。ブラウザなしの純粋な部分を確かめる。

use std::collections::{BTreeMap, BTreeSet};

use edictora::listing::ComparisonStatus;
use edictora::privacy::{
    self, DataKind, ExpectedDataUsage, ExpectedPrivacy, ExpectedRemoteCode, PrivacyField,
    PrivacyReading, compare, dedent, describe, format_kinds,
};

fn expected() -> ExpectedPrivacy {
    ExpectedPrivacy {
        single_purpose: Some("目的".to_string()),
        permissions: Some(vec![
            ("activeTab".to_string(), "タブを確認".to_string()),
            ("host".to_string(), "ホストの理由".to_string()),
        ]),
        remote_code: None,
        data_usage: Some(ExpectedDataUsage {
            collected: [DataKind::PersonallyIdentifiable, DataKind::Authentication]
                .into_iter()
                .collect(),
            certify: true,
        }),
    }
}

/// 正本と同じ内容の読み出し
fn matching_reading() -> PrivacyReading {
    PrivacyReading {
        single_purpose: Some("目的".to_string()),
        screen_permissions: vec!["activeTab".to_string(), "host".to_string()],
        permissions: BTreeMap::from([
            ("activeTab".to_string(), "タブを確認".to_string()),
            ("host".to_string(), "ホストの理由".to_string()),
        ]),
        collected: Some(
            [DataKind::PersonallyIdentifiable, DataKind::Authentication]
                .into_iter()
                .collect(),
        ),
        certified: Some(true),
    }
}

#[test]
fn 字下げを取り除き前後の空行を除く() {
    assert_eq!(dedent("  あ\n  い"), "あ\nい");
    // 共通の字下げだけを取り除く（内側の字下げは残す）
    assert_eq!(dedent("  あ\n    い\n  う"), "あ\n  い\nう");
    // 前後の空行を除く
    assert_eq!(dedent("\n\n  あ\n\n"), "あ");
    // 空白だけの行は空行に寄せる（字下げの長さに依らず完全一致で比べられる）
    assert_eq!(dedent("  あ\n   \n  い"), "あ\n\nい");
    // 字下げが無ければそのまま
    assert_eq!(dedent("あ\nい"), "あ\nい");
    // タブの字下げも取り除く
    assert_eq!(dedent("\tあ\n\tい"), "あ\nい");
    assert_eq!(dedent(""), "");
}

#[test]
fn データの種類は画面の並びで設定キーと対応する() {
    let keys: Vec<&str> = DataKind::ALL.iter().map(|k| k.config_key()).collect();
    assert_eq!(
        keys,
        [
            "personallyIdentifiable",
            "health",
            "financial",
            "authentication",
            "personalCommunications",
            "location",
            "webHistory",
            "userActivity",
            "websiteContent"
        ]
    );
    for kind in DataKind::ALL {
        assert_eq!(DataKind::parse(kind.config_key()), Some(kind));
    }
    // 大文字小文字は区別する・未知の名前は None
    assert_eq!(DataKind::parse("Health"), None);
    assert_eq!(DataKind::parse("unknown"), None);
    // 集合は画面の並び（Ord）で文字列にする
    let set: BTreeSet<DataKind> = [DataKind::Authentication, DataKind::PersonallyIdentifiable]
        .into_iter()
        .collect();
    assert_eq!(format_kinds(&set), "personallyIdentifiable, authentication");
}

#[test]
fn 対象の欄は設定に書いた欄だけを表示順に返す() {
    let e = expected();
    assert_eq!(
        e.fields(),
        vec![
            PrivacyField::SinglePurpose,
            PrivacyField::Permission("activeTab".to_string()),
            PrivacyField::Permission("host".to_string()),
            PrivacyField::DataCollected,
            PrivacyField::Certify,
        ]
    );
    assert_eq!(
        e.value_of(&PrivacyField::SinglePurpose).as_deref(),
        Some("目的")
    );
    assert_eq!(
        e.value_of(&PrivacyField::Permission("host".to_string()))
            .as_deref(),
        Some("ホストの理由")
    );
    assert_eq!(
        e.value_of(&PrivacyField::DataCollected).as_deref(),
        Some("personallyIdentifiable, authentication")
    );
    assert_eq!(e.value_of(&PrivacyField::Certify).as_deref(), Some("true"));
    assert_eq!(
        e.value_of(&PrivacyField::Permission("tabs".to_string())),
        None
    );

    // 書いていない欄は対象外
    let only_purpose = ExpectedPrivacy {
        single_purpose: Some("目的".to_string()),
        permissions: None,
        remote_code: None,
        data_usage: None,
    };
    assert_eq!(only_purpose.fields(), vec![PrivacyField::SinglePurpose]);
    assert!(!only_purpose.is_empty());
    assert!(
        ExpectedPrivacy {
            single_purpose: None,
            permissions: None,
            remote_code: None,
            data_usage: None
        }
        .is_empty()
    );
}

#[test]
fn すべて一致すれば一致() {
    let c = compare("id", "名前", &expected(), Some(&matching_reading()));
    assert_eq!(c.status(), ComparisonStatus::Match);
    assert_eq!(c.rows.len(), 5);
    assert!(c.rows.iter().all(|r| r.status == ComparisonStatus::Match));
    assert!(c.different_fields().is_empty());
    assert!(c.blockers().is_empty());
}

#[test]
fn 本文の違いは前後の空白も差分にする() {
    let mut reading = matching_reading();
    reading.single_purpose = Some("目的 ".to_string());
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(c.status(), ComparisonStatus::Different);
    assert_eq!(c.different_fields(), vec![&PrivacyField::SinglePurpose]);
    // 差分は書き換えで直せるので、止める理由にはならない
    assert!(c.blockers().is_empty());
}

#[test]
fn データの種類と宣誓の違いを検出する() {
    let mut reading = matching_reading();
    reading.collected = Some(
        [DataKind::PersonallyIdentifiable, DataKind::Health]
            .into_iter()
            .collect(),
    );
    reading.certified = Some(false);
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(
        c.different_fields(),
        vec![&PrivacyField::DataCollected, &PrivacyField::Certify]
    );
    let row = &c.rows[3];
    assert_eq!(
        row.current.as_deref(),
        Some("personallyIdentifiable, health")
    );
    assert_eq!(row.expected, "personallyIdentifiable, authentication");
}

#[test]
fn 設定にある権限の欄が画面に無ければ不整合として出す() {
    let mut reading = matching_reading();
    reading.screen_permissions = vec!["host".to_string()];
    reading.permissions.remove("activeTab");
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(c.status(), ComparisonStatus::Error);
    let blockers = c.blockers();
    assert_eq!(blockers.len(), 1);
    assert!(blockers[0].contains("activeTab"), "{blockers:?}");
    let row = &c.rows[1];
    assert_eq!(row.status, ComparisonStatus::Error);
    assert_eq!(row.current, None);
    assert!(row.note.is_some());
}

#[test]
fn 画面にある権限の欄が設定に無ければ不整合として出す() {
    let mut reading = matching_reading();
    reading.screen_permissions.push("storage".to_string());
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(c.status(), ComparisonStatus::Error);
    let blockers = c.blockers();
    assert_eq!(blockers.len(), 1);
    assert!(blockers[0].contains("storage"), "{blockers:?}");
    // 設定の欄は一致のまま（不整合の行が足されるだけ）
    assert_eq!(c.rows.len(), 6);
    assert_eq!(
        c.rows[5].field,
        PrivacyField::Permission("storage".to_string())
    );
}

#[test]
fn permissionsを書かなければ画面の権限は気にしない() {
    let mut e = expected();
    e.permissions = None;
    let mut reading = matching_reading();
    reading.screen_permissions.push("storage".to_string());
    let c = compare("id", "名前", &e, Some(&reading));
    assert_eq!(c.status(), ComparisonStatus::Match);
    assert_eq!(c.rows.len(), 3);
}

#[test]
fn 単一用途の欄が画面に無ければ不整合() {
    let mut reading = matching_reading();
    reading.single_purpose = None;
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(c.rows[0].status, ComparisonStatus::Error);
    assert_eq!(c.status(), ComparisonStatus::Error);
}

#[test]
fn データ使用の欄が画面に無ければ不整合() {
    let mut reading = matching_reading();
    reading.collected = None;
    reading.certified = None;
    let c = compare("id", "名前", &expected(), Some(&reading));
    assert_eq!(c.rows[3].status, ComparisonStatus::Error);
    assert_eq!(c.rows[4].status, ComparisonStatus::Error);
}

#[test]
fn リモートコードは画面側が未対応で不整合になる() {
    let mut e = expected();
    e.remote_code = Some(ExpectedRemoteCode {
        uses: false,
        justification: None,
    });
    let c = compare("id", "名前", &e, Some(&matching_reading()));
    assert_eq!(c.status(), ComparisonStatus::Error);
    let row = c
        .rows
        .iter()
        .find(|r| r.field == PrivacyField::RemoteCodeUses)
        .unwrap();
    assert_eq!(row.status, ComparisonStatus::Error);
    assert_eq!(row.expected, "false");
    // 使うときは正当性の欄も対象に入る
    e.remote_code = Some(ExpectedRemoteCode {
        uses: true,
        justification: Some("理由".to_string()),
    });
    assert!(e.fields().contains(&PrivacyField::RemoteCodeJustification));
}

#[test]
fn 読み出していなければ全欄が未取得() {
    let c = compare("id", "名前", &expected(), None);
    assert_eq!(c.status(), ComparisonStatus::NotFetched);
    assert!(
        c.rows
            .iter()
            .all(|r| r.status == ComparisonStatus::NotFetched)
    );
}

#[test]
fn アイテム全体の読み出し失敗はエラーが勝つ() {
    let mut c = compare("id", "名前", &expected(), None);
    c.error = Some("開けませんでした".to_string());
    assert_eq!(c.status(), ComparisonStatus::Error);
}

#[test]
fn 説明文の整形は欄の種類で変わる() {
    let mut reading = matching_reading();
    reading.single_purpose = Some("古い目的".to_string());
    reading.collected = Some(BTreeSet::new());
    let c = compare("id", "名前", &expected(), Some(&reading));

    // 一致は 1 行
    let match_row = &c.rows[1];
    assert_eq!(
        describe(match_row),
        "[プライバシー] 権限の正当性（activeTab）: 一致\n"
    );
    // 長い本文の欄は行単位の差分
    let text_row = &c.rows[0];
    let text = describe(text_row);
    assert!(
        text.starts_with("[プライバシー] 単一用途の説明: 差分あり\n"),
        "{text}"
    );
    assert!(text.contains("古い目的") && text.contains("目的"), "{text}");
    // 短い欄は現在値と正本を並べる
    let kind_row = &c.rows[3];
    assert_eq!(
        describe(kind_row),
        "[プライバシー] 収集するデータの種類: 差分あり\n- \n+ personallyIdentifiable, authentication\n"
    );
    // 不整合は理由つき
    reading.screen_permissions = vec!["host".to_string()];
    let c = compare("id", "名前", &expected(), Some(&reading));
    let e = describe(&c.rows[1]);
    assert!(
        e.starts_with("[プライバシー] 権限の正当性（activeTab）: エラー: "),
        "{e}"
    );
}

#[test]
fn 定数の値() {
    assert_eq!(privacy::TEXT_MAX_LENGTH, 1_000);
    assert_eq!(privacy::HOST_PERMISSION_KEY, "host");
}
