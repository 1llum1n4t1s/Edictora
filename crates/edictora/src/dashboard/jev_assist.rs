//! `--jev` モードで、画面から集めた要素（[`ElementInfo`]）を Jev への候補に変え、
//! ルール（[`crate::dashboard::selectors`] の述語）の結果と Jev の答えを突き合わせて 1 つを選ぶ。
//!
//! CDP には触らないので、偽の [`ElementChooser`] を渡せばブラウザなしでテストできる。
//! `item_page` は「要素を集める → ここで選ぶ → 選んだ要素をクリック・入力する」だけを行う。
//!
//! 対象は入力欄（説明 textarea・URL 欄）・言語のコンボボックス・言語の選択肢だけ。
//! 取り消せない判断（write_gate のステータス、下書き保存・審査送信のボタン、送信ダイアログ）はここを通さない。
//!
//! ## 送る情報（最小限）
//! - 候補ごと: 役割・タグ・input の type・ラベル行・短い表示テキスト。キーは不透明な `c0`、`c1`…
//! - 入力欄（textarea / input）は表示テキストを送らない（textarea の innerText は説明文そのものの恐れがある）
//! - ラベル行は、`://` を含む行・長い行（[`MAX_LINE_CHARS`] 超）・秘匿したい文字列（欄の値・書き込む値・
//!   パブリッシャーの選択欄の表示）に当たる行・文字数カウンタの行を落とし、[`MAX_LABEL_LINES`] 行までにする
//! - 言語の選択肢は「言語名 – code」と解釈できるものだけを候補にし（パブリッシャー名の選択肢を除く）、ラベル行は送らない
//! - コンボボックスは「言語」を含むものだけを候補にし、表示テキストは「言語」の行とロケール形式の行だけを送る
//!
//! ## ポーリング中は Jev を呼ばない
//! 状態の確認（一覧が閉じたか・切り替わったか・説明欄が落ち着いたか）は [`JevAssist::reselect`] で行う。
//! ルールでちょうど 1 つに決まればそれを使い、決まらなければ直前に Jev で採用した要素
//! （事前絞り込み後の位置・候補数・タグ）と同じものを使う。同じと確かめられなければ止める。

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::cancel::CancelToken;
use crate::dashboard::selectors::{
    self as sel, ElementInfo, is_field_with_label, is_language_combobox, is_locale_option,
};
use crate::dashboard::text::{LANGUAGE_LABEL, field_label};
use crate::error::{Error, Result};
use crate::jev::decision::{CONFIDENCE_THRESHOLD, Decision, RuleOutcome, StopReason, decide};
use crate::jev::protocol::{candidate_key, shorten};
use crate::jev::{Candidate, ChoiceAnswer, ChoiceRequest, ElementChooser, JevPick, MAX_CANDIDATES};
use crate::listing::ListingField;
use crate::logging::{LogLevel, log};

/// 送るラベル行・表示テキストの 1 行の最大文字数（これを超える行は説明文などの恐れがあるので送らない）
pub const MAX_LINE_CHARS: usize = 80;

/// 候補 1 つあたりに送るラベル行の上限
pub const MAX_LABEL_LINES: usize = 8;

/// 選ぶ対象
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 言語を切り替えるコンボボックス
    LanguageCombobox,
    /// 言語の選択肢（ロケールコード）
    LocaleOption(String),
    /// 掲載情報の入力欄（説明は textarea、URL は input）
    Field(ListingField),
}

impl Target {
    /// 操作の目的（Jev へ送る。利用者の入力値は含めない。ロケールコードは設定の値だが秘密ではない）
    pub fn purpose(&self) -> String {
        match self {
            Target::LanguageCombobox => {
                "編集中の言語を切り替える「言語」のコンボボックス".to_string()
            }
            Target::LocaleOption(code) => {
                format!(
                    "言語の選択肢のうち、ロケールコードが {code} のもの（表記は「言語名 – {code}」）"
                )
            }
            Target::Field(ListingField::Description) => {
                "ストアの掲載情報の「説明」の入力欄（複数行）".to_string()
            }
            Target::Field(field) => format!(
                "ストアの掲載情報の「{}」の入力欄",
                field_label(*field).unwrap_or(field.display_name())
            ),
        }
    }

