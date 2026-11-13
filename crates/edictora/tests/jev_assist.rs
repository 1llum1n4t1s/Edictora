//! `dashboard::jev_assist` のテスト。偽の ElementChooser を使い、ブラウザも本物の API も使わない。
//!
//! - 判断表の適用（一致・食い違い・ルールで決まらないときの採用と中止）
//! - 送る情報の最小化（説明文・URL・アカウント名が送信本文に入らないこと）
//! - ポーリング中の再特定（Jev を呼ばない）と、同じ質問の再利用
// テスト名に Jev・URL などの英大文字を含めるため
#![allow(non_snake_case)]

use std::cell::RefCell;

use edictora::Error;
use edictora::cancel::CancelToken;
use edictora::dashboard::jev_assist::{
    JevAssist, Target, build_request, non_language_combobox_texts,
};
use edictora::dashboard::selectors::ElementInfo;
use edictora::jev::protocol::build_request_body;
use edictora::jev::{ChoiceAnswer, ChoiceRequest, ElementChooser, JevPick};
use edictora::listing::ListingField;

/// 決まった答えを返し、受け取った質問を記録する偽物
struct FakeChooser<'r> {
    answer: Result<ChoiceAnswer, Error>,
    seen: &'r RefCell<Vec<ChoiceRequest>>,
}

impl ElementChooser for FakeChooser<'_> {
    fn choose(&self, request: &ChoiceRequest, _: &CancelToken) -> edictora::Result<ChoiceAnswer> {
        self.seen.borrow_mut().push(request.clone());
        self.answer.clone()
    }
}

fn assist<'r>(
    pick: JevPick,
    confidence: f64,
    seen: &'r RefCell<Vec<ChoiceRequest>>,
    report: &'r RefCell<Vec<String>>,
) -> JevAssist<'r> {
    JevAssist::new(
        Box::new(FakeChooser {
            answer: Ok(ChoiceAnswer { pick, confidence }),
            seen,
        }),
        Box::new(move |m: &str| report.borrow_mut().push(m.to_string())),
    )
}

fn el(index: i32, tag: &str, text: &str, label: &str) -> ElementInfo {
    ElementInfo {
        index,
        tag: tag.to_string(),
        text: text.to_string(),
        label: label.to_string(),
        r#type: if tag == "input" {
            "text".to_string()
        } else {
            String::new()
        },
        ..ElementInfo::default()
    }
}

/// パブリッシャーの選択欄 + 言語の選択欄 + カテゴリ欄（「すべての言語用」）
fn comboboxes() -> Vec<ElementInfo> {
    vec![
        el(0, "div", "Kagayoi Publisher", "Kagayoi Publisher"),
        el(
            1,
            "div",
            "言語\n英語 – en （デフォルト）",
            "言語\n英語 – en （デフォルト）",
        ),
        el(
            2,
            "div",
            "仕事効率化",
            "カテゴリ\nすべての言語用\n仕事効率化",
        ),
    ]
}

fn cancel() -> CancelToken {
    CancelToken::new()
}

#[test]
fn ルールとJevが一致すれば採用し確信度を進捗に出す() {
    let seen = RefCell::new(Vec::new());
    let report = RefCell::new(Vec::new());
    let a = assist(JevPick::Candidate(0), 0.97, &seen, &report);
    let combos = comboboxes();
    let sensitive = non_language_combobox_texts(&combos);

    let chosen = a
        .select(&Target::LanguageCombobox, &combos, &sensitive, &cancel())
        .unwrap();
    assert_eq!(chosen.index, 1);

    // パブリッシャーの選択欄は候補に入らない（「言語」を含むものだけ）
    let req = &seen.borrow()[0];
    assert_eq!(req.candidates.len(), 2);
    let line = &report.borrow()[0];
    assert!(line.contains("ルールと一致"), "{line}");
    assert!(line.contains("0.97"), "{line}");
}

#[test]
fn ルールで1つなのにJevが別のものを選んだら止める() {
    let seen = RefCell::new(Vec::new());
    let report = RefCell::new(Vec::new());
    let a = assist(JevPick::Candidate(1), 0.99, &seen, &report);
    let r = a.select(&Target::LanguageCombobox, &comboboxes(), &[], &cancel());
    match r {
        Err(Error::Dashboard(m)) => assert!(m.contains("食い違う"), "{m}"),
        other => panic!("止まるはず: {other:?}"),
    }
    assert!(report.borrow()[0].contains("止めます"));
}

fn two_textareas() -> Vec<ElementInfo> {
    vec![
        el(0, "textarea", "", "メモ"),
        el(1, "textarea", "", "説明\n0/16,000"),
    ]
}

