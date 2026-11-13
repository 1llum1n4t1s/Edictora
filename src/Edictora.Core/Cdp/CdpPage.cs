using System.Text.Json;
using System.Text.Json.Nodes;

namespace Edictora.Core.Cdp;

/// <summary>Runtime.evaluate がページ側の例外で失敗した</summary>
public sealed class JavaScriptException(string message) : Exception(message);

/// <summary>
/// 1 つのページ（タブ）に対する低レベル操作: スクリプト評価・マウス・キー入力・遷移。
/// </summary>
/// <remarks>
/// puppeteer が裏でやっていた処理を CDP で直接行う。注意点:
/// - クリックは mouseMoved → mousePressed → mouseReleased を clickCount=1 で送る（clickCount が無いと click イベントが起きない）
/// - Runtime.evaluate は returnByValue で値を受け取り、exceptionDetails があれば失敗として扱う
/// - 読み込み完了イベントは、遷移コマンドを送る「前」に待ち始める（取りこぼし防止）
/// - beforeunload の確認ダイアログが出たら遷移を中止する（未保存の変更を勝手に捨てない）
/// </remarks>
public sealed class CdpPage(CdpSession session)
{
    /// <summary>CDP セッション</summary>
    public CdpSession Session { get; } = session;

    /// <summary>ページのドメインを有効化する（読み込み・ダイアログのイベントを受けるため）</summary>
    /// <remarks>
    /// Runtime.enable は呼ばない。Runtime.evaluate は有効化なしで使え、Runtime.enable はページ側から
    /// 自動操作の検知に使われることがあるため（ログインを弾かれる要因を増やさない）。
    /// </remarks>
    public Task EnableAsync(CancellationToken ct)
        => Session.SendAsync("Page.enable", ct: ct);

    /// <summary>
    /// ウィンドウを前面に出し、フォーカスを持っている扱いにする。
    /// 裏に回るとクリックが固まったため、操作の前に毎回呼ぶ。
    /// </summary>
    public async Task BringToFrontAsync(CancellationToken ct)
    {
        await Session.SendAsync("Page.bringToFront", ct: ct).ConfigureAwait(false);
        await Session.SendAsync("Emulation.setFocusEmulationEnabled", new JsonObject { ["enabled"] = true }, ct: ct).ConfigureAwait(false);
    }

    /// <summary>今の URL（Target 情報から取る。遷移中でも失敗しにくい）</summary>
    public async Task<string?> GetUrlAsync(CancellationToken ct)
    {
        var info = await Session.Client.SendAsync(
            "Target.getTargetInfo", new JsonObject { ["targetId"] = Session.TargetId }, ct: ct).ConfigureAwait(false);
        return info.TryGetProperty("targetInfo", out var t) && t.TryGetProperty("url", out var url) ? url.GetString() : null;
    }

    /// <summary>
    /// 式を評価して値を返す。Promise は待つ。ページ側で例外が起きたら <see cref="JavaScriptException"/>。
    /// </summary>
    public async Task<JsonElement> EvaluateAsync(string expression, CancellationToken ct, TimeSpan? timeout = null)
    {
        var result = await Session.SendAsync("Runtime.evaluate", new JsonObject
        {
            ["expression"] = expression,
            ["returnByValue"] = true,
            ["awaitPromise"] = true,
            ["userGesture"] = false,
        }, timeout ?? TimeSpan.FromSeconds(20), ct).ConfigureAwait(false);

        if (result.TryGetProperty("exceptionDetails", out var ex))
        {
            var text = ex.TryGetProperty("exception", out var e) && e.TryGetProperty("description", out var d)
                ? d.GetString()
                : ex.TryGetProperty("text", out var t) ? t.GetString() : "不明なエラー";
            throw new JavaScriptException($"ページ内スクリプトの実行に失敗しました: {text}");
        }

        return result.TryGetProperty("result", out var r) && r.TryGetProperty("value", out var v) ? v.Clone() : default;
    }

    /// <summary>ビューポート座標 (x, y) を左クリックする（実際の入力イベントとして届く）</summary>
    public async Task ClickAtAsync(double x, double y, CancellationToken ct)
    {
        await MouseAsync("mouseMoved", x, y, 0, ct).ConfigureAwait(false);
        await MouseAsync("mousePressed", x, y, 1, ct).ConfigureAwait(false);
        await MouseAsync("mouseReleased", x, y, 1, ct).ConfigureAwait(false);
    }

