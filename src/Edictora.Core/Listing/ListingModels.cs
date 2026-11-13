using Edictora.Core.Config;

namespace Edictora.Core.Listing;

/// <summary>ロケール 1 件の正本の値（ファイルから取り出し済み）</summary>
/// <param name="Code">ロケールコード</param>
/// <param name="Values">対象欄 → 正本の値（設定に書いた欄だけを持つ）</param>
public sealed record ExpectedLocale(string Code, IReadOnlyDictionary<ListingField, string> Values)
{
    /// <summary>対象欄（表示順）</summary>
    public IReadOnlyList<ListingField> Fields => [.. ListingFieldInfo.All.Where(Values.ContainsKey)];
}

/// <summary>アイテム 1 件の正本</summary>
/// <param name="ItemId">アイテム ID</param>
/// <param name="DisplayName">表示名</param>
/// <param name="Locales">ロケールごとの正本</param>
public sealed record ExpectedItem(string ItemId, string DisplayName, IReadOnlyList<ExpectedLocale> Locales)
{
    /// <summary>ロケールで絞り込む（null・空なら全ロケール）。未設定のロケールを指定したら例外。</summary>
    public ExpectedItem FilterLocales(IReadOnlyCollection<string>? codes)
    {
        if (codes is null || codes.Count == 0) return this;
        var selected = new List<ExpectedLocale>();
        foreach (var code in codes)
        {
            var normalized = ConfigLoader.NormalizeLocaleCode(code);
            var match = Locales.FirstOrDefault(l => string.Equals(l.Code, normalized, StringComparison.OrdinalIgnoreCase))
                ?? throw new ConfigException($"{DisplayName} ({ItemId}) の設定にロケール {code} がありません");
            if (!selected.Contains(match)) selected.Add(match);
        }
        return this with { Locales = selected };
    }
}

/// <summary>設定から正本の値を組み立てる</summary>
public static class ExpectedListingBuilder
{
    /// <summary>
    /// 設定のアイテムから正本を作る。説明文はここでファイルから取り出す（失敗は <see cref="ConfigException"/>）。
    /// </summary>
    public static ExpectedItem Build(ItemConfig item)
    {
        var locales = new List<ExpectedLocale>();
        foreach (var locale in item.Locales)
        {
            var values = new Dictionary<ListingField, string>();
            if (locale.Description is not null)
            {
                var text = SectionExtractor.Extract(locale.Description);
                if (text.Length > DescriptionMaxLength)
                {
                    throw new ConfigException(
                        $"{item.DisplayName} ({item.ItemId}) [{locale.Code}] の説明が {text.Length:N0} 文字あり、上限 {DescriptionMaxLength:N0} 文字を超えています");
                }
                values[ListingField.Description] = text;
            }
            foreach (var (field, url) in locale.Urls) values[field] = url;
            locales.Add(new ExpectedLocale(locale.Code, values));
        }
        return new ExpectedItem(item.ItemId, item.DisplayName, locales);
    }

    /// <summary>ダッシュボードの説明欄の上限（文字数カウンタ「n/16,000」）</summary>
    public const int DescriptionMaxLength = 16_000;
}

/// <summary>ダッシュボードから読み出したロケール 1 件の現在値</summary>
/// <param name="Code">ロケールコード</param>
/// <param name="Values">欄 → 現在値</param>
/// <param name="Error">読み出しに失敗した場合の理由（成功なら null）</param>
public sealed record LocaleSnapshot(string Code, IReadOnlyDictionary<ListingField, string> Values, string? Error = null);

/// <summary>ダッシュボードから読み出したアイテム 1 件の現在値</summary>
/// <param name="ItemId">アイテム ID</param>
/// <param name="StatusText">ステータス表示（「公開済み」など。読めなければ null）</param>
/// <param name="AvailableLocales">アイテムが持つロケール（言語の選択肢から取得）</param>
/// <param name="Locales">読み出したロケール</param>
/// <param name="Error">アイテム全体の読み出し失敗の理由（成功なら null）</param>
public sealed record ItemSnapshot(
    string ItemId,
    string? StatusText,
    IReadOnlyList<string> AvailableLocales,
    IReadOnlyList<LocaleSnapshot> Locales,
    string? Error = null)
{
    /// <summary>ロケールの現在値を探す</summary>
    public LocaleSnapshot? Find(string code)
        => Locales.FirstOrDefault(l => string.Equals(l.Code, code, StringComparison.OrdinalIgnoreCase));
}

/// <summary>比較結果の状態</summary>
public enum ComparisonStatus
{
    /// <summary>まだダッシュボードから読み出していない</summary>
    NotFetched,

    /// <summary>正本と一致</summary>
    Match,

    /// <summary>正本と異なる</summary>
    Different,

    /// <summary>読み出しに失敗した</summary>
    Error,
}

/// <summary>欄 1 つの比較結果</summary>
/// <param name="Field">欄</param>
/// <param name="Expected">正本</param>
/// <param name="Current">現在値（未取得・失敗は null）</param>
/// <param name="Status">状態</param>
public sealed record FieldComparison(ListingField Field, string Expected, string? Current, ComparisonStatus Status);

/// <summary>アイテム × ロケール 1 件の比較結果</summary>
/// <param name="ItemId">アイテム ID</param>
/// <param name="ItemName">表示名</param>
/// <param name="Locale">ロケールコード</param>
/// <param name="Fields">欄ごとの結果</param>
/// <param name="Error">失敗の理由（読み出し失敗など）</param>
public sealed record LocaleComparison(
    string ItemId,
    string ItemName,
    string Locale,
    IReadOnlyList<FieldComparison> Fields,
    string? Error = null)
{
    /// <summary>
    /// 行全体の状態。エラー &gt; 未取得 &gt; 差分あり &gt; 一致 の順に強い方を採る。
    /// </summary>
    public ComparisonStatus Status
    {
        get
        {
            if (Error is not null || Fields.Any(f => f.Status == ComparisonStatus.Error)) return ComparisonStatus.Error;
            if (Fields.Count == 0 || Fields.Any(f => f.Status == ComparisonStatus.NotFetched)) return ComparisonStatus.NotFetched;
            if (Fields.Any(f => f.Status == ComparisonStatus.Different)) return ComparisonStatus.Different;
            return ComparisonStatus.Match;
        }
    }

    /// <summary>差分のある欄</summary>
    public IReadOnlyList<ListingField> DifferentFields
        => [.. Fields.Where(f => f.Status == ComparisonStatus.Different).Select(f => f.Field)];
}

/// <summary>状態の表示名</summary>
public static class ComparisonStatusInfo
{
    /// <summary>画面・CLI に出す日本語名</summary>
    public static string DisplayName(this ComparisonStatus status) => status switch
    {
        ComparisonStatus.NotFetched => "未取得",
        ComparisonStatus.Match => "一致",
        ComparisonStatus.Different => "差分あり",
        ComparisonStatus.Error => "エラー",
        _ => status.ToString(),
    };
}
