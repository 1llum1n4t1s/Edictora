using System.Collections.Concurrent;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Edictora.Core.Logging;

namespace Edictora.Core.Cdp;

/// <summary>CDP イベント（method + params + 発生元 sessionId）</summary>
public sealed record CdpEvent(string Method, JsonElement Params, string? SessionId);

/// <summary>CDP コマンドがエラー応答を返したときの例外</summary>
public sealed class CdpException(string message) : Exception(message);

/// <summary>
/// Chrome DevTools Protocol クライアント。
/// ブラウザレベルの WebSocket エンドポイントに接続し、flatten モードの
/// sessionId ルーティングでコマンド送信・イベント受信を行う。
/// </summary>
public sealed class CdpClient : IAsyncDisposable
{
    private readonly ClientWebSocket _ws;
    private readonly CancellationTokenSource _cts = new();
    private readonly ConcurrentDictionary<long, TaskCompletionSource<JsonElement>> _pending = new();
    private readonly SemaphoreSlim _sendLock = new(1, 1);
    private Task? _receiveLoop;
    private long _nextId;
    private int _disposed;

    /// <summary>CDP イベント受信時に発火する。受信ループスレッドから同期的に呼ばれるため、ハンドラ内で長時間ブロックしないこと。</summary>
    public event Action<CdpEvent>? EventReceived;

    /// <summary>接続が生きているか（利用者が Chrome を閉じると false になる）</summary>
    public bool IsConnected => Volatile.Read(ref _disposed) == 0 && _ws.State == WebSocketState.Open &&
                               _receiveLoop is { IsCompleted: false };

    private CdpClient(ClientWebSocket ws)
    {
        _ws = ws;
    }

    /// <summary>ブラウザレベルの WebSocket エンドポイントに接続する</summary>
    public static async Task<CdpClient> ConnectAsync(Uri wsUri, CancellationToken ct = default)
    {
        var ws = new ClientWebSocket();
        ws.Options.KeepAliveInterval = TimeSpan.FromSeconds(15);
        await ws.ConnectAsync(wsUri, ct).ConfigureAwait(false);
        var client = new CdpClient(ws);
        client._receiveLoop = Task.Run(client.ReceiveLoopAsync, CancellationToken.None);
        return client;
    }