#[test]
fn ルールで決まらなければしきい値以上のJevの選択を採用する() {
    let seen = RefCell::new(Vec::new());
    let report = RefCell::new(Vec::new());
    let a = assist(JevPick::Candidate(1), 0.92, &seen, &report);
    let target = Target::Field(ListingField::Description);
    let chosen = a.select(&target, &two_textareas(), &[], &cancel()).unwrap();
    assert_eq!(chosen.index, 1);
    assert!(report.borrow()[0].contains("Jev の選択を採用"));

    // ポーリング中の再特定は Jev を呼ばず、採用した要素（位置・候補数・タグ）を使う
    let again = a.reselect(&target, &two_textareas()).unwrap();
    assert_eq!(again.index, 1);
    assert_eq!(seen.borrow().len(), 1);

    // 候補の数が変わったら同じ要素と確かめられないので止める
    let mut three = two_textareas();
    three.push(el(2, "textarea", "", "その他"));
    assert!(matches!(
        a.reselect(&target, &three),
        Err(Error::Dashboard(_))
    ));

    // 画面を開き直したら記憶を捨てる
    a.forget_pins();
    assert!(a.reselect(&target, &two_textareas()).is_err());
}

#[test]
fn ルールで決まらずJevの確信度が低いか該当なしなら止める() {
    let target = Target::Field(ListingField::Description);
    for (pick, conf) in [(JevPick::Candidate(1), 0.6), (JevPick::NoneOfThem, 0.99)] {
        let seen = RefCell::new(Vec::new());
        let report = RefCell::new(Vec::new());
        let a = assist(pick, conf, &seen, &report);
        assert!(matches!(
            a.select(&target, &two_textareas(), &[], &cancel()),
            Err(Error::Dashboard(_))
        ));
    }
}

#[test]
fn ルールで0個でも候補があれば聞き_候補が0個なら聞かずに止める() {
    // ラベルが変わって「公式 URL」の行が無い（ルールで 0 個）
    let inputs = vec![
        el(0, "input", "", "公式サイトのURL"),
        el(1, "input", "", "ホームページ URL"),
    ];
    let seen = RefCell::new(Vec::new());
    let report = RefCell::new(Vec::new());
    let a = assist(JevPick::Candidate(0), 0.9, &seen, &report);
    let chosen = a
        .select(
            &Target::Field(ListingField::OfficialUrl),
            &inputs,
            &[],
            &cancel(),
        )
        .unwrap();
    assert_eq!(chosen.index, 0);

    let seen2 = RefCell::new(Vec::new());
    let a2 = assist(JevPick::Candidate(0), 0.9, &seen2, &report);
    assert!(matches!(
        a2.select(
            &Target::Field(ListingField::OfficialUrl),
            &[],
            &[],
            &cancel()
        ),
        Err(Error::Dashboard(_))
    ));
    assert!(seen2.borrow().is_empty());
}

#[test]
fn 同じ質問は繰り返さず前回の答えを使う() {
    let seen = RefCell::new(Vec::new());
    let report = RefCell::new(Vec::new());
    let a = assist(JevPick::Candidate(0), 0.97, &seen, &report);
    for _ in 0..3 {
        a.select(&Target::LanguageCombobox, &comboboxes(), &[], &cancel())
            .unwrap();
    }
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(report.borrow().len(), 1);
}

#[test]
fn Jevの通信失敗はそのまま返す() {
    let seen = RefCell::new(Vec::new());
    let a = JevAssist::new(
        Box::new(FakeChooser {
            answer: Err(Error::Jev("タイムアウト".to_string())),
            seen: &seen,
        }),
        Box::new(|_: &str| {}),
    );
    assert!(matches!(
        a.select(&Target::LanguageCombobox, &comboboxes(), &[], &cancel()),
        Err(Error::Jev(_))
    ));
}

#[test]
fn 選択肢はロケール形式のものだけを送りパブリッシャー名を除く() {
    let options = vec![
        el(0, "div", "Kagayoi Publisher", ""),
        el(
            1,
            "div",
            "英語 – en （デフォルト）",
            "Kagayoi Publisher\n英語 – en",
        ),
        el(2, "div", "日本語 – ja", ""),
        // パブリッシャー名がたまたまロケール形式でも、選択欄の表示と同じなら送らない
        el(3, "div", "Studio – ab", ""),
    ];
    let mut combos = comboboxes();
    combos[0].text = "Studio – ab".to_string();
    let sensitive = non_language_combobox_texts(&combos);
    let p = build_request(
        &Target::LocaleOption("ja".to_string()),
        &options,
        &sensitive,
    )
    .unwrap();
    assert_eq!(p.element_indices, vec![1, 2]);
    let body = serde_json::to_string(&p.request).unwrap();
    assert!(!body.contains("Kagayoi"), "{body}");
    assert!(!body.contains("Studio"), "{body}");
}

