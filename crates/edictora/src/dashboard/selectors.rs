//! 集めた要素から目的の 1 つを選ぶ純粋関数群（C# の `ElementSelectors`）。
//!
//! ページ内スクリプト（page-scripts/）は候補要素の情報を集めて返すだけで、
//! 「どれを選ぶか」はすべてここで判断する（ブラウザなしでテストできるようにするため）。
//!
//! 曖昧なときに「最初の 1 つ」を選ぶと、別の欄を書き換える事故につながる。
//! そのため、候補がちょうど 1 つに絞れないときは必ず止める（`Dashboard` エラー）。

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::dashboard::text::{
    AUTO_PUBLISH_MARKER, DEFAULT_LOCALE_MARKER, GUIDE_NOTICE_MARKER, LANGUAGE_LABEL,
    SAVE_DRAFT_BUTTON, SINGLE_LOCALE_NAMES, STATUS_LABEL, field_label,
};
use crate::dashboard::write_gate::ItemPageState;
use crate::error::{Error, Result};
use crate::listing::ListingField;
use crate::util::eq_ic;

/// ページから集めた要素 1 つの情報（JS 側で集め、Rust 側で選ぶ）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementInfo {
    /// ページ側の一時配列での位置（クリック・入力のときに使う）
    pub index: i32,
    /// タグ名（小文字）
    pub tag: String,
    /// innerText（前後空白を除いたもの）
    pub text: String,
    /// ラベル候補を連結した文字列（aria-label・aria-labelledby・label 要素・その欄だけを含む最大の祖先の innerText）
    pub label: String,
    /// disabled または aria-disabled="true"
    pub disabled: bool,
    /// readOnly または aria-readonly="true"
    pub read_only: bool,
    /// input の type（小文字。input 以外は空）
    pub r#type: String,
    /// チェックボックスの状態（チェックボックス以外は None）
    pub checked: Option<bool>,
    /// aria-expanded の値（"true"/"false"。属性が無ければ None）。コンボボックスの開閉判定に使う
    pub expanded: Option<String>,
}

/// 言語の選択肢 1 つ（「日本語 – ja」「英語 – en （デフォルト）」）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleOption {
    /// 言語名
    pub name: String,
    /// ロケールコード
    pub code: String,
    /// （デフォルト）表記があるか
    pub is_default: bool,
}

