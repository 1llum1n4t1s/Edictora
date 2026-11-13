using Edictora.Core.Dashboard;
using Edictora.Core.Listing;
using Edictora.Core.Logging;

namespace Edictora.Core.Services;

/// <summary>ロケール 1 件の書き換え結果</summary>
public enum LocalePushOutcome
{
    /// <summary>差分がなく、何もしなかった</summary>
    Unchanged,

    /// <summary>書き換えて下書き保存し、読み直して一致を確認した</summary>
    Saved,
}

/// <summary>ロケール 1 件の書き換え結果</summary>
/// <param name="Locale">ロケール</param>
/// <param name="Outcome">結果</param>
/// <param name="UpdatedFields">書き換えた欄</param>
public sealed record LocalePushResult(string Locale, LocalePushOutcome Outcome, IReadOnlyList<ListingField> UpdatedFields);

/// <summary>アイテム 1 件の書き換え結果</summary>
/// <param name="ItemId">アイテム ID</param>
/// <param name="ItemName">表示名</param>
/// <param name="Locales">完了したロケール</param>
/// <param name="FinalComparison">最後に読み直して照合した結果</param>
public sealed record PushResult(
    string ItemId,
    string ItemName,
    IReadOnlyList<LocalePushResult> Locales,
    IReadOnlyList<LocaleComparison> FinalComparison);

/// <summary>
/// 掲載情報の読み出し・比較・書き換え・審査送信の手順。CLI と UI の両方がこれを使う。
/// </summary>
/// <remarks>
/// 画面操作そのものは <see cref="IListingPage"/> に任せ、ここでは「何をどの順でやり、どこで止めるか」だけを決める。
/// 安全策:
/// - 書き込みの前に <see cref="WriteGate"/> で状態を確かめ、確認できない状態なら止める（審査の取り消しはしない）
/// - ロケールごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行う（複数言語をまとめて保存する手順は未検証のため）
/// - 途中で 1 つでも失敗したら、そのアイテムの残りは行わない（未保存の変更を次の保存に巻き込まないため）
/// - 最後にもう一度読み込み直して、全対象欄が正本と一致するか確かめる
/// </remarks>
public sealed class ListingService
{
    /// <summary>
    /// 現在値を読み出す（読み取りのみ）。ロケール単位の失敗はそのロケールのエラーとして返し、他は続ける。
    /// </summary>
    public async Task<ItemSnapshot> PullAsync(IListingPage page, ExpectedItem expected, IProgress<string>? progress, CancellationToken ct)
    {
        progress?.Report($"{expected.DisplayName}: 編集画面を開いています…");
        try
        {
            await page.OpenItemAsync(expected.ItemId, ct).ConfigureAwait(false);
            var state = await page.GetStateAsync(ct).ConfigureAwait(false);
            var available = await page.GetLocalesAsync(ct).ConfigureAwait(false);

            var locales = new List<LocaleSnapshot>();
            foreach (var locale in expected.Locales)
            {
                ct.ThrowIfCancellationRequested();
                if (!available.Contains(locale.Code, StringComparer.OrdinalIgnoreCase))
                {
                    locales.Add(new LocaleSnapshot(locale.Code, new Dictionary<ListingField, string>(),
                        $"ダッシュボードにロケール {locale.Code} がありません（ある言語: {string.Join(", ", available)}）"));
                    continue;
                }

                progress?.Report($"{expected.DisplayName}: [{locale.Code}] を読み出しています…");
                try
                {
                    await page.SwitchLocaleAsync(locale.Code, ct).ConfigureAwait(false);
                    var values = await page.ReadFieldsAsync(locale.Fields, ct).ConfigureAwait(false);
                    locales.Add(new LocaleSnapshot(locale.Code, values));
                }
                catch (DashboardException ex)
                {
                    LoggerService.Log($"読み出し失敗 {expected.ItemId} [{locale.Code}]: {ex.Message}", LogLevel.Warning);
                    locales.Add(new LocaleSnapshot(locale.Code, new Dictionary<ListingField, string>(), ex.Message));
                }
            }
            return new ItemSnapshot(expected.ItemId, state.StatusText, available, locales);
        }
        catch (LoginRequiredException)
        {
            throw;
        }
        catch (DashboardException ex)
        {
            LoggerService.Log($"読み出し失敗 {expected.ItemId}: {ex.Message}", LogLevel.Warning);
            return new ItemSnapshot(expected.ItemId, null, [], [], ex.Message);
        }
    }

