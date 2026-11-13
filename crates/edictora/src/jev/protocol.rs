//! TypeSafe Jev（`POST /v1/systemone`）の送信本文の組み立てと応答の検証（純粋関数）。
//!
//! 仕様（https://api.typesafe.ai/openapi.json）のうち、使うのは choice 質問だけ。
//! - 送信: `{"model", "state", "questions": {"target": {"type":"choice","instructions","criteria"}}}`
//! - criteria のキーは不透明な `c0`、`c1`… と、明示的な「該当なし」の `none`（1〜255 個）
//! - 応答: `answers.target` の `type` が `choice` で、`choice` が送ったキーのどれか、`confidence` が 0〜1 の有限値
//!
//! 応答が想定の形でなければ推測で補わず、[`Error::Jev`] にする。

use serde_json::{Map, Value, json};

use crate::error::{Error, Result};
use crate::jev::{ChoiceAnswer, ChoiceRequest, JevPick, MAX_CANDIDATES};

/// 質問の名前（応答の `answers` のキー）
pub const QUESTION_NAME: &str = "target";

/// 「該当なし」の選択肢のキー
pub const NONE_KEY: &str = "none";

/// Jev への指示文。候補の情報は state に、選択肢は不透明なキーで渡す
pub const INSTRUCTIONS: &str = "state.purpose は、ウェブ画面（Chrome ウェブストアのデベロッパー ダッシュボードの「ストアの掲載情報」）で次に操作する要素の目的です。state.candidates は画面上の候補要素で、それぞれ key・役割（role）・タグ・ラベル行・表示テキストを持ちます。目的に合う要素の key をちょうど 1 つ選んでください。どの候補も目的に合わない、または判断できない場合は none を選んでください。";

/// 候補の位置からキーを作る（`c<位置>`）
pub fn candidate_key(position: usize) -> String {
    format!("c{position}")
}

/// 送信本文を組み立てる。候補が 0 個・多すぎる場合はエラー（呼び出し側の事前絞り込みの誤り）
pub fn build_request_body(model: &str, request: &ChoiceRequest) -> Result<Value> {
    if request.candidates.is_empty() {
        return Err(Error::Dashboard(format!(
            "{}の候補が 0 個のため、Jev に聞けません",
            request.purpose
        )));
    }
    if request.candidates.len() > MAX_CANDIDATES {
        return Err(Error::Dashboard(format!(
            "{}の候補が {} 個あり、Jev に送れる上限（{MAX_CANDIDATES} 個）を超えています",
            request.purpose,
            request.candidates.len()
        )));
    }
    // キーは位置と必ず一致させる（応答のキーを位置へ戻すときに使う）
    for (i, c) in request.candidates.iter().enumerate() {
        if c.key != candidate_key(i) {
            return Err(Error::Dashboard(format!(
                "Jev へ送る候補のキーが位置と一致しません（{} の位置 {i}）",
                c.key
            )));
        }
    }

    let mut criteria = Map::new();
    for c in &request.candidates {
        criteria.insert(
            c.key.clone(),
            Value::String(format!(
                "state.candidates のうち key が \"{}\" の要素",
                c.key
            )),
        );
    }
    criteria.insert(
        NONE_KEY.to_string(),
        Value::String("どの候補も目的の要素ではない、または判断できない（該当なし）".to_string()),
    );

    let state = serde_json::to_value(request)
        .map_err(|e| Error::Jev(format!("Jev へ送る内容を JSON にできませんでした: {e}")))?;
    Ok(json!({
        "model": model,
        "state": state,
        "questions": {
            QUESTION_NAME: {
                "type": "choice",
                "instructions": INSTRUCTIONS,
                "criteria": Value::Object(criteria),
            }
        }
    }))
}

/// 成功応答（200）を検証して答えにする。`candidate_count` は送った候補の数
pub fn parse_response(body: &Value, candidate_count: usize) -> Result<ChoiceAnswer> {
    let answer = body
        .get("answers")
        .and_then(|a| a.get(QUESTION_NAME))
        .ok_or_else(|| Error::Jev("Jev の応答に answers.target がありません".to_string()))?;
    if answer.get("type").and_then(Value::as_str) != Some("choice") {
        return Err(Error::Jev(
            "Jev の応答の answers.target が choice の答えではありません".to_string(),
        ));
    }
    let choice = answer
        .get("choice")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Jev("Jev の応答に choice がありません".to_string()))?;
    let confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|c| c.is_finite() && (0.0..=1.0).contains(c))
        .ok_or_else(|| {
            Error::Jev("Jev の応答の confidence が 0〜1 の数値ではありません".to_string())
        })?;
    let pick = parse_choice_key(choice, candidate_count).ok_or_else(|| {
        Error::Jev(format!(
            "Jev が送っていない選択肢を返しました: {}",
            shorten(choice, 40)
        ))
    })?;
    Ok(ChoiceAnswer { pick, confidence })
}

/// 選択肢のキーを位置へ戻す。送ったキー（`c0`〜`c<n-1>` と `none`）以外は None
pub fn parse_choice_key(key: &str, candidate_count: usize) -> Option<JevPick> {
    if key == NONE_KEY {
        return Some(JevPick::NoneOfThem);
    }
    let digits = key.strip_prefix('c')?;
    let position: usize = digits.parse().ok()?;
    // 「c01」「c+1」などは送っていないので受け付けない（作ったキーと完全一致するものだけ）
    (position < candidate_count && candidate_key(position) == key)
        .then_some(JevPick::Candidate(position))
}

/// 失敗応答（400 / 422 など）の `detail` を短い説明にする。読めなければ None
pub fn error_detail(body: &Value) -> Option<String> {
    let detail = body.get("detail")?;
    let text = match detail {
        Value::String(s) => s.clone(),
        // 422: [{loc, msg, type, ...}]
        Value::Array(items) => items
            .iter()
            .map(|i| {
                let msg = i.get("msg").and_then(Value::as_str).unwrap_or("?");
                let loc = i
                    .get("loc")
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .map(|x| match x {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(".")
                    })
                    .unwrap_or_default();
                if loc.is_empty() {
                    msg.to_string()
                } else {
                    format!("{msg}（{loc}）")
                }
            })
            .collect::<Vec<_>>()
            .join(" / "),
        other => other.to_string(),
    };
    Some(shorten(&text, 300))
}

/// 長い文字列を切り詰める（UTF-16 換算ではなく文字数で数える）
pub fn shorten(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max_chars).collect();
    s.push('…');
    s
}
