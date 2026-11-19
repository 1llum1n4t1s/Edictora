//! 「プライバシー」タブの手順（`services::pull_privacy` / `diff_privacy` / `push_privacy`）を、メモリ上の偽ページで確かめる。
//!
//! 画面の代わりに「保存済みの値」と「編集中の値」を持つ偽ページを使い、手順（順番・中止判断・保存と照合）だけを見る。
//! 偽ページは `PrivacyPage` だけを実装する（`ListingPage` を持たないので、型の上でもこの手順は審査送信を呼べない）。

use std::collections::{BTreeMap, BTreeSet};

use edictora::cancel::CancelToken;
use edictora::dashboard::write_gate::ItemPageState;
use edictora::dashboard::{EditorPage, PrivacyPage};
use edictora::listing::{ComparisonStatus, ExpectedItem};
use edictora::privacy::{
    DataKind, ExpectedDataUsage, ExpectedPrivacy, PrivacyField, PrivacyReading,
};
use edictora::services;
use edictora::{Error, Result};

mod common;
use common::ITEM_A as ITEM;

/// 画面の「プライバシー」タブの中身
#[derive(Debug, Clone, PartialEq, Eq)]
struct Screen {
    single_purpose: String,
    /// 画面にある権限の欄（キーは権限名か host。画面の並び）→ 本文
    permissions: Vec<(String, String)>,
    collected: BTreeSet<DataKind>,
    certified: bool,
    /// プライバシー ポリシー URL（None なら画面にその欄が無い）
    policy_url: Option<String>,
}

struct FakePrivacy {
    status: String,
    saved: Screen,
    editing: Screen,
    log: Vec<String>,
    /// n 回目（0 始まり）のテキスト書き換えを失敗させる
    fail_write_at: Option<usize>,
    writes: usize,
    /// 保存したとき、単一用途の本文を変えてしまう（保存後の照合失敗の再現）
    corrupt_on_save: bool,
    /// 開く操作そのものが失敗する（ダッシュボード系のエラー）
    fail_open: bool,
    save_button_enabled: bool,
}

fn screen() -> Screen {
    Screen {
        single_purpose: "目的".to_string(),
        permissions: vec![
            ("activeTab".to_string(), "タブ".to_string()),
            ("storage".to_string(), String::new()),
            ("host".to_string(), "ホスト".to_string()),
        ],
        collected: [DataKind::PersonallyIdentifiable].into_iter().collect(),
        certified: false,
        policy_url: Some("https://old.example.com/privacy".to_string()),
    }
}

impl FakePrivacy {
    fn new() -> FakePrivacy {
        FakePrivacy {
            status: "公開済み - 一般公開".to_string(),
            saved: screen(),
            editing: screen(),
            log: Vec::new(),
            fail_write_at: None,
            writes: 0,
            corrupt_on_save: false,
            fail_open: false,
            save_button_enabled: true,
        }
    }

    fn count(&self, prefix: &str) -> usize {
        self.log.iter().filter(|l| l.starts_with(prefix)).count()
    }
}

impl EditorPage for FakePrivacy {
    fn open_item(&mut self, _: &str, _: &CancelToken) -> Result<()> {
        panic!("プライバシー タブの手順が掲載情報の編集画面を開きました");
    }

    fn reload(&mut self, _: &CancelToken) -> Result<()> {
        self.log.push("reload".to_string());
        self.editing = self.saved.clone();
        Ok(())
    }

    fn get_state(&mut self, _: &CancelToken) -> Result<ItemPageState> {
        Ok(ItemPageState {
            status_text: Some(self.status.clone()),
            description_found: true,
            description_editable: true,
            save_button_found: true,
            save_button_enabled: self.save_button_enabled,
        })
    }

    fn get_locales(&mut self, _: &CancelToken) -> Result<Vec<String>> {
        panic!("プライバシー タブの手順が言語を読みました");
    }

