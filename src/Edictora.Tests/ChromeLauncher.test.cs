using Edictora.Core.Chrome;

namespace Edictora.Tests;

[TestClass]
public sealed class ChromeLauncherTests
{
    private static List<string> Args() => ChromeLauncher.BuildArgs(new ChromeLaunchOptions
    {
        ChromePath = "chrome.exe",
        UserDataDir = @"C:\profiles\edictora",
    });

    [TestMethod]
    public void BuildArgs_専用プロファイルとポート自動割り当てを指定する()
    {
        var args = Args();

        CollectionAssert.Contains(args, @"--user-data-dir=C:\profiles\edictora");
        CollectionAssert.Contains(args, "--remote-debugging-port=0");
        CollectionAssert.Contains(args, "--no-first-run");
        CollectionAssert.Contains(args, "--no-default-browser-check");
    }

    [TestMethod]
    public void BuildArgs_ログインを妨げる自動化フラグや画面外配置を付けない()
    {
        var args = Args();

        Assert.IsFalse(args.Any(a => a.StartsWith("--enable-automation", StringComparison.Ordinal)));
        Assert.IsFalse(args.Any(a => a.StartsWith("--headless", StringComparison.Ordinal)));
        Assert.IsFalse(args.Any(a => a.StartsWith("--window-position", StringComparison.Ordinal)));
        Assert.IsFalse(args.Any(a => a.Contains("extension", StringComparison.OrdinalIgnoreCase)));
    }

    [TestMethod]
    public void BuildArgs_許可した引数だけを付ける()
    {
        var allowed = new[]
        {
            "--user-data-dir=", "--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check",
            "--disable-backgrounding-occluded-windows", "--disable-renderer-backgrounding",
            "--disable-background-timer-throttling", "about:blank",
        };

        foreach (var arg in Args())
        {
            Assert.IsTrue(allowed.Any(a => arg.StartsWith(a, StringComparison.Ordinal)), $"想定外の起動引数: {arg}");
        }
    }
}
