namespace Edictora.Core.Dashboard;

/// <summary>
/// 編集画面の状態（書き込み可否の判断材料）。
/// </summary>
/// <param name="StatusText">「ステータス: …」の値（読めなければ null）</param>
/// <param name="DescriptionFound">説明欄（textarea）が 1 つ見つかったか</param>
/// <param name="DescriptionEditable">説明欄が disabled / readOnly でないか</param>
/// <param name="SaveButtonFound">「下書きとして保存する」ボタンが 1 つ見つかったか</param>
/// <param name="SaveButtonEnabled">そのボタンが押せる状態か</param>
public sealed record ItemPageState(
    string? StatusText,
    bool DescriptionFound,
    bool DescriptionEditable,
    bool SaveButtonFound,
    bool SaveButtonEnabled);

/// <summary>書き込み可否の判定結果</summary>
/// <param name="Allowed">書き込んでよいか</param>
/// <param name="Reason">不可の理由（可なら空）</param>
public sealed record WriteDecision(bool Allowed, string Reason);

/// <summary>
/// 下書き保存・審査送信をしてよい状態かを判定する純粋関数。
/// </summary>
/// <remarks>
/// 審査待ちのアイテムの編集画面はまだ実画面で確認していない（DESIGN.md「まだ確かめていないこと」）。
/// そのため「書いてよいと確認できた状態」以外はすべて止める（許可リスト方式）。
/// - ステータスが読めない → 止める
/// - 審査中を示す語を含む → 止める（審査の取り消しは絶対にしない）
/// - 既知の編集可能な状態（公開済み・下書きなど）でない → 止める
/// - 説明欄が編集不可、または保存ボタンが無い・押せない → 止める
/// 実画面で新しいステータス表記を確認したら、<see cref="EditableStatusKeywords"/> に足す。
/// </remarks>
public static class WriteGate
{
    /// <summary>審査中・ロック中を示す語（1 つでも含めば書き込まない）</summary>
    public static IReadOnlyList<string> LockedStatusKeywords { get; } =
        ["審査待ち", "審査中", "審査保留", "保留中", "Pending", "In review", "Under review"];

    /// <summary>書き込んでよいと判断できる状態の語</summary>
    public static IReadOnlyList<string> EditableStatusKeywords { get; } =
        ["公開済み", "下書き", "Published", "Draft"];

    /// <summary>書き込み可否を判定する</summary>
    public static WriteDecision Evaluate(ItemPageState state)
    {
        if (string.IsNullOrWhiteSpace(state.StatusText))
        {
            return Deny("アイテムのステータスを読み取れません。安全のため書き込みを止めます");
        }

        var status = state.StatusText.Trim();
        if (LockedStatusKeywords.Any(k => status.Contains(k, StringComparison.OrdinalIgnoreCase)))
        {
            return Deny($"ステータスが「{status}」のため書き込みません（審査の取り消しはしません）");
        }

        if (!EditableStatusKeywords.Any(k => status.Contains(k, StringComparison.OrdinalIgnoreCase)))
        {
            return Deny($"ステータス「{status}」は書き込みを確認済みの状態ではないため、安全のため止めます");
        }

        if (!state.DescriptionFound) return Deny("説明欄が見つからないため書き込みを止めます");
        if (!state.DescriptionEditable) return Deny("説明欄が編集できない状態のため書き込みを止めます");
        if (!state.SaveButtonFound) return Deny($"「{DashboardText.SaveDraftButton}」ボタンが見つからないため書き込みを止めます");
        if (!state.SaveButtonEnabled) return Deny($"「{DashboardText.SaveDraftButton}」ボタンが押せない状態のため書き込みを止めます");

        return new WriteDecision(true, "");
    }

    /// <summary>判定し、不可なら <see cref="WriteBlockedException"/> を投げる</summary>
    public static void EnsureAllowed(ItemPageState state)
    {
        var decision = Evaluate(state);
        if (!decision.Allowed) throw new WriteBlockedException(decision.Reason);
    }

    private static WriteDecision Deny(string reason) => new(false, reason);
}
