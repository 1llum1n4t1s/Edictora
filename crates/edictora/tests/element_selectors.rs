//! C# の ElementSelectors.test.cs の移植（実画面の回帰 2 件を含む）

use edictora::Error;
use edictora::dashboard::selectors::*;
use edictora::dashboard::text::{SAVE_DRAFT_BUTTON, SAVED_MESSAGE, SUBMIT_BUTTON};
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
    assert_eq!(pick_language_combobox(&combos).unwrap().index, 1);
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
    assert_eq!(pick_language_combobox(&combos).unwrap().index, 0);
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
        pick_field_by_label(&inputs, ListingField::HomepageUrl)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        pick_field_by_label(&inputs, ListingField::OfficialUrl)
            .unwrap()
            .index,
        0
    );
}

#[test]
fn pick_language_combobox_無い場合と複数ある場合は例外() {
    assert!(is_dashboard(pick_language_combobox(&[
        el(0).text("Kagayoi")
    ])));
    assert!(is_dashboard(pick_language_combobox(&[
        el(0).label("言語"),
        el(1).label("言語")
    ])));
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
        dashboard_message(pick_language_combobox(&[el(0).text("Kagayoi")])),
        "「言語」のコンボボックスが見つかりません（ダッシュボードが日本語表示か確認してください）"
    );
    assert_eq!(
        dashboard_message(pick_language_combobox(&[
            el(0).label("言語"),
            el(1).label("言語"),
            el(2).text("言語"),
        ])),
        "「言語」のコンボボックスが 3 個あり、特定できません"
    );
}

#[test]
fn pick_locale_option_0件と複数件のエラー文言() {
    let options = [el(0).text("英語 – en"), el(1).text("日本語 – ja")];
    assert_eq!(
        dashboard_message(pick_locale_option(&options, "fr")),
        "言語の選択肢にロケール fr がありません（ある言語: en, ja）"
    );
    assert_eq!(
        dashboard_message(pick_locale_option(
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
            SUBMIT_BUTTON
        )),
        "「審査のため送信」ボタンが 2 個あり、特定できません"
    );
}

#[test]
fn pick_field_by_label_0件と複数件のエラー文言() {
    assert_eq!(
        dashboard_message(pick_field_by_label(
            &[el(0).label("公式 URL")],
            ListingField::SupportUrl
        )),
        "「サポート URL」の入力欄が見つかりません"
    );
    assert_eq!(
        dashboard_message(pick_field_by_label(
            &[el(0).label("サポート URL"), el(1).label("サポートURL")],
            ListingField::SupportUrl
        )),
        "「サポート URL」の入力欄が 2 個あり、特定できません"
    );
    // 説明欄はラベルで探さない（共通化の対象外の早期リターン）
    assert_eq!(
        dashboard_message(pick_field_by_label(&[], ListingField::Description)),
        "説明欄はラベルではなく textarea で特定します"
    );
}

#[test]
fn parse_locale_option_言語名とコードとデフォルト表記を読む() {
    let en = parse_locale_option(Some("英語 – en （デフォルト）"));
    let ja = parse_locale_option(Some("日本語 – ja"));
    let pt = parse_locale_option(Some("ポルトガル語（ブラジル） – pt-BR"));

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
    assert!(parse_locale_option(Some("言語")).is_none());
    assert!(parse_locale_option(Some("")).is_none());
}

#[test]
fn pick_locale_option_コードが完全一致するものだけを選ぶ() {
    let options = [
        el(0).text("英語（イギリス） – en-GB"),
        el(1).text("英語 – en （デフォルト）"),
        el(2).text("日本語 – ja"),
    ];
    assert_eq!(pick_locale_option(&options, "en").unwrap().index, 1);
    assert_eq!(pick_locale_option(&options, "en-GB").unwrap().index, 0);
    assert!(is_dashboard(pick_locale_option(&options, "fr")));
    assert_eq!(locale_codes(&options), vec!["en-GB", "en", "ja"]);
}

#[test]
fn current_locale_code_表示の最後の言語行を読む() {
    assert_eq!(
        current_locale_code(&el(0).text("言語\n英語 – en （デフォルト）")).as_deref(),
        Some("en")
    );
    assert!(current_locale_code(&el(0).text("言語")).is_none());
}

#[test]
fn single_locale_code_言語が1つだけのアイテムは言語名からコードを決める() {
    // 2026-11-14 の実画面: 言語が 1 つだけのアイテムの選択欄はコードなしの言語名だけ
    assert_eq!(
        single_locale_code(&el(0).text("言語\n言語\n言語\n日本語")).as_deref(),
        Some("ja")
    );
    assert_eq!(
        single_locale_code(&el(0).text("言語\n英語 ")).as_deref(),
        Some("en")
    );
    // コード付き（複数言語のアイテム）は対象外。切り替えの手順を使う
    assert!(single_locale_code(&el(0).text("言語\n日本語 – ja")).is_none());
    // 対応表に無い言語名は推測しない
    assert!(single_locale_code(&el(0).text("言語\nフランス語")).is_none());
    // 値が無い（ラベルだけ）も推測しない
    assert!(single_locale_code(&el(0).text("言語")).is_none());
}

