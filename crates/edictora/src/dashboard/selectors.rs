//! 集めた要素から目的の 1 つを選ぶ純粋関数群（C# の `ElementSelectors`）。
//!
//! ページ内スクリプト（page-scripts/）は候補要素の情報を集めて返すだけで、
//! 「どれを選ぶか」はすべてここで判断する（ブラウザなしでテストできるようにするため）。
//!
//! 曖昧なときに「最初の 1 つ」を選ぶと、別の欄を書き換える事故につながる。
//! そのため、候補がちょうど 1 つに絞れないときは必ず止める（`Dashboard` エラー）。

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::dashboard::PageImage;
use crate::dashboard::text::{CategoryText, DashboardLang, DashboardText, PrivacyText};
use crate::dashboard::write_gate::ItemPageState;
use crate::error::{Error, Result};
use crate::images::ImageSlot;
use crate::listing::ListingField;
use crate::privacy::{DataKind, HOST_PERMISSION_KEY};
use crate::util::{contains_ic, eq_ic};
use crate::{tr, trs};

/// `document.documentElement.lang` の値から、ダッシュボードの表示言語を決める。
///
/// 主サブタグ（`ja-JP` の `ja`、`en-GB` の `en`）を大文字小文字・前後の空白を問わずに見る。
/// 日本語・英語以外は推測せずにエラーで止める。文言で要素を探すため、
/// 知らない言語の画面で操作を続けると、見つからないか、取り違える恐れがあるため。
/// lang が空（または読めない）ときは、英語対応の前と同じ日本語の文言セットにする。
/// 日本語表示の実画面で lang 属性の値をまだ確かめておらず、確認済みの日本語の経路を
/// lang 属性の有無で止めないため。英語の画面だった場合は日本語の文言が見つからずに止まる（安全側）。
/// 地域違いの英語（en-GB など）は同じ文言セットを使う。文言が違えば要素が見つからずに止まる（安全側）。
pub fn dashboard_lang(lang: Option<&str>) -> Result<DashboardLang> {
    let raw = lang.unwrap_or("").trim();
    let primary = raw
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match primary.as_str() {
        "ja" => Ok(DashboardLang::Ja),
        "en" => Ok(DashboardLang::En),
        _ if raw.is_empty() => Ok(DashboardLang::Ja),
        _ => Err(Error::Dashboard(tr!(
            "dashboard.the_dashboard_display_language_is_not_supported",
            raw = raw
        ))),
    }
}

/// [`dashboard_lang`] で決めた言語の文言セット
pub fn dashboard_text_for_lang(lang: Option<&str>) -> Result<&'static DashboardText> {
    dashboard_lang(lang).map(DashboardLang::text)
}

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
    /// `fileInput` のみ: accept 属性
    pub accept: String,
    /// `fileInput` のみ: スロットの中の画像（画面の並び順）
    pub images: Vec<PageImage>,
    /// `slotImage` / `imageButton` のみ: 枠の大きさ（0 ならまだ表示されていない）。
    /// `listboxOption` では、所属する一覧の枠の大きさ（閉じている間は 0）
    pub width: i32,
    pub height: i32,
    /// `listboxOption` のみ: `data-value` 属性（無ければ空）
    pub data_value: String,
    /// `listboxOption` のみ: aria-selected（"true" / "false" 以外・無ければ None）
    pub selected: Option<bool>,
    /// `listboxOption` のみ: 所属する一覧の、ページ内の `[role=listbox]` の通し番号（一覧に属さなければ -1）。
    /// 同じラベルの一覧が複数あっても、選択肢が混ざったことに気づけるようにする
    pub group: i32,
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

/// クリックしようとした要素が覆われていたときのエラー文を組み立てる（`point` の結果から）。
///
/// 目的: エラー文だけで原因（何が重なっていたか・画面の大きさ・要素の位置）を絞れるようにする。
/// これが無いと、呼び出した側（AI エージェントを含む）が「画面サイズのせいかも」と推測して
/// `--show-browser` で表示して試すことになり、利用者の画面にウィンドウが出てしまう（2026-11-15 に実際に起きた）。
/// 古いページ内スクリプトなどで項目が欠けていても、分かる部分だけを出す。
pub fn covered_error_message(what: &str, point: &Value) -> String {
    let num = |v: Option<&Value>| v.and_then(Value::as_f64).map(|n| n.round() as i64);
    let mut details = Vec::new();
    if let Some(cover) = point.get("cover").and_then(Value::as_str) {
        // ページ内スクリプトは出力の言語を知らないので、「中心が画面の外」は固定の印で返し、ここで文に直す
        let cover = if cover == OFFSCREEN_COVER {
            trs!("dashboard.no_element_the_center_is_outside_the")
        } else {
            cover
        };
        details.push(tr!("dashboard.element_at_the_center", element = cover));
    }
    if let (Some(x), Some(y)) = (num(point.get("x")), num(point.get("y"))) {
        details.push(tr!("dashboard.center", x = x, y = y));
    }
    if let Some(r) = point.get("rect")
        && let (Some(l), Some(t), Some(w), Some(h)) = (
            num(r.get("left")),
            num(r.get("top")),
            num(r.get("width")),
            num(r.get("height")),
        )
    {
        details.push(tr!(
            "dashboard.element_box_left_top_width_height",
            left = l,
            top = t,
            width = w,
            height = h
        ));
    }
    if let Some(v) = point.get("viewport")
        && let (Some(w), Some(h)) = (num(v.get("width")), num(v.get("height")))
    {
        details.push(tr!("dashboard.viewport", width = w, height = h));
    }
    if details.is_empty() {
        tr!(
            "dashboard.cannot_click_because_another_element_covers_it",
            what = what
        )
    } else {
        let joined = details.join(trs!("dashboard.list_separator"));
        tr!(
            "dashboard.cannot_click_because_another_element_covers_it_2",
            what = what,
            joined = joined
        )
    }
}

