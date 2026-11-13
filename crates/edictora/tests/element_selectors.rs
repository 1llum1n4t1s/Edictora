//! C# の ElementSelectors.test.cs の移植（実画面の回帰 2 件を含む）

use edictora::Error;
use edictora::dashboard::selectors::*;
use edictora::dashboard::text::{SAVE_DRAFT_BUTTON, SAVED_MESSAGE, SUBMIT_BUTTON};
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
