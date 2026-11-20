//! C# の ElementSelectors.test.cs の移植（実画面の回帰 2 件を含む）

use edictora::Error;
use edictora::dashboard::selectors::*;
use edictora::dashboard::text::{DashboardLang, EN, JA};
use edictora::dashboard::write_gate::ItemPageState;
use edictora::listing::ListingField;

fn el(index: i32) -> ElementInfo {
    ElementInfo {
        index,
        tag: "div".to_string(),
        ..ElementInfo::default()
    }
}

trait With {
    fn text(self, t: &str) -> Self;
    fn label(self, l: &str) -> Self;
    fn tag(self, t: &str) -> Self;
    fn ty(self, t: &str) -> Self;
    fn disabled(self) -> Self;
    fn read_only(self) -> Self;
}

impl With for ElementInfo {
    fn text(mut self, t: &str) -> Self {
        self.text = t.to_string();
        self
    }
    fn label(mut self, l: &str) -> Self {
        self.label = l.to_string();
        self
    }
    fn tag(mut self, t: &str) -> Self {
        self.tag = t.to_string();
        self
    }
    fn ty(mut self, t: &str) -> Self {
        self.r#type = t.to_string();
        self
    }
    fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
    fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

fn is_dashboard<T: std::fmt::Debug>(r: edictora::Result<T>) -> bool {
    matches!(r, Err(Error::Dashboard(_)))
}

#[test]
fn pick_language_combobox_最初のパブリッシャー選択ではなく言語ラベルのものを選ぶ() {
    let combos = [
        el(0).text("Kagayoi"),
        el(1).text("言語\n英語 – en （デフォルト）"),
        el(2).text("仕事効率化").label("カテゴリ"),
    ];
    assert_eq!(pick_language_combobox(&JA, &combos).unwrap().index, 1);
}

#[test]
fn pick_language_combobox_親要素の説明文に言語を含むカテゴリ欄には当たらない() {
    // 2026-11-13 の実ダッシュボードの構成を再現する。カテゴリ欄の親要素には「すべての言語用」という説明があり、
    // 部分一致の判定だとカテゴリ欄も「言語」に当たって 2 件になり、特定できずに止まった。
    let combos = [
        el(0)
            .text("言語\n言語\n言語\n英語 – en （デフォルト）")
            .label("言語\n言語\n言語\n言語\n英語 – en （デフォルト）"),
        el(1)
            .text("カテゴリ\nカテゴリ\nカテゴリ\nユーザー補助機能")
            .label("カテゴリ\nカテゴリ\nカテゴリ\nカテゴリ\nユーザー補助機能\nすべての言語用"),
    ];
    assert_eq!(pick_language_combobox(&JA, &combos).unwrap().index, 0);
}

#[test]
fn pick_field_by_label_説明文に別の欄名を含む欄には当たらない() {
    let inputs = [
        el(0)
            .label("公式 URL\n所有しているウェブサイトの 1 つがアイテムと関連している場合はホームページ URL の代わりに表示されます")
            .tag("input"),
        el(1).label("ホームページ URL\nアイテムのウェブサイトへのリンク").tag("input"),
    ];
    assert_eq!(
        pick_field_by_label(&JA, &inputs, ListingField::HomepageUrl)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        pick_field_by_label(&JA, &inputs, ListingField::OfficialUrl)
            .unwrap()
            .index,
        0
    );
}

/// `Dashboard` エラーの本文を取り出す（文言をバイト単位で固定するテスト用）。
/// `Dashboard` 以外・成功のときは panic させ、種類の取り違えも検出する。
fn dashboard_message<T: std::fmt::Debug>(r: edictora::Result<T>) -> String {
    match r {
        Err(Error::Dashboard(m)) => m,
        other => panic!("Dashboard エラーを期待しましたが {other:?} でした"),
    }
}

// ───────── 「ちょうど 1 つ」に絞れないときのエラー文言の固定 ─────────
// 選択処理を共通化（`exactly_one`）しても、利用者に見える文言が一字一句変わらないことを確かめる。

#[test]
fn pick_language_combobox_0件と複数件のエラー文言() {
    assert_eq!(
        dashboard_message(pick_language_combobox(&JA, &[el(0).text("Kagayoi")])),
        "「言語」のコンボボックスが見つかりません（ダッシュボードが日本語か英語で表示されているか確認してください）"
    );
    assert_eq!(
        dashboard_message(pick_language_combobox(
            &JA,
            &[el(0).label("言語"), el(1).label("言語"), el(2).text("言語"),]
        )),
        "「言語」のコンボボックスが 3 個あり、特定できません"
    );
}

#[test]
fn pick_locale_option_0件と複数件のエラー文言() {
    let options = [el(0).text("英語 – en"), el(1).text("日本語 – ja")];
    assert_eq!(
        dashboard_message(pick_locale_option(&JA, &options, "fr")),
        "言語の選択肢にロケール fr がありません（ある言語: en, ja）"
    );
    assert_eq!(
        dashboard_message(pick_locale_option(
            &JA,
            &[el(0).text("英語 – en"), el(1).text("English – EN")],
            "en"
        )),
        "言語の選択肢にロケール en が 2 個あり、特定できません"
    );
}

#[test]
fn pick_button_0件と複数件のエラー文言() {
    assert_eq!(
        dashboard_message(pick_button(&[el(0).text("公開")], "削除")),
        "「削除」ボタンが見つかりません"
    );
    assert_eq!(
        dashboard_message(pick_button(
            &[el(0).text("審査のため送信"), el(1).text("審査のため送信")],
            JA.submit_button
        )),
        "「審査のため送信」ボタンが 2 個あり、特定できません"
    );
}

#[test]
fn pick_field_by_label_0件と複数件のエラー文言() {
    assert_eq!(
        dashboard_message(pick_field_by_label(
            &JA,
            &[el(0).label("公式 URL")],
            ListingField::SupportUrl
        )),
        "「サポート URL」の入力欄が見つかりません"
    );
    assert_eq!(
        dashboard_message(pick_field_by_label(
            &JA,
            &[el(0).label("サポート URL"), el(1).label("サポートURL")],
            ListingField::SupportUrl
        )),
        "「サポート URL」の入力欄が 2 個あり、特定できません"
    );
    // 説明欄はラベルで探さない（共通化の対象外の早期リターン）
    assert_eq!(
        dashboard_message(pick_field_by_label(&JA, &[], ListingField::Description)),
        "説明欄はラベルではなく textarea で特定します"
    );
}

#[test]
fn parse_locale_option_言語名とコードとデフォルト表記を読む() {
    let en = parse_locale_option(&JA, Some("英語 – en （デフォルト）"));
    let ja = parse_locale_option(&JA, Some("日本語 – ja"));
    let pt = parse_locale_option(&JA, Some("ポルトガル語（ブラジル） – pt-BR"));

    assert_eq!(
        en,
        Some(LocaleOption {
            name: "英語".to_string(),
            code: "en".to_string(),
            is_default: true
        })
    );
    assert_eq!(
        ja,
        Some(LocaleOption {
            name: "日本語".to_string(),
            code: "ja".to_string(),
            is_default: false
        })
    );
    assert_eq!(pt.map(|p| p.code).as_deref(), Some("pt-BR"));
    assert!(parse_locale_option(&JA, Some("言語")).is_none());
    assert!(parse_locale_option(&JA, Some("")).is_none());
}

#[test]
fn pick_locale_option_コードが完全一致するものだけを選ぶ() {
    let options = [
        el(0).text("英語（イギリス） – en-GB"),
        el(1).text("英語 – en （デフォルト）"),
        el(2).text("日本語 – ja"),
    ];
    assert_eq!(pick_locale_option(&JA, &options, "en").unwrap().index, 1);
    assert_eq!(pick_locale_option(&JA, &options, "en-GB").unwrap().index, 0);
    assert!(is_dashboard(pick_locale_option(&JA, &options, "fr")));
    assert_eq!(locale_codes(&JA, &options), vec!["en-GB", "en", "ja"]);
}

#[test]
fn current_locale_code_表示の最後の言語行を読む() {
    assert_eq!(
        current_locale_code(&JA, &el(0).text("言語\n英語 – en （デフォルト）")).as_deref(),
        Some("en")
    );
    assert!(current_locale_code(&JA, &el(0).text("言語")).is_none());
}

#[test]
fn single_locale_code_言語が1つだけのアイテムは言語名からコードを決める() {
    // 2026-11-14 の実画面: 言語が 1 つだけのアイテムの選択欄はコードなしの言語名だけ
    assert_eq!(
        single_locale_code(&JA, &el(0).text("言語\n言語\n言語\n日本語")).as_deref(),
        Some("ja")
    );
    assert_eq!(
        single_locale_code(&JA, &el(0).text("言語\n英語 ")).as_deref(),
        Some("en")
    );
    // コード付き（複数言語のアイテム）は対象外。切り替えの手順を使う
    assert!(single_locale_code(&JA, &el(0).text("言語\n日本語 – ja")).is_none());
    // 対応表に無い言語名は推測しない
    assert!(single_locale_code(&JA, &el(0).text("言語\nフランス語")).is_none());
    // 値が無い（ラベルだけ）も推測しない
    assert!(single_locale_code(&JA, &el(0).text("言語")).is_none());
}

#[test]
fn pick_button_文言の完全一致で1つだけ選ぶ() {
    let buttons = [
        el(0).text("公開"),
        el(1).text(" 下書きとして保存する "),
        el(2).text("審査のため送信"),
    ];
    assert_eq!(
        pick_button(&buttons, JA.save_draft_button).unwrap().index,
        1
    );
    assert!(is_dashboard(pick_button(&buttons, "削除")));
}

#[test]
fn pick_field_by_label_空白の有無を無視してラベルで特定する() {
    let inputs = [
        el(0)
            .label("公式URL\n確認済みのサイト")
            .tag("input")
            .ty("text"),
        el(1).label("ホームページ URL").tag("input").ty("text"),
        el(2).label("サポート URL").tag("input").ty("text"),
    ];
    assert_eq!(
        pick_field_by_label(&JA, &inputs, ListingField::OfficialUrl)
            .unwrap()
            .index,
        0
    );
    assert_eq!(
        pick_field_by_label(&JA, &inputs, ListingField::HomepageUrl)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        pick_field_by_label(&JA, &inputs, ListingField::SupportUrl)
            .unwrap()
            .index,
        2
    );
    assert!(is_dashboard(pick_field_by_label(
        &JA,
        &[el(0).label("サポート URL"), el(1).label("サポート URL")],
        ListingField::SupportUrl
    )));
}

#[test]
fn pick_description_textareaが1つでなければ例外() {
    assert_eq!(pick_description(&[el(3).tag("textarea")]).unwrap().index, 3);
    assert!(is_dashboard(pick_description(&[])));
    assert!(is_dashboard(pick_description(&[el(0), el(1)])));
}

#[test]
fn ensure_writable_テキスト欄でない要素や編集不可の欄は止める() {
    ensure_writable(&el(0).tag("textarea"), ListingField::Description).unwrap();
    ensure_writable(&el(0).tag("input").ty("url"), ListingField::SupportUrl).unwrap();

    assert!(matches!(
        ensure_writable(&el(0).tag("input").ty("checkbox"), ListingField::SupportUrl),
        Err(Error::Dashboard(_))
    ));
    assert!(matches!(
        ensure_writable(
            &el(0).tag("textarea").read_only(),
            ListingField::Description
        ),
        Err(Error::WriteBlocked(_))
    ));
    assert!(matches!(
        ensure_writable(&el(0).tag("input").disabled(), ListingField::HomepageUrl),
        Err(Error::WriteBlocked(_))
    ));
}

#[test]
fn has_message_常設の案内通知を除いて判定する() {
    let messages = vec![
        "ガイドやヒントが必要な場合は、ヘルプをご覧ください。保存されました",
        "",
    ];
    assert!(!has_message(&JA, &messages, JA.saved_message));
    let mut more = messages.clone();
    more.push("アイテムが保存されました。");
    assert!(has_message(&JA, &more, JA.saved_message));
}

// ───────── 編集画面の状態の導出（item_page の get_state から切り出した純粋関数）─────────

#[test]
fn item_page_state_説明欄と保存ボタンが1つずつなら見つかり操作できる() {
    let state = item_page_state(
        &JA,
        Some("下書き".to_string()),
        &[el(0).tag("textarea")],
        &[el(0).text("公開"), el(1).text(" 下書きとして保存する ")],
    );
    assert_eq!(
        state,
        ItemPageState {
            status_text: Some("下書き".to_string()),
            description_found: true,
            description_editable: true,
            save_button_found: true,
            save_button_enabled: true,
        }
    );
}

#[test]
fn item_page_state_編集不可の説明欄と押せない保存ボタン() {
    let readonly = item_page_state(
        &JA,
        None,
        &[el(0).tag("textarea").read_only()],
        &[el(0).text(JA.save_draft_button).disabled()],
    );
    assert!(readonly.description_found);
    assert!(!readonly.description_editable);
    assert!(readonly.save_button_found);
    assert!(!readonly.save_button_enabled);
    assert_eq!(readonly.status_text, None);

    let disabled = item_page_state(&JA, None, &[el(0).tag("textarea").disabled()], &[]);
    assert!(disabled.description_found);
    assert!(!disabled.description_editable);
}

#[test]
fn item_page_state_0個と複数個は見つからない扱い() {
    // 0 個（添字 0 を読まずに false になる）
    let none = item_page_state(&JA, None, &[], &[el(0).text("公開")]);
    assert!(!none.description_found);
    assert!(!none.description_editable);
    assert!(!none.save_button_found);
    assert!(!none.save_button_enabled);

    // 複数個（どれか 1 つを選ばない）
    let many = item_page_state(
        &JA,
        None,
        &[el(0).tag("textarea"), el(1).tag("textarea")],
        &[
            el(0).text(JA.save_draft_button),
            el(1).text(JA.save_draft_button),
        ],
    );
    assert!(!many.description_found);
    assert!(!many.description_editable);
    assert!(!many.save_button_found);
    assert!(!many.save_button_enabled);
}

// ───────── 自動公開チェックの読み取り ─────────

fn check(index: i32, label: &str, checked: Option<bool>) -> ElementInfo {
    ElementInfo {
        checked,
        ..el(index).label(label).tag("input").ty("checkbox")
    }
}

#[test]
fn auto_publish_state_ラベルが一致するチェックの状態を読む() {
    let checks = [
        check(0, "利用規約に同意する", Some(false)),
        check(1, "審査に合格したら自動的に公開する", Some(true)),
    ];
    assert_eq!(auto_publish_state(&JA, &checks), Some(true));
    // 空白の有無は無視する
    assert_eq!(
        auto_publish_state(&JA, &[check(0, "自動的に 公開する", Some(false))]),
        Some(false)
    );
}

#[test]
fn auto_publish_state_ラベルが無くてもチェックが1つならその状態() {
    assert_eq!(
        auto_publish_state(&JA, &[check(0, "", Some(true))]),
        Some(true)
    );
}

#[test]
fn auto_publish_state_0個とラベルなし複数は不明() {
    assert_eq!(auto_publish_state(&JA, &[]), None);
    assert_eq!(
        auto_publish_state(
            &JA,
            &[check(0, "", Some(true)), check(1, "別の項目", Some(false))]
        ),
        None
    );
}

#[test]
fn auto_publish_state_ラベル一致の状態が不明なら1個でも不明() {
    // ラベルで見つけた要素を優先し、「1 つだけ」の判断へ切り替えない
    assert_eq!(
        auto_publish_state(&JA, &[check(0, "自動的に公開する", None)]),
        None
    );
}

#[test]
fn parse_status_ステータス行の値を読む() {
    assert_eq!(
        parse_status(&JA, Some("アイテム\nステータス: 審査待ち\nパッケージ")).as_deref(),
        Some("審査待ち")
    );
    assert_eq!(
        parse_status(&JA, Some("ステータス：公開済み")).as_deref(),
        Some("公開済み")
    );
    assert!(parse_status(&JA, Some("なし")).is_none());
}

#[test]
fn covered_error_message_覆っていた要素と枠と画面の大きさを含める() {
    let point = serde_json::json!({
        "x": 640.4, "y": 812.6, "hit": false,
        "cover": "div[role=option]「英語 – en」",
        "rect": { "left": 600, "top": 790, "width": 81, "height": 45 },
        "viewport": { "width": 1280, "height": 900 }
    });
    assert_eq!(
        covered_error_message("言語の選択肢（ja）", &point),
        "言語の選択肢（ja）が他の要素に覆われていて押せません（中心にあった要素: div[role=option]「英語 – en」、中心: (640, 813)、要素の枠: 左 600・上 790・幅 81・高さ 45、画面: 1280×900）"
    );
}

#[test]
fn covered_error_message_項目が無ければ従来の文だけにする() {
    let point = serde_json::json!({ "hit": false });
    assert_eq!(
        covered_error_message("保存ボタン", &point),
        "保存ボタンが他の要素に覆われていて押せません"
    );
}

// ───────── 画面の表示言語の判定（<html lang> の解釈）─────────

#[test]
fn dashboard_lang_日本語と英語を主サブタグで判定する() {
    assert_eq!(dashboard_lang(Some("ja")).unwrap(), DashboardLang::Ja);
    assert_eq!(dashboard_lang(Some(" ja-JP ")).unwrap(), DashboardLang::Ja);
    assert_eq!(dashboard_lang(Some("en")).unwrap(), DashboardLang::En);
    assert_eq!(dashboard_lang(Some("en-US")).unwrap(), DashboardLang::En);
    assert_eq!(dashboard_lang(Some("EN_gb")).unwrap(), DashboardLang::En);
    assert_eq!(dashboard_text_for_lang(Some("ja")).unwrap(), &JA);
    assert_eq!(dashboard_text_for_lang(Some("en-US")).unwrap(), &EN);
}

#[test]
fn dashboard_lang_日本語と英語以外は推測せずに止める() {
    let fr = dashboard_message(dashboard_lang(Some("fr-FR")));
    assert!(fr.contains("fr-FR"), "{fr}");
    assert!(is_dashboard(dashboard_lang(Some("jav"))));
    assert!(is_dashboard(dashboard_lang(Some("english"))));
}

#[test]
fn dashboard_lang_空は従来どおり日本語にする() {
    // 日本語表示の実画面で lang 属性を確かめるまで、確認済みの日本語の経路を止めない
    assert_eq!(dashboard_lang(Some("  ")).unwrap(), DashboardLang::Ja);
    assert_eq!(dashboard_lang(None).unwrap(), DashboardLang::Ja);
}

// ───────── 英語表示の画面（文言は実画面で未確認の暫定値）─────────

#[test]
fn 英語_言語の選択欄をラベルの行の完全一致で選ぶ() {
    let combos = [
        el(0).text("Kagayoi"),
        el(1)
            .text("Language\nEnglish – en (Default)")
            .label("Language\nLanguage\nEnglish – en (Default)"),
        // 親要素の説明文に "languages" を含むカテゴリ欄には当たらない
        el(2)
            .text("Category\nProductivity")
            .label("Category\nProductivity\nFor all languages"),
    ];
    assert_eq!(pick_language_combobox(&EN, &combos).unwrap().index, 1);
    // 英語の文言セットでは日本語のラベルに当たらない（1 セットだけで探す）
    assert!(is_dashboard(pick_language_combobox(
        &EN,
        &[el(0).label("言語")]
    )));
    assert!(is_dashboard(pick_language_combobox(
        &JA,
        &[el(0).label("Language")]
    )));
}

#[test]
fn 英語_言語の選択肢と既定の表記を読む() {
    assert_eq!(
        parse_locale_option(&EN, Some("English – en (Default)")),
        Some(LocaleOption {
            name: "English".to_string(),
            code: "en".to_string(),
            is_default: true
        })
    );
    // 全角の括弧・小文字の default も受ける
    assert!(
        parse_locale_option(&EN, Some("English – en （default）"))
            .unwrap()
            .is_default
    );
    assert_eq!(
        parse_locale_option(&EN, Some("Japanese – ja")),
        Some(LocaleOption {
            name: "Japanese".to_string(),
            code: "ja".to_string(),
            is_default: false
        })
    );
    assert_eq!(
        parse_locale_option(&EN, Some("Portuguese (Brazil) – pt-BR"))
            .map(|p| p.code)
            .as_deref(),
        Some("pt-BR")
    );
    assert!(parse_locale_option(&EN, Some("Language")).is_none());

    let options = [
        el(0).text("English (United Kingdom) – en-GB"),
        el(1).text("English – en (Default)"),
        el(2).text("Japanese – ja"),
    ];
    assert_eq!(pick_locale_option(&EN, &options, "en").unwrap().index, 1);
    assert_eq!(locale_codes(&EN, &options), vec!["en-GB", "en", "ja"]);
    assert_eq!(
        current_locale_code(&EN, &el(0).text("Language\nEnglish – en (Default)")).as_deref(),
        Some("en")
    );
}

#[test]
fn 英語_言語が1つだけのアイテムは言語名からコードを決める() {
    assert_eq!(
        single_locale_code(&EN, &el(0).text("Language\nLanguage\nEnglish")).as_deref(),
        Some("en")
    );
    assert_eq!(
        single_locale_code(&EN, &el(0).text("Language\nJapanese ")).as_deref(),
        Some("ja")
    );
    assert!(single_locale_code(&EN, &el(0).text("Language\nFrench")).is_none());
    // 日本語の言語名は英語の文言セットでは解釈しない
    assert!(single_locale_code(&EN, &el(0).text("Language\n日本語")).is_none());
}

#[test]
fn 英語_ボタンとurl欄を英語の文言で選ぶ() {
    let buttons = [
        el(0).text("Publish"),
        el(1).text(" Save draft "),
        el(2).text("Submit for review"),
    ];
    assert_eq!(
        pick_button(&buttons, EN.save_draft_button).unwrap().index,
        1
    );
    assert_eq!(pick_button(&buttons, EN.submit_button).unwrap().index, 2);

    let inputs = [
        el(0)
            .label("Official URL\nShown instead of the homepage URL when ...")
            .tag("input"),
        el(1)
            .label("Homepage URL\nA link to your website")
            .tag("input"),
        el(2).label("Support URL").tag("input"),
    ];
    assert_eq!(
        pick_field_by_label(&EN, &inputs, ListingField::OfficialUrl)
            .unwrap()
            .index,
        0
    );
    assert_eq!(
        pick_field_by_label(&EN, &inputs, ListingField::HomepageUrl)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        pick_field_by_label(&EN, &inputs, ListingField::SupportUrl)
            .unwrap()
            .index,
        2
    );

    let state = item_page_state(
        &EN,
        Some("Draft".to_string()),
        &[el(0).tag("textarea")],
        &buttons,
    );
    assert!(state.save_button_found && state.save_button_enabled);
    // 日本語の文言セットでは英語のボタンは見つからない
    assert!(!item_page_state(&JA, None, &[], &buttons).save_button_found);
}

#[test]
fn 英語_ステータス行は行頭のラベルだけを読む() {
    assert_eq!(
        parse_status(&EN, Some("Item\nStatus: Published\nPackage")).as_deref(),
        Some("Published")
    );
    assert_eq!(
        parse_status(&EN, Some("  Status：Pending review")).as_deref(),
        Some("Pending review")
    );
    // 本文中の別の文の "status:" には当たらない
    assert!(parse_status(&EN, Some("Check the review status: see help")).is_none());
    assert!(parse_status(&EN, Some("Item status: Draft")).is_none());
    // 日本語の文言セットでは英語のラベルを読まない
    assert!(parse_status(&JA, Some("Status: Published")).is_none());
}

#[test]
fn 英語_成功メッセージは単語単位で否定の文を除いて判定する() {
    let saved = EN.saved_message;
    assert!(has_message(&EN, &["Item saved."], saved));
    assert!(has_message(&EN, &["Your changes have been SAVED"], saved));
    // 失敗・否定の文は成功と見なさない
    assert!(!has_message(&EN, &["Item could not be saved."], saved));
    assert!(!has_message(&EN, &["Changes weren't saved"], saved));
    assert!(!has_message(&EN, &["Failed: item saved partially"], saved));
    // 単語の一部（unsaved）には当たらない
    assert!(!has_message(&EN, &["You have unsaved changes"], saved));
    let submitted = EN.submitted_message;
    assert!(has_message(&EN, &["Item submitted for review."], submitted));
    assert!(!has_message(
        &EN,
        &["The item was not submitted"],
        submitted
    ));
    // 英語では案内通知を除外しない（語が不明なため）。空文字で全部が消えることもない
    assert_eq!(
        result_messages(&EN, &["Need help? See the guide", " ", "Item saved."]),
        vec!["Need help? See the guide", "Item saved."]
    );
    // 日本語の判定は従来どおり（部分一致・否定語なし）
    assert!(has_message(
        &JA,
        &["アイテムが保存されました。"],
        JA.saved_message
    ));
}

#[test]
fn 英語_自動公開チェックを大文字小文字を問わず見つける() {
    let checks = [
        check(0, "I agree to the terms", Some(false)),
        check(
            1,
            "Publish Automatically after the review passes",
            Some(true),
        ),
    ];
    assert_eq!(auto_publish_state(&EN, &checks), Some(true));
    assert_eq!(
        auto_publish_state(
            &EN,
            &[
                check(0, "publish automatically", Some(false)),
                check(1, "", Some(true))
            ]
        ),
        Some(false)
    );
}

#[test]
fn covered_error_message_画面の外の印を文に直す() {
    let point = serde_json::json!({ "hit": false, "cover": OFFSCREEN_COVER });
    assert_eq!(
        covered_error_message("保存ボタン", &point),
        "保存ボタンが他の要素に覆われていて押せません（中心にあった要素: （要素なし: 中心が画面の外））"
    );
}

// ───────── 画像アセット（掲載画像）─────────

/// 実画面（2026-11-15、Vuora・SearchClock）の `input[type=file]` 1 つ分。text はその入力を含む枠の文字列
fn file_input(index: i32, text: &str, images: &[(&str, &str)]) -> ElementInfo {
    ElementInfo {
        index,
        tag: "input".to_string(),
        r#type: "file".to_string(),
        accept: ".png,.jpg,.jpeg".to_string(),
        text: text.to_string(),
        images: images
            .iter()
            .map(|(alt, src)| edictora::dashboard::PageImage {
                alt: alt.to_string(),
                src: src.to_string(),
            })
            .collect(),
        ..ElementInfo::default()
    }
}

const ICON: &str = "ショップ アイコン\n128x128 ピクセル\nアイコンが画像に関するガイドラインに沿っているか確認してください";
const LOCALIZED: &str = "ローカライズ版アセット\nローカライズ版プロモーション動画\nYouTube 動画の URL を入力してください。\nローカライズ版スクリーンショット\n指定できるのは 5 枚までです\n1280x800 または 640x400\nJPEG または 24 ビット PNG（アルファなし）\n少なくとも 1 枚指定してください";
const GLOBAL: &str = "全言語向けアセット\n全言語向けプロモーション動画\nYouTube 動画の URL を入力してください。\n全言語向けスクリーンショット\n指定できるのは 5 枚までです\n1280x800 または 640x400\nJPEG または 24 ビット PNG（アルファなし）\n少なくとも 1 枚指定してください";
const TILE: &str =
    "プロモーション タイル（小）\n440×280 キャンバス\nJPEG または 24 ビット PNG（アルファなし）";
const MARQUEE: &str = "マーキー プロモーション タイル\n1400×560 キャンバス\nJPEG または 24 ビット PNG（アルファなし）";

/// 複数言語のアイテムの 5 つの入力（アイコン・ローカライズ版・全言語向け・小タイル・マーキー）
fn multi_language_inputs() -> Vec<ElementInfo> {
    vec![
        file_input(
            0,
            ICON,
            &[(
                "ショップ アイコン",
                "https://lh3.googleusercontent.com/a=s128-rw",
            )],
        ),
        file_input(
            1,
            LOCALIZED,
            &[(
                "スクリーンショット 1",
                "https://lh3.googleusercontent.com/l1=w160-h100-rw",
            )],
        ),
        file_input(
            2,
            GLOBAL,
            &[
                (
                    "スクリーンショット 1",
                    "https://lh3.googleusercontent.com/g1=w160-h100-rw",
                ),
                (
                    "スクリーンショット 2",
                    "https://lh3.googleusercontent.com/g2=w160-h100-rw",
                ),
            ],
        ),
        file_input(
            3,
            TILE,
            &[(
                "プロモーション タイル（小）",
                "https://lh3.googleusercontent.com/t=w158-h100-rw",
            )],
        ),
        file_input(
            4,
            MARQUEE,
            &[(
                "マーキー プロモーション タイル",
                "https://lh3.googleusercontent.com/m=w258-h100-rw",
            )],
        ),
    ]
}

#[test]
fn 画像の置き場を寸法と見出しで1つに絞る() {
    use edictora::images::ImageSlot::*;
    let inputs = multi_language_inputs();
    let pick = |slot| pick_image_slot(&JA, &inputs, slot).map(|i| i.index);
    // ストア アイコン（実画面の見出しは「ショップ アイコン」。全言語向け・ローカライズ版の見出しは持たない）
    assert_eq!(pick(Icon).unwrap(), 0);
    assert_eq!(pick(SmallTile).unwrap(), 3);
    assert_eq!(pick(Marquee).unwrap(), 4);
    // 同じ寸法のスクリーンショットは、見出しの行（全言語向けアセット / ローカライズ版アセット）で分ける
    assert_eq!(pick(GlobalScreenshots).unwrap(), 2);
    assert_eq!(pick(LocalizedScreenshots).unwrap(), 1);
}

#[test]
fn 言語が1つのアイテムにはローカライズ版の置き場が無いので止まる() {
    use edictora::images::ImageSlot::*;
    // SearchClock: アイコン・全言語向け・小タイル・マーキーの 4 つ
    let inputs = vec![
        file_input(0, ICON, &[]),
        file_input(1, GLOBAL, &[]),
        file_input(2, TILE, &[]),
        file_input(3, MARQUEE, &[]),
    ];
    assert!(matches!(
        pick_image_slot(&JA, &inputs, LocalizedScreenshots),
        Err(Error::Dashboard(_))
    ));
    assert_eq!(
        pick_image_slot(&JA, &inputs, GlobalScreenshots)
            .unwrap()
            .index,
        1
    );
}

#[test]
fn 置き場が1つに絞れなければ止める() {
    use edictora::images::ImageSlot::*;
    // 同じ置き場が 2 つ
    let mut inputs = multi_language_inputs();
    inputs.push(file_input(5, TILE, &[]));
    assert!(matches!(
        pick_image_slot(&JA, &inputs, SmallTile),
        Err(Error::Dashboard(m)) if m.contains("2")
    ));
    // 見出しが無いスクリーンショットは候補にしない（全言語向けかローカライズ版か分からない）
    let headless = vec![file_input(
        0,
        "スクリーンショット\n1280x800 または 640x400",
        &[],
    )];
    assert!(pick_image_slot(&JA, &headless, GlobalScreenshots).is_err());
    assert!(pick_image_slot(&JA, &headless, LocalizedScreenshots).is_err());
    // PNG を受け付けない入力・無効な入力は候補にしない
    let mut jpeg_only = multi_language_inputs();
    jpeg_only[3].accept = ".jpg".to_string();
    assert!(pick_image_slot(&JA, &jpeg_only, SmallTile).is_err());
    let mut disabled = multi_language_inputs();
    disabled[4].disabled = true;
    assert!(pick_image_slot(&JA, &disabled, Marquee).is_err());
    // 寸法の表記は × と x の両方を受ける
    let x = vec![file_input(
        0,
        "プロモーション タイル（小）\n440x280 キャンバス",
        &[],
    )];
    assert!(pick_image_slot(&JA, &x, SmallTile).is_ok());
}

#[test]
fn 画像のaltの並びを確かめる() {
    use edictora::dashboard::PageImage;
    use edictora::images::ImageSlot::*;
    let img = |alt: &str| PageImage {
        alt: alt.to_string(),
        src: String::new(),
    };
    assert_eq!(
        expected_image_alt(&JA, GlobalScreenshots, 2),
        "スクリーンショット 3"
    );
    assert_eq!(expected_image_alt(&JA, Icon, 0), "ショップ アイコン");
    assert_eq!(
        expected_image_alt(&JA, SmallTile, 0),
        "プロモーション タイル（小）"
    );
    assert_eq!(
        expected_image_alt(&JA, Marquee, 0),
        "マーキー プロモーション タイル"
    );

    let ok = [img("スクリーンショット 1"), img("スクリーンショット 2")];
    assert!(ensure_image_alts(&JA, LocalizedScreenshots, &ok).is_ok());
    let swapped = [img("スクリーンショット 2"), img("スクリーンショット 1")];
    assert!(ensure_image_alts(&JA, GlobalScreenshots, &swapped).is_err());
    assert!(ensure_image_alts(&JA, Marquee, &[img("スクリーンショット 1")]).is_err());
    assert!(ensure_image_alts(&JA, SmallTile, &[]).is_ok());
}

fn button(index: i32, label: &str) -> ElementInfo {
    ElementInfo {
        index,
        tag: "div".to_string(),
        label: label.to_string(),
        ..ElementInfo::default()
    }
}

#[test]
fn 削除ボタンはaria_labelの完全一致だけを選ぶ() {
    let buttons = vec![
        button(0, "画像を削除 スクリーンショット 1"),
        button(1, "画像を左に移動して並べ替え: スクリーンショット 1"),
        button(2, "画像を右に移動して並べ替え: スクリーンショット 1"),
        button(3, "画像を削除 スクリーンショット 2"),
        button(4, "画像アップロードのステータスを閉じる"),
    ];
    assert_eq!(
        pick_delete_button(&JA, &buttons, "スクリーンショット 2")
            .unwrap()
            .index,
        3
    );
    // 無い alt・部分一致（「スクリーンショット 1」は「…1」だけに当たる）
    assert!(pick_delete_button(&JA, &buttons, "スクリーンショット 9").is_err());
    assert_eq!(
        pick_delete_button(&JA, &buttons, "スクリーンショット 1")
            .unwrap()
            .index,
        0
    );
    // 同じラベルが 2 つなら止める（スロットの外のボタンが混ざった場合）
    let mut dup = buttons.clone();
    dup.push(button(5, "画像を削除 スクリーンショット 2"));
    assert!(pick_delete_button(&JA, &dup, "スクリーンショット 2").is_err());
}

#[test]
fn アイコンの削除ボタンも画面のaltの完全一致で選べる() {
    // 実画面（2026-11-16 Clean Reload）: アイコン欄のボタンは「画像を削除 ショップ アイコン」とステータスを閉じるだけ
    let buttons = vec![
        button(0, "画像を削除 ショップ アイコン"),
        button(1, "画像アップロードのステータスを閉じる"),
    ];
    let alt = expected_image_alt(&JA, edictora::images::ImageSlot::Icon, 0);
    assert_eq!(pick_delete_button(&JA, &buttons, &alt).unwrap().index, 0);
    assert!(ensure_image_click_allowed(&JA, &buttons[0].label).is_ok());
}

#[test]
fn 画像まわりで押してよいのは削除ボタンだけ() {
    assert!(ensure_image_click_allowed(&JA, "画像を削除 スクリーンショット 1").is_ok());
    for label in [
        "画像を左に移動して並べ替え: スクリーンショット 1",
        "画像を右に移動して並べ替え: スクリーンショット 1",
        "画像アップロードのステータスを閉じる",
        "ショップ アイコンを削除",
        "",
    ] {
        assert!(ensure_image_click_allowed(&JA, label).is_err(), "{label}");
    }
}

#[test]
fn アップロードのエラー表示を見分ける() {
    let messages = [
        "ガイドやヒントが必要な場合は…（常設の案内。除外される）エラー",
        "画像をアップロードしました",
        "画像のアップロードに失敗しました",
    ];
    let errors = image_error_messages(&JA, &messages);
    assert_eq!(errors, vec!["画像のアップロードに失敗しました".to_string()]);
    assert!(image_error_messages(&JA, &["画像をアップロードしました"]).is_empty());
}

// ───────── カテゴリ（2026-11-19 の実画面の形）─────────

fn category_text() -> &'static edictora::dashboard::text::CategoryText {
    JA.category.as_ref().expect("日本語はカテゴリの文言を持つ")
}