    /// 進捗・エラーに出す短い名前
    pub fn display(&self) -> String {
        match self {
            Target::LanguageCombobox => "言語の選択欄".to_string(),
            Target::LocaleOption(code) => format!("言語の選択肢（{code}）"),
            Target::Field(field) => format!("{}の欄", field.display_name()),
        }
    }

    /// 採用した要素を覚えるときのキー
    fn pin_key(&self) -> String {
        match self {
            Target::LanguageCombobox => "combobox".to_string(),
            Target::LocaleOption(code) => format!("option:{}", code.to_lowercase()),
            Target::Field(field) => format!("field:{}", field.config_key()),
        }
    }

    fn role(&self) -> &'static str {
        match self {
            Target::LanguageCombobox => "combobox",
            Target::LocaleOption(_) => "option",
            Target::Field(_) => "textbox",
        }
    }

    /// 事前絞り込み（Jev へ送ってよい候補か）。ルールに当てはまる要素は必ず通す
    pub fn is_candidate(&self, e: &ElementInfo) -> bool {
        match self {
            // 「言語」を含まないコンボボックス（パブリッシャーの選択欄など）は送らない
            // 条件は編集画面の待ち条件と共通（selectors::mentions_language_label）
            Target::LanguageCombobox => sel::mentions_language_label(e),
            // 「言語名 – code」と解釈できない選択肢（常に表示されるパブリッシャー名など）は送らない
            Target::LocaleOption(_) => sel::parse_locale_option(Some(&e.text)).is_some(),
            Target::Field(ListingField::Description) => e.tag == "textarea",
            Target::Field(_) => e.tag == "input",
        }
    }

    /// ルール（`selectors` の `pick_*` と同じ条件）
    pub fn rule_matches(&self, e: &ElementInfo) -> bool {
        match self {
            Target::LanguageCombobox => is_language_combobox(e),
            Target::LocaleOption(code) => is_locale_option(e, code),
            // 説明欄は textarea がちょうど 1 つあること（`pick_description` と同じ）
            Target::Field(ListingField::Description) => true,
            Target::Field(field) => {
                field_label(*field).is_some_and(|label| is_field_with_label(e, label))
            }
        }
    }
}

/// パブリッシャーの選択欄など、言語以外のコンボボックスの表示テキスト（アカウント名を含む恐れがある）。
/// これらの行は Jev へ送らない（[`build_request`] の `sensitive` に渡す）
pub fn non_language_combobox_texts(comboboxes: &[ElementInfo]) -> Vec<String> {
    comboboxes
        .iter()
        .filter(|c| !Target::LanguageCombobox.is_candidate(c))
        .flat_map(|c| c.text.split('\n').map(|l| l.trim().to_string()))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Jev への質問と、候補 → 要素の対応・ルールの結果
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedChoice {
    /// 送る質問
    pub request: ChoiceRequest,
    /// 候補の位置 → `elements` の添字
    pub element_indices: Vec<usize>,
    /// ルールの結果（候補の位置で表す）
    pub rule: RuleOutcome,
}

/// 要素の一覧から Jev への質問を作る。`sensitive` は送ってはならない文字列（欄の値・書き込む値・アカウント名など）
pub fn build_request(
    target: &Target,
    elements: &[ElementInfo],
    sensitive: &[String],
) -> Result<PreparedChoice> {
    let element_indices: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| target.is_candidate(e))
        // 表示テキストが秘匿したい文字列そのものの候補は送らない（パブリッシャー名がロケール形式だった場合など）
        .filter(|(_, e)| {
            !matches!(target, Target::LocaleOption(_)) || !text_is_sensitive(e, sensitive)
        })
        .map(|(i, _)| i)
        .collect();
    if element_indices.len() > MAX_CANDIDATES {
        return Err(Error::Dashboard(format!(
            "{}の候補が {} 個あり、Jev に送れる上限（{MAX_CANDIDATES} 個）を超えるため止めます",
            target.display(),
            element_indices.len()
        )));
    }

    let candidates: Vec<Candidate> = element_indices
        .iter()
        .enumerate()
        .map(|(pos, &i)| candidate_view(target, pos, &elements[i], sensitive))
        .collect();
    let rule = RuleOutcome::from_matches(
        element_indices
            .iter()
            .enumerate()
            .filter(|(_, i)| target.rule_matches(&elements[**i]))
            .map(|(pos, _)| pos)
            .collect(),
    );
    Ok(PreparedChoice {
        request: ChoiceRequest {
            purpose: target.purpose(),
            candidates,
        },
        element_indices,
        rule,
    })
}

