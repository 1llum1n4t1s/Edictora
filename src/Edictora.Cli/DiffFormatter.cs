using System.Text;
using Edictora.Core.Listing;

namespace Edictora.Cli;

/// <summary>比較結果を CLI 向けの文字列にする</summary>
internal static class DiffFormatter
{
    /// <summary>アイテム × ロケール 1 行分を整形する</summary>
    public static string Format(LocaleComparison row)
    {
        var sb = new StringBuilder();
        if (row.Error is not null)
        {
            sb.Append($"[{row.Locale}] エラー: {row.Error}\n");
            return sb.ToString();
        }

        foreach (var field in row.Fields)
        {
            switch (field.Status)
            {
                case ComparisonStatus.Match:
                    sb.Append($"[{row.Locale}] {field.Field.DisplayName()}: 一致\n");
                    break;
                case ComparisonStatus.NotFetched:
                    sb.Append($"[{row.Locale}] {field.Field.DisplayName()}: 未取得\n");
                    break;
                case ComparisonStatus.Different:
                    var current = field.Current ?? "";
                    if (field.Field == ListingField.Description)
                    {
                        sb.Append($"[{row.Locale}] {field.Field.DisplayName()}: 差分あり（現在 {current.Length:N0} 文字 / 正本 {field.Expected.Length:N0} 文字）\n");
                        var lines = TextDiff.Compute(current, field.Expected);
                        if (lines.All(l => l.Kind == DiffKind.Same))
                        {
                            // 行単位では同じ（不可視文字・行末空白などの違い）
                            sb.Append($"  （行単位では同じです。{TextDiff.FirstDifferenceIndex(current, field.Expected)} 文字目に見えない違いがあります）\n");
                        }
                        else
                        {
                            sb.Append(TextDiff.Format(lines));
                        }
                    }
                    else
                    {
                        sb.Append($"[{row.Locale}] {field.Field.DisplayName()}: 差分あり\n");
                        sb.Append($"- {current}\n+ {field.Expected}\n");
                    }
                    break;
                default:
                    sb.Append($"[{row.Locale}] {field.Field.DisplayName()}: {field.Status.DisplayName()}\n");
                    break;
            }
        }
        return sb.ToString();
    }
}
