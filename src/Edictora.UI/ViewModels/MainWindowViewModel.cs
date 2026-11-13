using System.Collections.ObjectModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Edictora.Core;
using Edictora.Core.Config;
using Edictora.Core.Dashboard;
using Edictora.Core.Listing;
using Edictora.Core.Logging;
using Edictora.Core.Services;
using Edictora.UI.Services;

namespace Edictora.UI.ViewModels;

/// <summary>
/// メインウィンドウの ViewModel。
/// </summary>
/// <remarks>
/// 画面操作の手順（読み出し・比較・書き換え・送信）は Core の <see cref="ListingService"/> にあり、
/// ここでは「どの行を対象にするか」「結果をどう表示するか」「確認を取るか」だけを扱う。
/// ブラウザ操作は同時に 1 つだけ行う（<see cref="IsBusy"/> で直列化）。
/// </remarks>
public sealed partial class MainWindowViewModel : ObservableObject, IAsyncDisposable
{
    private readonly SettingsService _settings;
    private readonly DashboardSessionManager _session = new();
    private readonly ListingService _service = new();
    private readonly Dictionary<string, ExpectedItem> _expected = new(StringComparer.Ordinal);
    private EdictoraConfig? _config;
    private CancellationTokenSource? _cts;

    /// <summary>確認ダイアログ（View から注入。title, message → OK なら true）</summary>
    public Func<string, string, Task<bool>>? ConfirmAsync { get; set; }

    /// <summary>設定ファイル選択ダイアログ（View から注入）</summary>
    public Func<Task<string?>>? PickConfigFileAsync { get; set; }

    /// <summary>一覧（アイテム × ロケール）</summary>
    public ObservableCollection<ListingRowViewModel> Rows { get; } = [];

    /// <summary>ログ表示</summary>
    public ObservableCollection<string> LogLines { get; } = [];

    /// <summary>設定ファイルのパス</summary>
    [ObservableProperty]
    public partial string ConfigPath { get; set; } = "";

    /// <summary>設定の要約（プロファイルの場所など）</summary>
    [ObservableProperty]
    public partial string ConfigSummary { get; set; } = "";