/// 要素 1 つを、送ってよい情報だけの候補にする
pub fn candidate_view(
    target: &Target,
    position: usize,
    e: &ElementInfo,
    sensitive: &[String],
) -> Candidate {
    let text_lines: Vec<String> = match target {
        // 入力欄の表示テキストは送らない（説明文そのものの恐れがある）
        Target::Field(_) => Vec::new(),
        // 「言語」の行とロケール形式の行だけ（パブリッシャー名などは送らない）
        Target::LanguageCombobox => e
            .text
            .split('\n')
            .map(str::trim)
            .filter(|l| *l == LANGUAGE_LABEL || sel::parse_locale_option(Some(l)).is_some())
            .filter(|l| l.chars().count() <= MAX_LINE_CHARS && !is_sensitive_line(l, sensitive))
            .map(str::to_string)
            .collect(),
        Target::LocaleOption(_) => e
            .text
            .split('\n')
            .map(str::trim)
            .filter(|l| sel::parse_locale_option(Some(l)).is_some())
            .filter(|l| !is_sensitive_line(l, sensitive))
            .map(|l| shorten(l, MAX_LINE_CHARS))
            .collect(),
    };
    let labels = match target {
        // 選択肢は表示テキストだけで足りる（祖先の文字列にほかの情報が混ざるのを避ける）
        Target::LocaleOption(_) => Vec::new(),
        _ => safe_label_lines(&e.label, &text_lines, sensitive),
    };
    Candidate {
        key: candidate_key(position),
        role: target.role().to_string(),
        tag: e.tag.clone(),
        r#type: (e.tag == "input").then(|| e.r#type.clone()),
        labels,
        text: (!text_lines.is_empty()).then(|| text_lines.join("\n")),
    }
}

/// ラベル候補の文字列から、送ってよい行だけを取り出す
fn safe_label_lines(raw: &str, text_lines: &[String], sensitive: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.split('\n').map(str::trim) {
        if line.is_empty()
            || line.contains("://")
            || is_counter_line(line)
            || line.chars().count() > MAX_LINE_CHARS
            || text_lines.iter().any(|t| t == line)
            || is_sensitive_line(line, sensitive)
            || lines.iter().any(|l| l == line)
        {
            continue;
        }
        lines.push(line.to_string());
        if lines.len() >= MAX_LABEL_LINES {
            break;
        }
    }
    lines
}

/// 文字数カウンタの行か。説明文の長さが分かり、書き込みのたびに質問が変わるので送らない。
/// 表示上の「1,234/16,000」に加え、読み上げ用ラベルの「16000文字中1492文字入力しました」
/// （2026-11-13 の実画面で aria のラベル行に現れた）も対象にする。
fn is_counter_line(line: &str) -> bool {
    // 固定の正規表現は初回参照時に 1 度だけコンパイルする
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:[0-9][0-9,]*\s*/\s*[0-9][0-9,]*|[0-9][0-9,]*\s*文字中\s*[0-9][0-9,]*\s*文字.*)$",
        )
        .expect("固定の正規表現")
    });
    RE.is_match(line)
}

/// 1 行が秘匿したい文字列に当たるか。
///
/// - 完全一致は常に当たる
/// - 8 文字以上の行が秘匿文字列（説明文など）の一部なら当たる（短い行は「説明」「公式 URL」など画面の見出しと区別できないので通す）
/// - 4 文字以上の秘匿文字列（URL・アカウント名など）を含む行は当たる
fn is_sensitive_line(line: &str, sensitive: &[String]) -> bool {
    sensitive
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .any(|s| {
            line == s
                || (line.chars().count() >= 8 && s.contains(line))
                || (s.chars().count() >= 4 && line.contains(s))
        })
}

fn text_is_sensitive(e: &ElementInfo, sensitive: &[String]) -> bool {
    e.text
        .split('\n')
        .map(str::trim)
        .any(|l| !l.is_empty() && sensitive.iter().any(|s| s.trim() == l))
}