/// 一覧の選択肢 1 つ（`label` は所属する一覧のラベル、`width`/`height` は一覧の枠）
fn list_option(
    index: i32,
    group: i32,
    list_label: &str,
    data_value: &str,
    selected: Option<bool>,
    open: bool,
) -> ElementInfo {
    ElementInfo {
        index,
        tag: "li".to_string(),
        label: list_label.to_string(),
        data_value: data_value.to_string(),
        selected,
        group,
        width: if open { 240 } else { 0 },
        height: if open { 400 } else { 0 },
        ..ElementInfo::default()
    }
}

/// 実画面の構成: 言語の一覧（group 0）と、カテゴリの一覧（group 1。`selected` が選択中）
fn category_page(selected: &str, open: bool) -> Vec<ElementInfo> {
    let mut options = vec![
        list_option(0, 0, "言語", "ja", Some(true), false),
        list_option(1, 0, "言語", "en", Some(false), false),
    ];
    let values = std::iter::once("Select a category").chain(
        edictora::listing::category::CATEGORIES
            .iter()
            .map(|(_, v)| *v),
    );
    for (index, value) in (2..).zip(values) {
        options.push(list_option(
            index,
            1,
            "カテゴリ",
            value,
            Some(value == selected),
            open,
        ));
    }
    options
}

