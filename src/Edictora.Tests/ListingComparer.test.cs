using Edictora.Core.Listing;

namespace Edictora.Tests;

[TestClass]
public sealed class ListingComparerTests
{
    private const string Item = "abcdefghijklmnopabcdefghijklmnop";

    private static ExpectedItem Expected() => new(Item, "テスト", [
        new ExpectedLocale("ja", new Dictionary<ListingField, string>
        {
            [ListingField.Description] = "説明1\n説明2",
            [ListingField.SupportUrl] = "https://example.com/support",
        }),
        new ExpectedLocale("en", new Dictionary<ListingField, string>
        {
            [ListingField.Description] = "Desc",
        }),
    ]);

    [TestMethod]
    public void Compare_スナップショットなしは全行未取得()
    {
        var rows = ListingComparer.Compare(Expected(), null);

        Assert.HasCount(2, rows);
        Assert.IsTrue(rows.All(r => r.Status == ComparisonStatus.NotFetched));
    }

    [TestMethod]
    public void Compare_完全一致と差分を欄ごとに判定する()
    {
        var snapshot = new ItemSnapshot(Item, "公開済み", ["en", "ja"], [
            new LocaleSnapshot("ja", new Dictionary<ListingField, string>
            {
                [ListingField.Description] = "説明1\n説明2",
                [ListingField.SupportUrl] = "https://example.com/support/",
            }),
            new LocaleSnapshot("en", new Dictionary<ListingField, string> { [ListingField.Description] = "Desc" }),
        ]);

        var rows = ListingComparer.Compare(Expected(), snapshot);
        var ja = rows.Single(r => r.Locale == "ja");
        var en = rows.Single(r => r.Locale == "en");

        Assert.AreEqual(ComparisonStatus.Different, ja.Status);
        CollectionAssert.AreEqual(new[] { ListingField.SupportUrl }, ja.DifferentFields.ToArray());
        Assert.AreEqual(ComparisonStatus.Match, en.Status);
    }

    [TestMethod]
    public void CompareValue_末尾の空白や改行の違いも差分にする()
    {
        Assert.AreEqual(ComparisonStatus.Different, ListingComparer.CompareValue("abc", "abc\n"));
        Assert.AreEqual(ComparisonStatus.Different, ListingComparer.CompareValue("abc", "abc "));
        Assert.AreEqual(ComparisonStatus.Match, ListingComparer.CompareValue("abc", "abc"));
        Assert.AreEqual(ComparisonStatus.NotFetched, ListingComparer.CompareValue("abc", null));
    }

    [TestMethod]
    public void Compare_ダッシュボードに無いロケールはエラー()
    {
        var snapshot = new ItemSnapshot(Item, "公開済み", ["en"], [
            new LocaleSnapshot("en", new Dictionary<ListingField, string> { [ListingField.Description] = "Desc" }),
        ]);

        var ja = ListingComparer.Compare(Expected(), snapshot).Single(r => r.Locale == "ja");

        Assert.AreEqual(ComparisonStatus.Error, ja.Status);
        StringAssert.Contains(ja.Error, "ja");
    }

    [TestMethod]
    public void Compare_読み出し失敗はエラーとして伝える()
    {
        var snapshot = new ItemSnapshot(Item, null, [], [], "編集画面を開けませんでした");

        var rows = ListingComparer.Compare(Expected(), snapshot);

        Assert.IsTrue(rows.All(r => r.Status == ComparisonStatus.Error && r.Error == "編集画面を開けませんでした"));
    }

    [TestMethod]
    public void TextDiff_変更行だけを前後の文脈付きで示す()
    {
        var before = "a\nb\nc\nd\ne\nf\ng";
        var after = "a\nb\nc\nD\ne\nf\ng";

        var text = TextDiff.Format(TextDiff.Compute(before, after), context: 1);

        Assert.AreEqual("  …\n  c\n- d\n+ D\n  e\n  …\n", text);
    }

    [TestMethod]
    public void TextDiff_最初の相違位置を返す()
    {
        Assert.AreEqual(-1, TextDiff.FirstDifferenceIndex("abc", "abc"));
        Assert.AreEqual(2, TextDiff.FirstDifferenceIndex("abc", "abX"));
        Assert.AreEqual(3, TextDiff.FirstDifferenceIndex("abc", "abc "));
    }
}