/// 採用した要素（ポーリング中の再特定に使う）
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pin {
    /// 事前絞り込み後の位置
    position: usize,
    /// 事前絞り込み後の候補数
    count: usize,
    /// タグ名
    tag: String,
}

/// `--jev` の選択役。`item_page` に注入する
pub struct JevAssist<'a> {
    chooser: Box<dyn ElementChooser + 'a>,
    report: Box<dyn Fn(&str) + 'a>,
    /// 質問（候補の情報）→ 答え。同じ画面で同じ質問を繰り返さない
    answers: RefCell<HashMap<String, ChoiceAnswer>>,
    /// 対象 → 採用した要素
    pins: RefCell<HashMap<String, Pin>>,
}

impl<'a> JevAssist<'a> {
    /// 作る。`report` は進捗（標準エラー）への出力
    pub fn new(chooser: Box<dyn ElementChooser + 'a>, report: Box<dyn Fn(&str) + 'a>) -> Self {
        JevAssist {
            chooser,
            report,
            answers: RefCell::new(HashMap::new()),
            pins: RefCell::new(HashMap::new()),
        }
    }

    /// 画面を開き直したら、採用した要素の記憶を捨てる（答えの記憶は質問の内容で引くので残す）
    pub fn forget_pins(&self) {
        self.pins.borrow_mut().clear();
    }

    /// 操作の対象を 1 つ選ぶ（Jev に聞き、判断表で採用・中止を決める）
    pub fn select(
        &self,
        target: &Target,
        elements: &[ElementInfo],
        sensitive: &[String],
        cancel: &CancelToken,
    ) -> Result<ElementInfo> {
        let prepared = build_request(target, elements, sensitive)?;
        if prepared.element_indices.is_empty() {
            // 候補が無ければ Jev に聞けない（criteria が「該当なし」だけになる）
            let message = format!(
                "{}が見つかりません（Jev に送れる候補もありません。ダッシュボードが日本語表示か確認してください）",
                target.display()
            );
            self.say(&format!(
                "Jev: {}の候補が 0 個のため止めます",
                target.display()
            ));
            return Err(Error::Dashboard(message));
        }

        let key = serde_json::to_string(&prepared.request)
            .map_err(|e| Error::Jev(format!("Jev へ送る内容を JSON にできませんでした: {e}")))?;
        let cached = self.answers.borrow().get(&key).copied();
        let (answer, fresh) = match cached {
            Some(a) => (a, false),
            None => {
                let a = self.chooser.choose(&prepared.request, cancel)?;
                self.answers.borrow_mut().insert(key, a);
                (a, true)
            }
        };

        let count = prepared.element_indices.len();
        let decision = decide(&prepared.rule, &answer);
        let line = describe(target, &prepared, &answer, &decision);
        if fresh {
            self.say(&line);
        } else {
            // select はポーリングから呼ばれないので、再利用でも採用の記録を残す（release でも残る Info）
            log(&format!("{line}（前回の答えを再利用）"), LogLevel::Info);
        }

        match decision {
            Decision::Agreed(pos) | Decision::JevAdopted(pos) => {
                let element = elements[prepared.element_indices[pos]].clone();
                self.pin(target, elements, &element);
                Ok(element)
            }
            Decision::Stop(reason) => Err(Error::Dashboard(stop_message(
                target, &reason, count, &answer,
            ))),
        }
    }

    /// ポーリング中の再特定（Jev は呼ばない）。ルールで 1 つに決まればそれ、決まらなければ採用済みの要素
    pub fn reselect(&self, target: &Target, elements: &[ElementInfo]) -> Result<ElementInfo> {
        let pre: Vec<usize> = (0..elements.len())
            .filter(|&i| target.is_candidate(&elements[i]))
            .collect();
        let matches: Vec<usize> = pre
            .iter()
            .copied()
            .filter(|&i| target.rule_matches(&elements[i]))
            .collect();
        if matches.len() == 1 {
            return Ok(elements[matches[0]].clone());
        }
        let pins = self.pins.borrow();
        if let Some(pin) = pins.get(&target.pin_key())
            && pin.count == pre.len()
            && pre
                .get(pin.position)
                .is_some_and(|&i| elements[i].tag == pin.tag)
        {
            return Ok(elements[pre[pin.position]].clone());
        }
        Err(Error::Dashboard(format!(
            "{}を特定し直せません（ルールでは {} 個が当てはまり、Jev で選んだ要素とも対応づけられません）",
            target.display(),
            matches.len()
        )))
    }