#[test]
fn カテゴリの現在値は閉じたまま選択中のdata_valueで読む() {
    let c = category_text();
    let options = category_page("CATEGORY_FUNCTIONALITY_AND_UI", false);
    assert_eq!(
        selected_category(c, &options).unwrap(),
        "CATEGORY_FUNCTIONALITY_AND_UI"
    );
    // 言語の一覧の選択（ja）は混ざらない
    assert_eq!(category_options(c, &options).unwrap().len(), 22);
}

#[test]
fn カテゴリが未選択ならプレースホルダーとして読む() {
    let c = category_text();
    let options = category_page("Select a category", false);
    assert_eq!(selected_category(c, &options).unwrap(), "Select a category");
}

#[test]
fn カテゴリの選択中がちょうど1つでなければ止める() {
    let c = category_text();
    // 0 個
    let mut none = category_page("CATEGORY_TOOLS", false);
    for o in none.iter_mut().filter(|o| o.group == 1) {
        o.selected = Some(false);
    }
    assert!(matches!(
        selected_category(c, &none),
        Err(Error::Dashboard(_))
    ));
    // 2 個
    let mut two = category_page("CATEGORY_TOOLS", false);
    for o in two.iter_mut().filter(|o| o.data_value == "CATEGORY_GAMES") {
        o.selected = Some(true);
    }
    assert!(matches!(
        selected_category(c, &two),
        Err(Error::Dashboard(_))
    ));
    // aria-selected が無い選択肢だけ
    let mut unknown = category_page("CATEGORY_TOOLS", false);
    for o in unknown.iter_mut() {
        o.selected = None;
    }
    assert!(selected_category(c, &unknown).is_err());
}

