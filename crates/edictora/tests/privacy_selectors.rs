//! 「プライバシー」タブの要素の選び方（`dashboard::selectors` の純粋関数）。
//! ラベルの形は 2026-11-18 の実画面（ReviewForMD ほか 15 アイテム）で確かめたもの:
//! textarea のラベルは aria-labelledby の見出し（同じ行が重なる）と、カウンタを含む祖先の文字列、
//! チェックボックスは aria-label と、例の説明を含むラベル。

use edictora::Error;
use edictora::dashboard::selectors::{
    ElementInfo, certified, classify_privacy_textareas, collected_kinds,
    ensure_privacy_text_writable, pick_certify_checkbox, pick_data_kind_checkbox,
    privacy_page_state,
};
use edictora::dashboard::text::{EN, JA, PrivacyText};
use edictora::privacy::DataKind;

fn p() -> PrivacyText {
    JA.privacy.expect("日本語の画面は文言を持つ")
}

/// 実画面の textarea のラベル（見出しの行が 3 回重なり、カウンタの行が続く）
fn textarea(index: i32, heading: &str) -> ElementInfo {
    ElementInfo {
        index,
        tag: "textarea".to_string(),
        label: format!(
            "{heading}\n{heading}\n{heading}\n{heading}\n126/1,000\n1000文字中126文字入力しました"
        ),
        ..ElementInfo::default()
    }
}

fn checkbox(index: i32, aria: &str, checked: bool) -> ElementInfo {
    ElementInfo {
        index,
        tag: "input".to_string(),
        r#type: "checkbox".to_string(),
        label: format!("{aria}\n{aria}\n例: 名前、住所"),
        checked: Some(checked),
        ..ElementInfo::default()
    }
}

/// 9 つのデータの種類（`checked` に含まれるものだけチェック済み）と 3 つの宣誓
fn checkboxes(checked: &[DataKind], certify: [bool; 3]) -> Vec<ElementInfo> {
    let p = p();
    let mut v = Vec::new();
    for (i, kind) in DataKind::ALL.iter().enumerate() {
        v.push(checkbox(
            i as i32,
            p.data_kind_labels[i],
            checked.contains(kind),
        ));
    }
    for (i, label) in p.certify_labels.iter().enumerate() {
        v.push(checkbox(9 + i as i32, label, certify[i]));
    }
    v
}

#[test]
fn 英語の画面はプライバシータブの文言を持たない() {
    // 実画面で未確認の言語は、このタブの読み書きを許さない（推測で文言を足さない）
    assert!(EN.privacy.is_none());
    assert!(JA.privacy.is_some());
}

#[test]
fn textareaを単一用途と権限に分類する() {
    let t = [
        textarea(0, "単一用途の説明"),
        textarea(1, "activeTab が必要な理由"),
        textarea(2, "storage が必要な理由"),
        textarea(3, "ホスト権限 が必要な理由"),
    ];
    let areas = classify_privacy_textareas(&p(), &t).unwrap();
    assert_eq!(areas.single_purpose.unwrap().index, 0);
    let keys: Vec<(&str, i32)> = areas
        .permissions
        .iter()
        .map(|(k, e)| (k.as_str(), e.index))
        .collect();
    // ホスト権限は特別なキー host
    assert_eq!(keys, [("activeTab", 1), ("storage", 2), ("host", 3)]);
}

#[test]
fn 空白の有無は区別しない() {
    let t = [
        textarea(0, "単一用途の説明"),
        textarea(1, "scripting が必要な理由"),
        textarea(2, "ホスト権限が必要な理由"),
    ];
    let areas = classify_privacy_textareas(&p(), &t).unwrap();
    let keys: Vec<&str> = areas.permissions.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["scripting", "host"]);
}

#[test]
fn 説明文の行は権限の欄に当たらない() {
    // ホスト権限の欄の祖先には「ホスト権限は、…マッチパターンです」の説明の行が入る
    let mut host = textarea(3, "ホスト権限 が必要な理由");
    host.label.push_str(
        "\nホスト権限は、拡張機能のマニフェストの「permissions」フィールドと「content_scripts」フィールドで指定されたマッチパターンです",
    );
    let t = [textarea(0, "単一用途の説明"), host];
    let areas = classify_privacy_textareas(&p(), &t).unwrap();
    assert_eq!(areas.permissions.len(), 1);
    assert_eq!(areas.permissions[0].0, "host");
}

#[test]
fn どちらでもない入力欄は無視する() {
    let t = [
        textarea(0, "単一用途の説明"),
        textarea(1, "リモートコードを使用する理由"),
    ];
    let areas = classify_privacy_textareas(&p(), &t).unwrap();
    assert!(areas.permissions.is_empty());
    assert!(areas.single_purpose.is_some());
}

#[test]
fn 欄が無ければnone() {
    let areas = classify_privacy_textareas(&p(), &[]).unwrap();
    assert!(areas.single_purpose.is_none() && areas.permissions.is_empty());
}

#[test]
fn 絞れないときは止める() {
    // 単一用途の欄が 2 つ
    let two_purposes = [textarea(0, "単一用途の説明"), textarea(1, "単一用途の説明")];
    assert!(matches!(
        classify_privacy_textareas(&p(), &two_purposes),
        Err(Error::Dashboard(_))
    ));
    // 同じ権限の欄が 2 つ
    let two_same = [
        textarea(0, "storage が必要な理由"),
        textarea(1, "storage が必要な理由"),
    ];
    assert!(matches!(
        classify_privacy_textareas(&p(), &two_same),
        Err(Error::Dashboard(_))
    ));
    // 1 つの欄が複数の権限名に当たる
    let mut two_names = textarea(0, "storage が必要な理由");
    two_names.label.push_str("\ntabs が必要な理由");
    assert!(matches!(
        classify_privacy_textareas(&p(), &[two_names]),
        Err(Error::Dashboard(_))
    ));
    // 単一用途と権限の両方に当たる
    let mut both = textarea(0, "単一用途の説明");
    both.label.push_str("\nstorage が必要な理由");
    assert!(matches!(
        classify_privacy_textareas(&p(), &[both]),
        Err(Error::Dashboard(_))
    ));
}

