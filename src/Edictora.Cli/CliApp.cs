using System.Reflection;
using Edictora.Core.Config;
using Edictora.Core.Dashboard;
using Edictora.Core.Listing;
using Edictora.Core.Logging;
using Edictora.Core.Services;

namespace Edictora.Cli;

/// <summary>終了コード</summary>
internal static class ExitCodes
{
    public const int Success = 0;
    public const int Failure = 1;
    public const int Usage = 2;
    public const int Differences = 3;
    public const int Blocked = 4;
    public const int LoginRequired = 5;
    public const int Cancelled = 130;
}

/// <summary>
/// CLI のコマンド実行。手順の中身は Core の <see cref="ListingService"/> に任せ、ここは入出力と終了コードだけを扱う。
/// </summary>
/// <remarks>
/// 設定・引数の誤りは、Chrome を開く前にすべて検出して終了コード 2 で止める。
/// 画面へ出す文字列にも publisherId が混ざらないよう、エラーは LoggerService.Redact を通して出す。
/// </remarks>
internal sealed class CliApp(TextWriter stdout, TextWriter stderr)
{
    /// <summary>login の既定の待ち時間</summary>
    private static readonly TimeSpan DefaultLoginWait = TimeSpan.FromMinutes(10);

    /// <summary>引数を解釈して実行する</summary>
    public async Task<int> RunAsync(IReadOnlyList<string> args, CancellationToken ct)
    {
        CliCommand command;
        try
        {
            command = CliArguments.Parse(args);
        }
        catch (CliUsageException ex)
        {
            await stderr.WriteLineAsync($"エラー: {ex.Message}").ConfigureAwait(false);
            return ExitCodes.Usage;
        }

        switch (command.Kind)
        {
            case CliCommandKind.Help:
                await stdout.WriteLineAsync(CliArguments.HelpText).ConfigureAwait(false);
                return ExitCodes.Success;
            case CliCommandKind.Version:
                await stdout.WriteLineAsync($"edictora {GetVersion()}").ConfigureAwait(false);
                return ExitCodes.Success;
        }

        // ── ここから先は設定の検証（まだ Chrome は開かない）──
        EdictoraConfig config;
        IReadOnlyList<ExpectedItem> targets = [];
        try
        {
            config = ConfigLoader.Load(command.ConfigPath, command.ProfileDir, command.ChromePath,
                requireFile: command.Kind != CliCommandKind.Login);
            if (command.Kind != CliCommandKind.Login) targets = SelectTargets(config, command);
        }
        catch (Exception ex) when (ex is ConfigException or CliUsageException)
        {
            await stderr.WriteLineAsync($"エラー: {ex.Message}").ConfigureAwait(false);
            return ExitCodes.Usage;
        }

        // ── ブラウザ操作 ──
        try
        {
            return command.Kind switch
            {
                CliCommandKind.Login => await LoginAsync(config, command, ct).ConfigureAwait(false),
                CliCommandKind.Pull => await PullAsync(config, targets, ct).ConfigureAwait(false),
                CliCommandKind.Diff => await DiffAsync(config, targets, ct).ConfigureAwait(false),
                CliCommandKind.Push => command.Yes
                    ? await PushAsync(config, targets, ct).ConfigureAwait(false)
                    : await DiffAsync(config, targets, ct, pushHint: true).ConfigureAwait(false),
                CliCommandKind.Submit => await SubmitAsync(config, targets[0], ct).ConfigureAwait(false),
                _ => ExitCodes.Usage,
            };
        }
        catch (OperationCanceledException)
        {
            await stderr.WriteLineAsync("中断しました。").ConfigureAwait(false);
            return ExitCodes.Cancelled;
        }
        catch (LoginRequiredException ex)
        {
            await stderr.WriteLineAsync($"エラー: {ex.Message}").ConfigureAwait(false);
            return ExitCodes.LoginRequired;
        }
        catch (WriteBlockedException ex)
        {
            await stderr.WriteLineAsync($"中止: {LoggerService.Redact(ex.Message)}").ConfigureAwait(false);
            return ExitCodes.Blocked;
        }
        catch (Exception ex)
        {
            LoggerService.LogException($"{command.Kind} の実行に失敗", ex);
            await stderr.WriteLineAsync($"エラー: {LoggerService.Redact(ex.Message)}").ConfigureAwait(false);
            return ExitCodes.Failure;
        }
    }