/// ページ内スクリプトの `point` が「中心に要素が無い（画面の外）」ときに `cover` へ入れる固定の印。
/// page-scripts/src/edictora.ts と同じ文字列にする（テストで固定）
pub const OFFSCREEN_COVER: &str = "__EDICTORA_OFFSCREEN__";

/// ページ内スクリプトの `get` が「要素が画面から消えた」ときに投げる例外の印（後ろに種類名が続く）。
/// page-scripts/src/edictora.ts と同じ文字列にする（テストで固定）
pub const ELEMENT_GONE_MARKER: &str = "EDICTORA_ELEMENT_GONE:";

/// ページ内スクリプトの例外の文（`cdp` の `JavaScript` エラーの本文）を、利用者向けの文に直す。
///
/// ページ内スクリプトは出力の言語を知らないので、「要素が画面から消えた」は固定の印と種類名だけを投げる。
/// 印の部分だけを文に置き換え、前後（cdp が付けた接頭辞・例外の種類・スタック）はそのまま残す
/// （日本語の文を以前の「要素が画面から消えました: <種類>」と同じにするため）。印が無ければ元の文のまま。
pub fn page_script_error_message(message: &str) -> String {
    message.replace(
        ELEMENT_GONE_MARKER,
        trs!("dashboard.the_element_disappeared_from_the_page"),
    )
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
            accept: str_of(e, "accept"),
            images: e
                .get("images")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|m| PageImage {
                            alt: str_of(m, "alt"),
                            src: str_of(m, "src"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            width: e.get("width").and_then(Value::as_i64).unwrap_or(0) as i32,
            height: e.get("height").and_then(Value::as_i64).unwrap_or(0) as i32,
            data_value: str_of(e, "dataValue"),
            selected: match e.get("selected").and_then(Value::as_str) {
                Some("true") => Some(true),
                Some("false") => Some(false),
                _ => None,
            },
            group: e.get("group").and_then(Value::as_i64).unwrap_or(-1) as i32,
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
pub fn pick_language_combobox<'a>(
    t: &DashboardText,
    comboboxes: &'a [ElementInfo],
) -> Result<&'a ElementInfo> {
    let label = t.language_label;
    // 部分一致ではなく「ラベルの 1 行が完全一致」で選ぶ。
    // 実画面ではカテゴリ欄の親要素に「すべての言語用」という説明があり、部分一致だと
    // カテゴリ欄まで「言語」に当たって 2 件になった（2026-11-13 の実ダッシュボードで確認）。
    exactly_one(
        comboboxes.iter().filter(|c| {
            has_label_line(Some(&c.label), label) || has_label_line(Some(&c.text), label)
        }),
        || tr!("dashboard.the_combo_box_was_not_found_check", label = label),
        |n| {
            tr!(
                "dashboard.found_combo_boxes_cannot_identify_which_one",
                count = n,
                label = label
            )
        },
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
/// 後ろに「（デフォルト）」「(Default)」などが付くことがある（全角・半角のどちらの括弧も受ける）。
/// コードは完全一致で比べる（en が en-GB に当たらないように）。
/// 言語名とコードの形は日本語・英語の画面で同じ（「日本語 – ja」「English – en (Default)」）なので、
/// 文言セットから使うのは既定の言語の表記（[`DashboardText::default_locale_marker`]）だけ。
pub fn parse_locale_option(t: &DashboardText, text: Option<&str>) -> Option<LocaleOption> {
    let text = text?;
    if text.trim().is_empty() {
        return None;
    }
    if text.contains('\n') {
        // 選択肢の innerText にアイコン等の行が混ざる場合に備え、解釈できた最後の行を使う
        return text
            .split('\n')
            .rev()
            .find_map(|line| parse_locale_option(t, Some(line)));
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
        // 「（デフォルト）」「(Default)」表記の判定。文言は text.rs に集めている。
        // 英語の大文字小文字の揺れ（"(default)"）に備えて区別しない（日本語には影響しない）
        is_default: contains_ic(rest, t.default_locale_marker),
    })
}

/// コンボボックスの表示テキストから、いま選ばれているロケールを読む。
/// 表示は「言語\n英語 – en （デフォルト）」のように複数行のことがあるので、最後に解釈できた行を使う
pub fn current_locale_code(t: &DashboardText, combobox: &ElementInfo) -> Option<String> {
    combobox
        .text
        .split('\n')
        .rev()
        .find_map(|line| parse_locale_option(t, Some(line)))
        .map(|p| p.code)
}

/// 言語が 1 つだけのアイテムの言語コード。
///
/// 選択欄の表示がコード付き（「日本語 – ja」）なら複数言語のアイテムなので None。
/// コードなしの言語名（「日本語」）だけなら、[`DashboardText::single_locale_names`] で対応するコードを返す。
/// 表示の最後の行（ラベル「言語」の繰り返しの後にある現在値）を、前後の空白を除いて完全一致で比べる。
/// 対応表に無い言語名も None（呼び出し側が「解釈できない」として止める）
pub fn single_locale_code(t: &DashboardText, combobox: &ElementInfo) -> Option<String> {
    if current_locale_code(t, combobox).is_some() {
        return None;
    }
    let shown = combobox
        .text
        .split('\n')
        .map(str::trim)
        .rfind(|line| !line.is_empty())?;
    t.single_locale_names
        .iter()
        .find(|(name, _)| *name == shown)
        .map(|(_, code)| (*code).to_string())
}

/// 選択肢一覧からロケールコードの一覧を作る（解釈できない行は無視、重複は除く）
pub fn locale_codes(t: &DashboardText, options: &[ElementInfo]) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for o in options {
        if let Some(p) = parse_locale_option(t, Some(&o.text))
            && !codes.iter().any(|c| eq_ic(c, &p.code))
        {
            codes.push(p.code);
        }
    }
    codes
}

/// コードが完全一致する選択肢を 1 つ選ぶ
pub fn pick_locale_option<'a>(
    t: &DashboardText,
    options: &'a [ElementInfo],
    code: &str,
) -> Result<&'a ElementInfo> {
    exactly_one(
        options.iter().filter(|o| {
            parse_locale_option(t, Some(&o.text)).is_some_and(|p| eq_ic(&p.code, code))
        }),
        || {
            let have = locale_codes(t, options).join(", ");
            tr!(
                "dashboard.the_language_options_have_no_locale_available",
                code = code,
                have = have
            )
        },
        |n| {
            tr!(
                "dashboard.found_language_options_for_locale_cannot_identify",
                count = n,
                code = code
            )
        },
    )
}

