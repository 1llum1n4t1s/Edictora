namespace Edictora.Core.Dashboard;

/// <summary>
/// デベロッパー ダッシュボードの URL を組み立て・解釈する。
/// </summary>
/// <remarks>
/// publisherId はログイン後の URL（/webstore/devconsole/&lt;publisherId&gt;）から実行時に取得する。
/// コード・設定・ログには書かない（ログは LoggerService.Redact で伏せる）。
/// </remarks>
public static class DashboardUrls
{
    /// <summary>ダッシュボードのホスト</summary>
    public const string Host = "chrome.google.com";

    /// <summary>ダッシュボードの入口（ログイン済みなら publisherId 付きの URL へ移る）</summary>
    public const string Root = "https://chrome.google.com/webstore/devconsole/";

    /// <summary>
    /// URL から publisherId を取り出す。ダッシュボードの URL でなければ false。
    /// </summary>
    public static bool TryParsePublisherId(string? url, out string publisherId)
    {
        publisherId = "";
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri)) return false;
        if (uri.Scheme != Uri.UriSchemeHttps || !string.Equals(uri.Host, Host, StringComparison.OrdinalIgnoreCase)) return false;

        var segments = uri.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries);
        if (segments.Length < 3) return false;
        if (!string.Equals(segments[0], "webstore", StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(segments[1], "devconsole", StringComparison.OrdinalIgnoreCase)) return false;

        var candidate = segments[2];
        // publisherId は英数字とハイフンからなる識別子（GUID 形式）。それ以外の文字は受け付けない。
        if (candidate.Length is < 8 or > 64 || !candidate.All(c => char.IsAsciiLetterOrDigit(c) || c == '-')) return false;

        publisherId = candidate;
        return true;
    }

    /// <summary>Google のログイン画面か</summary>
    public static bool IsLoginPage(string? url)
        => Uri.TryCreate(url, UriKind.Absolute, out var uri) &&
           string.Equals(uri.Host, "accounts.google.com", StringComparison.OrdinalIgnoreCase);

    /// <summary>アイテムの編集画面（ストアの掲載情報）の URL</summary>
    public static string EditUrl(string publisherId, string itemId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(publisherId);
        ArgumentException.ThrowIfNullOrWhiteSpace(itemId);
        return $"{Root}{Uri.EscapeDataString(publisherId)}/{Uri.EscapeDataString(itemId)}/edit";
    }

    /// <summary>URL がそのアイテムの編集画面か（publisherId は問わない）</summary>
    public static bool IsEditPageOf(string? url, string itemId)
    {
        if (!TryParsePublisherId(url, out _)) return false;
        var path = new Uri(url!).AbsolutePath.TrimEnd('/');
        return path.EndsWith($"/{itemId}/edit", StringComparison.OrdinalIgnoreCase);
    }
}
