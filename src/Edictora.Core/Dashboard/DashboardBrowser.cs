using System.Text.Json.Nodes;
using Edictora.Core.Cdp;
using Edictora.Core.Chrome;
using Edictora.Core.Logging;

namespace Edictora.Core.Dashboard;

/// <summary>ダッシュボード用ブラウザの開き方</summary>
/// <param name="ProfileDirectory">専用プロファイル（ログイン状態を保持する。削除しない）</param>
/// <param name="ChromePath">chrome.exe（null なら自動検出）</param>
public sealed record DashboardBrowserOptions(string ProfileDirectory, string? ChromePath);

/// <summary>
/// 専用プロファイルの Chrome と、その 1 タブへの CDP 接続をまとめたもの。
/// </summary>
/// <remarks>
/// 使い方: <see cref="OpenAsync"/> → <see cref="EnsureLoggedInAsync"/>（または <see cref="WaitForLoginAsync"/>）
/// → <see cref="CreateItemPage"/> で編集画面を操作 → <see cref="DisposeAsync"/> で閉じる。
/// publisherId はログイン確認時に URL から取り出してメモリにだけ持つ。外へは出さない。
///
/// ログイン待ちの間はタブに接続しない（2026-11-13 の検証で手動ログインできたのは、タブに何も接続していない状態だったため）。
/// ブラウザ全体の接続から Target.getTargets で URL を見るだけにし、タブへの接続はログイン確認後の操作時に行う。
/// </remarks>
public sealed class DashboardBrowser : IAsyncDisposable
{
    private readonly ChromeInstance _chrome;
    private readonly CdpClient _client;
    private CdpPage? _page;
    private string? _publisherId;
    private int _disposed;

    /// <summary>ログインを確認済みか</summary>
    public bool IsLoggedIn => _publisherId is not null;

    /// <summary>Chrome との接続が生きているか（利用者が Chrome を閉じると false）</summary>
    public bool IsConnected => Volatile.Read(ref _disposed) == 0 && _client.IsConnected;

    /// <summary>このツールが起動した Chrome か（false なら既存のウィンドウに接続した）</summary>
    public bool OwnsChrome => _chrome.OwnedByTool;

    private DashboardBrowser(ChromeInstance chrome, CdpClient client)
    {
        _chrome = chrome;
        _client = client;
    }

    /// <summary>
    /// Chrome を起動（または既存に接続）し、ブラウザ全体の CDP に接続する。タブにはまだ接続しない。
    /// 新しく起動する場合は最初からダッシュボードの入口を開く。
    /// </summary>
    public static async Task<DashboardBrowser> OpenAsync(DashboardBrowserOptions options, CancellationToken ct)
    {
        var chromePath = options.ChromePath ?? ChromeLocator.FindChrome()
            ?? throw new DashboardException("Chrome が見つかりません。設定ファイルの chromePath か --chrome で chrome.exe を指定してください。");
        if (!File.Exists(chromePath)) throw new DashboardException($"chrome.exe がありません: {chromePath}");

        var chrome = await ChromeLauncher.LaunchOrAttachAsync(new ChromeLaunchOptions
        {
            ChromePath = chromePath,
            UserDataDir = options.ProfileDirectory,
            StartUrl = DashboardUrls.Root,
        }, ct).ConfigureAwait(false);

        try
        {
            var client = await CdpClient.ConnectAsync(chrome.BrowserWsUri, ct).ConfigureAwait(false);
            return new DashboardBrowser(chrome, client);
        }
        catch
        {
            await chrome.CloseAsync(null).ConfigureAwait(false);
            await chrome.DisposeAsync().ConfigureAwait(false);
            throw;
        }
    }

