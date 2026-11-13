using System.Text;

namespace Edictora.Core.Listing;

/// <summary>差分 1 行の種類</summary>
public enum DiffKind
{
    /// <summary>共通行</summary>
    Same,

    /// <summary>現在値にだけある行（正本で消える行）</summary>
    Removed,

    /// <summary>正本にだけある行（正本で増える行）</summary>
    Added,
}

/// <summary>差分 1 行</summary>
public readonly record struct DiffLine(DiffKind Kind, string Text);

/// <summary>
/// 行単位の簡易差分（LCS）。CLI の diff 表示と UI の補助表示に使う。
/// </summary>
/// <remarks>
/// 説明欄は最大 16,000 文字なので行数は多くても数百行。O(n×m) の表で十分速い。
/// 念のため表が大きすぎるときは、行ごとの対応づけを諦めて「全削除 + 全追加」で返す。
/// </remarks>
public static class TextDiff
{
    /// <summary>LCS 表の上限セル数（これを超えたら簡易表示）</summary>
    private const long MaxCells = 4_000_000;

    /// <summary>現在値（before）から正本（after）への行差分を返す</summary>
    public static IReadOnlyList<DiffLine> Compute(string before, string after)
    {
        var a = before.Replace("\r\n", "\n").Split('\n');
        var b = after.Replace("\r\n", "\n").Split('\n');

        if ((long)a.Length * b.Length > MaxCells)
        {
            return [.. a.Select(l => new DiffLine(DiffKind.Removed, l)), .. b.Select(l => new DiffLine(DiffKind.Added, l))];
        }

        // lcs[i, j] = a[i..] と b[j..] の最長共通部分列の長さ
        var lcs = new int[a.Length + 1, b.Length + 1];
        for (var i = a.Length - 1; i >= 0; i--)
        {
            for (var j = b.Length - 1; j >= 0; j--)
            {
                lcs[i, j] = string.Equals(a[i], b[j], StringComparison.Ordinal)
                    ? lcs[i + 1, j + 1] + 1
                    : Math.Max(lcs[i + 1, j], lcs[i, j + 1]);
            }
        }

        var result = new List<DiffLine>();
        int x = 0, y = 0;
        while (x < a.Length && y < b.Length)
        {
            if (string.Equals(a[x], b[y], StringComparison.Ordinal))
            {
                result.Add(new DiffLine(DiffKind.Same, a[x]));
                x++;
                y++;
            }
            else if (lcs[x + 1, y] >= lcs[x, y + 1])
            {
                result.Add(new DiffLine(DiffKind.Removed, a[x++]));
            }
            else
            {
                result.Add(new DiffLine(DiffKind.Added, b[y++]));
            }
        }
        while (x < a.Length) result.Add(new DiffLine(DiffKind.Removed, a[x++]));
        while (y < b.Length) result.Add(new DiffLine(DiffKind.Added, b[y++]));
        return result;
    }

    /// <summary>
    /// 変更行の前後 <paramref name="context"/> 行だけを残した表示用テキストにする。
    /// 「-」は現在値にだけある行、「+」は正本にだけある行。省略した共通行は「…」で示す。
    /// </summary>
    public static string Format(IReadOnlyList<DiffLine> lines, int context = 2)
    {
        var keep = new bool[lines.Count];
        for (var i = 0; i < lines.Count; i++)
        {
            if (lines[i].Kind == DiffKind.Same) continue;
            for (var k = Math.Max(0, i - context); k <= Math.Min(lines.Count - 1, i + context); k++) keep[k] = true;
        }

        var sb = new StringBuilder();
        var skipped = false;
        for (var i = 0; i < lines.Count; i++)
        {
            if (!keep[i])
            {
                skipped = true;
                continue;
            }
            if (skipped)
            {
                sb.Append("  …\n");
                skipped = false;
            }
            var prefix = lines[i].Kind switch { DiffKind.Removed => "- ", DiffKind.Added => "+ ", _ => "  " };
            sb.Append(prefix).Append(lines[i].Text).Append('\n');
        }
        if (skipped) sb.Append("  …\n");
        return sb.ToString();
    }

    /// <summary>
    /// 最初に食い違う文字の位置（0 始まり）。一致していれば -1。
    /// 行差分では見えない行末空白や不可視文字の違いを知らせるために使う。
    /// </summary>
    public static int FirstDifferenceIndex(string a, string b)
    {
        var n = Math.Min(a.Length, b.Length);
        for (var i = 0; i < n; i++)
        {
            if (a[i] != b[i]) return i;
        }
        return a.Length == b.Length ? -1 : n;
    }
}
