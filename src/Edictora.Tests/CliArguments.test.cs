using Edictora.Cli;

namespace Edictora.Tests;

[TestClass]
public sealed class CliArgumentsTests
{
    private const string Item = "abcdefghijklmnopabcdefghijklmnop";
    private const string Item2 = "ponmlkjihgfedcbaponmlkjihgfedcba";

    [TestMethod]
    public void Parse_引数なしは差分表示()
    {
        Assert.AreEqual(CliCommandKind.Diff, CliArguments.Parse([]).Kind);
        Assert.AreEqual(CliCommandKind.Diff, CliArguments.Parse(["--item", Item]).Kind);
    }

    [TestMethod]
    public void Parse_ヘルプとバージョン()
    {
        Assert.AreEqual(CliCommandKind.Help, CliArguments.Parse(["--help"]).Kind);
        Assert.AreEqual(CliCommandKind.Help, CliArguments.Parse(["push", "-h"]).Kind);
        Assert.AreEqual(CliCommandKind.Version, CliArguments.Parse(["--version"]).Kind);
    }

    [TestMethod]
    public void Parse_pushのオプションを読む()
    {
        var cmd = CliArguments.Parse(["push", "--item", Item.ToUpperInvariant(), "--locale=ja", "--locale", "en", "--yes", "--config", "c.json", "--profile", "p"]);

        Assert.AreEqual(CliCommandKind.Push, cmd.Kind);
        CollectionAssert.AreEqual(new[] { Item }, cmd.ItemIds.ToArray());
        CollectionAssert.AreEqual(new[] { "ja", "en" }, cmd.LocaleCodes.ToArray());
        Assert.IsTrue(cmd.Yes);
        Assert.AreEqual("c.json", cmd.ConfigPath);
        Assert.AreEqual("p", cmd.ProfileDir);
    }

    [TestMethod]
    public void Parse_submitは1件とyesが必須()
    {
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["submit", "--yes"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["submit", "--item", Item]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["submit", "--item", Item, "--item", Item2, "--yes"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["submit", "--item", Item, "--locale", "ja", "--yes"]));

        var cmd = CliArguments.Parse(["submit", "--item", Item, "--yes"]);
        Assert.AreEqual(CliCommandKind.Submit, cmd.Kind);
    }

    [TestMethod]
    public void Parse_コマンドに無いオプションや誤った値は拒否する()
    {
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["diff", "--yes"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["pull", "--item", "short"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["pull", "--locale", "日本語"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["pull", "--item"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["pull", "--config", "a", "--config", "b"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["publish"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["pull", "extra"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["login", "--wait", "0"]));
        Assert.ThrowsExactly<CliUsageException>(() => CliArguments.Parse(["push", "--yes=true"]));
    }

    [TestMethod]
    public void Parse_loginの待ち時間()
    {
        Assert.AreEqual(TimeSpan.FromMinutes(5), CliArguments.Parse(["login", "--wait", "5"]).LoginWait);
    }
}

[TestClass]
public sealed class CliAppTests
{
    private string _dir = "";

    [TestInitialize]
    public void Setup()
    {
        _dir = Path.Combine(AppContext.BaseDirectory, "work", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_dir);
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_dir)) Directory.Delete(_dir, recursive: true);
    }

    private static async Task<(int Code, string Out, string Err)> RunAsync(params string[] args)
    {
        using var stdout = new StringWriter();
        using var stderr = new StringWriter();
        var code = await new CliApp(stdout, stderr).RunAsync(args, CancellationToken.None);
        return (code, stdout.ToString(), stderr.ToString());
    }

    [TestMethod]
    public async Task RunAsync_ヘルプは成功で使い方を出す()
    {
        var (code, output, _) = await RunAsync("--help");

        Assert.AreEqual(0, code);
        StringAssert.Contains(output, "edictora submit  --item <id> --yes");
    }

    [TestMethod]
    public async Task RunAsync_引数の誤りは終了コード2()
    {
        var (code, _, err) = await RunAsync("submit", "--item", "abc");

        Assert.AreEqual(2, code);
        StringAssert.Contains(err, "--item");
    }

    [TestMethod]
    public async Task RunAsync_設定ファイルが無ければブラウザを開かず終了コード2()
    {
        var (code, _, err) = await RunAsync("diff", "--config", Path.Combine(_dir, "none.json"));

        Assert.AreEqual(2, code);
        StringAssert.Contains(err, "設定ファイルがありません");
    }

    [TestMethod]
    public async Task RunAsync_設定に無いアイテムはブラウザを開かず終了コード2()
    {
        var config = Path.Combine(_dir, "config.json");
        await File.WriteAllTextAsync(config, """
            { "items": [ { "itemId": "abcdefghijklmnopabcdefghijklmnop", "locales": { "ja": { "homepageUrl": "https://example.com/" } } } ] }
            """);

        var (code, _, err) = await RunAsync("push", "--config", config, "--item", "ponmlkjihgfedcbaponmlkjihgfedcba", "--yes");

        Assert.AreEqual(2, code);
        StringAssert.Contains(err, "ponmlkjihgfedcbaponmlkjihgfedcba");
    }

    [TestMethod]
    public async Task RunAsync_指定ロケールを持たないアイテムを明示したら終了コード2()
    {
        var config = Path.Combine(_dir, "config.json");
        await File.WriteAllTextAsync(config, """
            { "items": [ { "itemId": "abcdefghijklmnopabcdefghijklmnop", "locales": { "ja": { "homepageUrl": "https://example.com/" } } } ] }
            """);

        var (code, _, err) = await RunAsync("diff", "--config", config, "--item", "abcdefghijklmnopabcdefghijklmnop", "--locale", "fr");

        Assert.AreEqual(2, code);
        StringAssert.Contains(err, "fr");
    }
}