#[test]
fn カテゴリの一覧が見つからない_または複数なら止める() {
    let c = category_text();
    // 一覧が無い（言語の一覧だけ）
    let only_language: Vec<ElementInfo> = category_page("CATEGORY_TOOLS", false)
        .into_iter()
        .filter(|o| o.group == 0)
        .collect();
    assert!(matches!(
        category_options(c, &only_language),
        Err(Error::Dashboard(_))
    ));
    // 同名の一覧が 2 つ（group が違う）。選択肢を混ぜずに止める
    let mut doubled = category_page("CATEGORY_TOOLS", false);
    doubled.push(list_option(
        99,
        2,
        "カテゴリ",
        "CATEGORY_TOOLS",
        Some(true),
        false,
    ));
    let e = category_options(c, &doubled).unwrap_err();
    assert!(matches!(e, Error::Dashboard(_)), "{e:?}");
    assert!(selected_category(c, &doubled).is_err());
    // ラベルが部分一致するだけの一覧は選ばない
    let mut other = vec![list_option(
        0,
        5,
        "カテゴリの選択",
        "CATEGORY_TOOLS",
        Some(true),
        false,
    )];
    assert!(category_options(c, &other).is_err());
    other[0].label = "すべての言語用\nカテゴリ".to_string();
    assert!(
        category_options(c, &other).is_ok(),
        "ラベルの 1 行が完全一致すれば選ぶ"
    );
}