    fn switch_locale(&mut self, _: &str, _: &CancelToken) -> Result<()> {
        panic!("プライバシー タブの手順が言語を切り替えました");
    }

    fn save_draft(&mut self, _: &CancelToken) -> Result<()> {
        self.log.push("save".to_string());
        if self.corrupt_on_save {
            self.editing.single_purpose.push_str("（変わった）");
        }
        self.saved = self.editing.clone();
        Ok(())
    }

    fn capture_diagnostics(&mut self, label: &str) -> Option<String> {
        self.log.push(format!("capture {label}"));
        None
    }
}

impl PrivacyPage for FakePrivacy {
    fn open_privacy(&mut self, item_id: &str, _: &CancelToken) -> Result<()> {
        self.log.push(format!("open {item_id}"));
        if self.fail_open {
            return Err(Error::Dashboard("開けませんでした".to_string()));
        }
        self.editing = self.saved.clone();
        Ok(())
    }

    fn read_privacy(
        &mut self,
        targets: &ExpectedPrivacy,
        _: &CancelToken,
    ) -> Result<PrivacyReading> {
        self.log.push("read".to_string());
        let mut r = PrivacyReading::default();
        if targets.single_purpose.is_some() {
            r.single_purpose = Some(self.editing.single_purpose.clone());
        }
        if let Some(wanted) = &targets.permissions {
            r.screen_permissions = self
                .editing
                .permissions
                .iter()
                .map(|(k, _)| k.clone())
                .collect();
            for (k, v) in &self.editing.permissions {
                if wanted.iter().any(|(w, _)| w == k) {
                    r.permissions.insert(k.clone(), v.clone());
                }
            }
        }
        if targets.data_usage.is_some() {
            r.collected = Some(self.editing.collected.clone());
            r.certified = Some(self.editing.certified);
        }
        if targets.policy_url.is_some() {
            r.policy_url = self.editing.policy_url.clone();
        }
        Ok(r)
    }

    fn write_privacy_text(
        &mut self,
        field: &PrivacyField,
        value: &str,
        _: &CancelToken,
    ) -> Result<()> {
        let n = self.writes;
        self.writes += 1;
        self.log.push(format!("write {field:?}"));
        if self.fail_write_at == Some(n) {
            return Err(Error::Dashboard("書き換えに失敗しました".to_string()));
        }
        match field {
            PrivacyField::SinglePurpose => self.editing.single_purpose = value.to_string(),
            PrivacyField::Permission(key) => {
                let slot = self
                    .editing
                    .permissions
                    .iter_mut()
                    .find(|(k, _)| k == key)
                    .expect("画面にある権限");
                slot.1 = value.to_string();
            }
            other => panic!("テキスト以外の欄を書き換えようとしました: {other:?}"),
        }
        Ok(())
    }

    fn set_data_kinds(&mut self, wanted: &BTreeSet<DataKind>, _: &CancelToken) -> Result<()> {
        self.log.push("set-kinds".to_string());
        self.editing.collected = wanted.clone();
        Ok(())
    }

    fn check_certifications(&mut self, _: &CancelToken) -> Result<()> {
        self.log.push("certify".to_string());
        self.editing.certified = true;
        Ok(())
    }

    fn write_policy_url(&mut self, value: &str, _: &CancelToken) -> Result<()> {
        self.log.push("write-url".to_string());
        if self.editing.policy_url.is_none() {
            return Err(Error::Dashboard(
                "プライバシー ポリシーの欄がありません".to_string(),
            ));
        }
        self.editing.policy_url = Some(value.to_string());
        Ok(())
    }
}

fn expected_privacy() -> ExpectedPrivacy {
    ExpectedPrivacy {
        single_purpose: Some("目的".to_string()),
        permissions: Some(vec![
            ("activeTab".to_string(), "タブ".to_string()),
            ("storage".to_string(), "保存の理由".to_string()),
            ("host".to_string(), "ホスト".to_string()),
        ]),
        remote_code: None,
        policy_url: None,
        data_usage: Some(ExpectedDataUsage {
            collected: [DataKind::PersonallyIdentifiable, DataKind::Authentication]
                .into_iter()
                .collect(),
            certify: true,
        }),
    }
}