    /// <summary>
    /// CDP コマンドを送信して result を返す。エラー応答は <see cref="CdpException"/>。
    /// </summary>
    /// <param name="method">CDP メソッド名（例: "Target.getTargets"）</param>
    /// <param name="parameters">パラメータ（<see cref="JsonObject"/> 等の <see cref="JsonNode"/>、null で省略）</param>
    /// <param name="sessionId">flatten セッション ID（ブラウザレベルなら null）</param>
    /// <param name="timeout">応答待ちタイムアウト（既定 30 秒）</param>
    public async Task<JsonElement> SendAsync(
        string method,
        JsonNode? parameters = null,
        string? sessionId = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _disposed) != 0, this);

        var id = Interlocked.Increment(ref _nextId);
        var tcs = new TaskCompletionSource<JsonElement>(TaskCreationOptions.RunContinuationsAsynchronously);
        _pending[id] = tcs;

        var payload = new JsonObject { ["id"] = id, ["method"] = method };
        if (parameters is not null) payload["params"] = parameters;
        if (sessionId is not null) payload["sessionId"] = sessionId;
        var bytes = Encoding.UTF8.GetBytes(payload.ToJsonString());

        try
        {
            await _sendLock.WaitAsync(ct).ConfigureAwait(false);
            try
            {
                await _ws.SendAsync(bytes, WebSocketMessageType.Text, endOfMessage: true, ct).ConfigureAwait(false);
            }
            finally
            {
                _sendLock.Release();
            }

            using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(ct, _cts.Token);
            timeoutCts.CancelAfter(timeout ?? TimeSpan.FromSeconds(30));
            await using var reg = timeoutCts.Token.Register(
                () => tcs.TrySetException(new TimeoutException($"CDP コマンド {method} の応答待ちがタイムアウトまたはキャンセルされました")));
            return await tcs.Task.ConfigureAwait(false);
        }
        finally
        {
            _pending.TryRemove(id, out _);
        }
    }

    /// <summary>
    /// 指定した CDP イベントの発生を待つ。predicate を渡すと一致するイベントのみ対象。
    /// </summary>
    public async Task<JsonElement> WaitForEventAsync(
        string method,
        string? sessionId = null,
        Func<JsonElement, bool>? predicate = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default)
    {
        var tcs = new TaskCompletionSource<JsonElement>(TaskCreationOptions.RunContinuationsAsynchronously);

        void Handler(CdpEvent e)
        {
            if (e.Method != method) return;
            if (sessionId is not null && e.SessionId != sessionId) return;
            try
            {
                if (predicate is not null && !predicate(e.Params)) return;
            }
            catch
            {
                return;
            }
            tcs.TrySetResult(e.Params);
        }

        EventReceived += Handler;
        try
        {
            using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(ct, _cts.Token);
            timeoutCts.CancelAfter(timeout ?? TimeSpan.FromSeconds(30));
            await using var reg = timeoutCts.Token.Register(
                () => tcs.TrySetException(new TimeoutException($"CDP イベント {method} の待機がタイムアウトまたはキャンセルされました")));
            return await tcs.Task.ConfigureAwait(false);
        }
        finally
        {
            EventReceived -= Handler;
        }
    }

    /// <summary>受信ループ。メッセージを蓄積して応答・イベントに振り分ける。</summary>
    private async Task ReceiveLoopAsync()
    {
        var buffer = new byte[64 * 1024];
        using var ms = new MemoryStream();
        try
        {
            while (!_cts.IsCancellationRequested && _ws.State == WebSocketState.Open)
            {
                ms.SetLength(0);
                WebSocketReceiveResult result;
                do
                {
                    result = await _ws.ReceiveAsync(buffer, _cts.Token).ConfigureAwait(false);
                    if (result.MessageType == WebSocketMessageType.Close) return;
                    ms.Write(buffer, 0, result.Count);
                } while (!result.EndOfMessage);

                HandleMessage(new ReadOnlyMemory<byte>(ms.GetBuffer(), 0, (int)ms.Length));
            }
        }
        catch (OperationCanceledException)
        {
            // DisposeAsync 経由の正常終了
        }
        catch (WebSocketException ex)
        {
            LoggerService.Log($"CDP 受信ループが切断されました: {ex.Message}", LogLevel.Warning);
        }
        catch (Exception ex)
        {
            LoggerService.LogException("CDP 受信ループで予期しないエラー", ex);
        }
        finally
        {
            FailAllPending();
        }
    }

    /// <summary>受信メッセージ 1 件を応答（id あり）またはイベント（method あり）として処理する</summary>
    private void HandleMessage(ReadOnlyMemory<byte> data)
    {
        using var doc = JsonDocument.Parse(data);
        var root = doc.RootElement;

        if (root.TryGetProperty("id", out var idProp))
        {
            if (!_pending.TryRemove(idProp.GetInt64(), out var tcs)) return;
            if (root.TryGetProperty("error", out var err))
            {
                var message = err.TryGetProperty("message", out var msg) ? msg.GetString() : err.ToString();
                tcs.TrySetException(new CdpException(message ?? "CDP error"));
            }
            else
            {
                // JsonDocument は using で破棄されるため Clone して寿命を切り離す
                tcs.TrySetResult(root.TryGetProperty("result", out var res) ? res.Clone() : default);
            }
        }
        else if (root.TryGetProperty("method", out var methodProp))
        {
            var evt = new CdpEvent(
                methodProp.GetString()!,
                root.TryGetProperty("params", out var p) ? p.Clone() : default,
                root.TryGetProperty("sessionId", out var s) ? s.GetString() : null);
            try
            {
                EventReceived?.Invoke(evt);
            }
            catch (Exception ex)
            {
                LoggerService.Log($"CDP イベントハンドラで例外: {ex.Message}", LogLevel.Warning);
            }
        }
    }

    /// <summary>接続断で未完了の応答待ちをすべて失敗させる</summary>
    private void FailAllPending()
    {
        foreach (var kv in _pending)
        {
            if (_pending.TryRemove(kv.Key, out var tcs))
            {
                tcs.TrySetException(new CdpException("CDP 接続が閉じられました"));
            }
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;
        await _cts.CancelAsync().ConfigureAwait(false);
        try
        {
            if (_ws.State == WebSocketState.Open)
            {
                using var closeCts = new CancellationTokenSource(TimeSpan.FromSeconds(2));
                await _ws.CloseOutputAsync(WebSocketCloseStatus.NormalClosure, "bye", closeCts.Token).ConfigureAwait(false);
            }
        }
        catch
        {
            // 切断時の失敗は無視（プロセスごと落とすため）
        }
        if (_receiveLoop is not null)
        {
            try { await _receiveLoop.WaitAsync(TimeSpan.FromSeconds(3)).ConfigureAwait(false); }
            catch { /* 受信ループの後始末失敗は無視 */ }
        }
        _ws.Dispose();
        _cts.Dispose();
        _sendLock.Dispose();
    }
}