#[test]
fn カテゴリの一覧が開いたかは枠の大きさで判定する() {
    let c = category_text();
    assert!(!category_list_visible(
        c,
        &category_page("CATEGORY_TOOLS", false)
    ));
    assert!(category_list_visible(
        c,
        &category_page("CATEGORY_TOOLS", true)
    ));
    // 一覧が見つからなければ開いていない扱い
    assert!(!category_list_visible(c, &[]));
}

#[test]
fn カテゴリの選択肢はdata_valueが一致する1つを選ぶ() {
    let c = category_text();
    let options = category_page("CATEGORY_FUNCTIONALITY_AND_UI", true);
    let found = pick_category_option(c, &options, "CATEGORY_PRODUCTIVITY").unwrap();
    assert_eq!(found.data_value, "CATEGORY_PRODUCTIVITY");
    assert_eq!(found.group, 1);
    // 言語の一覧の値と衝突しない（別の一覧は対象外）
    assert!(pick_category_option(c, &options, "ja").is_err());
    // 表に無い値
    assert!(pick_category_option(c, &options, "CATEGORY_NOPE").is_err());
}

#[test]
fn カテゴリのプレースホルダーは選ばない() {
    let c = category_text();
    let options = category_page("CATEGORY_TOOLS", true);
    assert!(pick_category_option(c, &options, "Select a category").is_err());
    assert!(pick_category_option(c, &options, "").is_err());
}

