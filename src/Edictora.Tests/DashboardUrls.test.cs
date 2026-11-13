using Edictora.Core.Dashboard;
using Edictora.Core.Logging;

namespace Edictora.Tests;

[TestClass]
public sealed class DashboardUrlsTests
{
    // テスト用の架空の publisherId（実在の値は使わない）
    private const string Publisher = "00000000-1111-2222-3333-444444444444";
    private const string Item = "abcdefghijklmnopabcdefghijklmnop";

    [TestMethod]
    public void TryParsePublisherId_ダッシュボードのURLから取り出す()
    {
        Assert.IsTrue(DashboardUrls.TryParsePublisherId($"https://chrome.google.com/webstore/devconsole/{Publisher}", out var a));
        Assert.AreEqual(Publisher, a);
        Assert.IsTrue(DashboardUrls.TryParsePublisherId($"https://chrome.google.com/webstore/devconsole/{Publisher}/{Item}/edit?hl=ja", out var b));
        Assert.AreEqual(Publisher, b);
    }

    [TestMethod]
    public void TryParsePublisherId_入口やほかのサイトは対象外()
    {
        Assert.IsFalse(DashboardUrls.TryParsePublisherId(DashboardUrls.Root, out _));
        Assert.IsFalse(DashboardUrls.TryParsePublisherId("https://accounts.google.com/signin", out _));
        Assert.IsFalse(DashboardUrls.TryParsePublisherId($"https://evil.example/webstore/devconsole/{Publisher}", out _));
        Assert.IsFalse(DashboardUrls.TryParsePublisherId($"http://chrome.google.com/webstore/devconsole/{Publisher}", out _));
        Assert.IsFalse(DashboardUrls.TryParsePublisherId(null, out _));
    }

    [TestMethod]
    public void EditUrl_編集画面のURLを作る()
    {
        var url = DashboardUrls.EditUrl(Publisher, Item);

        Assert.AreEqual($"https://chrome.google.com/webstore/devconsole/{Publisher}/{Item}/edit", url);
        Assert.IsTrue(DashboardUrls.IsEditPageOf(url, Item));
        Assert.IsFalse(DashboardUrls.IsEditPageOf($"https://chrome.google.com/webstore/devconsole/{Publisher}", Item));
    }

    [TestMethod]
    public void IsLoginPage_Googleのログイン画面を判定する()
    {
        Assert.IsTrue(DashboardUrls.IsLoginPage("https://accounts.google.com/v3/signin/identifier"));
        Assert.IsFalse(DashboardUrls.IsLoginPage(DashboardUrls.Root));
    }

    [TestMethod]
    public void Redact_ログに出す文字列からpublisherIdを伏せる()
    {
        var text = $"移動先: https://chrome.google.com/webstore/devconsole/{Publisher}/{Item}/edit";

        var redacted = LoggerService.Redact(text);

        Assert.DoesNotContain(Publisher, redacted);
        StringAssert.Contains(redacted, $"devconsole/<publisher>/{Item}/edit");
        // 2 回通しても変わらない
        Assert.AreEqual(redacted, LoggerService.Redact(redacted));
    }
}
