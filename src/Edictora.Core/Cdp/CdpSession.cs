using System.Text.Json;
using System.Text.Json.Nodes;

namespace Edictora.Core.Cdp;

/// <summary>
/// flatten モードで attach した個別ターゲット（page / service_worker 等）への
/// sessionId 付きコマンド送信ラッパー。
/// </summary>
public sealed class CdpSession(CdpClient client, string sessionId, string targetId)
{
    /// <summary>flatten セッション ID</summary>
    public string SessionId { get; } = sessionId;

    /// <summary>attach 元のターゲット ID</summary>
    public string TargetId { get; } = targetId;

    /// <summary>親クライアント</summary>
    public CdpClient Client { get; } = client;

    /// <summary>このセッション宛てに CDP コマンドを送信する</summary>
    public Task<JsonElement> SendAsync(
        string method,
        JsonNode? parameters = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default)
        => Client.SendAsync(method, parameters, SessionId, timeout, ct);

    /// <summary>このセッションのイベントを待機する</summary>
    public Task<JsonElement> WaitForEventAsync(
        string method,
        Func<JsonElement, bool>? predicate = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default)
        => Client.WaitForEventAsync(method, SessionId, predicate, timeout, ct);
}