#[test]
fn カテゴリの選択肢が重複していれば止める() {
    let c = category_text();
    let mut options = category_page("CATEGORY_TOOLS", true);
    let dup = ElementInfo {
        index: 100,
        ..options
            .iter()
            .find(|o| o.data_value == "CATEGORY_GAMES")
            .unwrap()
            .clone()
    };
    options.push(dup);
    assert!(pick_category_option(c, &options, "CATEGORY_GAMES").is_err());
}

#[test]
fn カテゴリのコンボボックスはラベルの1行の完全一致で選ぶ() {
    let c = category_text();
    // 実画面: カテゴリ欄の innerText は「カテゴリ\nカテゴリ\nカテゴリ\n機能と UI」、祖先に「すべての言語用」
    let category = ElementInfo {
        index: 3,
        tag: "div".to_string(),
        label: "カテゴリ\nカテゴリ\nすべての言語用".to_string(),
        text: "カテゴリ\nカテゴリ\nカテゴリ\n機能と UI".to_string(),
        expanded: Some("false".to_string()),
        ..ElementInfo::default()
    };
    let language = ElementInfo {
        index: 1,
        tag: "div".to_string(),
        label: "言語".to_string(),
        text: "言語\n日本語 – ja".to_string(),
        ..ElementInfo::default()
    };
    let publisher = ElementInfo {
        index: 0,
        tag: "div".to_string(),
        text: "パブリッシャー".to_string(),
        ..ElementInfo::default()
    };
    let all = vec![publisher.clone(), language.clone(), category.clone()];
    assert_eq!(pick_category_combobox(c, &all).unwrap().index, 3);
    // 言語欄の選択は従来どおり（カテゴリ欄の「すべての言語用」は「言語」の 1 行に当たらない）
    assert!(pick_language_combobox(&JA, &all).is_ok_and(|l| l.index == 1));
    // 見つからない・複数
    assert!(pick_category_combobox(c, &[publisher, language]).is_err());
    let twin = ElementInfo {
        index: 9,
        ..category.clone()
    };
    assert!(pick_category_combobox(c, &[category, twin]).is_err());
}