fn item(privacy: ExpectedPrivacy) -> ExpectedItem {
    ExpectedItem {
        item_id: ITEM.to_string(),
        display_name: "テスト".to_string(),
        locales: Vec::new(),
        images: None,
        category: None,
        privacy: Some(privacy),
    }
}

fn no_progress(_: &str) {}

/// 画面の値が正本と同じ状態にした偽ページ
fn matching_page() -> FakePrivacy {
    let mut page = FakePrivacy::new();
    let e = expected_privacy();
    page.saved.permissions[1].1 = "保存の理由".to_string();
    page.saved.collected = e.data_usage.unwrap().collected;
    page.saved.certified = true;
    page.editing = page.saved.clone();
    page
}

#[test]
fn 一致していれば何も書かず保存もしない() {
    let mut page = matching_page();
    let result = services::push_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(result.updated.is_empty());
    assert_eq!(result.final_comparison.status(), ComparisonStatus::Match);
    assert_eq!(page.count("write"), 0);
    assert_eq!(page.count("save"), 0);
    assert_eq!(page.count("reload"), 0);
    assert_eq!(page.count("open"), 1);
}

#[test]
fn 差分のある欄だけを書き換えて保存し読み直して照合する() {
    let mut page = FakePrivacy::new();
    let result = services::push_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    // storage の本文・データの種類・宣誓だけが差分（単一用途・activeTab・host は一致）
    assert_eq!(
        result.updated,
        vec![
            PrivacyField::Permission("storage".to_string()),
            PrivacyField::DataCollected,
            PrivacyField::Certify
        ]
    );
    assert_eq!(page.count("write"), 1);
    assert!(
        page.log
            .contains(&"write Permission(\"storage\")".to_string())
    );
    assert_eq!(page.count("set-kinds"), 1);
    assert_eq!(page.count("certify"), 1);
    assert_eq!(page.count("save"), 1);
    assert_eq!(page.count("reload"), 1);
    assert_eq!(result.final_comparison.status(), ComparisonStatus::Match);
    assert_eq!(page.saved.permissions[1].1, "保存の理由");
    assert!(page.saved.certified);
    assert_eq!(page.saved.collected.len(), 2);

    // 順序: 開く → 読む → 書く → 保存前の照合（読む）→ 保存 → 読み直し → 読む
    let kinds: Vec<&str> = page
        .log
        .iter()
        .map(|l| l.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "open",
            "read",
            "write",
            "set-kinds",
            "certify",
            "read",
            "save",
            "reload",
            "read"
        ]
    );
    // 画面の他の値は動かさない
    assert_eq!(page.saved.single_purpose, "目的");
    assert_eq!(page.saved.permissions[0].1, "タブ");
}

#[test]
fn 画面に無い権限が設定にあれば何も書かずに止める() {
    let mut page = matching_page();
    let mut e = expected_privacy();
    e.permissions
        .as_mut()
        .unwrap()
        .push(("tabs".to_string(), "理由".to_string()));
    // 差分のある欄があっても、不整合があれば何も書かない
    e.single_purpose = Some("別の目的".to_string());
    let err =
        services::push_privacy(&mut page, &item(e), &no_progress, &CancelToken::new()).unwrap_err();
    let Error::WriteBlocked(m) = err else {
        panic!("WriteBlocked を期待しました");
    };
    assert!(m.contains("tabs"), "{m}");
    assert_eq!(page.count("write"), 0);
    assert_eq!(page.count("save"), 0);
    // 失敗時の画面を保存する
    assert_eq!(page.count("capture push-privacy-"), 1);
}

