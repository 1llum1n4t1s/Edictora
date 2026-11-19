//! ダッシュボードの画面上の文言（画面の表示言語ごとの文言セット。C# の `DashboardText`）。
//!
//! 要素はクラス名ではなく、ラベル・役割・ボタンの文言で探す。クラス名は予告なく変わるため。
//! その代わり、ダッシュボードの表示言語ごとに文言が要る。日本語（[`JA`]、2026-11-13 に実画面で確認）と
//! 英語（[`EN`]、実画面で未確認の暫定値）の 2 セットを持つ。文言が変わったら、ここだけを直せばよいように 1 か所へ集めている。
//!
//! 画面の言語はページごとに 1 回だけ判定し（`selectors::dashboard_text_for_lang`）、以降はその 1 セットだけを使う。
//! 要素ごとに「日本語か英語のどちらかに一致」で探すことはしない（候補が倍になり、取り違えの危険が増えるため）。
//!
//! これは画面の文言で、利用者に見せる出力の言語（`i18n`）とは別物。日本語の画面を英語の出力で使うこともある。

use crate::listing::ListingField;

/// ダッシュボードの表示言語（`<html lang>` から判定する）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardLang {
    /// 日本語表示（実画面で確認済み）
    Ja,
    /// 英語表示（文言は実画面で未確認の暫定値）
    En,
}

/// 画面の表示言語 1 つ分の文言セット
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardText {
    /// この文言セットの言語
    pub lang: DashboardLang,

    /// 言語切替コンボボックスのラベル
    pub language_label: &'static str,

    /// 言語が 1 つだけのアイテムで、言語の選択欄に出る言語名とロケールコードの対応。
    ///
    /// 2 つ以上の言語があるアイテムでは選択欄が「日本語 – ja」の形（コード付き）で、開くと登録済みの
    /// 言語だけが並ぶ。1 つだけのアイテムでは選択欄が「日本語」だけ（コードなし）になり、開くと全言語の
    /// 一覧（既定の言語を選ぶもの）が出る（2026-11-14 に実画面で確認）。後者は開かずに、表示中の言語名
    /// からコードを決める。ここに無い言語名は解釈できないとして止める（推測しない）。
    pub single_locale_names: &'static [(&'static str, &'static str)],

    /// 下書き保存ボタン（前後の空白を除いた完全一致で探す）
    pub save_draft_button: &'static str,

    /// 審査送信ボタン（本体と確認ダイアログ内で同じ文言）
    pub submit_button: &'static str,

    /// 確認ダイアログのキャンセルボタン
    pub cancel_button: &'static str,

    /// 保存成功メッセージの一部（日本語は「アイテムが保存されました。」の一部）
    pub saved_message: &'static str,

    /// 送信成功メッセージの一部（日本語は「アイテムを送信しました。」の一部）
    pub submitted_message: &'static str,

    /// 成功メッセージの判定で、これを含むメッセージは成功と見なさない語（否定・失敗を表す語）。
    ///
    /// 英語の成功の語（"saved" など）は "could not be saved"・"unsaved" のような失敗の文にも含まれるため、
    /// 英語だけで使う。日本語の成功の語（「保存されました」）は肯定の過去形で失敗の文に含まれにくく、
    /// 実画面で確認した判定を変えないよう空にしている。照合は大文字小文字を区別しない部分一致
    /// （"Note" の "not" にも当たるが、それは「成功を確かめられない → 止まる」側なので許容する）。
    pub failure_markers: &'static [&'static str],

    /// 成功メッセージの語を、単語単位・大文字小文字を区別せずに照合するか。
    ///
    /// 英語は true（"unsaved" の中の "saved" に当たらないように）。日本語は単語の区切りが無いので false（従来どおりの部分一致）。
    pub match_whole_word: bool,

    /// 常設の案内通知を見分ける語（結果メッセージ・ダイアログの判定から除外する）。None なら除外しない。
    ///
    /// 空文字にすると「すべてを含む」扱いになり、すべてのメッセージ・ダイアログが除外されてしまうので、無いときは None にする。
    pub guide_notice_marker: Option<&'static str>,

    /// 確認ダイアログの自動公開チェックボックスのラベルの一部（空白・大文字小文字を無視した部分一致）
    pub auto_publish_marker: &'static str,

    /// ステータス行のラベル（「ステータス: 審査待ち」の「ステータス」）。
    ///
    /// 区切りのコロンは含めない。実画面の区切りが半角「:」か全角「：」かに依らず読めるよう、
    /// 区切りは `selectors::parse_status` の正規表現側で両方を受け付ける。
    pub status_label: &'static str,

    /// ステータス行のラベルを行頭に限るか。
    ///
    /// 英語の "Status" は本文中の別の文（"... status: ..."）にも出やすいので、行頭（前の空白は可）に限る。
    /// 日本語は実画面で確認した挙動を変えないよう false（行のどこにあってもよい）。
    pub status_at_line_start: bool,

    /// 既定の言語を示す表記（選択肢「英語 – en （デフォルト）」の括弧の中身）。大文字小文字を区別しない
    pub default_locale_marker: &'static str,

    /// 公式 URL 欄のラベル
    pub official_url_label: &'static str,
    /// ホームページ URL 欄のラベル
    pub homepage_url_label: &'static str,
    /// サポート URL 欄のラベル
    pub support_url_label: &'static str,

    // ───── 画像アセット（掲載画像）─────
    // 日本語は 2026-11-15 に実画面（SearchClock・Vuora）で確認した。英語は実画面で未確認の暫定値
    // （英語の画面では画像の書き込みを許さず、読み出しも要素が絞れなければ止まる）。
    /// 全言語向けアセットの見出し（画像の置き場を、見出しの行で見分ける）
    pub global_assets_heading: &'static str,
    /// ローカライズ版アセットの見出し
    pub localized_assets_heading: &'static str,
    /// スクリーンショットの alt の接頭辞（alt は「スクリーンショット 1」の形。番号は 1 始まり）
    pub screenshot_alt_prefix: &'static str,
    /// ストア アイコンの alt。画面ではアイコン欄の見出し・画像の alt とも「ショップ アイコン」
    /// （2026-11-16 に Clean Reload の実画面で確認。削除ボタンの aria-label は「画像を削除 ショップ アイコン」）。
    /// 英語は実画面で未確認の暫定値
    pub icon_alt: &'static str,
    /// 小タイルの alt
    pub small_tile_alt: &'static str,
    /// マーキーの alt
    pub marquee_alt: &'static str,
    /// 画像の削除ボタンの aria-label の接頭辞（`画像を削除 <alt>`。この形以外の画像ボタンは押さない）
    pub delete_image_prefix: &'static str,
    /// 画像の削除を押した後に出る確認ダイアログの文面の一部（「この画像を削除してもよろしいですか？…」）。
    /// これを含むダイアログだけを確認として扱い、別のダイアログなら押さずに止める
    pub delete_image_confirm_marker: &'static str,
    /// その確認ダイアログの確定ボタン（前後の空白を除いた完全一致で探す）
    pub delete_image_confirm_button: &'static str,

    /// カテゴリ欄の文言。None の言語では、カテゴリの読み書きを許さない（文言を推測で足さない）
    pub category: Option<CategoryText>,

    /// 「プライバシー」タブの文言。None の言語では、このタブの読み書きを許さない（文言を推測で足さない）
    pub privacy: Option<PrivacyText>,
}

