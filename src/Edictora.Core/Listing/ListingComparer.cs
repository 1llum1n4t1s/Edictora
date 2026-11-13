namespace Edictora.Core.Listing;

/// <summary>
/// 正本とダッシュボードの現在値を比べる。
/// </summary>
/// <remarks>
/// 比較は完全一致（序数比較）。前後の空白や改行の違いも差分として扱う。
/// 正本側は SectionExtractor で改行を \n に正規化済み、ダッシュボードの textarea の value も \n なので、
/// 正規化の差で誤検出することはない。曖昧に「ほぼ同じ」とみなすと、書き換え漏れに気づけなくなるため厳密に比べる。
/// </remarks>
public static class ListingComparer
{
    /// <summary>値 1 つを比べる</summary>
    public static ComparisonStatus CompareValue(string expected, string? current)
        => current is null ? ComparisonStatus.NotFetched
            : string.Equals(expected, current, StringComparison.Ordinal) ? ComparisonStatus.Match
            : ComparisonStatus.Different;

    /// <summary>
    /// アイテム 1 件を比べる。<paramref name="snapshot"/> が null なら全行「未取得」。
    /// </summary>
    public static IReadOnlyList<LocaleComparison> Compare(ExpectedItem expected, ItemSnapshot? snapshot)
    {
        var result = new List<LocaleComparison>();
        foreach (var locale in expected.Locales)
        {
            if (snapshot is null)
            {
                result.Add(NotFetched(expected, locale));
                continue;
            }

            if (snapshot.Error is not null)
            {
                result.Add(NotFetched(expected, locale) with { Error = snapshot.Error });
                continue;
            }

            var current = snapshot.Find(locale.Code);
            if (current is null)
            {
                var error = snapshot.AvailableLocales.Count > 0 &&
                    !snapshot.AvailableLocales.Contains(locale.Code, StringComparer.OrdinalIgnoreCase)
                    ? $"ダッシュボードにロケール {locale.Code} がありません（ある言語: {string.Join(", ", snapshot.AvailableLocales)}）"
                    : null;
                result.Add(NotFetched(expected, locale) with { Error = error });
                continue;
            }

            if (current.Error is not null)
            {
                result.Add(NotFetched(expected, locale) with { Error = current.Error });
                continue;
            }

            var fields = locale.Fields
                .Select(f =>
                {
                    var value = current.Values.TryGetValue(f, out var v) ? v : null;
                    return new FieldComparison(f, locale.Values[f], value, CompareValue(locale.Values[f], value));
                })
                .ToList();
            result.Add(new LocaleComparison(expected.ItemId, expected.DisplayName, locale.Code, fields));
        }
        return result;
    }

    private static LocaleComparison NotFetched(ExpectedItem item, ExpectedLocale locale)
        => new(item.ItemId, item.DisplayName, locale.Code,
            [.. locale.Fields.Select(f => new FieldComparison(f, locale.Values[f], null, ComparisonStatus.NotFetched))]);
}
