using System.Security.Cryptography;
using System.Text;
using Edictora.Core.Config;

namespace Edictora.Tests;

[TestClass]
public sealed class SectionExtractorTests
{
    private static string Fixture(string name)
        => Path.Combine(AppContext.BaseDirectory, "Fixtures", "ReplaceTranslator", name);

    private static string Sha256(string text)
        => Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(text)));

    // 期待値は 2026-11-13 の検証スクリプト（push.mjs と同じ切り出し関数）を Bun で実行して得たもの。
    private const string JaSha256 = "034207b5b6dce2d8fde506c2ef269998b88b267ceca0d69dff5151f3d2608839";
    private const string EnSha256 = "5e9f8dfa9191fa32d6d7beff9d65a50fa97873b72b69bb19a4af9d2cc1905145";

    [TestMethod]
    public void Extract_ReplaceTranslatorの日本語説明が検証済みの切り出しと一致する()
    {
        var text = SectionExtractor.Extract(new TextSource(Fixture("store-listing.txt"), "^■ 説明", "^■ カテゴリ"));

        Assert.AreEqual(1492, text.Length);
        Assert.AreEqual(JaSha256, Sha256(text));
        Assert.StartsWith("お好みのクラウド LLM を使って、表示中の Web ページをその場で置換翻訳す", text);
    }

    [TestMethod]
    public void Extract_ReplaceTranslatorの英語説明が検証済みの切り出しと一致する()
    {
        var text = SectionExtractor.Extract(new TextSource(Fixture("store-listing.en.txt"), "^■ Description", "^■ Category"));

        Assert.AreEqual(3172, text.Length);
        Assert.AreEqual(EnSha256, Sha256(text));
        Assert.StartsWith("Translate the web page you are viewing i", text);
    }

    [TestMethod]
    public void Extract_CRLFのファイルでも同じ結果になる()
    {
        var lf = File.ReadAllText(Fixture("store-listing.txt")).Replace("\r\n", "\n");
        var crlf = lf.Replace("\n", "\r\n");

        var text = SectionExtractor.Extract(crlf, "^■ 説明", "^■ カテゴリ");

        Assert.AreEqual(JaSha256, Sha256(text));
        Assert.DoesNotContain("\r", text);
    }

    [TestMethod]
    public void Extract_見出しの次から終端見出しの手前までを前後の空行を除いて返す()
    {
        const string source = "■ 概要\n短い\n■ 説明\n\n\n  一行目\n二行目  \n\n■ カテゴリ\n仕事効率化\n";

        var text = SectionExtractor.Extract(source, "^■ 説明", "^■ カテゴリ");

        // 先頭は空行だけを落とし行頭の空白は残す。末尾の空白・空行は落とす。
        Assert.AreEqual("  一行目\n二行目", text);
    }

    [TestMethod]
    public void Extract_末尾の全角空白もJSの空白として落とす()
    {
        // 全角空白（U+3000）と NBSP（U+00A0）は JS の \s に含まれる
        var source = "■ A\n本文" + (char)0x3000 + (char)0x00A0 + "\n■ B";
        var text = SectionExtractor.Extract(source, "^■ A", "^■ B");

        Assert.AreEqual("本文", text);
    }

    [TestMethod]
    public void Extract_終端見出しは開始見出しより後ろだけを探す()
    {
        var text = SectionExtractor.Extract("■ B\n前\n■ A\n本文\n■ B\n後", "^■ A", "^■ B");

        Assert.AreEqual("本文", text);
    }

    [TestMethod]
    public void Extract_開始見出しが無ければ例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() => SectionExtractor.Extract("本文", "^■ 説明", "^■ カテゴリ"));
        StringAssert.Contains(ex.Message, "開始見出し");
    }

    [TestMethod]
    public void Extract_終端見出しが無ければ例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() => SectionExtractor.Extract("■ 説明\n本文", "^■ 説明", "^■ カテゴリ"));
        StringAssert.Contains(ex.Message, "終端見出し");
    }

    [TestMethod]
    public void Extract_区間指定なしはファイル全体を同じ規則で整える()
    {
        var text = SectionExtractor.Extract("\r\n\r\n本文1\r\n本文2\r\n\r\n", null, null);

        Assert.AreEqual("本文1\n本文2", text);
    }

    [TestMethod]
    public void Extract_区間指定が片方だけなら例外()
    {
        Assert.ThrowsExactly<ConfigException>(() => SectionExtractor.Extract("本文", "^■ 説明", null));
    }

    [TestMethod]
    public void ValidatePattern_壊れた正規表現は例外()
    {
        Assert.ThrowsExactly<ConfigException>(() => SectionExtractor.ValidatePattern("([", "sectionStart"));
    }

    [TestMethod]
    public void IsJsWhiteSpace_JSの空白定義に合わせる()
    {
        Assert.IsTrue(SectionExtractor.IsJsWhiteSpace((char)0x3000));
        Assert.IsTrue(SectionExtractor.IsJsWhiteSpace((char)0xFEFF));
        Assert.IsTrue(SectionExtractor.IsJsWhiteSpace((char)0x2005));
        // .NET では空白だが JS の \s ではない
        Assert.IsFalse(SectionExtractor.IsJsWhiteSpace((char)0x0085));
        Assert.IsFalse(SectionExtractor.IsJsWhiteSpace('a'));
    }
}
