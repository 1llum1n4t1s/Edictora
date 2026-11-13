//! `--jev` の純粋部分のテスト: 判断表（ルール × Jev の答え）と、送信本文・応答の検証。
//! 本物の API は呼ばない。
// テスト名に Jev・URL などの英大文字を含めるため
#![allow(non_snake_case)]

use edictora::Error;
use edictora::jev::decision::{CONFIDENCE_THRESHOLD, Decision, RuleOutcome, StopReason, decide};
use edictora::jev::protocol::{
    NONE_KEY, build_request_body, error_detail, parse_choice_key, parse_response,
};
use edictora::jev::{Candidate, ChoiceAnswer, ChoiceRequest, JevPick, MAX_CANDIDATES};
use serde_json::json;

fn answer(pick: JevPick, confidence: f64) -> ChoiceAnswer {
    ChoiceAnswer { pick, confidence }
}

fn c(i: usize) -> JevPick {
    JevPick::Candidate(i)
}

// ───────── 判断表 ─────────

#[test]
fn ルールで1つに決まりJevも同じなら確信度に関係なく進める() {
    let rule = RuleOutcome::Unique(2);
    assert_eq!(decide(&rule, &answer(c(2), 0.99)), Decision::Agreed(2));
    // 同じものを選んでいれば、確信度が低くても進める（ルールの裏付けがあるため）
    assert_eq!(decide(&rule, &answer(c(2), 0.1)), Decision::Agreed(2));
}

#[test]
fn ルールで1つに決まりJevが別のものや該当なしを選んだら止める() {
    let rule = RuleOutcome::Unique(0);
    assert_eq!(
        decide(&rule, &answer(c(1), 0.99)),
        Decision::Stop(StopReason::Disagreed { rule: 0, jev: c(1) })
    );
    assert_eq!(
        decide(&rule, &answer(JevPick::NoneOfThem, 0.99)),
        Decision::Stop(StopReason::Disagreed {
            rule: 0,
            jev: JevPick::NoneOfThem
        })
    );
}

#[test]
fn ルールで0個ならしきい値以上のJevの選択を採用し未満なら止める() {
    let rule = RuleOutcome::NotFound;
    assert_eq!(
        decide(&rule, &answer(c(3), CONFIDENCE_THRESHOLD)),
        Decision::JevAdopted(3)
    );
    assert_eq!(decide(&rule, &answer(c(3), 0.95)), Decision::JevAdopted(3));
    assert_eq!(
        decide(&rule, &answer(c(3), 0.79)),
        Decision::Stop(StopReason::LowConfidence {
            pick: 3,
            confidence: 0.79
        })
    );
}

#[test]
fn ルールで複数ならその中のJevの選択だけを採用する() {
    let rule = RuleOutcome::Ambiguous(vec![1, 4]);
    assert_eq!(decide(&rule, &answer(c(4), 0.9)), Decision::JevAdopted(4));
    assert_eq!(
        decide(&rule, &answer(c(4), 0.5)),
        Decision::Stop(StopReason::LowConfidence {
            pick: 4,
            confidence: 0.5
        })
    );
    // ルールの候補の外は、確信度が高くても採らない
    assert_eq!(
        decide(&rule, &answer(c(2), 0.99)),
        Decision::Stop(StopReason::OutsideRuleMatches { pick: 2 })
    );
}

#[test]
fn ルールで決まらずJevが該当なしなら止める() {
    for rule in [RuleOutcome::NotFound, RuleOutcome::Ambiguous(vec![0, 1])] {
        assert_eq!(
            decide(&rule, &answer(JevPick::NoneOfThem, 1.0)),
            Decision::Stop(StopReason::NoneAnswered)
        );
    }
}

#[test]
fn ルールの結果を位置の一覧から作る() {
    assert_eq!(RuleOutcome::from_matches(vec![]), RuleOutcome::NotFound);
    assert_eq!(RuleOutcome::from_matches(vec![5]), RuleOutcome::Unique(5));
    assert_eq!(
        RuleOutcome::from_matches(vec![0, 2]),
        RuleOutcome::Ambiguous(vec![0, 2])
    );
}

#[test]
fn しきい値は0_8() {
    assert_eq!(CONFIDENCE_THRESHOLD, 0.8);
}

// ───────── 送信本文 ─────────

fn candidate(i: usize, role: &str) -> Candidate {
    Candidate {
        key: format!("c{i}"),
        role: role.to_string(),
        tag: "div".to_string(),
        r#type: None,
        labels: vec!["言語".to_string()],
        text: Some("英語 – en".to_string()),
    }
}

fn request(n: usize) -> ChoiceRequest {
    ChoiceRequest {
        purpose: "言語の選択肢".to_string(),
        candidates: (0..n).map(|i| candidate(i, "option")).collect(),
    }
}