    private Task<JsonElement> MouseAsync(string type, double x, double y, int clickCount, CancellationToken ct)
    {
        var p = new JsonObject
        {
            ["type"] = type,
            ["x"] = x,
            ["y"] = y,
            ["button"] = type == "mouseMoved" ? "none" : "left",
            ["buttons"] = type == "mousePressed" ? 1 : 0,
        };
        if (clickCount > 0) p["clickCount"] = clickCount;
        return Session.SendAsync("Input.dispatchMouseEvent", p, ct: ct);
    }

    /// <summary>
    /// フォーカス中の欄へ文字列を入れる（選択範囲は置き換わる）。ページには通常の入力として届く。
    /// </summary>
    public Task InsertTextAsync(string text, CancellationToken ct)
        => Session.SendAsync("Input.insertText", new JsonObject { ["text"] = text }, TimeSpan.FromSeconds(30), ct);

    /// <summary>特殊キーを 1 回押す（Delete / Escape など）</summary>
    public async Task PressKeyAsync(string key, int windowsVirtualKeyCode, CancellationToken ct)
    {
        foreach (var type in new[] { "rawKeyDown", "keyUp" })
        {
            await Session.SendAsync("Input.dispatchKeyEvent", new JsonObject
            {
                ["type"] = type,
                ["key"] = key,
                ["code"] = key,
                ["windowsVirtualKeyCode"] = windowsVirtualKeyCode,
                ["nativeVirtualKeyCode"] = windowsVirtualKeyCode,
            }, ct: ct).ConfigureAwait(false);
        }
    }

    /// <summary>URL へ移動し、load イベントを待つ</summary>
    public Task NavigateAsync(string url, CancellationToken ct)
        => RunNavigationAsync(async () =>
        {
            var res = await Session.SendAsync("Page.navigate", new JsonObject { ["url"] = url }, ct: ct).ConfigureAwait(false);
            if (res.TryGetProperty("errorText", out var err) && !string.IsNullOrEmpty(err.GetString()))
            {
                throw new CdpException($"ページを開けませんでした: {err.GetString()}");
            }
        }, ct);

    /// <summary>読み込み直し、load イベントを待つ</summary>
    public Task ReloadAsync(CancellationToken ct)
        => RunNavigationAsync(() => Session.SendAsync("Page.reload", new JsonObject { ["ignoreCache"] = false }, ct: ct), ct);

    /// <summary>
    /// 遷移を実行し、load イベントか beforeunload ダイアログのどちらかを待つ。
    /// beforeunload が出たら「留まる」を選んで中止する（未保存の変更を捨てない）。
    /// </summary>
    private async Task RunNavigationAsync(Func<Task> start, CancellationToken ct)
    {
        using var linked = CancellationTokenSource.CreateLinkedTokenSource(ct);
        // 先に待ち始めてから遷移を送る（イベントの取りこぼし防止）
        var load = Session.WaitForEventAsync("Page.loadEventFired", timeout: TimeSpan.FromSeconds(60), ct: linked.Token);
        var dialog = Session.WaitForEventAsync("Page.javascriptDialogOpening",
            p => p.TryGetProperty("type", out var t) && t.GetString() == "beforeunload",
            TimeSpan.FromSeconds(60), linked.Token);

        try
        {
            await start().ConfigureAwait(false);
            var first = await Task.WhenAny(load, dialog).ConfigureAwait(false);
            if (first == dialog && dialog.IsCompletedSuccessfully)
            {
                await Session.SendAsync("Page.handleJavaScriptDialog", new JsonObject { ["accept"] = false }, ct: ct).ConfigureAwait(false);
                throw new CdpException("未保存の変更があるため、ページの移動を中止しました");
            }
            await load.ConfigureAwait(false);
        }
        finally
        {
            await linked.CancelAsync().ConfigureAwait(false);
            // 残った待機タスクの例外を観測済みにする
            _ = load.ContinueWith(t => _ = t.Exception, TaskScheduler.Default);
            _ = dialog.ContinueWith(t => _ = t.Exception, TaskScheduler.Default);
        }
    }

    /// <summary>画面を PNG で取得する</summary>
    public async Task<byte[]> CaptureScreenshotAsync(CancellationToken ct)
    {
        var res = await Session.SendAsync("Page.captureScreenshot", new JsonObject { ["format"] = "png" }, TimeSpan.FromSeconds(30), ct).ConfigureAwait(false);
        return Convert.FromBase64String(res.GetProperty("data").GetString() ?? "");
    }
}
