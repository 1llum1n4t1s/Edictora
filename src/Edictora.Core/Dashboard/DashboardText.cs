using Edictora.Core.Listing;

namespace Edictora.Core.Dashboard;

/// <summary>
/// ダッシュボードの画面上の文言（2026-11-13 に日本語表示の実画面で確認したもの）。
/// </summary>
/// <remarks>
/// 要素はクラス名ではなく、ラベル・役割・ボタンの文言で探す。クラス名は予告なく変わるため。
/// その代わり、ダッシュボードが日本語で表示されていることが前提になる（英語表示は未対応）。
/// 文言が変わったら、ここだけを直せばよいように 1 か所へ集めている。
/// </remarks>
public static class DashboardText
{
    /// <summary>言語切替コンボボックスのラベル</summary>
    public const string LanguageLabel = "言語";

    /// <summary>下書き保存ボタン</summary>
    public const string SaveDraftButton = "下書きとして保存する";

    /// <summary>審査送信ボタン（本体と確認ダイアログ内で同じ文言）</summary>
    public const string SubmitButton = "審査のため送信";

    /// <summary>確認ダイアログのキャンセルボタン</summary>
    public const string CancelButton = "キャンセル";

    /// <summary>保存成功メッセージ（「アイテムが保存されました。」の一部）</summary>
    public const string SavedMessage = "保存されました";

    /// <summary>送信成功メッセージ（「アイテムを送信しました。」の一部）</summary>
    public const string SubmittedMessage = "アイテムを送信しました";

    /// <summary>常設の案内通知（結果メッセージ・ダイアログの判定から除外する）</summary>
    public const string GuideNoticeMarker = "ガイドやヒント";

    /// <summary>確認ダイアログの自動公開チェックボックスのラベルの一部</summary>
    public const string AutoPublishMarker = "自動的に公開する";

    /// <summary>ステータス行の接頭辞（「ステータス: 審査待ち」）</summary>
    public const string StatusPrefix = "ステータス:";

    /// <summary>URL 欄のラベル</summary>
    public static string FieldLabel(ListingField field) => field switch
    {
        ListingField.OfficialUrl => "公式 URL",
        ListingField.HomepageUrl => "ホームページ URL",
        ListingField.SupportUrl => "サポート URL",
        _ => throw new ArgumentOutOfRangeException(nameof(field), field, "説明欄はラベルではなく textarea で特定する"),
    };
}