#[test]
fn 送信本文にはモデルと候補と不透明なキーと該当なしが入る() {
    let body = build_request_body("jev-latest", &request(2)).unwrap();
    assert_eq!(body["model"], "jev-latest");
    assert_eq!(body["state"]["purpose"], "言語の選択肢");
    assert_eq!(body["state"]["candidates"][1]["key"], "c1");
    let q = &body["questions"]["target"];
    assert_eq!(q["type"], "choice");
    assert!(q["instructions"].as_str().unwrap().contains("none"));
    let criteria = q["criteria"].as_object().unwrap();
    let mut keys: Vec<&str> = criteria.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["c0", "c1", NONE_KEY]);
    // input 以外は type を送らない
    assert!(body["state"]["candidates"][0].get("type").is_none());
}

#[test]
fn 候補が0個や上限超えやキーの不整合なら送らない() {
    assert!(matches!(
        build_request_body("m", &request(0)),
        Err(Error::Dashboard(_))
    ));
    assert!(matches!(
        build_request_body("m", &request(MAX_CANDIDATES + 1)),
        Err(Error::Dashboard(_))
    ));
    // 上限ちょうど（criteria は 254 + none = 255 個）は送れる
    let body = build_request_body("m", &request(MAX_CANDIDATES)).unwrap();
    assert_eq!(
        body["questions"]["target"]["criteria"]
            .as_object()
            .unwrap()
            .len(),
        255
    );
    let mut bad = request(2);
    bad.candidates[1].key = "c5".to_string();
    assert!(build_request_body("m", &bad).is_err());
}

// ───────── 応答の検証 ─────────

fn ok_response(choice: &str, confidence: f64) -> serde_json::Value {
    json!({
        "model": "jev-2026-11",
        "answers": { "target": { "type": "choice", "choice": choice, "confidence": confidence,
                                 "probabilities": { choice: confidence } } },
        "usage": { "input_tokens": 10, "output_tokens": 1 }
    })
}

#[test]
fn 応答の選択肢を候補の位置へ戻す() {
    let a = parse_response(&ok_response("c1", 0.93), 2).unwrap();
    assert_eq!(a.pick, JevPick::Candidate(1));
    assert_eq!(a.confidence, 0.93);
    let none = parse_response(&ok_response("none", 0.7), 2).unwrap();
    assert_eq!(none.pick, JevPick::NoneOfThem);
}

#[test]
fn 送っていないキーは拒否する() {
    for bad in ["c2", "c01", "c+1", "C1", "1", "", "none ", "c-1"] {
        assert!(
            matches!(
                parse_response(&ok_response(bad, 0.9), 2),
                Err(Error::Jev(_))
            ),
            "{bad}"
        );
    }
    assert_eq!(parse_choice_key("c10", 11), Some(JevPick::Candidate(10)));
    assert_eq!(parse_choice_key("c10", 10), None);
}

#[test]
fn 応答の形が違えば拒否する() {
    let cases = [
        json!({}),
        json!({ "answers": {} }),
        json!({ "answers": { "other": { "type": "choice", "choice": "c0", "confidence": 0.9 } } }),
        json!({ "answers": { "target": { "type": "score", "choice": "c0", "confidence": 0.9 } } }),
        json!({ "answers": { "target": { "type": "choice", "confidence": 0.9 } } }),
        json!({ "answers": { "target": { "type": "choice", "choice": "c0" } } }),
        json!({ "answers": { "target": { "type": "choice", "choice": "c0", "confidence": 1.5 } } }),
        json!({ "answers": { "target": { "type": "choice", "choice": "c0", "confidence": -0.1 } } }),
        json!({ "answers": { "target": { "type": "choice", "choice": "c0", "confidence": "0.9" } } }),
    ];
    for body in cases {
        assert!(
            matches!(parse_response(&body, 1), Err(Error::Jev(_))),
            "{body}"
        );
    }
}

#[test]
fn エラー応答のdetailを短くまとめる() {
    assert_eq!(
        error_detail(&json!({ "detail": "model not found" })).as_deref(),
        Some("model not found")
    );
    let d = error_detail(&json!({ "detail": [
        { "loc": ["body", "questions", "target", "criteria"], "msg": "too many", "type": "x" },
        { "loc": ["body", 0], "msg": "bad", "type": "y" }
    ] }))
    .unwrap();
    assert_eq!(
        d,
        "too many（body.questions.target.criteria） / bad（body.0）"
    );
    assert_eq!(error_detail(&json!({})), None);
    let long = "あ".repeat(500);
    assert!(
        error_detail(&json!({ "detail": long }))
            .unwrap()
            .ends_with('…')
    );
}