#[test]
fn pick_button_文言の完全一致で1つだけ選ぶ() {
    let buttons = [
        el(0).text("公開"),
        el(1).text(" 下書きとして保存する "),
        el(2).text("審査のため送信"),
    ];
    assert_eq!(pick_button(&buttons, SAVE_DRAFT_BUTTON).unwrap().index, 1);
    assert!(is_dashboard(pick_button(&buttons, "削除")));
    assert!(is_dashboard(pick_button(
        &[el(0).text("審査のため送信"), el(1).text("審査のため送信")],
        SUBMIT_BUTTON
    )));
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
        pick_field_by_label(&inputs, ListingField::OfficialUrl)
            .unwrap()
            .index,
        0
    );
    assert_eq!(
        pick_field_by_label(&inputs, ListingField::HomepageUrl)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        pick_field_by_label(&inputs, ListingField::SupportUrl)
            .unwrap()
            .index,
        2
    );
    assert!(is_dashboard(pick_field_by_label(
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
    assert!(!has_message(&messages, SAVED_MESSAGE));
    let mut more = messages.clone();
    more.push("アイテムが保存されました。");
    assert!(has_message(&more, SAVED_MESSAGE));
}

// ───────── 編集画面の状態の導出（item_page の get_state から切り出した純粋関数）─────────

#[test]
fn item_page_state_説明欄と保存ボタンが1つずつなら見つかり操作できる() {
    let state = item_page_state(
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
        None,
        &[el(0).tag("textarea").read_only()],
        &[el(0).text(SAVE_DRAFT_BUTTON).disabled()],
    );
    assert!(readonly.description_found);
    assert!(!readonly.description_editable);
    assert!(readonly.save_button_found);
    assert!(!readonly.save_button_enabled);
    assert_eq!(readonly.status_text, None);

    let disabled = item_page_state(None, &[el(0).tag("textarea").disabled()], &[]);
    assert!(disabled.description_found);
    assert!(!disabled.description_editable);
}

#[test]
fn item_page_state_0個と複数個は見つからない扱い() {
    // 0 個（添字 0 を読まずに false になる）
    let none = item_page_state(None, &[], &[el(0).text("公開")]);
    assert!(!none.description_found);
    assert!(!none.description_editable);
    assert!(!none.save_button_found);
    assert!(!none.save_button_enabled);

    // 複数個（どれか 1 つを選ばない）
    let many = item_page_state(
        None,
        &[el(0).tag("textarea"), el(1).tag("textarea")],
        &[el(0).text(SAVE_DRAFT_BUTTON), el(1).text(SAVE_DRAFT_BUTTON)],
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
    assert_eq!(auto_publish_state(&checks), Some(true));
    // 空白の有無は無視する
    assert_eq!(
        auto_publish_state(&[check(0, "自動的に 公開する", Some(false))]),
        Some(false)
    );
}

#[test]
fn auto_publish_state_ラベルが無くてもチェックが1つならその状態() {
    assert_eq!(auto_publish_state(&[check(0, "", Some(true))]), Some(true));
}

#[test]
fn auto_publish_state_0個とラベルなし複数は不明() {
    assert_eq!(auto_publish_state(&[]), None);
    assert_eq!(
        auto_publish_state(&[check(0, "", Some(true)), check(1, "別の項目", Some(false))]),
        None
    );
}

#[test]
fn auto_publish_state_ラベル一致の状態が不明なら1個でも不明() {
    // ラベルで見つけた要素を優先し、「1 つだけ」の判断へ切り替えない
    assert_eq!(
        auto_publish_state(&[check(0, "自動的に公開する", None)]),
        None
    );
}

#[test]
fn mentions_language_label_ラベルか表示テキストに言語を含めば当たる() {
    assert!(mentions_language_label(&el(0).label("言語")));
    assert!(mentions_language_label(&el(0).text("言 語\n英語 – en")));
    // 部分一致（説明文中の「すべての言語用」にも当たる。pick_language_combobox の完全一致より緩い）
    assert!(mentions_language_label(
        &el(0).label("カテゴリ\nすべての言語用")
    ));
    assert!(pick_language_combobox(&[el(0).label("カテゴリ\nすべての言語用")]).is_err());
    assert!(!mentions_language_label(&el(0).text("Kagayoi")));
}

#[test]
fn parse_status_ステータス行の値を読む() {
    assert_eq!(
        parse_status(Some("アイテム\nステータス: 審査待ち\nパッケージ")).as_deref(),
        Some("審査待ち")
    );
    assert_eq!(
        parse_status(Some("ステータス：公開済み")).as_deref(),
        Some("公開済み")
    );
    assert!(parse_status(Some("なし")).is_none());
}