    /// <summary>
    /// 対象アイテムを選び、正本を組み立てる（説明文をファイルから取り出す）。
    /// --item はすべて設定に存在する必要がある（打ち間違いで別のアイテムを触らないため）。
    /// </summary>
    internal static IReadOnlyList<ExpectedItem> SelectTargets(EdictoraConfig config, CliCommand command)
    {
        if (config.Items.Count == 0) throw new ConfigException($"設定ファイルに items がありません: {config.SourcePath}");

        IReadOnlyList<ItemConfig> items;
        if (command.ItemIds.Count > 0)
        {
            var list = new List<ItemConfig>();
            foreach (var id in command.ItemIds)
            {
                list.Add(config.FindItem(id) ?? throw new ConfigException($"設定ファイルにアイテム {id} がありません"));
            }
            items = list;
        }
        else
        {
            items = config.Items;
        }

        var result = new List<ExpectedItem>();
        foreach (var item in items)
        {
            var expected = ExpectedListingBuilder.Build(item);
            if (command.LocaleCodes.Count > 0)
            {
                if (command.ItemIds.Count > 0)
                {
                    // アイテムを明示したなら、指定ロケールが無いのは誤り
                    expected = expected.FilterLocales(command.LocaleCodes);
                }
                else
                {
                    // 全アイテムが対象なら、指定ロケールを持つアイテムだけを残す
                    var codes = command.LocaleCodes
                        .Where(c => expected.Locales.Any(l => string.Equals(l.Code, c, StringComparison.OrdinalIgnoreCase)))
                        .ToList();
                    if (codes.Count == 0) continue;
                    expected = expected.FilterLocales(codes);
                }
            }
            result.Add(expected);
        }

        if (result.Count == 0) throw new ConfigException($"指定したロケール（{string.Join(", ", command.LocaleCodes)}）を持つアイテムが設定にありません");
        return result;
    }

    private async Task<int> LoginAsync(EdictoraConfig config, CliCommand command, CancellationToken ct)
    {
        await stdout.WriteLineAsync($"Chrome を開きます（プロファイル: {config.ProfileDirectory}）").ConfigureAwait(false);
        await using var browser = await DashboardBrowser.OpenAsync(new DashboardBrowserOptions(config.ProfileDirectory, config.ChromePath), ct).ConfigureAwait(false);
        await browser.WaitForLoginAsync(command.LoginWait ?? DefaultLoginWait, Progress(), ct).ConfigureAwait(false);
        await stdout.WriteLineAsync("ログイン状態をプロファイルに保存しました。以後は pull / diff / push が使えます。").ConfigureAwait(false);
        return ExitCodes.Success;
    }

    private async Task<int> PullAsync(EdictoraConfig config, IReadOnlyList<ExpectedItem> targets, CancellationToken ct)
    {
        await using var browser = await OpenLoggedInAsync(config, ct).ConfigureAwait(false);
        var page = browser.CreateItemPage();
        var service = new ListingService();
        var failed = false;

        foreach (var expected in targets)
        {
            var snapshot = await service.PullAsync(page, expected, Progress(), ct).ConfigureAwait(false);
            await stdout.WriteLineAsync(ItemHeader(expected, snapshot)).ConfigureAwait(false);
            if (snapshot.Error is not null)
            {
                failed = true;
                await stdout.WriteLineAsync($"  エラー: {snapshot.Error}").ConfigureAwait(false);
                continue;
            }
            foreach (var locale in expected.Locales)
            {
                var current = snapshot.Find(locale.Code);
                if (current?.Error is not null || current is null)
                {
                    failed = true;
                    await stdout.WriteLineAsync($"-- [{locale.Code}] エラー: {current?.Error ?? "読み出せませんでした"}").ConfigureAwait(false);
                    continue;
                }
                foreach (var field in locale.Fields)
                {
                    var value = current.Values.GetValueOrDefault(field) ?? "";
                    await stdout.WriteLineAsync(field == ListingField.Description
                        ? $"-- [{locale.Code}] {field.DisplayName()}（{value.Length:N0} 文字）"
                        : $"-- [{locale.Code}] {field.DisplayName()}").ConfigureAwait(false);
                    await stdout.WriteLineAsync(value).ConfigureAwait(false);
                }
            }
            await stdout.WriteLineAsync().ConfigureAwait(false);
        }
        return failed ? ExitCodes.Failure : ExitCodes.Success;
    }

    private async Task<int> DiffAsync(EdictoraConfig config, IReadOnlyList<ExpectedItem> targets, CancellationToken ct, bool pushHint = false)
    {
        await using var browser = await OpenLoggedInAsync(config, ct).ConfigureAwait(false);
        var page = browser.CreateItemPage();
        var service = new ListingService();
        var anyDiff = false;
        var anyError = false;

        foreach (var expected in targets)
        {
            var (snapshot, comparisons) = await service.DiffAsync(page, expected, Progress(), ct).ConfigureAwait(false);
            await stdout.WriteLineAsync(ItemHeader(expected, snapshot)).ConfigureAwait(false);
            foreach (var row in comparisons)
            {
                await stdout.WriteAsync(DiffFormatter.Format(row)).ConfigureAwait(false);
                anyDiff |= row.Status == ComparisonStatus.Different;
                anyError |= row.Status is ComparisonStatus.Error or ComparisonStatus.NotFetched;
            }
            await stdout.WriteLineAsync().ConfigureAwait(false);
        }

        if (pushHint && anyDiff)
        {
            await stdout.WriteLineAsync("差分のある欄を書き換えるには、--yes を付けて push を再実行してください（下書き保存まで行い、審査には送信しません）。").ConfigureAwait(false);
        }
        return anyError ? ExitCodes.Failure : anyDiff ? ExitCodes.Differences : ExitCodes.Success;
    }