    /// <summary>読み出して比較する（読み取りのみ）</summary>
    public async Task<(ItemSnapshot Snapshot, IReadOnlyList<LocaleComparison> Comparison)> DiffAsync(
        IListingPage page, ExpectedItem expected, IProgress<string>? progress, CancellationToken ct)
    {
        var snapshot = await PullAsync(page, expected, progress, ct).ConfigureAwait(false);
        return (snapshot, ListingComparer.Compare(expected, snapshot));
    }

    /// <summary>
    /// 差分のある欄だけを書き換えて下書き保存し、読み直して照合する。
    /// 失敗したら例外（<see cref="WriteBlockedException"/> / <see cref="DashboardException"/>）。
    /// </summary>
    public async Task<PushResult> PushAsync(IListingPage page, ExpectedItem expected, IProgress<string>? progress, CancellationToken ct)
    {
        try
        {
            return await PushCoreAsync(page, expected, progress, ct).ConfigureAwait(false);
        }
        catch (DashboardException ex) when (ex is not LoginRequiredException)
        {
            var shot = await page.CaptureDiagnosticsAsync($"push-{expected.ItemId}", CancellationToken.None).ConfigureAwait(false);
            LoggerService.Log($"push 失敗 {expected.ItemId}: {ex.Message}" + (shot is null ? "" : $"（画面: {shot}）"), LogLevel.Error);
            if (shot is not null) progress?.Report($"失敗時の画面を保存しました: {shot}");
            throw;
        }
    }

    private async Task<PushResult> PushCoreAsync(IListingPage page, ExpectedItem expected, IProgress<string>? progress, CancellationToken ct)
    {
        progress?.Report($"{expected.DisplayName}: 編集画面を開いています…");
        await page.OpenItemAsync(expected.ItemId, ct).ConfigureAwait(false);
        WriteGate.EnsureAllowed(await page.GetStateAsync(ct).ConfigureAwait(false));

        var available = await page.GetLocalesAsync(ct).ConfigureAwait(false);
        var missing = expected.Locales.Where(l => !available.Contains(l.Code, StringComparer.OrdinalIgnoreCase)).Select(l => l.Code).ToList();
        if (missing.Count > 0)
        {
            throw new DashboardException(
                $"ダッシュボードにロケール {string.Join(", ", missing)} がありません（ある言語: {string.Join(", ", available)}）。何も書き換えずに止めます");
        }

        var results = new List<LocalePushResult>();
        foreach (var locale in expected.Locales)
        {
            ct.ThrowIfCancellationRequested();
            progress?.Report($"{expected.DisplayName}: [{locale.Code}] の現在値を確認しています…");
            await page.SwitchLocaleAsync(locale.Code, ct).ConfigureAwait(false);
            var current = await page.ReadFieldsAsync(locale.Fields, ct).ConfigureAwait(false);
            var targets = locale.Fields
                .Where(f => ListingComparer.CompareValue(locale.Values[f], current.GetValueOrDefault(f)) != ComparisonStatus.Match)
                .ToList();
            if (targets.Count == 0)
            {
                progress?.Report($"{expected.DisplayName}: [{locale.Code}] は正本と一致しています");
                results.Add(new LocalePushResult(locale.Code, LocalePushOutcome.Unchanged, []));
                continue;
            }

            foreach (var field in targets)
            {
                progress?.Report($"{expected.DisplayName}: [{locale.Code}] {field.DisplayName()}を書き換えています…");
                await page.WriteFieldAsync(field, locale.Values[field], ct).ConfigureAwait(false);
            }

            // 保存前に、このロケールの全対象欄が正本どおりか確かめる（書き換えていない欄が動いていないかも含む）
            EnsureMatches(locale, await page.ReadFieldsAsync(locale.Fields, ct).ConfigureAwait(false), "保存前の照合");

            // 保存の直前にも状態を確かめる
            WriteGate.EnsureAllowed(await page.GetStateAsync(ct).ConfigureAwait(false));
            progress?.Report($"{expected.DisplayName}: [{locale.Code}] を下書き保存しています…");
            await page.SaveDraftAsync(ct).ConfigureAwait(false);

            // 読み込み直して、保存された値を確かめる
            await page.ReloadAsync(ct).ConfigureAwait(false);
            await page.SwitchLocaleAsync(locale.Code, ct).ConfigureAwait(false);
            EnsureMatches(locale, await page.ReadFieldsAsync(locale.Fields, ct).ConfigureAwait(false), "保存後の照合");
            progress?.Report($"{expected.DisplayName}: [{locale.Code}] を保存し、読み直して一致を確認しました");
            results.Add(new LocalePushResult(locale.Code, LocalePushOutcome.Saved, targets));
        }

        // 最後にもう一度読み込み直し、全対象ロケールを照合する（あるロケールの保存が他に影響していないか）
        IReadOnlyList<LocaleComparison> final = [];
        if (results.Any(r => r.Outcome == LocalePushOutcome.Saved))
        {
            progress?.Report($"{expected.DisplayName}: 全言語を読み直して照合しています…");
            await page.ReloadAsync(ct).ConfigureAwait(false);
            var snapshots = new List<LocaleSnapshot>();
            foreach (var locale in expected.Locales)
            {
                await page.SwitchLocaleAsync(locale.Code, ct).ConfigureAwait(false);
                snapshots.Add(new LocaleSnapshot(locale.Code, await page.ReadFieldsAsync(locale.Fields, ct).ConfigureAwait(false)));
            }
            final = ListingComparer.Compare(expected, new ItemSnapshot(expected.ItemId, null, available, snapshots));
            var bad = final.Where(c => c.Status != ComparisonStatus.Match).ToList();
            if (bad.Count > 0)
            {
                throw new DashboardException(
                    $"最終照合で正本と一致しない欄があります: {string.Join(", ", bad.Select(b => $"[{b.Locale}] {string.Join("・", b.DifferentFields.Select(f => f.DisplayName()))}"))}");
            }
        }
        else
        {
            final = ListingComparer.Compare(expected, new ItemSnapshot(expected.ItemId, null, available,
                [.. expected.Locales.Select(l => new LocaleSnapshot(l.Code, l.Values))]));
        }

        return new PushResult(expected.ItemId, expected.DisplayName, results, final);
    }