    /// <summary>
    /// ダッシュボードの入口を開き、ログイン済みか確かめる。未ログインなら <see cref="LoginRequiredException"/>。
    /// </summary>
    public async Task EnsureLoggedInAsync(CancellationToken ct)
    {
        if (!IsConnected) throw new DashboardException("Chrome との接続が切れています。開き直してください。");
        var page = await GetPageAsync(ct).ConfigureAwait(false);
        if (_publisherId is not null) return;

        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        await page.NavigateAsync(DashboardUrls.Root, ct).ConfigureAwait(false);

        // ダッシュボードは読み込み後に publisherId 付きの URL へ移る。ログイン画面へ移れば未ログイン。
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(30);
        while (DateTime.UtcNow < deadline)
        {
            var url = await page.GetUrlAsync(ct).ConfigureAwait(false);
            if (DashboardUrls.TryParsePublisherId(url, out var pid))
            {
                _publisherId = pid;
                LoggerService.Log("ダッシュボードへのログインを確認しました", LogLevel.Info);
                return;
            }
            if (DashboardUrls.IsLoginPage(url)) throw new LoginRequiredException();
            await Task.Delay(500, ct).ConfigureAwait(false);
        }
        throw new DashboardException("ダッシュボードに到達できませんでした（30 秒）。login を実行してログイン状態を確認してください。");
    }

    /// <summary>
    /// 利用者が手動でログインしてダッシュボードに着くまで待つ。
    /// パスワードや確認コードには一切触れない（入力は利用者が Chrome の画面で行う）。タブにも接続しない。
    /// </summary>
    /// <param name="timeout">待つ上限</param>
    /// <param name="progress">案内の出力先</param>
    /// <param name="ct">中止</param>
    public async Task WaitForLoginAsync(TimeSpan timeout, IProgress<string>? progress, CancellationToken ct)
    {
        // 既に開いていた Chrome に接続した場合は、入口のタブを新しく開く（起動した場合は起動時に開いている）
        if (!_chrome.OwnedByTool && await FindDashboardUrlAsync(ct).ConfigureAwait(false) is null)
        {
            await _client.SendAsync("Target.createTarget", new JsonObject { ["url"] = DashboardUrls.Root }, ct: ct).ConfigureAwait(false);
        }
        progress?.Report("開いた Chrome で Google にログインし、デベロッパー ダッシュボードを表示してください。");

        var deadline = DateTime.UtcNow + timeout;
        var announced = false;
        while (DateTime.UtcNow < deadline)
        {
            if (!IsConnected) throw new DashboardException("Chrome が閉じられたため、ログインの確認を中止しました。");

            var (dashboardUrl, onLoginPage) = await ScanTargetsAsync(ct).ConfigureAwait(false);
            if (DashboardUrls.TryParsePublisherId(dashboardUrl, out var pid))
            {
                _publisherId = pid;
                progress?.Report("ダッシュボードへのログインを確認しました。");
                LoggerService.Log("login: ダッシュボード到達を確認", LogLevel.Info);
                return;
            }
            if (!announced && onLoginPage)
            {
                progress?.Report("Google のログイン画面を表示しています。ログインが終わるまで待っています…");
                announced = true;
            }
            await Task.Delay(1000, ct).ConfigureAwait(false);
        }
        throw new DashboardException($"ログインの完了を {timeout.TotalMinutes:0} 分待ちましたが、ダッシュボードに到達しませんでした。");
    }

    /// <summary>アイテムの編集画面を操作するオブジェクトを作る（先にログイン確認が必要）</summary>
    public IListingPage CreateItemPage()
    {
        if (_publisherId is null || _page is null)
        {
            throw new InvalidOperationException("先に EnsureLoggedInAsync でログインを確認してください");
        }
        return new DashboardItemPage(_page, _publisherId);
    }

