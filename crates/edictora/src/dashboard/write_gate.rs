//! 下書き保存・審査送信をしてよい状態かを判定する純粋関数（C# の `WriteGate`）。
//!
//! 審査待ちのアイテムの編集画面はまだ実画面で確認していない（DESIGN.md「まだ確かめていないこと」）。
//! そのため「書いてよいと確認できた状態」以外はすべて止める（許可リスト方式）。
//! - ステータスが読めない → 止める
//! - 審査中を示す語を含む → 止める（審査の取り消しは絶対にしない）
//! - 既知の編集可能な状態（公開済み・下書きなど）でない → 止める
//! - 説明欄が編集不可、または保存ボタンが無い・押せない → 止める
//!
//! 実画面で新しいステータス表記を確認したら、[`EDITABLE_STATUS_KEYWORDS`] に足す。

use crate::dashboard::text::SAVE_DRAFT_BUTTON;
use crate::error::{Error, Result};
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

/// 書き込んでよいと判断できる状態の語
pub const EDITABLE_STATUS_KEYWORDS: [&str; 4] = ["公開済み", "下書き", "Published", "Draft"];

/// 書き込み可否を判定する
pub fn evaluate(state: &ItemPageState) -> WriteDecision {
    let status = match state.status_text.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return deny(
                "アイテムのステータスを読み取れません。安全のため書き込みを止めます".to_string(),
            );
        }
    };

    if LOCKED_STATUS_KEYWORDS
        .iter()
        .any(|k| contains_ic(status, k))
    {
        return deny(format!(
            "ステータスが「{status}」のため書き込みません（審査の取り消しはしません）"
        ));
    }
    if !EDITABLE_STATUS_KEYWORDS
        .iter()
        .any(|k| contains_ic(status, k))
    {
        return deny(format!(
            "ステータス「{status}」は書き込みを確認済みの状態ではないため、安全のため止めます"
        ));
    }
    if !state.description_found {
        return deny("説明欄が見つからないため書き込みを止めます".to_string());
    }
    if !state.description_editable {
        return deny("説明欄が編集できない状態のため書き込みを止めます".to_string());
    }
    if !state.save_button_found {
        return deny(format!(
            "「{SAVE_DRAFT_BUTTON}」ボタンが見つからないため書き込みを止めます"
        ));
    }
    if !state.save_button_enabled {
        return deny(format!(
            "「{SAVE_DRAFT_BUTTON}」ボタンが押せない状態のため書き込みを止めます"
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
