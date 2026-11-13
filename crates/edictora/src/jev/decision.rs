//! ルールの結果 × Jev の答え → 採用・中止 を決める判断表（純粋関数）。
//!
//! | ルール | Jev の答え | 結果 |
//! |---|---|---|
//! | ちょうど 1 つ（i） | i | 採用（一致） |
//! | ちょうど 1 つ（i） | i 以外の候補・該当なし | 中止（食い違い） |
//! | 0 個 | 候補 j、確信度 ≥ しきい値 | 採用（Jev） |
//! | 複数（集合 M） | j ∈ M、確信度 ≥ しきい値 | 採用（Jev） |
//! | 複数（集合 M） | j ∉ M | 中止（ルールの候補の外） |
//! | 0 個・複数 | 確信度 < しきい値 | 中止 |
//! | 0 個・複数 | 該当なし | 中止 |
//!
//! ルールが複数に当てはまったときは、正解はその中にあるはずなので、Jev の選択もその中に限る
//! （ルールが当てはまらない要素を Jev だけの判断で選ぶのは、ルールが 0 個のときだけ）。

use crate::jev::{ChoiceAnswer, JevPick};

/// ルールで特定できなかったときに Jev の選択を採用する確信度のしきい値。
///
/// 0.8 にする理由: ルールの裏付けがない状態で別の欄を書き換える事故は取り返しがつきにくい。
/// Jev の確信度は選んだ選択肢の確率なので、0.8 以上なら他の選択肢（「該当なし」を含む）の合計は 0.2 以下で、
/// 迷っている答えを採らずに止められる。高すぎると、ルールが壊れたときの代わりとして使えなくなる。
/// 実画面で誤りや止まりすぎが見つかったら、ここだけを見直す。
pub const CONFIDENCE_THRESHOLD: f64 = 0.8;

/// ルール（`dashboard::selectors` の述語）で絞った結果。位置は Jev へ送る候補の添字
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleOutcome {
    /// ちょうど 1 つに決まった
    Unique(usize),
    /// 当てはまる候補がない
    NotFound,
    /// 複数に当てはまった（位置の一覧）
    Ambiguous(Vec<usize>),
}

impl RuleOutcome {
    /// ルールに当てはまった位置の一覧から作る
    pub fn from_matches(matches: Vec<usize>) -> RuleOutcome {
        match matches.len() {
            0 => RuleOutcome::NotFound,
            1 => RuleOutcome::Unique(matches[0]),
            _ => RuleOutcome::Ambiguous(matches),
        }
    }
}

/// 判断の結果
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// ルールと Jev が同じ候補を選んだ
    Agreed(usize),
    /// ルールで決まらず、Jev の選択を採用した
    JevAdopted(usize),
    /// 止める
    Stop(StopReason),
}

/// 止める理由
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    /// ルールで 1 つに決まったが、Jev は別の候補（または該当なし）を選んだ
    Disagreed { rule: usize, jev: JevPick },
    /// ルールで決まらず、Jev も「該当なし」と答えた
    NoneAnswered,
    /// ルールで決まらず、Jev の確信度がしきい値に届かない
    LowConfidence { pick: usize, confidence: f64 },
    /// ルールが複数に当てはまったが、Jev はその外の候補を選んだ
    OutsideRuleMatches { pick: usize },
}

/// 判断表を適用する
pub fn decide(rule: &RuleOutcome, answer: &ChoiceAnswer) -> Decision {
    match (rule, answer.pick) {
        (RuleOutcome::Unique(r), JevPick::Candidate(j)) if *r == j => Decision::Agreed(j),
        (RuleOutcome::Unique(r), jev) => Decision::Stop(StopReason::Disagreed { rule: *r, jev }),
        (_, JevPick::NoneOfThem) => Decision::Stop(StopReason::NoneAnswered),
        (RuleOutcome::Ambiguous(matches), JevPick::Candidate(j)) if !matches.contains(&j) => {
            Decision::Stop(StopReason::OutsideRuleMatches { pick: j })
        }
        (_, JevPick::Candidate(j)) if answer.confidence >= CONFIDENCE_THRESHOLD => {
            Decision::JevAdopted(j)
        }
        (_, JevPick::Candidate(j)) => Decision::Stop(StopReason::LowConfidence {
            pick: j,
            confidence: answer.confidence,
        }),
    }
}