#[test]
fn 英語の画面はカテゴリとプライバシーの文言を持たず書き込みを許さない() {
    assert!(JA.category.is_some());
    assert!(EN.category.is_none());
    assert!(EN.privacy.is_none());
}

// ───────── プライバシー ポリシー URL 欄 ─────────

fn url_input(index: i32, label: &str) -> ElementInfo {
    ElementInfo {
        index,
        tag: "input".to_string(),
        r#type: "text".to_string(),
        label: label.to_string(),
        ..ElementInfo::default()
    }
}

#[test]
fn プライバシーポリシーurl欄はラベルの1行の完全一致で選ぶ() {
    let p = JA.privacy.as_ref().unwrap();
    // 実画面: aria-labelledby の参照先が「プライバシー ポリシーの URL」、祖先に「39/2,048」のカウンタ
    let target = url_input(
        2,
        "プライバシー ポリシーの URL\nプライバシー ポリシーの URL\n39/2,048",
    );
    let other = url_input(1, "公式 URL");
    let inputs = vec![other.clone(), target.clone()];
    assert_eq!(find_policy_url_input(p, &inputs).unwrap().unwrap().index, 2);
    // 空白の有無は区別しない
    let tight = url_input(3, "プライバシーポリシーのURL");
    assert_eq!(
        find_policy_url_input(p, &[tight]).unwrap().unwrap().index,
        3
    );
    // 無ければ None（欄が無い扱い）
    assert!(find_policy_url_input(p, &[other]).unwrap().is_none());
    // 説明文に含まれるだけの input は選ばない
    let mention = url_input(4, "プライバシー ポリシーの URL を入力してください");
    assert!(find_policy_url_input(p, &[mention]).unwrap().is_none());
    // 複数なら止める
    let twin = url_input(5, "プライバシー ポリシーの URL");
    assert!(find_policy_url_input(p, &[target, twin]).is_err());
}

