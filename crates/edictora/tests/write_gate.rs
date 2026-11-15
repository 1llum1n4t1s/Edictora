//! C# の WriteGate.test.cs の移植

use edictora::Error;
use edictora::dashboard::write_gate::{ItemPageState, ensure_allowed, evaluate};

fn state(
    status: Option<&str>,
    desc_editable: bool,
    save_found: bool,
    save_enabled: bool,
) -> ItemPageState {
    ItemPageState {
        status_text: status.map(str::to_string),
        description_found: true,
        description_editable: desc_editable,
        save_button_found: save_found,
        save_button_enabled: save_enabled,
    }
}

fn ok_state(status: Option<&str>) -> ItemPageState {
    state(status, true, true, true)
}

#[test]
fn evaluate_確認済みの編集可能な状態なら許可する() {
    assert!(evaluate(&ok_state(Some("公開済み"))).allowed);
    assert!(evaluate(&ok_state(Some("下書き"))).allowed);
    // 審査で却下された状態。掲載情報を直して再送信するために編集できる必要がある
    assert!(evaluate(&ok_state(Some("拒否"))).allowed);
    assert!(evaluate(&ok_state(Some("Rejected"))).allowed);
    // 却下でも、欄や保存ボタンが使えなければ止める
    assert!(!evaluate(&state(Some("拒否"), true, true, false)).allowed);
}

#[test]
fn evaluate_審査待ちは止める() {
    let decision = evaluate(&ok_state(Some("審査待ち")));
    assert!(!decision.allowed);
    assert!(decision.reason.contains("審査"));
}

#[test]
fn evaluate_ステータスが読めないか未知なら安全側に倒す() {
    assert!(!evaluate(&ok_state(None)).allowed);
    assert!(!evaluate(&ok_state(Some("  "))).allowed);
    assert!(!evaluate(&ok_state(Some("取り下げ"))).allowed);
}

#[test]
fn evaluate_審査中の語を含めば編集可能な語があっても止める() {
    assert!(!evaluate(&ok_state(Some("公開済み（更新は審査待ち）"))).allowed);
}

#[test]
fn evaluate_欄や保存ボタンが使えなければ止める() {
    assert!(!evaluate(&state(Some("公開済み"), false, true, true)).allowed);
    assert!(!evaluate(&state(Some("公開済み"), true, false, true)).allowed);
    assert!(!evaluate(&state(Some("公開済み"), true, true, false)).allowed);
    let not_found = ItemPageState {
        status_text: Some("公開済み".to_string()),
        description_found: false,
        description_editable: false,
        save_button_found: true,
        save_button_enabled: true,
    };
    assert!(!evaluate(&not_found).allowed);
}

#[test]
fn ensure_allowed_不可ならwrite_blocked() {
    assert!(matches!(
        ensure_allowed(&ok_state(Some("審査待ち"))),
        Err(Error::WriteBlocked(_))
    ));
    ensure_allowed(&ok_state(Some("公開済み"))).unwrap();
}

#[test]
fn evaluate_英語のステータス() {
    // 英語の表記は実画面で未確認。許可リストの語だけを許し、審査中の語は止める
    assert!(evaluate(&ok_state(Some("Published"))).allowed);
    assert!(evaluate(&ok_state(Some("Draft"))).allowed);
    assert!(!evaluate(&ok_state(Some("Pending review"))).allowed);
    assert!(!evaluate(&ok_state(Some("In review"))).allowed);
    assert!(!evaluate(&ok_state(Some("Published (update pending review)"))).allowed);
    assert!(!evaluate(&ok_state(Some("Taken down"))).allowed);
}

#[test]
fn evaluate_unpublishedは部分一致でpublishedに当たっても止める() {
    // 大文字小文字を区別しない部分一致だと "Unpublished" は "Published" を含む。確認済みの状態ではないので止める
    assert!(!evaluate(&ok_state(Some("Not published"))).allowed);
    let decision = evaluate(&ok_state(Some("Unpublished")));
    assert!(!decision.allowed);
    assert!(
        decision.reason.contains("確認済みの状態ではない"),
        "{}",
        decision.reason
    );
}
