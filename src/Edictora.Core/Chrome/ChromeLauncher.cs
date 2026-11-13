using System.Diagnostics;
using System.Net.Http;
using System.Text.Json;
using Edictora.Core.Cdp;
using Edictora.Core.Logging;

namespace Edictora.Core.Chrome;

/// <summary>Chrome 起動オプション</summary>
public sealed class ChromeLaunchOptions
{
    /// <summary>chrome.exe のフルパス</summary>
    public required string ChromePath { get; init; }

    /// <summary>
    /// 専用プロファイル（--user-data-dir）。ログイン状態を保持するため使い回す。
    /// ログイン情報が入る秘密情報なので、このツールは絶対に削除しない。
    /// </summary>
    public required string UserDataDir { get; init; }

    /// <summary>起動時に開く URL</summary>
    public string StartUrl { get; init; } = "about:blank";
}

/// <summary>
/// 起動済み（または既に起動していた）Chrome。
/// </summary>
/// <remarks>
/// 旧 ExtWeigh の ChromeInstance は Dispose でプロファイルを削除していたが、
/// Edictora のプロファイルはログイン済みの秘密情報なので削除の経路を一切持たない。
/// 終了は <see cref="CloseAsync"/> で CDP の Browser.close を送り、正常終了させる（Cookie を確実に書き出すため）。
/// 応答しない場合に限り、プロセスを止める。
/// </remarks>
public sealed class ChromeInstance : IAsyncDisposable
{
    private readonly Process? _process;
    private int _closed;

    /// <summary>ブラウザレベル CDP WebSocket エンドポイント</summary>
    public Uri BrowserWsUri { get; }

    /// <summary>このツールが起動したか（false なら既に開いていた Chrome に接続しただけ）</summary>
    public bool OwnedByTool => _process is not null;

    internal ChromeInstance(Process? process, Uri browserWsUri)
    {
        _process = process;
        BrowserWsUri = browserWsUri;
    }

    /// <summary>
    /// 自分で起動した Chrome を正常終了させる。既に開いていた Chrome は閉じない（利用者の操作中かもしれないため）。
    /// </summary>
    /// <param name="client">接続中の CDP クライアント（null なら Browser.close を送れないので終了待ちのみ）</param>
    public async Task CloseAsync(CdpClient? client)
    {
        if (Interlocked.Exchange(ref _closed, 1) != 0) return;
        if (_process is null) return;

        try
        {
            if (!_process.HasExited && client is not null)
            {
                try
                {
                    await client.SendAsync("Browser.close", timeout: TimeSpan.FromSeconds(10)).ConfigureAwait(false);
                }
                catch (Exception ex) when (ex is CdpException or TimeoutException or ObjectDisposedException)
                {
                    // Browser.close は応答前に接続が切れることがある。終了待ちで判断する。
                }
            }

            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(20));
            try
            {
                await _process.WaitForExitAsync(cts.Token).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                // 正常終了しなかったときだけ止める。プロファイルには触れない。
                LoggerService.Log("Chrome が時間内に終了しなかったため、プロセスを停止します", LogLevel.Warning);
                try { _process.Kill(entireProcessTree: true); } catch (InvalidOperationException) { /* 既に終了 */ }
            }
        }
        catch (Exception ex)
        {
            LoggerService.Log($"Chrome の終了処理で例外: {ex.Message}", LogLevel.Warning);
        }
    }

    /// <summary>プロセスハンドルを解放する（Chrome 自体は閉じない。閉じるのは <see cref="CloseAsync"/>）</summary>
    public ValueTask DisposeAsync()
    {
        _process?.Dispose();
        return ValueTask.CompletedTask;
    }
}

/// <summary>Chrome を専用プロファイル + リモートデバッグで起動する（または既存に接続する）</summary>
public static class ChromeLauncher
{
    /// <summary>
    /// 専用プロファイルの Chrome に接続する。
    /// そのプロファイルの Chrome がデバッグポート付きで既に開いていれば接続し、なければ新しく起動する。
    /// </summary>
    /// <remarks>
    /// 同じプロファイルの Chrome が開いていると、新しい chrome.exe は既存のウィンドウに処理を渡してすぐ終了する。
    /// そのため先に既存への接続を試す。デバッグポートなしで開かれていた場合は接続できないので、
    /// 「閉じてから再実行」を案内するエラーにする。
    /// </remarks>
    public static async Task<ChromeInstance> LaunchOrAttachAsync(ChromeLaunchOptions options, CancellationToken ct = default)
    {
        Directory.CreateDirectory(options.UserDataDir);
        var portFile = Path.Combine(options.UserDataDir, "DevToolsActivePort");

        // 1) 既に開いている Chrome（前回の login で開いたままのウィンドウなど）に接続を試す
        if (File.Exists(portFile))
        {
            var existing = await TryReadPortFileAsync(portFile, ct).ConfigureAwait(false);
            if (existing is { } e && await ProbeAsync(e.Port, e.WsPath, ct).ConfigureAwait(false) is { } ws)
            {
                LoggerService.Log($"既に開いている Chrome に接続します (port={e.Port})", LogLevel.Info);
                return new ChromeInstance(null, ws);
            }
        }

        // 2) 新しく起動する。前回のポートファイルが残っていても読まないよう、起動時刻より新しいものだけを採る。
        var launchedAt = DateTime.UtcNow.AddSeconds(-1);
        var psi = new ProcessStartInfo(options.ChromePath)
        {
            UseShellExecute = false,
            CreateNoWindow = false,
        };
        foreach (var arg in BuildArgs(options)) psi.ArgumentList.Add(arg);

        var process = Process.Start(psi)
            ?? throw new InvalidOperationException($"Chrome の起動に失敗しました: {options.ChromePath}");

        try
        {
            var ws = await WaitForDevToolsAsync(portFile, launchedAt, process, ct).ConfigureAwait(false);
            LoggerService.Log($"Chrome 起動完了 (pid={process.Id})", LogLevel.Debug);
            return new ChromeInstance(process, ws);
        }
        catch
        {
            // 起動に失敗したプロセスだけを止める。プロファイルは削除しない。
            try { if (!process.HasExited) process.Kill(entireProcessTree: true); } catch { /* 起動失敗時の後始末 */ }
            process.Dispose();
            throw;
        }
    }

