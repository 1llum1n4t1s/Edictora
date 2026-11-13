using Edictora.Core.Dashboard;
using Edictora.Core.Listing;
using Edictora.Core.Services;

namespace Edictora.Tests;

/// <summary>
/// 画面の代わりにメモリ上の値で動く <see cref="IListingPage"/>。手順（順番・中止判断）だけを確かめる。
/// </summary>
internal sealed class FakeListingPage : IListingPage
{
    /// <summary>保存済み（読み込み直すと見える）値</summary>
    public Dictionary<string, Dictionary<ListingField, string>> Saved { get; } = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>編集中（未保存）の値</summary>
    private Dictionary<string, Dictionary<ListingField, string>> _editing = new(StringComparer.OrdinalIgnoreCase);

    public string? CurrentLocale { get; private set; }
    public string Status { get; set; } = "公開済み";
    public List<string> Log { get; } = [];

    /// <summary>書き込み時に値を壊す（照合失敗の再現用）</summary>
    public bool CorruptWrites { get; set; }

    /// <summary>送信の確認で渡されたダイアログ</summary>
    public SubmitDialogInfo Dialog { get; set; } = new("「テスト」を審査のために送信しますか？", true);

    public Task OpenItemAsync(string itemId, CancellationToken ct)
    {
        Log.Add($"open {itemId}");
        _editing = Clone(Saved);
        CurrentLocale = Saved.Keys.First();
        return Task.CompletedTask;
    }

    public Task ReloadAsync(CancellationToken ct)
    {
        Log.Add("reload");
        _editing = Clone(Saved);
        CurrentLocale = Saved.Keys.First();
        return Task.CompletedTask;
    }

    public Task<ItemPageState> GetStateAsync(CancellationToken ct)
        => Task.FromResult(new ItemPageState(Status, true, true, true, true));

    public Task<IReadOnlyList<string>> GetLocalesAsync(CancellationToken ct)
        => Task.FromResult<IReadOnlyList<string>>([.. Saved.Keys]);

    public Task SwitchLocaleAsync(string code, CancellationToken ct)
    {
        Log.Add($"switch {code}");
        CurrentLocale = code;
        return Task.CompletedTask;
    }

    public Task<IReadOnlyDictionary<ListingField, string>> ReadFieldsAsync(IReadOnlyCollection<ListingField> fields, CancellationToken ct)
        => Task.FromResult<IReadOnlyDictionary<ListingField, string>>(
            fields.ToDictionary(f => f, f => _editing[CurrentLocale!].GetValueOrDefault(f) ?? ""));

    public Task WriteFieldAsync(ListingField field, string value, CancellationToken ct)
    {
        Log.Add($"write {CurrentLocale} {field}");
        _editing[CurrentLocale!][field] = CorruptWrites ? value + "!" : value;
        return Task.CompletedTask;
    }

    public Task SaveDraftAsync(CancellationToken ct)
    {
        Log.Add("save");
        Saved[CurrentLocale!] = new Dictionary<ListingField, string>(_editing[CurrentLocale!]);
        return Task.CompletedTask;
    }

    public async Task<SubmitResult> SubmitForReviewAsync(Func<SubmitDialogInfo, CancellationToken, Task<bool>> confirm, CancellationToken ct)
    {
        Log.Add("submit-dialog");
        if (!await confirm(Dialog, ct)) return new SubmitResult(false, Dialog, null, Status);
        Status = "審査待ち";
        Log.Add("submit-confirm");
        return new SubmitResult(true, Dialog, "アイテムを送信しました。", Status);
    }

    public Task<string?> CaptureDiagnosticsAsync(string label, CancellationToken ct)
    {
        Log.Add($"capture {label}");
        return Task.FromResult<string?>(null);
    }

    private static Dictionary<string, Dictionary<ListingField, string>> Clone(Dictionary<string, Dictionary<ListingField, string>> src)
        => src.ToDictionary(kv => kv.Key, kv => new Dictionary<ListingField, string>(kv.Value), StringComparer.OrdinalIgnoreCase);
}

[TestClass]
public sealed class ListingServiceTests
{
    private const string Item = "abcdefghijklmnopabcdefghijklmnop";

    private static ExpectedItem Expected() => new(Item, "テスト", [
        new ExpectedLocale("en", new Dictionary<ListingField, string> { [ListingField.Description] = "New EN" }),
        new ExpectedLocale("ja", new Dictionary<ListingField, string>
        {
            [ListingField.Description] = "日本語",
            [ListingField.SupportUrl] = "https://example.com/support",
        }),
    ]);