    fn pin(&self, target: &Target, elements: &[ElementInfo], chosen: &ElementInfo) {
        let pre: Vec<&ElementInfo> = elements.iter().filter(|e| target.is_candidate(e)).collect();
        if let Some(position) = pre.iter().position(|e| e.index == chosen.index) {
            self.pins.borrow_mut().insert(
                target.pin_key(),
                Pin {
                    position,
                    count: pre.len(),
                    tag: chosen.tag.clone(),
                },
            );
        }
    }

    fn say(&self, line: &str) {
        (self.report)(line);
        log(line, LogLevel::Info);
    }
}

/// 候補を、送った情報だけで短く表す（進捗用）
fn candidate_summary(c: &Candidate) -> String {
    let first = c
        .text
        .as_deref()
        .and_then(|t| t.split('\n').next_back())
        .or_else(|| c.labels.first().map(String::as_str))
        .unwrap_or("");
    if first.is_empty() {
        format!("{} {}", c.key, c.role)
    } else {
        format!("{} {}「{}」", c.key, c.role, shorten(first, 30))
    }
}

fn pick_summary(prepared: &PreparedChoice, pick: JevPick) -> String {
    match pick {
        JevPick::Candidate(p) => prepared
            .request
            .candidates
            .get(p)
            .map(candidate_summary)
            .unwrap_or_else(|| candidate_key(p)),
        JevPick::NoneOfThem => "該当なし".to_string(),
    }
}

fn rule_summary(rule: &RuleOutcome) -> String {
    match rule {
        RuleOutcome::Unique(p) => format!("ルールは {} に特定", candidate_key(*p)),
        RuleOutcome::NotFound => "ルールでは該当なし".to_string(),
        RuleOutcome::Ambiguous(m) => format!("ルールでは {} 個が該当", m.len()),
    }
}

/// 進捗とログに出す 1 行（採用・中止のどちらも、Jev の選択と確信度を出す）
fn describe(
    target: &Target,
    prepared: &PreparedChoice,
    answer: &ChoiceAnswer,
    decision: &Decision,
) -> String {
    let head = format!(
        "Jev: {}（候補 {} 個、{}）→ Jev の選択 {}、確信度 {:.2}",
        target.display(),
        prepared.element_indices.len(),
        rule_summary(&prepared.rule),
        pick_summary(prepared, answer.pick),
        answer.confidence
    );
    let tail = match decision {
        Decision::Agreed(_) => "ルールと一致したので進めます".to_string(),
        Decision::JevAdopted(_) => {
            format!("しきい値 {CONFIDENCE_THRESHOLD:.2} 以上なので Jev の選択を採用します")
        }
        Decision::Stop(_) => "止めます".to_string(),
    };
    format!("{head}: {tail}")
}

fn stop_message(
    target: &Target,
    reason: &StopReason,
    count: usize,
    answer: &ChoiceAnswer,
) -> String {
    let what = target.display();
    match reason {
        StopReason::Disagreed { rule, jev } => format!(
            "{what}について、ルール（{}）と Jev（{}、確信度 {:.2}）の選択が食い違うため止めます",
            candidate_key(*rule),
            match jev {
                JevPick::Candidate(p) => candidate_key(*p),
                JevPick::NoneOfThem => "該当なし".to_string(),
            },
            answer.confidence
        ),
        StopReason::NoneAnswered => format!(
            "{what}をルールで特定できず（候補 {count} 個）、Jev も「該当なし」と答えたため止めます"
        ),
        StopReason::LowConfidence { pick, confidence } => format!(
            "{what}をルールで特定できず、Jev の選択（{}）の確信度 {confidence:.2} がしきい値 {CONFIDENCE_THRESHOLD:.2} に届かないため止めます",
            candidate_key(*pick)
        ),
        StopReason::OutsideRuleMatches { pick } => format!(
            "{what}はルールで複数が当てはまりましたが、Jev はその外の候補（{}）を選んだため止めます",
            candidate_key(*pick)
        ),
    }
}