/// 文言が完全一致するボタンを 1 つ選ぶ（前後の空白は無視）
pub fn pick_button<'a>(buttons: &'a [ElementInfo], text: &str) -> Result<&'a ElementInfo> {
    exactly_one(
        buttons.iter().filter(|b| b.text.trim() == text),
        || tr!("dashboard.the_button_was_not_found", text = text),
        |n| {
            tr!(
                "dashboard.found_buttons_cannot_identify_which_one_to",
                count = n,
                text = text
            )
        },
    )
}

/// ラベルで URL 欄を 1 つ選ぶ。空白の有無（「公式 URL」と「公式URL」）は区別しない
pub fn pick_field_by_label<'a>(
    t: &DashboardText,
    inputs: &'a [ElementInfo],
    field: ListingField,
) -> Result<&'a ElementInfo> {
    let Some(label) = t.field_label(field) else {
        return Err(Error::Dashboard(tr!(
            "dashboard.the_description_field_is_identified_by_its"
        )));
    };
    // 言語欄と同じ理由で、ラベルの 1 行が完全一致するものだけを選ぶ（親要素の説明文に別の欄名が含まれても当たらない）。
    exactly_one(
        inputs
            .iter()
            .filter(|i| has_label_line(Some(&i.label), label)),
        || tr!("dashboard.the_input_field_was_not_found", label = label),
        |n| {
            tr!(
                "dashboard.found_input_fields_cannot_identify_which_one",
                count = n,
                label = label
            )
        },
    )
}