#[test]
fn 設定に無い権限が画面にあれば何も書かずに止める() {
    let mut page = matching_page();
    page.saved
        .permissions
        .push(("tabs".to_string(), String::new()));
    page.editing = page.saved.clone();
    let err = services::push_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();
    let Error::WriteBlocked(m) = err else {
        panic!("WriteBlocked を期待しました");
    };
    assert!(m.contains("tabs"), "{m}");
    assert_eq!(page.count("write") + page.count("save"), 0);
}

#[test]
fn リモートコードを設定に書くと止まる() {
    let mut page = matching_page();
    let mut e = expected_privacy();
    e.remote_code = Some(edictora::privacy::ExpectedRemoteCode {
        uses: false,
        justification: None,
    });
    let err =
        services::push_privacy(&mut page, &item(e), &no_progress, &CancelToken::new()).unwrap_err();
    assert!(matches!(err, Error::WriteBlocked(_)));
    assert_eq!(page.count("write") + page.count("save"), 0);
}

#[test]
fn 書き込めない状態なら何も触らずに止める() {
    for status in ["審査待ち", "審査中"] {
        let mut page = FakePrivacy::new();
        page.status = status.to_string();
        let err = services::push_privacy(
            &mut page,
            &item(expected_privacy()),
            &no_progress,
            &CancelToken::new(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::WriteBlocked(_)), "{status}");
        // 読みもせず、書きもしない
        assert_eq!(
            page.count("read") + page.count("write") + page.count("save"),
            0
        );
    }
}

#[test]
fn 保存ボタンが押せなければ保存せずに止める() {
    let mut page = FakePrivacy::new();
    page.save_button_enabled = false;
    // 最初の確認で止まる（書き換える前）
    let err = services::push_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::WriteBlocked(_)));
    assert_eq!(page.count("write") + page.count("save"), 0);
}

#[test]
fn 書き換えに失敗したら保存せずに残りも行わない() {
    let mut page = FakePrivacy::new();
    page.fail_write_at = Some(0);
    let mut e = expected_privacy();
    // 差分のあるテキスト欄を 2 つにする
    e.single_purpose = Some("別の目的".to_string());
    let err =
        services::push_privacy(&mut page, &item(e), &no_progress, &CancelToken::new()).unwrap_err();
    assert!(matches!(err, Error::Dashboard(_)));
    assert_eq!(page.count("write"), 1, "1 つ目の失敗で残りは行わない");
    assert_eq!(page.count("set-kinds") + page.count("certify"), 0);
    assert_eq!(page.count("save"), 0);
    // 失敗時の画面を保存する
    assert_eq!(page.count("capture push-privacy-"), 1);
    // 保存していないので、保存済みの値は元のまま
    assert_eq!(page.saved, screen());
}

#[test]
fn 保存後の照合が合わなければ失敗にする() {
    let mut page = FakePrivacy::new();
    page.corrupt_on_save = true;
    let mut e = expected_privacy();
    e.single_purpose = Some("別の目的".to_string());
    let err =
        services::push_privacy(&mut page, &item(e), &no_progress, &CancelToken::new()).unwrap_err();
    let Error::Dashboard(m) = err else {
        panic!("Dashboard エラーを期待しました");
    };
    assert!(m.contains("単一用途の説明"), "{m}");
    assert!(m.contains("保存後"), "{m}");
    assert_eq!(page.count("save"), 1);
    assert_eq!(page.count("capture push-privacy-"), 1);
}

