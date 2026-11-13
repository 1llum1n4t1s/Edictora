using System.Text.RegularExpressions;

namespace Edictora.Core.Config;

/// <summary>
/// 正本ファイルから説明文を取り出す。
/// </summary>
/// <remarks>
/// 2026-11-13 の検証で使った puppeteer スクリプト（push.mjs）の切り出しと、完全に同じ結果を返すことが要件。
/// push.mjs の処理:
/// <code>
/// const lines = text.replace(/\r\n/g, "\n").split("\n");
/// const s = lines.findIndex((l) => start.test(l));
/// const e = lines.findIndex((l, i) => i > s &amp;&amp; end.test(l));
/// return lines.slice(s + 1, e).join("\n").replace(/^\n+/, "").replace(/\s+$/, "");
/// </code>
/// 差異と注意点:
/// - JS 版は見出しが見つからないと黙って誤った範囲を切り出す（slice に -1 が入る）。ここでは例外で止める。
/// - 単独の CR も LF に寄せる（改行は \n に正規化する方針。CRLF だけのファイルでは JS 版と同じ結果）。
/// - 先頭は「空行（\n）」だけを落とし、行頭の空白は残す。末尾は JS の \s 相当の空白をすべて落とす。
///   .NET の char.IsWhiteSpace は JS の \s と範囲が少し違う（U+0085 / U+FEFF）ため、JS の定義を自前で持つ。
/// - 正規表現は 1 行ずつに当てる。RegexOptions.ECMAScript は \s を ASCII に狭めて全角空白を落とすので使わない。
/// - ファイル全体を使う場合も、同じ改行正規化と前後の除去を行う。
/// </remarks>
public static class SectionExtractor
{
    /// <summary>正規表現 1 回あたりの上限時間（壊れたパターンで固まらないようにする）</summary>
    private static readonly TimeSpan RegexTimeout = TimeSpan.FromSeconds(2);

    /// <summary>ファイルを読み、<paramref name="source"/> の指定どおりに説明文を取り出す</summary>
    public static string Extract(TextSource source)
    {
        string text;
        try
        {
            text = File.ReadAllText(source.FilePath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new ConfigException($"正本ファイルを読めません: {source.FilePath}（{ex.Message}）", ex);
        }

        return Extract(text, source.SectionStart, source.SectionEnd, source.FilePath);
    }

    /// <summary>
    /// 文字列から説明文を取り出す。<paramref name="sectionStart"/> と <paramref name="sectionEnd"/> が
    /// どちらも null ならファイル全体を、両方あれば見出し行の次から終端見出しの手前までを返す。
    /// </summary>
    /// <param name="text">ファイルの内容</param>
    /// <param name="sectionStart">開始見出し行の正規表現</param>
    /// <param name="sectionEnd">終端見出し行の正規表現</param>
    /// <param name="sourceName">エラーメッセージに出す取り出し元の名前</param>
    public static string Extract(string text, string? sectionStart, string? sectionEnd, string sourceName = "正本")
    {
        var normalized = NormalizeNewLines(text);
        if (sectionStart is null && sectionEnd is null) return TrimLikeSpike(normalized);
        if (sectionStart is null || sectionEnd is null)
        {
            throw new ConfigException($"sectionStart と sectionEnd は両方指定してください: {sourceName}");
        }

        var start = CreateRegex(sectionStart, "sectionStart");
        var end = CreateRegex(sectionEnd, "sectionEnd");
        var lines = normalized.Split('\n');

        var s = Array.FindIndex(lines, l => start.IsMatch(l));
        if (s < 0) throw new ConfigException($"開始見出し（{sectionStart}）が見つかりません: {sourceName}");

        var e = -1;
        for (var i = s + 1; i < lines.Length; i++)
        {
            if (end.IsMatch(lines[i]))
            {
                e = i;
                break;
            }
        }
        if (e < 0) throw new ConfigException($"終端見出し（{sectionEnd}）が開始見出しより後に見つかりません: {sourceName}");

        return TrimLikeSpike(string.Join('\n', lines[(s + 1)..e]));
    }

    /// <summary>正規表現の構文を検証する（設定読込時の早期検出用）</summary>
    public static void ValidatePattern(string pattern, string name) => _ = CreateRegex(pattern, name);

    /// <summary>CRLF と単独 CR を LF に寄せる</summary>
    internal static string NormalizeNewLines(string text) => text.Replace("\r\n", "\n").Replace('\r', '\n');

    /// <summary>先頭の連続した改行と、末尾の空白（JS の \s 相当）を落とす</summary>
    internal static string TrimLikeSpike(string text)
    {
        var start = 0;
        while (start < text.Length && text[start] == '\n') start++;
        var end = text.Length;
        while (end > start && IsJsWhiteSpace(text[end - 1])) end--;
        return text[start..end];
    }

    /// <summary>JavaScript の正規表現 \s に含まれる文字か（ECMAScript の WhiteSpace + LineTerminator）</summary>
    internal static bool IsJsWhiteSpace(char c) => (int)c switch
    {
        0x09 or 0x0A or 0x0B or 0x0C or 0x0D or 0x20 => true,
        0xA0 or 0x1680 or 0x2028 or 0x2029 or 0x202F or 0x205F or 0x3000 or 0xFEFF => true,
        >= 0x2000 and <= 0x200A => true,
        _ => false,
    };

    private static Regex CreateRegex(string pattern, string name)
    {
        try
        {
            return new Regex(pattern, RegexOptions.CultureInvariant, RegexTimeout);
        }
        catch (ArgumentException ex)
        {
            throw new ConfigException($"{name} の正規表現が正しくありません: {pattern}（{ex.Message}）", ex);
        }
    }
}