    private async Task<int> PushAsync(EdictoraConfig config, IReadOnlyList<ExpectedItem> targets, CancellationToken ct)
    {
        await using var browser = await OpenLoggedInAsync(config, ct).ConfigureAwait(false);
        var page = browser.CreateItemPage();
        var service = new ListingService();

        // 1 件でも失敗したら残りは行わない（例外がそのまま RunAsync へ上がる）
        foreach (var expected in targets)
        {
            var result = await service.PushAsync(page, expected, Progress(), ct).ConfigureAwait(false);
            await stdout.WriteLineAsync($"== {result.ItemName} ({result.ItemId})").ConfigureAwait(false);
            foreach (var locale in result.Locales)
            {
                var line = locale.Outcome == LocalePushOutcome.Saved
                    ? $"  [{locale.Locale}] 下書き保存・照合済み: {string.Join("・", locale.UpdatedFields.Select(f => f.DisplayName()))}"
                    : $"  [{locale.Locale}] 変更なし（正本と一致）";
                await stdout.WriteLineAsync(line).ConfigureAwait(false);
            }
        }
        await stdout.WriteLineAsync("完了しました。審査への送信は行っていません（必要なら submit を 1 件ずつ実行してください）。").ConfigureAwait(false);
        return ExitCodes.Success;
    }

    private async Task<int> SubmitAsync(EdictoraConfig config, ExpectedItem target, CancellationToken ct)
    {
        await using var browser = await OpenLoggedInAsync(config, ct).ConfigureAwait(false);
        var page = browser.CreateItemPage();
        var service = new ListingService();

        var result = await service.SubmitAsync(page, target.ItemId, async (dialog, token) =>
        {
            // --yes で同意済み。確認ダイアログの内容は記録として表示する。
            await stdout.WriteLineAsync("確認ダイアログ:").ConfigureAwait(false);
            await stdout.WriteLineAsync(Indent(dialog.Text)).ConfigureAwait(false);
            await stdout.WriteLineAsync($"  自動公開: {AutoPublishText(dialog.AutoPublish)}（変更しません）").ConfigureAwait(false);
            return true;
        }, Progress(), ct).ConfigureAwait(false);

        await stdout.WriteLineAsync($"== {target.DisplayName} ({target.ItemId})").ConfigureAwait(false);
        await stdout.WriteLineAsync($"  結果: {result.Message ?? "（メッセージなし）"}").ConfigureAwait(false);
        await stdout.WriteLineAsync($"  ステータス: {result.StatusText ?? "不明"}").ConfigureAwait(false);
        await stdout.WriteLineAsync($"  自動公開: {AutoPublishText(result.Dialog.AutoPublish)}").ConfigureAwait(false);
        return ExitCodes.Success;
    }

    private static async Task<DashboardBrowser> OpenLoggedInAsync(EdictoraConfig config, CancellationToken ct)
    {
        var browser = await DashboardBrowser.OpenAsync(new DashboardBrowserOptions(config.ProfileDirectory, config.ChromePath), ct).ConfigureAwait(false);
        try
        {
            await browser.EnsureLoggedInAsync(ct).ConfigureAwait(false);
            return browser;
        }
        catch
        {
            await browser.DisposeAsync().ConfigureAwait(false);
            throw;
        }
    }

    /// <summary>進捗は標準エラーへ（標準出力は結果だけにして、スクリプトから扱いやすくする）</summary>
    private IProgress<string> Progress() => new SyncProgress(m => stderr.WriteLine(LoggerService.Redact(m)));

    private static string ItemHeader(ExpectedItem item, ItemSnapshot snapshot)
    {
        var status = snapshot.StatusText is null ? "" : $"  ステータス: {snapshot.StatusText}";
        var locales = snapshot.AvailableLocales.Count == 0 ? "" : $"  言語: {string.Join(", ", snapshot.AvailableLocales)}";
        return $"== {item.DisplayName} ({item.ItemId}){status}{locales}";
    }

    internal static string AutoPublishText(bool? value) => value switch
    {
        true => "する",
        false => "しない",
        null => "不明",
    };

    private static string Indent(string text) => string.Join('\n', text.Split('\n').Select(l => "  " + l));

    private static string GetVersion()
        => typeof(CliApp).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion?.Split('+')[0]
           ?? typeof(CliApp).Assembly.GetName().Version?.ToString() ?? "不明";

    /// <summary>
    /// 同期的に呼ばれる IProgress（Progress&lt;T&gt; は SynchronizationContext 経由で遅れて出るため、CLI では使わない）
    /// </summary>
    private sealed class SyncProgress(Action<string> report) : IProgress<string>
    {
        public void Report(string value) => report(value);
    }
}