/// 説明欄（textarea）を 1 つ選ぶ
pub fn pick_description(textareas: &[ElementInfo]) -> Result<&ElementInfo> {
    match textareas.len() {
        1 => Ok(&textareas[0]),
        0 => Err(Error::Dashboard(tr!(
            "dashboard.the_description_field_textarea_was_not_found"
        ))),
        n => Err(Error::Dashboard(tr!(
            "dashboard.found_textareas_cannot_identify_the_description_field",
            count = n
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
    let name = field.display_name();
    let (tag, ty) = (&element.tag, &element.r#type);
    if !ok {
        return Err(Error::Dashboard(tr!(
            "dashboard.the_field_is_not_a_text_input",
            name = name,
            tag = tag,
            input_type = ty
        )));
    }
    if element.disabled || element.read_only {
        return Err(Error::WriteBlocked(tr!(
            "dashboard.the_field_cannot_be_edited",
            name = name
        )));
    }
    Ok(())
}

/// 常設の案内通知を除いた結果メッセージの一覧（案内通知の語が無い言語では除外しない）
pub fn result_messages<S: AsRef<str>>(t: &DashboardText, messages: &[S]) -> Vec<String> {
    messages
        .iter()
        .map(|m| m.as_ref().trim().to_string())
        .filter(|m| !m.is_empty() && !t.guide_notice_marker.is_some_and(|g| m.contains(g)))
        .collect()
}

/// 1 つのメッセージが成功の語 `keyword` を表しているか。
///
/// - 日本語（`match_whole_word` が false）: 従来どおりの部分一致（実画面で確認した判定を変えない）
/// - 英語: 大文字小文字を区別しない単語単位の一致（"unsaved" の中の "saved" に当たらない）
/// - どちらも、[`DashboardText::failure_markers`] の語（"not"・"fail" など）を含むメッセージは成功と見なさない
///   （"could not be saved" を成功と取り違えない。日本語は空なので影響しない）
pub fn message_matches(t: &DashboardText, message: &str, keyword: &str) -> bool {
    let hit = if t.match_whole_word {
        contains_word_ic(message, keyword)
    } else {
        message.contains(keyword)
    };
    hit && !t.failure_markers.iter().any(|f| contains_ic(message, f))
}

/// 結果メッセージの中に、成功の語 `keyword` を表すものがあるか（[`message_matches`]）
pub fn has_message<S: AsRef<str>>(t: &DashboardText, messages: &[S], keyword: &str) -> bool {
    result_messages(t, messages)
        .iter()
        .any(|m| message_matches(t, m, keyword))
}

/// 大文字小文字を区別しない単語単位の一致（前後が英数字でない位置に `word` がある）
fn contains_word_ic(haystack: &str, word: &str) -> bool {
    let h = haystack.to_lowercase();
    let w = word.to_lowercase();
    if w.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(off) = h[from..].find(&w) {
        let start = from + off;
        let end = start + w.len();
        let before = h[..start].chars().next_back();
        let after = h[end..].chars().next();
        let boundary = |c: Option<char>| !c.is_some_and(char::is_alphanumeric);
        if boundary(before) && boundary(after) {
            return true;
        }
        // 一致した先頭の次の文字から探し直す（UTF-8 の文字境界に合わせる）
        from = start + h[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// ページ本文から「ステータス: …」の値を取り出す。見つからなければ None
pub fn parse_status(t: &DashboardText, body_text: Option<&str>) -> Option<String> {
    let body = body_text.filter(|b| !b.is_empty())?;
    // 正規表現は言語ごとに初回だけコンパイルする（文言セットは 2 つだけなので static を言語ごとに持つ）
    static JA_RE: LazyLock<Regex> = LazyLock::new(|| status_regex(DashboardLang::Ja.text()));
    static EN_RE: LazyLock<Regex> = LazyLock::new(|| status_regex(DashboardLang::En.text()));
    let re = match t.lang {
        DashboardLang::Ja => &*JA_RE,
        DashboardLang::En => &*EN_RE,
    };
    re.captures(body).map(|c| c["status"].trim().to_string())
}

/// ステータス行の正規表現を組み立てる。
///
/// ラベルの文言は text.rs から組み立てる（正規表現の特殊文字を含んでも壊れないよう escape する）。
/// 区切りは半角の「:」と全角の「：」の両方を受け付ける（実画面の表記ゆれに備える）。
/// `status_at_line_start` の言語（英語）は、ラベルを行頭（前の空白は可）に限る。
fn status_regex(t: &DashboardText) -> Regex {
    let prefix = if t.status_at_line_start {
        r"(?m)^[ \t]*"
    } else {
        ""
    };
    Regex::new(&format!(
        r"{prefix}{}\s*[:：]\s*(?P<status>[^\n]+)",
        regex::escape(t.status_label)
    ))
    .expect("固定の正規表現")
}

/// コンボボックスが「言語」に触れているか（ラベルか表示テキストに、空白を無視して「言語」を含む）。
///
/// [`pick_language_combobox`]（行単位の完全一致）より緩い、部分一致の条件。
/// 編集画面の待ち条件（`item_page` の `wait_for_editor_ready`）で、言語の選択欄が描画されたかの目安として使う。
/// ここでは特定（1 つに絞る）は要らないので、緩い条件で足りる。
pub fn mentions_language_label(t: &DashboardText, c: &ElementInfo) -> bool {
    contains_ignoring_space(Some(&c.label), t.language_label)
        || contains_ignoring_space(Some(&c.text), t.language_label)
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
    t: &DashboardText,
    status: Option<String>,
    textareas: &[ElementInfo],
    buttons: &[ElementInfo],
) -> ItemPageState {
    let saves: Vec<&ElementInfo> = buttons
        .iter()
        .filter(|b| b.text.trim() == t.save_draft_button)
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
/// - ラベルに「自動的に公開する」を含む（空白と大文字小文字は無視）チェックボックスがあれば、その状態（先頭のもの）
/// - 無ければ、チェックボックスがちょうど 1 つのときだけその状態
/// - それ以外は不明（None）
///
/// ラベルが一致した要素の状態が分からない（`checked` が None）ときは、チェックボックスが 1 つでも None を返す
/// （ラベルで見つけた要素を優先し、別の判断に切り替えない）。
pub fn auto_publish_state(t: &DashboardText, checks: &[ElementInfo]) -> Option<bool> {
    // 英語のラベルは文頭で大文字になりうる（"Automatically publish …"）ので大文字小文字を区別しない。日本語には影響しない
    let marker = remove_white_space(t.auto_publish_marker).to_lowercase();
    match checks.iter().find(|c| {
        remove_white_space(&c.label)
            .to_lowercase()
            .contains(&marker)
    }) {
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

// ───────── 画像アセット ─────────

/// スロットの寸法の表記（画面の案内は「1280x800」「440×280」のように x か × で書かれる）
fn size_pattern(slot: ImageSlot) -> &'static Regex {
    static ICON: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b128\s*[x×]\s*128\b").expect("固定の正規表現"));
    static SMALL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b440\s*[x×]\s*280\b").expect("固定の正規表現"));
    static MARQUEE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b1400\s*[x×]\s*560\b").expect("固定の正規表現"));
    static SHOT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b1280\s*[x×]\s*800\b").expect("固定の正規表現"));
    match slot {
        ImageSlot::Icon => &ICON,
        ImageSlot::SmallTile => &SMALL,
        ImageSlot::Marquee => &MARQUEE,
        ImageSlot::GlobalScreenshots | ImageSlot::LocalizedScreenshots => &SHOT,
    }
}

/// 画像のスロット（`input[type=file]`）を 1 つ選ぶ。
///
/// 見分けは、その入力を含む枠の文字列の「寸法の表記」（1280x800・440×280・1400×560）と「見出しの行」
/// （全言語向けアセット / ローカライズ版アセット）で行う。クラス名・位置には頼らない。
/// - スクリーンショットは見出しの行が必要（全言語向けとローカライズ版は同じ寸法なので、見出しで分ける）
/// - アイコン・小タイル・マーキーは寸法だけで選び、ローカライズ版の見出しを持つ枠は除く
/// - 候補がちょうど 1 つに絞れなければ止める（別のスロットを書き換える事故を防ぐ）
/// - PNG を受け付けない入力・無効な入力は候補にしない
pub fn pick_image_slot<'a>(
    t: &DashboardText,
    inputs: &'a [ElementInfo],
    slot: ImageSlot,
) -> Result<&'a ElementInfo> {
    let pattern = size_pattern(slot);
    let global = |i: &ElementInfo| has_label_line(Some(&i.text), t.global_assets_heading);
    let localized = |i: &ElementInfo| has_label_line(Some(&i.text), t.localized_assets_heading);
    let name = slot.display_name();
    exactly_one(
        inputs
            .iter()
            .filter(|i| i.tag == "input" && i.r#type == "file" && !i.disabled)
            .filter(|i| contains_ic(&i.accept, ".png"))
            .filter(|i| pattern.is_match(&i.text))
            .filter(|i| match slot {
                ImageSlot::GlobalScreenshots => global(i) && !localized(i),
                ImageSlot::LocalizedScreenshots => localized(i) && !global(i),
                ImageSlot::Icon | ImageSlot::SmallTile | ImageSlot::Marquee => !localized(i),
            }),
        || tr!("dashboard.the_image_slot_was_not_found", name = name),
        |n| {
            tr!(
                "dashboard.found_image_slots_cannot_identify_which_one",
                count = n,
                name = name
            )
        },
    )
}

/// スロットの `position`（0 始まり）番目の画像に画面が付ける alt（「スクリーンショット 3」など）
pub fn expected_image_alt(t: &DashboardText, slot: ImageSlot, position: usize) -> String {
    match slot {
        ImageSlot::Icon => t.icon_alt.to_string(),
        ImageSlot::SmallTile => t.small_tile_alt.to_string(),
        ImageSlot::Marquee => t.marquee_alt.to_string(),
        ImageSlot::GlobalScreenshots | ImageSlot::LocalizedScreenshots => {
            format!("{} {}", t.screenshot_alt_prefix, position + 1)
        }
    }
}

/// スロットの画像の alt が、画面が付ける並び（スクリーンショット 1, 2, ...）どおりか確かめる。
/// 保存前の照合と、アップロードのたびの確認に使う（画像の取り違え・並びの崩れに気づくため）
pub fn ensure_image_alts(t: &DashboardText, slot: ImageSlot, images: &[PageImage]) -> Result<()> {
    for (i, image) in images.iter().enumerate() {
        let want = expected_image_alt(t, slot, i);
        if image.alt.trim() != want {
            return Err(Error::Dashboard(tr!(
                "dashboard.the_image_alt_does_not_match_the_expected",
                name = slot.display_name(),
                position = i + 1,
                alt = image.alt.trim(),
                want = want
            )));
        }
    }
    Ok(())
}

/// 画像の削除ボタンの aria-label（`画像を削除 <alt>`）
pub fn delete_image_label(t: &DashboardText, alt: &str) -> String {
    format!("{}{}", t.delete_image_prefix, alt.trim())
}

/// 画像の削除ボタンを 1 つ選ぶ（aria-label が `画像を削除 <alt>` に完全一致するものだけ）。
/// 並べ替え（「画像を左に移動して並べ替え」など）のボタンは候補にしない
pub fn pick_delete_button<'a>(
    t: &DashboardText,
    buttons: &'a [ElementInfo],
    alt: &str,
) -> Result<&'a ElementInfo> {
    let want = delete_image_label(t, alt);
    exactly_one(
        buttons.iter().filter(|b| b.label.trim() == want),
        || tr!("dashboard.the_button_was_not_found", text = want),
        |n| {
            tr!(
                "dashboard.found_buttons_cannot_identify_which_one_to",
                count = n,
                text = want
            )
        },
    )
}

/// 画像まわりのクリックを許すか。押してよいのは削除ボタン（`画像を削除 ` で始まるラベル）だけで、
/// 並べ替え・アップロード状況を閉じるなど、それ以外は押さない（並べ替えに触れない）
pub fn ensure_image_click_allowed(t: &DashboardText, label: &str) -> Result<()> {
    if label.trim().starts_with(t.delete_image_prefix.trim_end()) {
        Ok(())
    } else {
        Err(Error::Dashboard(tr!(
            "dashboard.refusing_to_click_an_image_button_other_than_delete",
            label = label
        )))
    }
}

/// 画像のアップロードのエラーを示すメッセージ（結果メッセージのうち、失敗を表す語を含むもの）。
///
/// 常設の案内通知は除く（`result_messages`）。日本語の語は「失敗」「エラー」「無効」「できません」。
/// 英語の画面では画像を書かない（`ensure_image_write_supported`）が、同じ判定が読み出しでも呼べるよう英語の語も持つ
pub fn image_error_messages<S: AsRef<str>>(t: &DashboardText, messages: &[S]) -> Vec<String> {
    const MARKERS: [&str; 8] = [
        "失敗",
        "エラー",
        "無効",
        "できません",
        "error",
        "fail",
        "invalid",
        "unable",
    ];
    result_messages(t, messages)
        .into_iter()
        .filter(|m| MARKERS.iter().any(|k| contains_ic(m, k)))
        .collect()
}

// ───────── 「プライバシー」タブ ─────────

/// 「プライバシー」タブの textarea を分類した結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyTextAreas<'a> {
    /// 単一用途の説明の欄（無ければ None）
    pub single_purpose: Option<&'a ElementInfo>,
    /// 権限の正当性の欄（キーは manifest の権限名か `host`。画面の並び）
    pub permissions: Vec<(String, &'a ElementInfo)>,
}

/// 行（ラベル候補の 1 行）が「<権限名> が必要な理由」の形なら、権限名を返す。
///
/// 空白の有無は区別しない（「ホスト権限 が必要な理由」と「ホスト権限が必要な理由」を同じに見る）。
/// 説明文の行（「ホスト権限は、拡張機能のマニフェストの…」など）は、後半が一致しないので当たらない。
fn permission_name_of_line(p: &PrivacyText, line: &str) -> Option<String> {
    let compact = remove_white_space(line);
    let suffix = remove_white_space(p.permission_label_suffix);
    let name = compact.strip_suffix(&suffix)?;
    (!name.is_empty()).then(|| name.to_string())
}

/// 「プライバシー」タブの textarea を、ラベルの行で「単一用途の説明」と「権限ごとの正当性」に分ける。
///
/// - 権限名の照合は行単位（`has_label_line` と同じ考え方）。ホスト権限の名前は特別なキー `host` に直す
/// - どちらにも当たらない textarea（リモートコードの理由など、未対応の欄）は無視する
/// - 1 つの textarea が複数の権限名に当たる・同じ権限の欄が複数ある・単一用途の欄が複数あるなど、
///   1 つに絞れないときは止める（別の欄を書き換える事故を防ぐ）
pub fn classify_privacy_textareas<'a>(
    p: &PrivacyText,
    textareas: &'a [ElementInfo],
) -> Result<PrivacyTextAreas<'a>> {
    let host = remove_white_space(p.host_permission_name);
    let mut single: Option<&ElementInfo> = None;
    let mut permissions: Vec<(String, &ElementInfo)> = Vec::new();
    for textarea in textareas {
        let is_single = has_label_line(Some(&textarea.label), p.single_purpose_label);
        let mut names: Vec<String> = Vec::new();
        for line in textarea.label.split('\n') {
            if let Some(name) = permission_name_of_line(p, line)
                && !names.contains(&name)
            {
                names.push(name);
            }
        }
        if is_single && !names.is_empty() {
            return Err(Error::Dashboard(tr!(
                "dashboard.privacy_textarea_is_ambiguous",
                label = p.single_purpose_label
            )));
        }
        if is_single {
            if single.is_some() {
                return Err(Error::Dashboard(tr!(
                    "dashboard.privacy_found_several_fields",
                    label = p.single_purpose_label
                )));
            }
            single = Some(textarea);
            continue;
        }
        match names.as_slice() {
            [] => {}
            [name] => {
                let key = if *name == host {
                    HOST_PERMISSION_KEY.to_string()
                } else {
                    name.clone()
                };
                if permissions.iter().any(|(k, _)| *k == key) {
                    return Err(Error::Dashboard(tr!(
                        "dashboard.privacy_found_several_fields",
                        label = key
                    )));
                }
                permissions.push((key, textarea));
            }
            _ => {
                return Err(Error::Dashboard(tr!(
                    "dashboard.privacy_textarea_is_ambiguous",
                    label = names.join(" / ")
                )));
            }
        }
    }
    Ok(PrivacyTextAreas {
        single_purpose: single,
        permissions,
    })
}

/// ラベルの行が `label` に完全一致するチェックボックスを 1 つ選ぶ。見つからなければ None、複数なら止める
fn find_checkbox<'a>(checks: &'a [ElementInfo], label: &str) -> Result<Option<&'a ElementInfo>> {
    let found: Vec<&ElementInfo> = checks
        .iter()
        .filter(|c| c.checked.is_some() && has_label_line(Some(&c.label), label))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found[0])),
        n => Err(Error::Dashboard(tr!(
            "dashboard.privacy_found_several_checkboxes",
            count = n,
            label = label
        ))),
    }
}