#[test]
fn データの種類のチェック状態を読む() {
    let checks = checkboxes(
        &[DataKind::PersonallyIdentifiable, DataKind::Authentication],
        [true, true, true],
    );
    let set = collected_kinds(&p(), &checks).unwrap().unwrap();
    assert_eq!(
        set.into_iter().collect::<Vec<_>>(),
        [DataKind::PersonallyIdentifiable, DataKind::Authentication]
    );
    assert_eq!(certified(&p(), &checks).unwrap(), Some(true));

    let partial = checkboxes(&[], [true, false, true]);
    assert!(collected_kinds(&p(), &partial).unwrap().unwrap().is_empty());
    assert_eq!(certified(&p(), &partial).unwrap(), Some(false));
}

#[test]
fn チェックボックスが揃っていなければnone() {
    let mut checks = checkboxes(&[], [true, true, true]);
    checks.remove(4); // 個人的コミュニケーション
    assert_eq!(collected_kinds(&p(), &checks).unwrap(), None);
    let mut checks = checkboxes(&[], [true, true, true]);
    checks.pop(); // 3 つ目の宣誓
    assert_eq!(certified(&p(), &checks).unwrap(), None);
    assert_eq!(collected_kinds(&p(), &[]).unwrap(), None);
}

#[test]
fn 状態の分からないチェックボックスは数えない() {
    let mut checks = checkboxes(&[], [true, true, true]);
    checks[0].checked = None;
    assert_eq!(collected_kinds(&p(), &checks).unwrap(), None);
}

#[test]
fn 同じラベルのチェックボックスが複数あれば止める() {
    let mut checks = checkboxes(&[], [true, true, true]);
    checks.push(checkbox(20, p().data_kind_labels[0], false));
    assert!(matches!(
        collected_kinds(&p(), &checks),
        Err(Error::Dashboard(_))
    ));
}

#[test]
fn ラベルの一部だけが一致するものは選ばない() {
    // 「個人を特定できる情報」を含むだけの別のラベルは当たらない（1 行の完全一致）
    let mut checks = checkboxes(&[], [true, true, true]);
    checks[0].label = "個人を特定できる情報以外\n例: x".to_string();
    assert_eq!(collected_kinds(&p(), &checks).unwrap(), None);
}

#[test]
fn チェックボックスを1つ選ぶ() {
    let checks = checkboxes(&[DataKind::Health], [true, true, true]);
    let health = pick_data_kind_checkbox(&p(), &checks, DataKind::Health).unwrap();
    assert_eq!(health.index, 1);
    assert_eq!(health.checked, Some(true));
    let third = pick_certify_checkbox(&p(), &checks, 2).unwrap();
    assert_eq!(third.index, 11);
    // 見つからなければ止める
    assert!(matches!(
        pick_data_kind_checkbox(&p(), &[], DataKind::Health),
        Err(Error::Dashboard(_))
    ));
    assert!(matches!(
        pick_certify_checkbox(&p(), &[], 0),
        Err(Error::Dashboard(_))
    ));
}

#[test]
fn 書き込み可否の判断材料は単一用途の欄と保存ボタンで作る() {
    let t = [
        textarea(0, "単一用途の説明"),
        textarea(1, "storage が必要な理由"),
    ];
    let save = ElementInfo {
        index: 0,
        tag: "button".to_string(),
        text: "下書きとして保存する".to_string(),
        ..ElementInfo::default()
    };
    let state = privacy_page_state(
        &JA,
        &p(),
        Some("公開済み - 一般公開".to_string()),
        &t,
        std::slice::from_ref(&save),
    );
    assert!(state.description_found && state.description_editable);
    assert!(state.save_button_found && state.save_button_enabled);
    assert_eq!(state.status_text.as_deref(), Some("公開済み - 一般公開"));

    // 単一用途の欄が編集できない
    let mut locked = t.clone();
    locked[0].read_only = true;
    let state = privacy_page_state(&JA, &p(), None, &locked, std::slice::from_ref(&save));
    assert!(state.description_found && !state.description_editable);

    // 単一用途の欄が無い・絞れない（分類できない）ときは「見つからない」
    let state = privacy_page_state(&JA, &p(), None, &t[1..], std::slice::from_ref(&save));
    assert!(!state.description_found);
    let two = [textarea(0, "単一用途の説明"), textarea(1, "単一用途の説明")];
    let state = privacy_page_state(&JA, &p(), None, &two, std::slice::from_ref(&save));
    assert!(!state.description_found);
}

#[test]
fn 書き込み先として使える欄か確かめる() {
    let ok = textarea(0, "単一用途の説明");
    ensure_privacy_text_writable(&ok, "単一用途の説明").unwrap();

    let mut input = ok.clone();
    input.tag = "input".to_string();
    assert!(matches!(
        ensure_privacy_text_writable(&input, "x"),
        Err(Error::Dashboard(_))
    ));
    let mut disabled = ok.clone();
    disabled.disabled = true;
    assert!(matches!(
        ensure_privacy_text_writable(&disabled, "x"),
        Err(Error::WriteBlocked(_))
    ));
    let mut read_only = ok;
    read_only.read_only = true;
    assert!(matches!(
        ensure_privacy_text_writable(&read_only, "x"),
        Err(Error::WriteBlocked(_))
    ));
}