/// 収集結果（JSON 配列）を [`ElementInfo`] に変換する（配列でなければ空）
pub fn parse_elements(array: &Value) -> Vec<ElementInfo> {
    let Some(items) = array.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .map(|e| ElementInfo {
            index: e.get("index").and_then(Value::as_i64).unwrap_or(0) as i32,
            tag: str_of(e, "tag"),
            text: str_of(e, "text"),
            label: str_of(e, "label"),
            disabled: e.get("disabled") == Some(&Value::Bool(true)),
            read_only: e.get("readOnly") == Some(&Value::Bool(true)),
            r#type: str_of(e, "type"),
            checked: e.get("checked").and_then(Value::as_bool),
            expanded: e
                .get("expanded")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect()
}

/// 文字列配列（JSON）を変換する（文字列でない要素は空文字）
pub fn parse_strings(array: &Value) -> Vec<String> {
    array
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn str_of(e: &Value, name: &str) -> String {
    e.get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// 言語切替のコンボボックスを選ぶ。最初の combobox はパブリッシャー選択なので、ラベル「言語」で絞り込む
pub fn pick_language_combobox(comboboxes: &[ElementInfo]) -> Result<&ElementInfo> {
    // 部分一致ではなく「ラベルの 1 行が完全一致」で選ぶ。
    // 実画面ではカテゴリ欄の親要素に「すべての言語用」という説明があり、部分一致だと
    // カテゴリ欄まで「言語」に当たって 2 件になった（2026-11-13 の実ダッシュボードで確認）。
    exactly_one(
        comboboxes.iter().filter(|c| {
            has_label_line(Some(&c.label), LANGUAGE_LABEL)
                || has_label_line(Some(&c.text), LANGUAGE_LABEL)
        }),
        || {
            format!(
                "「{LANGUAGE_LABEL}」のコンボボックスが見つかりません（ダッシュボードが日本語表示か確認してください）"
            )
        },
        |n| format!("「{LANGUAGE_LABEL}」のコンボボックスが {n} 個あり、特定できません"),
    )
}

/// 候補がちょうど 1 つならそれを返し、0 個・複数なら `Dashboard` エラーにする（`pick_*` 共通の判断）。
///
/// 曖昧なときに「最初の 1 つ」を選ばないという安全策（モジュール先頭のコメント参照）を 1 か所にまとめたもの。
/// エラー文言は呼び出し側ごとに違うので、0 個のとき・n 個のときの文言を作る関数を受け取る。
/// 0 個の文言は、ある言語の一覧を作るなど計算が要ることがあるため、失敗したときだけ呼ぶ（遅延評価）。
fn exactly_one<'a, T: 'a>(
    candidates: impl IntoIterator<Item = &'a T>,
    none_message: impl FnOnce() -> String,
    many_message: impl FnOnce(usize) -> String,
) -> Result<&'a T> {
    let matches: Vec<&'a T> = candidates.into_iter().collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(Error::Dashboard(none_message())),
        n => Err(Error::Dashboard(many_message(n))),
    }
}

/// 選択肢の表記「<言語名> – <code>」を解釈する。形式が違えば None。
///
/// 区切りは EN DASH（U+2013）。念のため EM DASH・ハイフンも受け付ける。
/// 後ろに「（デフォルト）」などが付くことがある。コードは完全一致で比べる（en が en-GB に当たらないように）。
pub fn parse_locale_option(text: Option<&str>) -> Option<LocaleOption> {
    let text = text?;
    if text.trim().is_empty() {
        return None;
    }
    if text.contains('\n') {
        // 選択肢の innerText にアイコン等の行が混ざる場合に備え、解釈できた最後の行を使う
        return text
            .split('\n')
            .rev()
            .find_map(|line| parse_locale_option(Some(line)));
    }
    // 固定の正規表現は初回だけコンパイルする（LazyLock は初回参照時に 1 度だけ初期化され、スレッド安全）
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<name>.+?)\s*[–—-]\s*(?P<code>[A-Za-z]{2,3}(?:[-_][A-Za-z0-9]{2,8})*)(?P<rest>(?:\s|（|\().*)?$")
            .expect("固定の正規表現")
    });
    let caps = RE.captures(text.trim())?;
    let code = caps["code"].replace('_', "-");
    let rest = caps.name("rest").map(|m| m.as_str()).unwrap_or("");
    Some(LocaleOption {
        name: caps["name"].trim().to_string(),
        code,
        // 「（デフォルト）」表記の判定。文言は text.rs に集めている
        is_default: rest.contains(DEFAULT_LOCALE_MARKER),
    })
}

/// コンボボックスの表示テキストから、いま選ばれているロケールを読む。
/// 表示は「言語\n英語 – en （デフォルト）」のように複数行のことがあるので、最後に解釈できた行を使う
pub fn current_locale_code(combobox: &ElementInfo) -> Option<String> {
    combobox
        .text
        .split('\n')
        .rev()
        .find_map(|line| parse_locale_option(Some(line)))
        .map(|p| p.code)
}

/// 言語が 1 つだけのアイテムの言語コード。
///
/// 選択欄の表示がコード付き（「日本語 – ja」）なら複数言語のアイテムなので None。
/// コードなしの言語名（「日本語」）だけなら、[`SINGLE_LOCALE_NAMES`] で対応するコードを返す。
/// 表示の最後の行（ラベル「言語」の繰り返しの後にある現在値）を、前後の空白を除いて完全一致で比べる。
/// 対応表に無い言語名も None（呼び出し側が「解釈できない」として止める）
pub fn single_locale_code(combobox: &ElementInfo) -> Option<String> {
    if current_locale_code(combobox).is_some() {
        return None;
    }
    let shown = combobox
        .text
        .split('\n')
        .map(str::trim)
        .rfind(|line| !line.is_empty())?;
    SINGLE_LOCALE_NAMES
        .iter()
        .find(|(name, _)| *name == shown)
        .map(|(_, code)| (*code).to_string())
}

/// 選択肢一覧からロケールコードの一覧を作る（解釈できない行は無視、重複は除く）
pub fn locale_codes(options: &[ElementInfo]) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for o in options {
        if let Some(p) = parse_locale_option(Some(&o.text))
            && !codes.iter().any(|c| eq_ic(c, &p.code))
        {
            codes.push(p.code);
        }
    }
    codes
}