/// データの種類のチェックボックスを 1 つ選ぶ（見つからなければ止める）
pub fn pick_data_kind_checkbox<'a>(
    p: &PrivacyText,
    checks: &'a [ElementInfo],
    kind: DataKind,
) -> Result<&'a ElementInfo> {
    let label = p.data_kind_labels[kind_index(kind)];
    find_checkbox(checks, label)?.ok_or_else(|| {
        Error::Dashboard(tr!(
            "dashboard.privacy_checkbox_was_not_found",
            label = label
        ))
    })
}

/// 宣誓チェック（0〜2 番目）を 1 つ選ぶ（見つからなければ止める）
pub fn pick_certify_checkbox<'a>(
    p: &PrivacyText,
    checks: &'a [ElementInfo],
    index: usize,
) -> Result<&'a ElementInfo> {
    let label = p.certify_labels[index];
    find_checkbox(checks, label)?.ok_or_else(|| {
        Error::Dashboard(tr!(
            "dashboard.privacy_checkbox_was_not_found",
            label = label
        ))
    })
}

fn kind_index(kind: DataKind) -> usize {
    DataKind::ALL
        .iter()
        .position(|k| *k == kind)
        .expect("ALL に全種類がある")
}

/// チェックされているデータの種類を読む。9 つのチェックボックスが 1 つでも見つからなければ None
/// （この欄が画面に無いものとして扱う）。ラベルが重なって絞れないときは止める
pub fn collected_kinds(
    p: &PrivacyText,
    checks: &[ElementInfo],
) -> Result<Option<BTreeSet<DataKind>>> {
    let mut set = BTreeSet::new();
    for kind in DataKind::ALL {
        match find_checkbox(checks, p.data_kind_labels[kind_index(kind)])? {
            None => return Ok(None),
            Some(c) => {
                if c.checked == Some(true) {
                    set.insert(kind);
                }
            }
        }
    }
    Ok(Some(set))
}

