//! ダッシュボードの画面上の文言（2026-11-13 に日本語表示の実画面で確認したもの。C# の `DashboardText`）。
//!
//! 要素はクラス名ではなく、ラベル・役割・ボタンの文言で探す。クラス名は予告なく変わるため。
//! その代わり、ダッシュボードが日本語で表示されていることが前提になる（英語表示は未対応）。
//! 文言が変わったら、ここだけを直せばよいように 1 か所へ集めている。

use crate::listing::ListingField;

/// 言語切替コンボボックスのラベル
pub const LANGUAGE_LABEL: &str = "言語";

/// 下書き保存ボタン
pub const SAVE_DRAFT_BUTTON: &str = "下書きとして保存する";

/// 審査送信ボタン（本体と確認ダイアログ内で同じ文言）
pub const SUBMIT_BUTTON: &str = "審査のため送信";

/// 確認ダイアログのキャンセルボタン
pub const CANCEL_BUTTON: &str = "キャンセル";

/// 保存成功メッセージ（「アイテムが保存されました。」の一部）
pub const SAVED_MESSAGE: &str = "保存されました";

/// 送信成功メッセージ（「アイテムを送信しました。」の一部）
pub const SUBMITTED_MESSAGE: &str = "アイテムを送信しました";

/// 常設の案内通知（結果メッセージ・ダイアログの判定から除外する）
pub const GUIDE_NOTICE_MARKER: &str = "ガイドやヒント";

/// 確認ダイアログの自動公開チェックボックスのラベルの一部
pub const AUTO_PUBLISH_MARKER: &str = "自動的に公開する";

/// ステータス行のラベル（「ステータス: 審査待ち」の「ステータス」）。
///
/// 区切りのコロンは含めない。実画面の区切りが半角「:」か全角「：」かに依らず読めるよう、
/// 区切りは `selectors::parse_status` の正規表現側で両方を受け付ける。
pub const STATUS_LABEL: &str = "ステータス";

/// 既定の言語を示す表記（選択肢「英語 – en （デフォルト）」の括弧の中身）
pub const DEFAULT_LOCALE_MARKER: &str = "デフォルト";

/// URL 欄のラベル（説明欄はラベルではなく textarea で特定するので None）
pub fn field_label(field: ListingField) -> Option<&'static str> {
    match field {
        ListingField::OfficialUrl => Some("公式 URL"),
        ListingField::HomepageUrl => Some("ホームページ URL"),
        ListingField::SupportUrl => Some("サポート URL"),
        ListingField::Description => None,
    }
}
