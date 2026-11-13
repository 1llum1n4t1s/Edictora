using Edictora.Core.Listing;
using Edictora.UI.ViewModels;

namespace Edictora.Tests;

[TestClass]
public sealed class ListingRowViewModelTests
{
    private static readonly ExpectedItem Item = new("abcdefghijklmnopabcdefghijklmnop", "テスト", [
        new ExpectedLocale("ja", new Dictionary<ListingField, string> { [ListingField.Description] = "正本" }),
    ]);

    [TestMethod]
    public void 作成直後は未取得()
    {
        var row = new ListingRowViewModel(Item, Item.Locales[0]);

        Assert.AreEqual(ComparisonStatus.NotFetched, row.Status);
        Assert.AreEqual("未取得", row.StatusLabel);
        Assert.AreEqual("（未取得）", row.Fields.Single().Current);
    }

    [TestMethod]
    public void Apply_差分ありを反映し一致したらチェックを外す()
    {
        var row = new ListingRowViewModel(Item, Item.Locales[0]) { IsChecked = true };
        var snapshot = new ItemSnapshot(Item.ItemId, "公開済み", ["ja"], [
            new LocaleSnapshot("ja", new Dictionary<ListingField, string> { [ListingField.Description] = "古い" }),
        ]);

        row.Apply(ListingComparer.Compare(Item, snapshot)[0]);
        Assert.IsTrue(row.IsDifferent);
        Assert.IsTrue(row.IsChecked);
        Assert.AreEqual("古い", row.Fields.Single().Current);

        var matched = snapshot with { Locales = [new LocaleSnapshot("ja", new Dictionary<ListingField, string> { [ListingField.Description] = "正本" })] };
        row.Apply(ListingComparer.Compare(Item, matched)[0]);
        Assert.IsTrue(row.IsMatch);
        Assert.IsFalse(row.IsChecked);
    }
}