    /// <summary>
    /// 起動引数を組み立てる。
    /// </summary>
    /// <remarks>
    /// Google ログインを弾かれないよう、最小限に保つ（2026-11-13 の検証で手動ログインできた構成）。
    /// - --enable-automation などの自動化フラグは付けない（navigator.webdriver が true になり、ログインを拒否されうる）
    /// - 描画の間引き抑止フラグだけは付ける（ウィンドウが裏に回るとクリックが応答しなくなったため）
    /// - ウィンドウは画面内に普通に表示する（画面外配置はしない。利用者がログイン操作をするため）
    /// 変える場合は実画面でログインを再検証すること。
    /// </remarks>
    internal static List<string> BuildArgs(ChromeLaunchOptions options) =>
    [
        $"--user-data-dir={options.UserDataDir}",
        "--remote-debugging-port=0",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
        "--disable-background-timer-throttling",
        options.StartUrl,
    ];

    /// <summary>DevToolsActivePort（1 行目: ポート、2 行目: ws パス）を読む</summary>
    private static async Task<(int Port, string WsPath)?> TryReadPortFileAsync(string portFile, CancellationToken ct)
    {
        try
        {
            var lines = await File.ReadAllLinesAsync(portFile, ct).ConfigureAwait(false);
            if (lines.Length >= 2 && int.TryParse(lines[0], out var port) && port is > 0 and < 65536)
            {
                return (port, lines[1]);
            }
        }
        catch (IOException)
        {
            // Chrome 書き込み中の読み取り競合。呼び出し側で再試行する。
        }
        return null;
    }

    /// <summary>
    /// ポートが本当にこのプロファイルの Chrome の DevTools か確かめ、ブラウザの WebSocket URL を返す（違えば null）。
    /// </summary>
    /// <param name="port">DevToolsActivePort の 1 行目</param>
    /// <param name="expectedPath">DevToolsActivePort の 2 行目（/devtools/browser/&lt;id&gt;）。
    /// 古いポートファイルの番号を別の Chrome が使っている場合に、誤って接続しないよう照合する。</param>
    /// <param name="ct">中止</param>
    internal static async Task<Uri?> ProbeAsync(int port, string expectedPath, CancellationToken ct)
    {
        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(3) };
            var json = await http.GetStringAsync($"http://127.0.0.1:{port}/json/version", ct).ConfigureAwait(false);
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("webSocketDebuggerUrl", out var url) &&
                Uri.TryCreate(url.GetString(), UriKind.Absolute, out var uri) &&
                uri.Scheme == "ws" && (uri.Host == "127.0.0.1" || uri.Host == "localhost") &&
                string.Equals(uri.AbsolutePath, expectedPath.Trim(), StringComparison.Ordinal))
            {
                return uri;
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            // 応答なし・別のプログラム・古いポートファイル
        }
        return null;
    }

    /// <summary>起動後に書かれた DevToolsActivePort を待ち、接続先を確かめる</summary>
    private static async Task<Uri> WaitForDevToolsAsync(string portFile, DateTime launchedAt, Process process, CancellationToken ct)
    {
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(30);
        while (DateTime.UtcNow < deadline)
        {
            ct.ThrowIfCancellationRequested();
            if (process.HasExited)
            {
                throw new InvalidOperationException(
                    "Chrome が起動直後に終了しました。同じプロファイルの Chrome がデバッグポートなしで開いている可能性があります。" +
                    "そのウィンドウを閉じてから再実行してください。");
            }

            if (File.Exists(portFile) && File.GetLastWriteTimeUtc(portFile) >= launchedAt)
            {
                var entry = await TryReadPortFileAsync(portFile, ct).ConfigureAwait(false);
                if (entry is { } e && await ProbeAsync(e.Port, e.WsPath, ct).ConfigureAwait(false) is { } ws)
                {
                    return ws;
                }
            }

            await Task.Delay(250, ct).ConfigureAwait(false);
        }

        throw new TimeoutException("Chrome の DevTools が 30 秒以内に応答しませんでした（Chrome の起動失敗）");
    }
}
