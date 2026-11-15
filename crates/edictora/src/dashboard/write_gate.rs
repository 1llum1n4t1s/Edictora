//! 下書き保存・審査送信をしてよい状態かを判定する純粋関数（C# の `WriteGate`）。
//!
//! 審査待ちのアイテムの編集画面はまだ実画面で確認していない（DESIGN.md「実画面での確認状況」）。
//! そのため「書いてよいと確認できた状態」以外はすべて止める（許可リスト方式）。
//! - ステータスが読めない → 止める
//! - 審査中を示す語を含む → 止める（審査の取り消しは絶対にしない）
//! - 既知の編集可能な状態（公開済み・下書きなど）でない → 止める
//! - 説明欄が編集不可、または保存ボタンが無い・押せない → 止める
//!
//! 実画面で新しいステータス表記を確認したら、[`EDITABLE_STATUS_KEYWORDS`] に足す。
//!
//! ステータスの語は日本語・英語の両方を 1 つの一覧で持つ（画面の言語に依らず同じ判定）。
//! 英語の語は実画面で未確認。英語で新しい語を足すときは「止める」側だけにし、「書いてよい」側に推測で足さない。

use crate::error::{Error, Result};
use crate::tr;
use crate::util::contains_ic;

/// 編集画面の状態（書き込み可否の判断材料）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemPageState {
    /// 「ステータス: …」の値（読めなければ None）
    pub status_text: Option<String>,
    /// 説明欄（textarea）が 1 つ見つかったか
    pub description_found: bool,
    /// 説明欄が disabled / readOnly でないか
    pub description_editable: bool,
    /// 「下書きとして保存する」ボタンが 1 つ見つかったか
    pub save_button_found: bool,
    /// そのボタンが押せる状態か
    pub save_button_enabled: bool,
}

/// 書き込み可否の判定結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDecision {
    /// 書き込んでよいか
    pub allowed: bool,
    /// 不可の理由（可なら空）
    pub reason: String,
}

/// 審査中・ロック中を示す語（1 つでも含めば書き込まない）
pub const LOCKED_STATUS_KEYWORDS: [&str; 7] = [
    "審査待ち",
    "審査中",
    "審査保留",
    "保留中",
    "Pending",
    "In review",
    "Under review",
];

/// 編集可能な状態の語を含んでいても、確認済みの状態ではないので止める語（部分一致の取り違えを防ぐ）。
///
/// 例: 英語の "Unpublished"・"Not published" は大文字小文字を区別しない部分一致だと "Published" を含み、許可されてしまう。
/// 非公開のアイテムへの書き込みは実画面で確かめていないので止める（英語の表記も実画面で未確認）。
pub const UNCONFIRMED_LOOKALIKE_KEYWORDS: [&str; 2] = ["Unpublished", "Not published"];

/// 書き込んでよいと判断できる状態の語。
/// 「拒否」は審査で却下された状態で、掲載情報を直して再送信するために編集する必要がある。
/// 実画面（2026-11-15、ステータス「拒否」）で説明欄が編集でき、「下書きとして保存する」が押せることを確認済み。
/// 説明欄・保存ボタンの確認は他の状態と同じく後段で行う。
pub const EDITABLE_STATUS_KEYWORDS: [&str; 6] = [
    "公開済み",
    "下書き",
    "拒否",
    "Published",
    "Draft",
    "Rejected",
];

/// 書き込み可否を判定する
pub fn evaluate(state: &ItemPageState) -> WriteDecision {
    let status = match state.status_text.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return deny(tr!("dashboard.cannot_read_the_item_status_stopping_the"));
        }
    };

    if LOCKED_STATUS_KEYWORDS
        .iter()
        .any(|k| contains_ic(status, k))
    {
        return deny(tr!(
            "dashboard.not_writing_because_the_status_is_the",
            status = status
        ));
    }
    let unconfirmed = UNCONFIRMED_LOOKALIKE_KEYWORDS
        .iter()
        .any(|k| contains_ic(status, k));
    if unconfirmed
        || !EDITABLE_STATUS_KEYWORDS
            .iter()
            .any(|k| contains_ic(status, k))
    {
        return deny(tr!(
            "dashboard.stopping_to_be_safe_because_writing_has",
            status = status
        ));
    }
    if !state.description_found {
        return deny(tr!(
            "dashboard.stopping_the_write_because_the_description_field"
        ));
    }
    if !state.description_editable {
        return deny(tr!(
            "dashboard.stopping_the_write_because_the_description_field_2"
        ));
    }
    // ボタンの文言は画面の言語で違うが、この判定は画面の言語を受け取らない（`ensure_allowed` の署名を保つため）。
    // 表示言語を問わず、文中のボタン名には日本語の画面の文言（`text::JA`）を {save} として渡す
    let save = crate::dashboard::text::JA.save_draft_button;
    if !state.save_button_found {
        return deny(tr!(
            "dashboard.stopping_the_write_because_the_save_draft",
            save = save
        ));
    }
    if !state.save_button_enabled {
        return deny(tr!(
            "dashboard.stopping_the_write_because_the_save_draft_2",
            save = save
        ));
    }
    WriteDecision {
        allowed: true,
        reason: String::new(),
    }
}

/// 判定し、不可なら `WriteBlocked` エラー
pub fn ensure_allowed(state: &ItemPageState) -> Result<()> {
    let decision = evaluate(state);
    if decision.allowed {
        Ok(())
    } else {
        Err(Error::WriteBlocked(decision.reason))
    }
}

fn deny(reason: String) -> WriteDecision {
    WriteDecision {
        allowed: false,
        reason,
    }
}