#[test]
fn プライバシーポリシーurl欄の書き込み先の検査はinputだけを通す() {
    let ok = url_input(1, "x");
    assert!(ensure_privacy_url_writable(&ok, "プライバシー ポリシー URL").is_ok());
    // textarea は通さない（textarea 用の検査を広げない）
    let textarea = ElementInfo {
        tag: "textarea".to_string(),
        ..ElementInfo::default()
    };
    assert!(matches!(
        ensure_privacy_url_writable(&textarea, "x"),
        Err(Error::Dashboard(_))
    ));
    assert!(ensure_privacy_text_writable(&ok, "x").is_err());
    // 編集できない欄は WriteBlocked
    let locked = ElementInfo {
        read_only: true,
        ..url_input(1, "x")
    };
    assert!(matches!(
        ensure_privacy_url_writable(&locked, "x"),
        Err(Error::WriteBlocked(_))
    ));
    let disabled = ElementInfo {
        disabled: true,
        ..url_input(1, "x")
    };
    assert!(matches!(
        ensure_privacy_url_writable(&disabled, "x"),
        Err(Error::WriteBlocked(_))
    ));
}

// ───────── カテゴリの読み取りが落ち着いているか ─────────

fn category_combobox(shown: &str) -> ElementInfo {
    ElementInfo {
        index: 3,
        tag: "div".to_string(),
        label: "カテゴリ".to_string(),
        text: format!("カテゴリ\nカテゴリ\nカテゴリ\n{shown}"),
        ..ElementInfo::default()
    }
}

/// 選択中の option の表示文言を設定したカテゴリの一覧
fn category_page_with_text(selected: &str, shown: &str) -> Vec<ElementInfo> {
    let mut options = category_page(selected, false);
    for o in options
        .iter_mut()
        .filter(|o| o.selected == Some(true) && o.group == 1)
    {
        o.text = shown.to_string();
    }
    options
}

#[test]
fn カテゴリの読み取りは表示値と選択中の文言が一致すれば落ち着いている() {
    let c = category_text();
    let options = category_page_with_text("CATEGORY_TOOLS", "ツール");
    assert!(category_reading_is_stable(
        c,
        Some(&category_combobox("ツール")),
        &options
    ));
    // 前後の空白は無視する
    assert!(category_reading_is_stable(
        c,
        Some(&category_combobox(" ツール ")),
        &options
    ));
    // コンボボックスが見つからないときは表示値の照合を省く
    assert!(category_reading_is_stable(c, None, &options));
}

#[test]
fn カテゴリの読み取りはプレースホルダーが選択中なら落ち着いていない() {
    let c = category_text();
    let options = category_page_with_text("Select a category", "カテゴリを選択してください");
    assert!(!category_reading_is_stable(
        c,
        Some(&category_combobox("カテゴリを選択してください")),
        &options
    ));
    assert!(!category_reading_is_stable(c, None, &options));
}

#[test]
fn カテゴリの読み取りは表示値と選択中の文言が食い違えば落ち着いていない() {
    let c = category_text();
    let options = category_page_with_text("CATEGORY_TOOLS", "ツール");
    assert!(!category_reading_is_stable(
        c,
        Some(&category_combobox("機能と UI")),
        &options
    ));
}

#[test]
fn カテゴリの読み取りは選択中がちょうど1つでなければ落ち着いていない() {
    let c = category_text();
    let mut none = category_page_with_text("CATEGORY_TOOLS", "ツール");
    for o in none.iter_mut() {
        o.selected = Some(false);
    }
    assert!(!category_reading_is_stable(c, None, &none));
    assert!(!category_reading_is_stable(c, None, &[]));
}