    /// <summary>ブラウザ全体の接続から、全タブの URL を見る（タブには接続しない）</summary>
    private async Task<(string? DashboardUrl, bool OnLoginPage)> ScanTargetsAsync(CancellationToken ct)
    {
        var targets = await _client.SendAsync("Target.getTargets", ct: ct).ConfigureAwait(false);
        string? dashboard = null;
        var login = false;
        foreach (var t in targets.GetProperty("targetInfos").EnumerateArray())
        {
            if (t.GetProperty("type").GetString() != "page") continue;
            var url = t.GetProperty("url").GetString();
            if (DashboardUrls.TryParsePublisherId(url, out _)) dashboard ??= url;
            if (DashboardUrls.IsLoginPage(url)) login = true;
        }
        return (dashboard, login);
    }

    private async Task<string?> FindDashboardUrlAsync(CancellationToken ct)
    {
        var targets = await _client.SendAsync("Target.getTargets", ct: ct).ConfigureAwait(false);
        foreach (var t in targets.GetProperty("targetInfos").EnumerateArray())
        {
            var url = t.GetProperty("url").GetString() ?? "";
            if (t.GetProperty("type").GetString() == "page" && url.StartsWith(DashboardUrls.Root, StringComparison.OrdinalIgnoreCase)) return url;
        }
        return null;
    }

    /// <summary>操作に使うタブへ接続する（初回だけ）</summary>
    private async Task<CdpPage> GetPageAsync(CancellationToken ct)
    {
        if (_page is not null) return _page;

        var targetId = await PickPageTargetAsync(ct).ConfigureAwait(false);
        var attach = await _client.SendAsync("Target.attachToTarget",
            new JsonObject { ["targetId"] = targetId, ["flatten"] = true }, ct: ct).ConfigureAwait(false);
        var sessionId = attach.GetProperty("sessionId").GetString()
            ?? throw new DashboardException("タブへの接続に失敗しました");

        var page = new CdpPage(new CdpSession(_client, sessionId, targetId));
        await page.EnableAsync(ct).ConfigureAwait(false);
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        _page = page;
        return page;
    }

    /// <summary>
    /// 操作に使うタブを選ぶ。ダッシュボードを開いているタブがあればそれを、なければ通常のタブを使う。
    /// タブが 1 つも無ければ新しく作る。
    /// </summary>
    private async Task<string> PickPageTargetAsync(CancellationToken ct)
    {
        var targets = await _client.SendAsync("Target.getTargets", ct: ct).ConfigureAwait(false);
        string? fallback = null;
        foreach (var t in targets.GetProperty("targetInfos").EnumerateArray())
        {
            if (t.GetProperty("type").GetString() != "page") continue;
            var id = t.GetProperty("targetId").GetString();
            var url = t.GetProperty("url").GetString() ?? "";
            if (url.StartsWith(DashboardUrls.Root, StringComparison.OrdinalIgnoreCase)) return id!;
            if (!url.StartsWith("devtools://", StringComparison.OrdinalIgnoreCase)) fallback ??= id;
        }
        if (fallback is not null) return fallback;

        var created = await _client.SendAsync("Target.createTarget", new JsonObject { ["url"] = "about:blank" }, ct: ct).ConfigureAwait(false);
        return created.GetProperty("targetId").GetString()!;
    }

    /// <summary>
    /// 終了する。自分で起動した Chrome は Browser.close で正常終了させる（プロファイルは残す）。
    /// 既に開いていた Chrome には接続を切るだけで、ウィンドウは閉じない。
    /// </summary>
    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;
        _publisherId = null;
        if (_chrome.OwnedByTool)
        {
            await _chrome.CloseAsync(_client).ConfigureAwait(false);
        }
        else if (_page is not null)
        {
            try
            {
                await _client.SendAsync("Target.detachFromTarget",
                    new JsonObject { ["sessionId"] = _page.Session.SessionId }, timeout: TimeSpan.FromSeconds(5)).ConfigureAwait(false);
            }
            catch (Exception ex) when (ex is CdpException or TimeoutException or ObjectDisposedException)
            {
                // 切断済みなら何もしない
            }
        }
        await _client.DisposeAsync().ConfigureAwait(false);
        await _chrome.DisposeAsync().ConfigureAwait(false);
    }
}
