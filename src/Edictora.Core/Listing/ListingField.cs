namespace Edictora.Core.Listing;

/// <summary>
/// Edictora が読み書きする掲載情報の欄。
/// </summary>
/// <remarks>
/// ダッシュボードの「ストアの掲載情報」で編集できる欄のうち、テキストで正本を持てるものだけを扱う。
/// カテゴリ・画像・プロモーション動画は対象外。
/// 設定ファイルに書かれていない欄は、読み出し・比較・書き換えのどれにも使わない。
/// </remarks>
public enum ListingField
{
    /// <summary>説明（textarea、最大 16,000 文字）</summary>
    Description,

    /// <summary>公式 URL</summary>
    OfficialUrl,

    /// <summary>ホームページ URL</summary>
    HomepageUrl,

    /// <summary>サポート URL</summary>
    SupportUrl,
}

/// <summary><see cref="ListingField"/> の表示名など</summary>
public static class ListingFieldInfo
{
    /// <summary>画面・CLI に出す日本語名</summary>
    public static string DisplayName(this ListingField field) => field switch
    {
        ListingField.Description => "説明",
        ListingField.OfficialUrl => "公式 URL",
        ListingField.HomepageUrl => "ホームページ URL",
        ListingField.SupportUrl => "サポート URL",
        _ => field.ToString(),
    };

    /// <summary>設定ファイル上のキー名</summary>
    public static string ConfigKey(this ListingField field) => field switch
    {
        ListingField.Description => "description",
        ListingField.OfficialUrl => "officialUrl",
        ListingField.HomepageUrl => "homepageUrl",
        ListingField.SupportUrl => "supportUrl",
        _ => field.ToString(),
    };

    /// <summary>URL 欄かどうか</summary>
    public static bool IsUrl(this ListingField field) => field != ListingField.Description;

    /// <summary>処理・表示の順番（説明 → 公式 → ホームページ → サポート）</summary>
    public static IReadOnlyList<ListingField> All { get; } =
        [ListingField.Description, ListingField.OfficialUrl, ListingField.HomepageUrl, ListingField.SupportUrl];
}