#[test]
fn 送信本文に説明文やURLやアカウント名を入れない() {
    let description = "これはダミーの説明文です。とても便利な拡張機能で、ページを翻訳します。";
    let url = "https://example.com/support";
    // 祖先の文字列に説明文の一部・URL・アカウント名・長い行が混ざった textarea と input
    let textareas = vec![el(
        0,
        "textarea",
        description,
        &format!(
            "説明\nこれはダミーの説明文です。\n{url}\nKagayoi Publisher\n{}\n0/16,000\n16000文字中1492文字入力しました",
            "長".repeat(81)
        ),
    )];
    let inputs = vec![el(
        5,
        "input",
        "",
        &format!("サポート URL\n{url}\nサポート用のページ https://example.com/help"),
    )];
    let combos = comboboxes();
    let mut sensitive = vec![description.to_string(), url.to_string()];
    sensitive.extend(non_language_combobox_texts(&combos));

    let d = build_request(
        &Target::Field(ListingField::Description),
        &textareas,
        &sensitive,
    )
    .unwrap();
    let u = build_request(
        &Target::Field(ListingField::SupportUrl),
        &inputs,
        &sensitive,
    )
    .unwrap();
    let l = build_request(&Target::LanguageCombobox, &combos, &sensitive).unwrap();
    for p in [&d, &u, &l] {
        let body = build_request_body("jev-latest", &p.request)
            .unwrap()
            .to_string();
        assert!(!body.contains("ダミーの説明文"), "{body}");
        assert!(!body.contains("example.com"), "{body}");
        assert!(!body.contains("Kagayoi"), "{body}");
        assert!(!body.contains(&"長".repeat(81)), "{body}");
    }
    // 見出しは残る（Jev が判断できるように）
    // 文字数カウンタ（説明文の長さが分かる）も送らない
    assert_eq!(d.request.candidates[0].labels, vec!["説明"]);
    assert_eq!(d.request.candidates[0].text, None);
    assert_eq!(u.request.candidates[0].labels, vec!["サポート URL"]);
    assert_eq!(u.request.candidates[0].r#type.as_deref(), Some("text"));
    assert_eq!(
        l.request.candidates[0].text.as_deref(),
        Some("言語\n英語 – en （デフォルト）")
    );
}

#[test]
fn 候補が多すぎれば送らずに止める() {
    let inputs: Vec<ElementInfo> = (0..255).map(|i| el(i, "input", "", "x")).collect();
    assert!(matches!(
        build_request(&Target::Field(ListingField::HomepageUrl), &inputs, &[]),
        Err(Error::Dashboard(_))
    ));
    let ok: Vec<ElementInfo> = (0..254).map(|i| el(i, "input", "", "x")).collect();
    assert!(build_request(&Target::Field(ListingField::HomepageUrl), &ok, &[]).is_ok());
}

#[test]
fn ルールの結果は既存のpick関数と同じ条件() {
    let inputs = vec![
        el(0, "input", "", "公式 URL"),
        el(1, "input", "", "公式URL"),
        el(2, "input", "", "ホームページ URL"),
    ];
    let p = build_request(&Target::Field(ListingField::OfficialUrl), &inputs, &[]).unwrap();
    // 空白の有無は区別しない → 2 個が当てはまる（pick_field_by_label でも曖昧で止まる）
    assert_eq!(
        p.rule,
        edictora::jev::decision::RuleOutcome::Ambiguous(vec![0, 1])
    );
}

/// 実際に送る本文を固定する（送る情報の範囲が意図せず広がったら気づけるように）
#[test]
fn 送信本文の実例() {
    let options = vec![
        el(0, "div", "Kagayoi Publisher", ""),
        el(1, "div", "日本語 – ja", ""),
        el(2, "div", "英語 – en （デフォルト）", ""),
    ];
    let sensitive = non_language_combobox_texts(&comboboxes());
    let p = build_request(
        &Target::LocaleOption("en".to_string()),
        &options,
        &sensitive,
    )
    .unwrap();
    let body = build_request_body("jev-latest", &p.request)
        .unwrap()
        .to_string();
    assert_eq!(body, EXPECTED_BODY);
}

/// パブリッシャー名（"Kagayoi Publisher"）は入らない。serde_json の既定でキーは辞書順に並ぶ
const EXPECTED_BODY: &str = r##"{"model":"jev-latest","questions":{"target":{"criteria":{"c0":"state.candidates のうち key が \"c0\" の要素","c1":"state.candidates のうち key が \"c1\" の要素","none":"どの候補も目的の要素ではない、または判断できない（該当なし）"},"instructions":"state.purpose は、ウェブ画面（Chrome ウェブストアのデベロッパー ダッシュボードの「ストアの掲載情報」）で次に操作する要素の目的です。state.candidates は画面上の候補要素で、それぞれ key・役割（role）・タグ・ラベル行・表示テキストを持ちます。目的に合う要素の key をちょうど 1 つ選んでください。どの候補も目的に合わない、または判断できない場合は none を選んでください。","type":"choice"}},"state":{"candidates":[{"key":"c0","labels":[],"role":"option","tag":"div","text":"日本語 – ja"},{"key":"c1","labels":[],"role":"option","tag":"div","text":"英語 – en （デフォルト）"}],"purpose":"言語の選択肢のうち、ロケールコードが en のもの（表記は「言語名 – en」）"}}"##;