    private static FakeListingPage Page()
    {
        var page = new FakeListingPage();
        page.Saved["en"] = new() { [ListingField.Description] = "Old EN" };
        page.Saved["ja"] = new() { [ListingField.Description] = "日本語", [ListingField.SupportUrl] = "https://example.com/support" };
        return page;
    }

    [TestMethod]
    public async Task DiffAsync_読み取りだけで差分を返す()
    {
        var page = Page();

        var (_, rows) = await new ListingService().DiffAsync(page, Expected(), null, CancellationToken.None);

        Assert.AreEqual(ComparisonStatus.Different, rows.Single(r => r.Locale == "en").Status);
        Assert.AreEqual(ComparisonStatus.Match, rows.Single(r => r.Locale == "ja").Status);
        Assert.IsFalse(page.Log.Any(l => l.StartsWith("write", StringComparison.Ordinal) || l == "save"));
    }

    [TestMethod]
    public async Task PushAsync_差分のあるロケールだけを書き換えて保存し読み直して照合する()
    {
        var page = Page();

        var result = await new ListingService().PushAsync(page, Expected(), null, CancellationToken.None);

        Assert.AreEqual(LocalePushOutcome.Saved, result.Locales.Single(l => l.Locale == "en").Outcome);
        Assert.AreEqual(LocalePushOutcome.Unchanged, result.Locales.Single(l => l.Locale == "ja").Outcome);
        Assert.AreEqual("New EN", page.Saved["en"][ListingField.Description]);
        Assert.IsTrue(result.FinalComparison.All(c => c.Status == ComparisonStatus.Match));
        // 書き換えたのは en の説明だけ。保存は 1 回で、保存後に読み込み直している。
        CollectionAssert.AreEqual(new[] { "write en Description" }, page.Log.Where(l => l.StartsWith("write", StringComparison.Ordinal)).ToArray());
        Assert.AreEqual(1, page.Log.Count(l => l == "save"));
        Assert.IsTrue(page.Log.IndexOf("reload") > page.Log.IndexOf("save"));
    }

    [TestMethod]
    public async Task PushAsync_審査待ちなら何も書き換えない()
    {
        var page = Page();
        page.Status = "審査待ち";

        await Assert.ThrowsExactlyAsync<WriteBlockedException>(() =>
            new ListingService().PushAsync(page, Expected(), null, CancellationToken.None));

        Assert.IsFalse(page.Log.Any(l => l.StartsWith("write", StringComparison.Ordinal) || l == "save"));
    }

    [TestMethod]
    public async Task PushAsync_書き換え後の値が一致しなければ保存しない()
    {
        var page = Page();
        page.CorruptWrites = true;

        await Assert.ThrowsExactlyAsync<DashboardException>(() =>
            new ListingService().PushAsync(page, Expected(), null, CancellationToken.None));

        Assert.IsFalse(page.Log.Contains("save"));
        Assert.AreEqual("Old EN", page.Saved["en"][ListingField.Description]);
        Assert.IsTrue(page.Log.Any(l => l.StartsWith("capture", StringComparison.Ordinal)));
    }

    [TestMethod]
    public async Task PushAsync_ダッシュボードに無いロケールがあれば何も書き換えない()
    {
        var page = Page();
        page.Saved.Remove("ja");

        await Assert.ThrowsExactlyAsync<DashboardException>(() =>
            new ListingService().PushAsync(page, Expected(), null, CancellationToken.None));

        Assert.IsFalse(page.Log.Any(l => l.StartsWith("write", StringComparison.Ordinal)));
    }

    [TestMethod]
    public async Task SubmitAsync_確認で取りやめたら送信しない()
    {
        var page = Page();

        var result = await new ListingService().SubmitAsync(page, Item, (_, _) => Task.FromResult(false), null, CancellationToken.None);

        Assert.IsFalse(result.Submitted);
        Assert.DoesNotContain("submit-confirm", page.Log);
    }

    [TestMethod]
    public async Task SubmitAsync_確認後に送信し結果を返す()
    {
        var page = Page();
        SubmitDialogInfo? seen = null;

        var result = await new ListingService().SubmitAsync(page, Item, (d, _) =>
        {
            seen = d;
            return Task.FromResult(true);
        }, null, CancellationToken.None);

        Assert.IsTrue(result.Submitted);
        Assert.AreEqual("審査待ち", result.StatusText);
        Assert.IsTrue(seen?.AutoPublish);
    }

    [TestMethod]
    public async Task SubmitAsync_審査待ちのアイテムは送信を始めない()
    {
        var page = Page();
        page.Status = "審査待ち";

        await Assert.ThrowsExactlyAsync<WriteBlockedException>(() =>
            new ListingService().SubmitAsync(page, Item, (_, _) => Task.FromResult(true), null, CancellationToken.None));

        Assert.DoesNotContain("submit-dialog", page.Log);
    }
}