/// コードが完全一致する選択肢を 1 つ選ぶ
pub fn pick_locale_option<'a>(options: &'a [ElementInfo], code: &str) -> Result<&'a ElementInfo> {
    exactly_one(
        options
            .iter()
            .filter(|o| parse_locale_option(Some(&o.text)).is_some_and(|p| eq_ic(&p.code, code))),
        || {
            format!(
                "言語の選択肢にロケール {code} がありません（ある言語: {}）",
                locale_codes(options).join(", ")
            )
        },
        |n| format!("言語の選択肢にロケール {code} が {n} 個あり、特定できません"),
    )
}

/// 文言が完全一致するボタンを 1 つ選ぶ（前後の空白は無視）
pub fn pick_button<'a>(buttons: &'a [ElementInfo], text: &str) -> Result<&'a ElementInfo> {
    exactly_one(
        buttons.iter().filter(|b| b.text.trim() == text),
        || format!("「{text}」ボタンが見つかりません"),
        |n| format!("「{text}」ボタンが {n} 個あり、特定できません"),
    )
}

/// ラベルで URL 欄を 1 つ選ぶ。空白の有無（「公式 URL」と「公式URL」）は区別しない
pub fn pick_field_by_label(inputs: &[ElementInfo], field: ListingField) -> Result<&ElementInfo> {
    let Some(label) = field_label(field) else {
        return Err(Error::Dashboard(
            "説明欄はラベルではなく textarea で特定します".to_string(),
        ));
    };
    // 言語欄と同じ理由で、ラベルの 1 行が完全一致するものだけを選ぶ（親要素の説明文に別の欄名が含まれても当たらない）。
    exactly_one(
        inputs
            .iter()
            .filter(|i| has_label_line(Some(&i.label), label)),
        || format!("「{label}」の入力欄が見つかりません"),
        |n| format!("「{label}」の入力欄が {n} 個あり、特定できません"),
    )
}

/// 説明欄（textarea）を 1 つ選ぶ
pub fn pick_description(textareas: &[ElementInfo]) -> Result<&ElementInfo> {
    match textareas.len() {
        1 => Ok(&textareas[0]),
        0 => Err(Error::Dashboard(
            "説明欄（textarea）が見つかりません".to_string(),
        )),
        n => Err(Error::Dashboard(format!(
            "textarea が {n} 個あり、説明欄を特定できません"
        ))),
    }
}

/// 書き込み先として使える入力欄か確かめる（不可ならエラー）
pub fn ensure_writable(element: &ElementInfo, field: ListingField) -> Result<()> {
    let ok = if field == ListingField::Description {
        element.tag == "textarea"
    } else {
        element.tag == "input" && matches!(element.r#type.as_str(), "" | "text" | "url")
    };
    if !ok {
        return Err(Error::Dashboard(format!(
            "{}の欄がテキスト入力欄ではありません（{} {}）。画面構成が想定と違うため止めます",
            field.display_name(),
            element.tag,
            element.r#type
        )));
    }
    if element.disabled || element.read_only {
        return Err(Error::WriteBlocked(format!(
            "{}の欄が編集できない状態です",
            field.display_name()
        )));
    }
    Ok(())
}

/// 常設の案内通知を除いた結果メッセージの一覧
pub fn result_messages<S: AsRef<str>>(messages: &[S]) -> Vec<String> {
    messages
        .iter()
        .map(|m| m.as_ref().trim().to_string())
        .filter(|m| !m.is_empty() && !m.contains(GUIDE_NOTICE_MARKER))
        .collect()
}

/// 結果メッセージの中に `keyword` を含むものがあるか
pub fn has_message<S: AsRef<str>>(messages: &[S], keyword: &str) -> bool {
    result_messages(messages)
        .iter()
        .any(|m| m.contains(keyword))
}

/// ページ本文から「ステータス: …」の値を取り出す。見つからなければ None
pub fn parse_status(body_text: Option<&str>) -> Option<String> {
    let body = body_text.filter(|b| !b.is_empty())?;
    // ラベルの文言は text.rs の定数から組み立てる（正規表現の特殊文字を含んでも壊れないよう escape する）。
    // 区切りは半角の「:」と全角の「：」の両方を受け付ける（実画面の表記ゆれに備える）。
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"{}\s*[:：]\s*(?P<status>[^\n]+)",
            regex::escape(STATUS_LABEL)
        ))
        .expect("固定の正規表現")
    });
    RE.captures(body).map(|c| c["status"].trim().to_string())
}

