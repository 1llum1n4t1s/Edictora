using Edictora.Core.Dashboard;

namespace Edictora.Core.Services;

/// <summary>
/// ダッシュボード用ブラウザを必要になった時点で開き、使い回す（UI 向け）。
/// </summary>
/// <remarks>
/// CLI は 1 コマンドごとに開いて閉じるが、UI は「取得 → 保存 → 送信」を続けて行うので、毎回ログイン確認をしないよう使い回す。
/// プロファイルやブラウザの指定が変わったら開き直す。Chrome が閉じられていたら次の操作で開き直す。
/// 同時に 2 つの操作をしないよう、呼び出し側（UI）で操作を直列化すること。
/// </remarks>
public sealed class DashboardSessionManager : IAsyncDisposable
{
    private DashboardBrowser? _browser;
    private DashboardBrowserOptions? _options;

    /// <summary>ブラウザを開いているか</summary>
    public bool IsOpen => _browser is not null;

    /// <summary>
    /// ログイン済みのブラウザを返す。未ログインなら <see cref="LoginRequiredException"/>。
    /// </summary>
    public async Task<DashboardBrowser> GetLoggedInAsync(DashboardBrowserOptions options, CancellationToken ct)
    {
        var browser = await GetOrOpenAsync(options, ct).ConfigureAwait(false);
        try
        {
            await browser.EnsureLoggedInAsync(ct).ConfigureAwait(false);
            return browser;
        }
        catch (Exception ex) when (ex is not LoginRequiredException and not OperationCanceledException)
        {
            // 利用者が Chrome を閉じた等で接続が切れていたら、1 度だけ開き直す
            await CloseAsync().ConfigureAwait(false);
            browser = await GetOrOpenAsync(options, ct).ConfigureAwait(false);
            await browser.EnsureLoggedInAsync(ct).ConfigureAwait(false);
            return browser;
        }
    }

    /// <summary>ブラウザを開き、利用者のログインを待つ</summary>
    public async Task LoginAsync(DashboardBrowserOptions options, TimeSpan timeout, IProgress<string>? progress, CancellationToken ct)
    {
        await CloseAsync().ConfigureAwait(false);
        var browser = await GetOrOpenAsync(options, ct).ConfigureAwait(false);
        await browser.WaitForLoginAsync(timeout, progress, ct).ConfigureAwait(false);
    }

    private async Task<DashboardBrowser> GetOrOpenAsync(DashboardBrowserOptions options, CancellationToken ct)
    {
        // 利用者が Chrome を閉じて接続が切れていたら、使い回さずに開き直す
        if (_browser is not null && _options == options && _browser.IsConnected) return _browser;
        await CloseAsync().ConfigureAwait(false);
        _browser = await DashboardBrowser.OpenAsync(options, ct).ConfigureAwait(false);
        _options = options;
        return _browser;
    }

    /// <summary>ブラウザを閉じる（自分で起動した Chrome だけ。プロファイルは残す）</summary>
    public async Task CloseAsync()
    {
        var browser = _browser;
        _browser = null;
        _options = null;
        if (browser is not null) await browser.DisposeAsync().ConfigureAwait(false);
    }

    public async ValueTask DisposeAsync() => await CloseAsync().ConfigureAwait(false);
}