/// 3 つの宣誓チェックがすべて入っているか。3 つが 1 つでも見つからなければ None
pub fn certified(p: &PrivacyText, checks: &[ElementInfo]) -> Result<Option<bool>> {
    let mut all = true;
    for label in p.certify_labels {
        match find_checkbox(checks, label)? {
            None => return Ok(None),
            Some(c) => all &= c.checked == Some(true),
        }
    }
    Ok(Some(all))
}

/// 「プライバシー」タブで書き込み可否の判断材料（[`ItemPageState`]）を組み立てる。
///
/// 「主の入力欄」は単一用途の説明（ちょうど 1 つ）、保存ボタンは編集画面と同じ文言。
/// 分類できない（絞れない）ときは、見つからなかった扱いにして止める
pub fn privacy_page_state(
    t: &DashboardText,
    p: &PrivacyText,
    status: Option<String>,
    textareas: &[ElementInfo],
    buttons: &[ElementInfo],
) -> ItemPageState {
    let main: Vec<ElementInfo> = classify_privacy_textareas(p, textareas)
        .ok()
        .and_then(|areas| areas.single_purpose.cloned())
        .into_iter()
        .collect();
    item_page_state(t, status, &main, buttons)
}

/// 書き込み先のテキスト欄として使えるか確かめる（textarea で、編集できること）
pub fn ensure_privacy_text_writable(element: &ElementInfo, name: &str) -> Result<()> {
    if element.tag != "textarea" {
        return Err(Error::Dashboard(tr!(
            "dashboard.the_field_is_not_a_text_input",
            name = name,
            tag = element.tag,
            input_type = element.r#type
        )));
    }
    if element.disabled || element.read_only {
        return Err(Error::WriteBlocked(tr!(
            "dashboard.the_field_cannot_be_edited",
            name = name
        )));
    }
    Ok(())
}