/// コンボボックスが「言語」に触れているか（ラベルか表示テキストに、空白を無視して「言語」を含む）。
///
/// [`pick_language_combobox`]（行単位の完全一致）より緩い、部分一致の条件。
/// 編集画面の待ち条件（`item_page` の `wait_for_editor_ready`）で、言語の選択欄が描画されたかの目安として使う。
/// ここでは特定（1 つに絞る）は要らないので、緩い条件で足りる。
pub fn mentions_language_label(c: &ElementInfo) -> bool {
    contains_ignoring_space(Some(&c.label), LANGUAGE_LABEL)
        || contains_ignoring_space(Some(&c.text), LANGUAGE_LABEL)
}

/// 編集画面から集めた要素で、書き込み可否の判断材料（[`ItemPageState`]）を組み立てる。
///
/// CDP での収集（本文 → textarea → ボタンの順）は `item_page` 側に残し、ここは導出だけを行う（ブラウザなしでテストするため）。
/// - 説明欄: textarea がちょうど 1 つのときだけ「見つかった」とし、その 1 つが disabled / readOnly でなければ編集可
/// - 保存ボタン: 文言が「下書きとして保存する」と完全一致（前後空白は無視）するものがちょうど 1 つのときだけ「見つかった」とし、
///   その 1 つが disabled でなければ押せる
///
/// `textareas[0]` / `saves[0]` は `len() == 1 &&` の短絡評価で、要素があるときだけ読む。
pub fn item_page_state(
    status: Option<String>,
    textareas: &[ElementInfo],
    buttons: &[ElementInfo],
) -> ItemPageState {
    let saves: Vec<&ElementInfo> = buttons
        .iter()
        .filter(|b| b.text.trim() == SAVE_DRAFT_BUTTON)
        .collect();
    ItemPageState {
        status_text: status,
        description_found: textareas.len() == 1,
        description_editable: textareas.len() == 1
            && !textareas[0].disabled
            && !textareas[0].read_only,
        save_button_found: saves.len() == 1,
        save_button_enabled: saves.len() == 1 && !saves[0].disabled,
    }
}

/// 審査送信の確認ダイアログにある「自動的に公開する」チェックの状態を読む（読むだけで変えない）。
///
/// - ラベルに「自動的に公開する」を含む（空白は無視）チェックボックスがあれば、その状態（先頭のもの）
/// - 無ければ、チェックボックスがちょうど 1 つのときだけその状態
/// - それ以外は不明（None）
///
/// ラベルが一致した要素の状態が分からない（`checked` が None）ときは、チェックボックスが 1 つでも None を返す
/// （ラベルで見つけた要素を優先し、別の判断に切り替えない）。
pub fn auto_publish_state(checks: &[ElementInfo]) -> Option<bool> {
    match checks
        .iter()
        .find(|c| contains_ignoring_space(Some(&c.label), AUTO_PUBLISH_MARKER))
    {
        Some(c) => c.checked,
        None if checks.len() == 1 => checks[0].checked,
        None => None,
    }
}

/// 改行区切りのラベル文字列のどれか 1 行が、`label` と完全一致するか（空白の有無は無視）。
///
/// ラベル候補には「その欄だけを含む最大の祖先」の文字列も入るため、部分一致だと説明文中の語に誤って当たる。
/// 実画面ではラベルが単独の行（aria-labelledby の要素や aria-label）として現れるので、行単位の完全一致で十分に特定できる。
pub fn has_label_line(labels: Option<&str>, label: &str) -> bool {
    let Some(labels) = labels else { return false };
    let target = remove_white_space(label);
    labels
        .split('\n')
        .any(|line| remove_white_space(line) == target)
}

/// 空白を無視した部分一致
pub fn contains_ignoring_space(haystack: Option<&str>, needle: &str) -> bool {
    haystack.is_some_and(|h| remove_white_space(h).contains(&remove_white_space(needle)))
}

fn remove_white_space(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}