#[test]
fn 書いていない欄は動かさない() {
    // データ使用だけを対象にする。画面の他の欄が違っていても触らない
    let mut page = FakePrivacy::new();
    let only_usage = ExpectedPrivacy {
        single_purpose: None,
        permissions: None,
        remote_code: None,
        policy_url: None,
        data_usage: Some(ExpectedDataUsage {
            collected: BTreeSet::new(),
            certify: true,
        }),
    };
    let result = services::push_privacy(
        &mut page,
        &item(only_usage),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(
        result.updated,
        vec![PrivacyField::DataCollected, PrivacyField::Certify]
    );
    assert_eq!(page.count("write"), 0);
    assert_eq!(page.saved.single_purpose, "目的");
    assert_eq!(page.saved.permissions, screen().permissions);
    assert!(page.saved.collected.is_empty());
}

#[test]
fn 読み出しはテキストの正本に書いた欄だけを返す() {
    let mut page = FakePrivacy::new();
    let snapshot = services::pull_privacy(
        &mut page,
        &item(ExpectedPrivacy {
            single_purpose: Some("x".to_string()),
            permissions: None,
            remote_code: None,
            policy_url: None,
            data_usage: None,
        }),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(snapshot.error.is_none());
    assert_eq!(snapshot.status_text.as_deref(), Some("公開済み - 一般公開"));
    let reading = snapshot.reading.unwrap();
    assert_eq!(reading.single_purpose.as_deref(), Some("目的"));
    assert!(reading.screen_permissions.is_empty());
    assert_eq!(reading.permissions, BTreeMap::new());
    assert!(reading.collected.is_none());
}

#[test]
fn 読み出しに失敗したらアイテム単位のエラーとして返す() {
    let mut page = FakePrivacy::new();
    page.fail_open = true;
    let snapshot = services::pull_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(
        snapshot
            .error
            .as_deref()
            .unwrap()
            .contains("開けませんでした")
    );
    assert!(snapshot.reading.is_none());
    // 失敗時の画面を保存する（pull の失敗）
    assert_eq!(page.count("capture pull-"), 1);
    // 比較は全欄が未取得＋エラー
    let c = services::compare_privacy(&item(expected_privacy()), &snapshot);
    assert_eq!(c.status(), ComparisonStatus::Error);
    assert!(
        c.rows
            .iter()
            .all(|r| r.status == ComparisonStatus::NotFetched)
    );
}

#[test]
fn ログイン切れはそのまま返す() {
    struct LoggedOut(FakePrivacy);
    impl EditorPage for LoggedOut {
        fn open_item(&mut self, i: &str, c: &CancelToken) -> Result<()> {
            self.0.open_item(i, c)
        }
        fn reload(&mut self, c: &CancelToken) -> Result<()> {
            self.0.reload(c)
        }
        fn get_state(&mut self, c: &CancelToken) -> Result<ItemPageState> {
            self.0.get_state(c)
        }
        fn get_locales(&mut self, c: &CancelToken) -> Result<Vec<String>> {
            self.0.get_locales(c)
        }
        fn switch_locale(&mut self, code: &str, c: &CancelToken) -> Result<()> {
            self.0.switch_locale(code, c)
        }
        fn save_draft(&mut self, c: &CancelToken) -> Result<()> {
            self.0.save_draft(c)
        }
        fn capture_diagnostics(&mut self, l: &str) -> Option<String> {
            self.0.capture_diagnostics(l)
        }
    }
    impl PrivacyPage for LoggedOut {
        fn open_privacy(&mut self, _: &str, _: &CancelToken) -> Result<()> {
            Err(Error::LoginRequired)
        }
        fn read_privacy(&mut self, t: &ExpectedPrivacy, c: &CancelToken) -> Result<PrivacyReading> {
            self.0.read_privacy(t, c)
        }
        fn write_privacy_text(&mut self, f: &PrivacyField, v: &str, c: &CancelToken) -> Result<()> {
            self.0.write_privacy_text(f, v, c)
        }
        fn set_data_kinds(&mut self, w: &BTreeSet<DataKind>, c: &CancelToken) -> Result<()> {
            self.0.set_data_kinds(w, c)
        }
        fn check_certifications(&mut self, c: &CancelToken) -> Result<()> {
            self.0.check_certifications(c)
        }
        fn write_policy_url(&mut self, v: &str, c: &CancelToken) -> Result<()> {
            self.0.write_policy_url(v, c)
        }
    }
    let mut page = LoggedOut(FakePrivacy::new());
    let e = item(expected_privacy());
    assert_eq!(
        services::pull_privacy(&mut page, &e, &no_progress, &CancelToken::new()).unwrap_err(),
        Error::LoginRequired
    );
    assert_eq!(
        services::push_privacy(&mut page, &e, &no_progress, &CancelToken::new()).unwrap_err(),
        Error::LoginRequired
    );
    // ログイン切れでは画面を保存しない
    assert_eq!(page.0.count("capture"), 0);
}

#[test]
fn diffは読み取りだけで書かない() {
    let mut page = FakePrivacy::new();
    let (snapshot, comparison) = services::diff_privacy(
        &mut page,
        &item(expected_privacy()),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(snapshot.error.is_none());
    assert_eq!(comparison.status(), ComparisonStatus::Different);
    assert_eq!(
        page.count("write") + page.count("save") + page.count("set-kinds"),
        0
    );
}

// ───────── プライバシー ポリシー URL ─────────

fn only_policy_url(url: &str) -> ExpectedPrivacy {
    ExpectedPrivacy {
        single_purpose: None,
        permissions: None,
        remote_code: None,
        data_usage: None,
        policy_url: Some(url.to_string()),
    }
}

#[test]
fn policyurlだけを書いたアイテムも対象になる() {
    let p = only_policy_url("https://example.com/privacy");
    assert!(!p.is_empty());
    assert_eq!(p.fields(), vec![PrivacyField::PolicyUrl]);
    assert!(item(p).has_privacy());
    // 空文字は「空欄が正本」で、対象として残る
    assert!(!only_policy_url("").is_empty());
}

#[test]
fn policyurlの差分だけを書き換え他の欄は動かさない() {
    let mut page = FakePrivacy::new();
    let result = services::push_privacy(
        &mut page,
        &item(only_policy_url("https://example.com/privacy")),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(result.updated, vec![PrivacyField::PolicyUrl]);
    assert_eq!(page.count("write-url"), 1);
    assert_eq!(page.count("write "), 0);
    assert_eq!(
        page.saved.policy_url.as_deref(),
        Some("https://example.com/privacy")
    );
    assert_eq!(page.saved.single_purpose, "目的");
    assert_eq!(page.count("save"), 1);
}

#[test]
fn policyurlが一致していれば書かず保存もしない() {
    let mut page = FakePrivacy::new();
    let result = services::push_privacy(
        &mut page,
        &item(only_policy_url("https://old.example.com/privacy")),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(result.updated.is_empty());
    assert_eq!(page.count("write-url"), 0);
    assert_eq!(page.count("save"), 0);
}

#[test]
fn policyurlの空文字の正本は空欄へ書き換える() {
    let mut page = FakePrivacy::new();
    let result = services::push_privacy(
        &mut page,
        &item(only_policy_url("")),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(result.updated, vec![PrivacyField::PolicyUrl]);
    assert_eq!(page.saved.policy_url.as_deref(), Some(""));
}

#[test]
fn policyurlの欄が画面に無ければ何も書かずに止める() {
    let mut page = FakePrivacy::new();
    page.saved.policy_url = None;
    let e = services::push_privacy(
        &mut page,
        &item(only_policy_url("https://example.com/privacy")),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::WriteBlocked(_)));
    assert_eq!(page.count("write-url"), 0);
    assert_eq!(page.count("save"), 0);
}

#[test]
fn policyurlの差分は1行の差分として出る() {
    let mut page = FakePrivacy::new();
    let (_, comparison) = services::diff_privacy(
        &mut page,
        &item(only_policy_url("https://example.com/privacy")),
        &no_progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(comparison.rows.len(), 1);
    let text = edictora::privacy::describe(&comparison.rows[0]);
    assert!(text.contains("- https://old.example.com/privacy"), "{text}");
    assert!(text.contains("+ https://example.com/privacy"), "{text}");
    assert!(!PrivacyField::PolicyUrl.is_long_text());
}