/// プライバシー ポリシー URL の入力欄（`input[type=text]`）を、ラベルの 1 行の完全一致で探す。
/// 画面に無ければ None（欄が無い扱い）、複数あれば止める（別の欄を書き換える事故を防ぐ）
pub fn find_policy_url_input<'a>(
    p: &PrivacyText,
    inputs: &'a [ElementInfo],
) -> Result<Option<&'a ElementInfo>> {
    let found: Vec<&ElementInfo> = inputs
        .iter()
        .filter(|i| has_label_line(Some(&i.label), p.policy_url_label))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found[0])),
        _ => Err(Error::Dashboard(tr!(
            "dashboard.privacy_found_several_fields",
            label = p.policy_url_label
        ))),
    }
}

/// 書き込み先のプライバシー ポリシー URL 欄として使えるか確かめる（input の 1 行テキストで、編集できること）。
/// textarea 用の [`ensure_privacy_text_writable`] とは別（textarea の検査を広げない）
pub fn ensure_privacy_url_writable(element: &ElementInfo, name: &str) -> Result<()> {
    if element.tag != "input" || !matches!(element.r#type.as_str(), "" | "text" | "url") {
        return Err(Error::Dashboard(tr!(
            "dashboard.the_field_is_not_a_text_input",
            name = name,
            tag = element.tag,
            input_type = element.r#type
        )));
    }
    if element.disabled || element.read_only {
        return Err(Error::WriteBlocked(tr!(
            "dashboard.the_field_cannot_be_edited",
            name = name
        )));
    }
    Ok(())
}

// ───────── カテゴリ ─────────