    /// <summary>選択中の行</summary>
    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(SubmitSelectedCommand))]
    public partial ListingRowViewModel? SelectedRow { get; set; }

    /// <summary>進捗の表示</summary>
    [ObservableProperty]
    public partial string ProgressText { get; set; } = "設定ファイルを読み込んでください";

    /// <summary>ブラウザ操作中か</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsIdle))]
    [NotifyCanExecuteChangedFor(nameof(LoginCommand), nameof(FetchCommand), nameof(PushCheckedCommand),
        nameof(SubmitSelectedCommand), nameof(ReloadConfigCommand), nameof(BrowseConfigCommand), nameof(CancelCommand))]
    public partial bool IsBusy { get; set; }

    /// <summary>操作できる状態か</summary>
    public bool IsIdle => !IsBusy;

    public MainWindowViewModel(SettingsService settings)
    {
        _settings = settings;
        ConfigPath = settings.Current.ConfigPath ?? AppPaths.DefaultConfigPath;
        LoadConfig();
    }

    // ───────── 設定 ─────────

    [RelayCommand(CanExecute = nameof(IsIdle))]
    private void ReloadConfig() => LoadConfig();

    [RelayCommand(CanExecute = nameof(IsIdle))]
    private async Task BrowseConfigAsync()
    {
        if (PickConfigFileAsync is null) return;
        var path = await PickConfigFileAsync();
        if (path is null) return;
        ConfigPath = path;
        _settings.MutateAndSave(s => s.ConfigPath = string.Equals(path, AppPaths.DefaultConfigPath, StringComparison.OrdinalIgnoreCase) ? null : path);
        LoadConfig();
    }

    /// <summary>設定を読み込み、一覧を作り直す（ブラウザは開かない）</summary>
    private void LoadConfig()
    {
        Rows.Clear();
        _expected.Clear();
        SelectedRow = null;
        try
        {
            _config = ConfigLoader.Load(ConfigPath);
        }
        catch (ConfigException ex)
        {
            _config = null;
            ConfigSummary = "";
            ProgressText = ex.Message;
            AppendLog($"設定エラー: {ex.Message}");
            return;
        }

        foreach (var item in _config.Items)
        {
            try
            {
                var expected = ExpectedListingBuilder.Build(item);
                _expected[item.ItemId] = expected;
                foreach (var locale in expected.Locales) Rows.Add(new ListingRowViewModel(expected, locale));
            }
            catch (ConfigException ex)
            {
                AppendLog($"{item.DisplayName}: {ex.Message}");
            }
        }

        ConfigSummary = $"アイテム {_config.Items.Count} 件 / プロファイル: {_config.ProfileDirectory}";
        ProgressText = Rows.Count > 0 ? "「現在値を取得」でダッシュボードの値を読み出します" : "対象がありません";
        SelectedRow = Rows.FirstOrDefault();
    }

    // ───────── ブラウザ操作 ─────────

    [RelayCommand(CanExecute = nameof(IsIdle))]
    private Task LoginAsync() => RunAsync("ログイン", async (options, progress, ct) =>
    {
        await _session.LoginAsync(options, TimeSpan.FromMinutes(10), progress, ct);
        progress.Report("ログインを確認しました。「現在値を取得」が使えます。");
    }, requireLogin: false);

    [RelayCommand(CanExecute = nameof(IsIdle))]
    private Task FetchAsync() => RunAsync("現在値の取得", async (options, progress, ct) =>
    {
        var browser = await _session.GetLoggedInAsync(options, ct);
        var page = browser.CreateItemPage();
        foreach (var expected in _expected.Values)
        {
            ct.ThrowIfCancellationRequested();
            var (_, comparisons) = await _service.DiffAsync(page, expected, progress, ct);
            ApplyComparisons(comparisons);
        }
        var diff = Rows.Count(r => r.IsDifferent);
        var error = Rows.Count(r => r.IsError);
        progress.Report($"取得しました（差分あり {diff} 行 / エラー {error} 行）");
    });

    [RelayCommand(CanExecute = nameof(IsIdle))]
    private async Task PushCheckedAsync()
    {
        var targets = Rows.Where(r => r.IsChecked).GroupBy(r => r.ItemId).ToList();
        if (targets.Count == 0)
        {
            ProgressText = "下書き保存する行にチェックを入れてください";
            return;
        }

        var summary = string.Join("\n", targets.Select(g => $"・{g.First().ItemName}: {string.Join(", ", g.Select(r => r.Locale))}"));
        if (ConfirmAsync is null || !await ConfirmAsync("下書き保存",
                $"次の行の差分のある欄を書き換えて、下書き保存します（審査には送信しません）。\n\n{summary}"))
        {
            return;
        }

        await RunAsync("下書き保存", async (options, progress, ct) =>
        {
            var browser = await _session.GetLoggedInAsync(options, ct);
            var page = browser.CreateItemPage();
            // 1 件でも失敗したら残りは行わない（例外は RunAsync で表示する）
            foreach (var group in targets)
            {
                var expected = _expected[group.Key].FilterLocales([.. group.Select(r => r.Locale)]);
                try
                {
                    var result = await _service.PushAsync(page, expected, progress, ct);
                    ApplyComparisons(result.FinalComparison);
                }
                catch (DashboardException ex)
                {
                    foreach (var row in group) row.ApplyError(ex.Message);
                    throw;
                }
            }
            progress.Report("下書き保存と照合が完了しました（審査には送信していません）");
        });
    }

    private bool CanSubmit() => IsIdle && SelectedRow is not null;

    [RelayCommand(CanExecute = nameof(CanSubmit))]
    private Task SubmitSelectedAsync()
    {
        var row = SelectedRow;
        if (row is null) return Task.CompletedTask;
        return RunAsync("審査送信", async (options, progress, ct) =>
        {
            var browser = await _session.GetLoggedInAsync(options, ct);
            var page = browser.CreateItemPage();
            var result = await _service.SubmitAsync(page, row.ItemId, async (dialog, _) =>
            {
                var auto = dialog.AutoPublish switch { true => "する", false => "しない", null => "不明" };
                return ConfirmAsync is not null && await ConfirmAsync("審査に送信",
                    $"{row.ItemName}（{row.ItemId}）を審査に送信します。\n\nダッシュボードの確認内容:\n{dialog.Text}\n\n審査合格後の自動公開: {auto}（この設定は変更しません）");
            }, progress, ct);

            progress.Report(result.Submitted
                ? $"{row.ItemName}: {result.Message} ステータス: {result.StatusText ?? "不明"}"
                : $"{row.ItemName}: 送信を取りやめました");
        });
    }

    [RelayCommand(CanExecute = nameof(IsBusy))]
    private void Cancel() => _cts?.Cancel();

    [RelayCommand]
    private void OpenLogFolder() => OpenFolder(LoggerService.GetLogDirectory());

    [RelayCommand]
    private void OpenConfigFolder() => OpenFolder(Path.GetDirectoryName(ConfigPath) ?? AppPaths.RoamingRoot);

    /// <summary>
    /// ブラウザ操作を 1 つ実行する共通処理（直列化・中止・例外表示）。
    /// </summary>
    private async Task RunAsync(string title, Func<DashboardBrowserOptions, IProgress<string>, CancellationToken, Task> action, bool requireLogin = true)
    {
        if (IsBusy) return;
        if (_config is null)
        {
            ProgressText = "設定ファイルを読み込めていません";
            return;
        }
        if (requireLogin && _expected.Count == 0)
        {
            ProgressText = "対象のアイテムがありません";
            return;
        }

        IsBusy = true;
        _cts = new CancellationTokenSource();
        // UI スレッドで作るので、Core 側（別スレッド）からの報告も UI スレッドで表示される
        var progress = new Progress<string>(m =>
        {
            ProgressText = m;
            AppendLog(m);
        });
        AppendLog($"── {title} ──");
        try
        {
            await action(new DashboardBrowserOptions(_config.ProfileDirectory, _config.ChromePath), progress, _cts.Token);
        }
        catch (OperationCanceledException)
        {
            ProgressText = $"{title}を中止しました";
            AppendLog(ProgressText);
        }
        catch (LoginRequiredException ex)
        {
            ProgressText = ex.Message;
            AppendLog(ex.Message);
        }
        catch (WriteBlockedException ex)
        {
            ProgressText = $"中止: {ex.Message}";
            AppendLog(ProgressText);
        }
        catch (Exception ex)
        {
            LoggerService.LogException($"{title}に失敗", ex);
            ProgressText = $"{title}に失敗しました: {LoggerService.Redact(ex.Message)}";
            AppendLog(ProgressText);
        }
        finally
        {
            _cts.Dispose();
            _cts = null;
            IsBusy = false;
        }
    }

    /// <summary>比較結果を一覧の該当行へ反映する</summary>
    private void ApplyComparisons(IEnumerable<LocaleComparison> comparisons)
    {
        foreach (var c in comparisons)
        {
            var row = Rows.FirstOrDefault(r => r.ItemId == c.ItemId && string.Equals(r.Locale, c.Locale, StringComparison.OrdinalIgnoreCase));
            row?.Apply(c);
        }
    }

    private void AppendLog(string message)
    {
        LogLines.Add($"{DateTime.Now:HH:mm:ss} {LoggerService.Redact(message)}");
        while (LogLines.Count > 500) LogLines.RemoveAt(0);
    }

    private static void OpenFolder(string path)
    {
        try
        {
            Directory.CreateDirectory(path);
            Process.Start(new ProcessStartInfo("explorer.exe", $"\"{path}\"") { UseShellExecute = true });
        }
        catch (Exception ex)
        {
            LoggerService.Log($"フォルダを開けませんでした: {ex.Message}", LogLevel.Warning);
        }
    }

    /// <summary>終了時にブラウザを閉じる（自分で起動した Chrome だけ。プロファイルは残す）</summary>
    public async ValueTask DisposeAsync()
    {
        _cts?.Cancel();
        await _session.DisposeAsync();
    }
}