/// 「ストアの掲載情報」タブのカテゴリ欄の文言。
///
/// 日本語は 2026-11-19 に実画面（SearchClock）で読み取った。コンボボックス（`div[role=combobox]`）は
/// aria-labelledby の参照先が「カテゴリ」で、一覧は `ul[role=listbox][aria-label="カテゴリ"]`。
/// 選択肢の `data-value` は言語に依らない識別子なので、文言は欄のラベルだけで足りる（対応表は `listing::category`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryText {
    /// カテゴリ欄のラベル（コンボボックスと一覧の両方を、ラベルの 1 行の完全一致で探す）
    pub label: &'static str,
}

/// 「プライバシー」タブ（編集画面の `/privacy`）の文言。
///
/// 日本語は 2026-11-18 に実画面（15 アイテム）で確認した。タブの名前は「プライバシー」（URL は `<編集画面>/privacy`）。
/// 単一用途の説明・権限の正当性は textarea（最大 1,000 文字）で、見出しが aria-labelledby でラベルになる。
/// データ使用の 9 種類と 3 つの宣誓は `input[type=checkbox]`（aria-label が項目の文言そのもの）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyText {
    /// 単一用途の説明の欄のラベル
    pub single_purpose_label: &'static str,
    /// 権限の正当性の欄のラベルの後半（「<権限名> が必要な理由」の「が必要な理由」。空白は照合で無視する）
    pub permission_label_suffix: &'static str,
    /// ホスト権限の欄の権限名（設定では特別なキー `host`）
    pub host_permission_name: &'static str,
    /// データの種類 9 つのチェックボックスのラベル（`DataKind::ALL` と同じ並び）
    pub data_kind_labels: [&'static str; 9],
    /// 3 つの宣誓チェックのラベル（画面の並び）
    pub certify_labels: [&'static str; 3],
    /// プライバシー ポリシー URL 欄のラベル（`input[type=text]`。aria-labelledby の参照先の文字列）
    pub policy_url_label: &'static str,
}

impl DashboardText {
    /// URL 欄のラベル（説明欄はラベルではなく textarea で特定するので None）
    pub fn field_label(&self, field: ListingField) -> Option<&'static str> {
        match field {
            ListingField::OfficialUrl => Some(self.official_url_label),
            ListingField::HomepageUrl => Some(self.homepage_url_label),
            ListingField::SupportUrl => Some(self.support_url_label),
            ListingField::Description => None,
        }
    }
}