    /// <summary>
    /// 審査に送信する（1 アイテムずつ）。送信前に書き込み可否を確かめ、確認ダイアログの内容を <paramref name="confirm"/> に渡す。
    /// </summary>
    public async Task<SubmitResult> SubmitAsync(
        IListingPage page,
        string itemId,
        Func<SubmitDialogInfo, CancellationToken, Task<bool>> confirm,
        IProgress<string>? progress,
        CancellationToken ct)
    {
        try
        {
            progress?.Report($"{itemId}: 編集画面を開いています…");
            await page.OpenItemAsync(itemId, ct).ConfigureAwait(false);
            WriteGate.EnsureAllowed(await page.GetStateAsync(ct).ConfigureAwait(false));
            progress?.Report($"{itemId}: 審査送信の確認ダイアログを開いています…");
            return await page.SubmitForReviewAsync(confirm, ct).ConfigureAwait(false);
        }
        catch (DashboardException ex) when (ex is not LoginRequiredException and not WriteBlockedException)
        {
            var shot = await page.CaptureDiagnosticsAsync($"submit-{itemId}", CancellationToken.None).ConfigureAwait(false);
            LoggerService.Log($"submit 失敗 {itemId}: {ex.Message}" + (shot is null ? "" : $"（画面: {shot}）"), LogLevel.Error);
            if (shot is not null) progress?.Report($"失敗時の画面を保存しました: {shot}");
            throw;
        }
    }

    private static void EnsureMatches(ExpectedLocale locale, IReadOnlyDictionary<ListingField, string> values, string phase)
    {
        var bad = locale.Fields
            .Where(f => ListingComparer.CompareValue(locale.Values[f], values.GetValueOrDefault(f)) != ComparisonStatus.Match)
            .ToList();
        if (bad.Count > 0)
        {
            throw new DashboardException(
                $"[{locale.Code}] {phase}で正本と一致しない欄があります: {string.Join("・", bad.Select(f => f.DisplayName()))}");
        }
    }
}