/// カテゴリ欄のコンボボックスを選ぶ。ラベルの 1 行が完全一致するものだけ（言語欄などの親要素の説明文に
/// 「カテゴリ」が含まれても当たらない）。ちょうど 1 つに絞れなければ止める
pub fn pick_category_combobox<'a>(
    c: &CategoryText,
    comboboxes: &'a [ElementInfo],
) -> Result<&'a ElementInfo> {
    let label = c.label;
    exactly_one(
        comboboxes.iter().filter(|b| {
            has_label_line(Some(&b.label), label) || has_label_line(Some(&b.text), label)
        }),
        || tr!("dashboard.the_combo_box_was_not_found_check", label = label),
        |n| {
            tr!(
                "dashboard.found_combo_boxes_cannot_identify_which_one",
                count = n,
                label = label
            )
        },
    )
}

/// カテゴリの一覧（ラベルの 1 行が完全一致する `[role=listbox]`）に属する選択肢だけを返す（閉じて見えないものも含む）。
/// 一覧がちょうど 1 つ（`group` が 1 種類）でなければ止める。同名の一覧が 2 つあるのに混ぜてしまわないため
pub fn category_options<'a>(
    c: &CategoryText,
    options: &'a [ElementInfo],
) -> Result<Vec<&'a ElementInfo>> {
    let found: Vec<&ElementInfo> = options
        .iter()
        .filter(|o| o.group >= 0 && has_label_line(Some(&o.label), c.label))
        .collect();
    let mut groups: Vec<i32> = found.iter().map(|o| o.group).collect();
    groups.sort_unstable();
    groups.dedup();
    match groups.len() {
        0 => Err(Error::Dashboard(tr!(
            "dashboard.category_options_not_found",
            label = c.label
        ))),
        1 => Ok(found),
        n => Err(Error::Dashboard(tr!(
            "dashboard.category_found_several_lists",
            count = n,
            label = c.label
        ))),
    }
}

/// いま選ばれているカテゴリの `data-value` を読む（一覧を閉じたまま読める）。
/// `aria-selected="true"` の選択肢がちょうど 1 つでなければ止める。未選択（プレースホルダー）はその値のまま返す
pub fn selected_category(c: &CategoryText, options: &[ElementInfo]) -> Result<String> {
    let all = category_options(c, options)?;
    let selected: Vec<&&ElementInfo> = all.iter().filter(|o| o.selected == Some(true)).collect();
    match selected.as_slice() {
        [one] if !one.data_value.is_empty() => Ok(one.data_value.clone()),
        [_] => Err(Error::Dashboard(tr!(
            "dashboard.category_selected_option_has_no_value",
            label = c.label
        ))),
        many => Err(Error::Dashboard(tr!(
            "dashboard.category_selected_count",
            count = many.len(),
            label = c.label
        ))),
    }
}

/// カテゴリの読み取りが落ち着いているか（純粋関数）。再読込の直後は、選択中の option が一時的にプレースホルダー
/// だったり、コンボボックスの表示値と選択中の option の表示文言が食い違ったりすることがある。
/// 選択中がプレースホルダーでなく、コンボボックスの表示値（最後の空でない行）が選択中の option の文言と一致するとき true。
/// コンボボックスが無い（None）ときは、表示値の照合を省く。選択中がちょうど 1 つに読めなければ false
pub fn category_reading_is_stable(
    c: &CategoryText,
    combobox: Option<&ElementInfo>,
    options: &[ElementInfo],
) -> bool {
    let Ok(all) = category_options(c, options) else {
        return false;
    };
    let selected: Vec<&&ElementInfo> = all.iter().filter(|o| o.selected == Some(true)).collect();
    let [one] = selected.as_slice() else {
        return false;
    };
    if one.data_value.is_empty() || one.data_value == crate::listing::category::PLACEHOLDER_VALUE {
        return false;
    }
    let Some(combobox) = combobox else {
        return true;
    };
    let shown = combobox
        .text
        .split('\n')
        .map(str::trim)
        .rfind(|line| !line.is_empty());
    shown == Some(one.text.trim())
}

/// カテゴリの一覧が開いて見えているか（一覧の枠に大きさがあるか。閉じている間は 0×0）
pub fn category_list_visible(c: &CategoryText, options: &[ElementInfo]) -> bool {
    category_options(c, options)
        .map(|all| all.iter().any(|o| o.width > 0 && o.height > 0))
        .unwrap_or(false)
}

/// `data-value` が一致するカテゴリの選択肢を 1 つ選ぶ。未選択のプレースホルダーは選ばない
pub fn pick_category_option<'a>(
    c: &CategoryText,
    options: &'a [ElementInfo],
    data_value: &str,
) -> Result<&'a ElementInfo> {
    let not_found = || {
        tr!(
            "dashboard.category_option_not_found",
            label = c.label,
            value = data_value
        )
    };
    if data_value.is_empty() || data_value == crate::listing::category::PLACEHOLDER_VALUE {
        return Err(Error::Dashboard(not_found()));
    }
    let all = category_options(c, options)?;
    exactly_one(
        all.into_iter().filter(|o| o.data_value == data_value),
        not_found,
        |n| {
            tr!(
                "dashboard.category_found_several_options",
                count = n,
                label = c.label,
                value = data_value
            )
        },
    )
}