/// 日本語表示のダッシュボードの文言（2026-11-13〜15 に実画面で確認したもの）
pub const JA: DashboardText = DashboardText {
    lang: DashboardLang::Ja,
    language_label: "言語",
    single_locale_names: &[("日本語", "ja"), ("英語", "en")],
    save_draft_button: "下書きとして保存する",
    submit_button: "審査のため送信",
    cancel_button: "キャンセル",
    saved_message: "保存されました",
    submitted_message: "アイテムを送信しました",
    failure_markers: &[],
    match_whole_word: false,
    guide_notice_marker: Some("ガイドやヒント"),
    auto_publish_marker: "自動的に公開する",
    status_label: "ステータス",
    status_at_line_start: false,
    default_locale_marker: "デフォルト",
    official_url_label: "公式 URL",
    homepage_url_label: "ホームページ URL",
    support_url_label: "サポート URL",
    global_assets_heading: "全言語向けアセット",
    localized_assets_heading: "ローカライズ版アセット",
    screenshot_alt_prefix: "スクリーンショット",
    icon_alt: "ショップ アイコン",
    small_tile_alt: "プロモーション タイル（小）",
    marquee_alt: "マーキー プロモーション タイル",
    delete_image_prefix: "画像を削除 ",
    delete_image_confirm_marker: "この画像を削除してもよろしいですか",
    delete_image_confirm_button: "削除",
    category: Some(CategoryText {
        label: "カテゴリ"
    }),
    privacy: Some(PrivacyText {
        single_purpose_label: "単一用途の説明",
        permission_label_suffix: "が必要な理由",
        host_permission_name: "ホスト権限",
        data_kind_labels: [
            "個人を特定できる情報",
            "健康に関する情報",
            "財務状況や支払いに関する情報",
            "認証に関する情報",
            "個人的コミュニケーション",
            "位置情報",
            "ウェブ履歴",
            "ユーザーのアクティビティ",
            "ウェブサイトのコンテンツ",
        ],
        certify_labels: [
            "私は、承認されている以外の用途で第三者にユーザーデータを販売、転送しません",
            "私はアイテムの唯一の目的と関係のない目的でユーザーデータを使用または転送しません",
            "私は信用力を判断する目的または融資目的でユーザーデータを使用または転送しません",
        ],
        policy_url_label: "プライバシー ポリシーの URL",
    }),
};

/// 英語表示のダッシュボードの文言。
///
/// **2026-11-15 時点で実画面未確認の暫定値。実画面で確認したら更新する。**
/// （取得を試みたが、Chrome が拡張機能からのダッシュボードへのアクセスを拒否したため確かめられていない）
///
/// 暫定値が外れても「見つからない → 止まる」になるよう、ボタン・ラベルは完全一致（行単位）で探す。
/// 成功メッセージの語は短いので、単語単位で照合し、否定・失敗の語を含むメッセージは成功と見なさない。
/// 案内通知に当たる英語は推測できないので除外しない（None）。除外しないと、
/// - 案内通知がダイアログ扱いなら、審査送信は「別のダイアログが開いている」で止まる（安全側）
/// - 案内通知が成功の語を含んでいたら、保存の前の「前回のメッセージが消えるまで待つ」・送信の前の確認で止まる（安全側）
pub const EN: DashboardText = DashboardText {
    lang: DashboardLang::En,
    language_label: "Language",
    single_locale_names: &[("Japanese", "ja"), ("English", "en")],
    save_draft_button: "Save draft",
    submit_button: "Submit for review",
    cancel_button: "Cancel",
    saved_message: "saved",
    submitted_message: "submitted",
    failure_markers: &["not", "n't", "unable", "fail", "error", "unsaved"],
    match_whole_word: true,
    guide_notice_marker: None,
    auto_publish_marker: "automatically",
    status_label: "Status",
    status_at_line_start: true,
    default_locale_marker: "Default",
    official_url_label: "Official URL",
    homepage_url_label: "Homepage URL",
    support_url_label: "Support URL",
    global_assets_heading: "Global assets",
    localized_assets_heading: "Localized assets",
    screenshot_alt_prefix: "Screenshot",
    icon_alt: "Store icon",
    small_tile_alt: "Small promo tile",
    marquee_alt: "Marquee promo tile",
    delete_image_prefix: "Delete image ",
    delete_image_confirm_marker: "delete this image",
    delete_image_confirm_button: "Delete",
    // 英語の画面のカテゴリ欄・「プライバシー」タブの文言は実画面で未確認なので、日本語の画面だけで扱う
    category: None,
    privacy: None,
};

impl DashboardLang {
    /// この言語の文言セット
    pub fn text(self) -> &'static DashboardText {
        match self {
            DashboardLang::Ja => &JA,
            DashboardLang::En => &EN,
        }
    }
}
