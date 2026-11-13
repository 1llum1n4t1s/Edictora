using Edictora.Core.Dashboard;
using Edictora.Core.Listing;

namespace Edictora.Tests;

[TestClass]
public sealed class ElementSelectorsTests
{
    private static ElementInfo El(int index, string text = "", string label = "", string tag = "div",
        bool disabled = false, bool readOnly = false, string type = "")
        => new(index, tag, text, label, disabled, readOnly, type);

    [TestMethod]
    public void PickLanguageCombobox_最初のパブリッシャー選択ではなく言語ラベルのものを選ぶ()
    {
        var combos = new[]
        {
            El(0, "Kagayoi"),
            El(1, "言語\n英語 – en （デフォルト）"),
            El(2, "仕事効率化", label: "カテゴリ"),
        };

        Assert.AreEqual(1, ElementSelectors.PickLanguageCombobox(combos).Index);
    }

    [TestMethod]
    public void PickLanguageCombobox_親要素の説明文に言語を含むカテゴリ欄には当たらない()
    {
        // 2026-11-13 の実ダッシュボードの構成を再現する。カテゴリ欄の親要素には「すべての言語用」という説明があり、
        // 部分一致の判定だとカテゴリ欄も「言語」に当たって 2 件になり、特定できずに止まった。
        var combos = new[]
        {
            El(0, "言語\n言語\n言語\n英語 – en （デフォルト）", label: "言語\n言語\n言語\n言語\n英語 – en （デフォルト）"),
            El(1, "カテゴリ\nカテゴリ\nカテゴリ\nユーザー補助機能", label: "カテゴリ\nカテゴリ\nカテゴリ\nカテゴリ\nユーザー補助機能\nすべての言語用"),
        };

        Assert.AreEqual(0, ElementSelectors.PickLanguageCombobox(combos).Index);
    }

    [TestMethod]
    public void PickFieldByLabel_説明文に別の欄名を含む欄には当たらない()
    {
        var inputs = new[]
        {
            El(0, label: "公式 URL\n所有しているウェブサイトの 1 つがアイテムと関連している場合はホームページ URL の代わりに表示されます", tag: "input"),
            El(1, label: "ホームページ URL\nアイテムのウェブサイトへのリンク", tag: "input"),
        };

        Assert.AreEqual(1, ElementSelectors.PickFieldByLabel(inputs, ListingField.HomepageUrl).Index);
        Assert.AreEqual(0, ElementSelectors.PickFieldByLabel(inputs, ListingField.OfficialUrl).Index);
    }

    [TestMethod]
    public void PickLanguageCombobox_無い場合と複数ある場合は例外()
    {
        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.PickLanguageCombobox([El(0, "Kagayoi")]));
        Assert.ThrowsExactly<DashboardException>(() =>
            ElementSelectors.PickLanguageCombobox([El(0, label: "言語"), El(1, label: "言語")]));
    }

    [TestMethod]
    public void ParseLocaleOption_言語名とコードとデフォルト表記を読む()
    {
        var en = ElementSelectors.ParseLocaleOption("英語 – en （デフォルト）");
        var ja = ElementSelectors.ParseLocaleOption("日本語 – ja");
        var pt = ElementSelectors.ParseLocaleOption("ポルトガル語（ブラジル） – pt-BR");

        Assert.AreEqual(new LocaleOption("英語", "en", true), en);
        Assert.AreEqual(new LocaleOption("日本語", "ja", false), ja);
        Assert.AreEqual("pt-BR", pt?.Code);
        Assert.IsNull(ElementSelectors.ParseLocaleOption("言語"));
        Assert.IsNull(ElementSelectors.ParseLocaleOption(""));
    }

    [TestMethod]
    public void PickLocaleOption_コードが完全一致するものだけを選ぶ()
    {
        var options = new[]
        {
            El(0, "英語（イギリス） – en-GB"),
            El(1, "英語 – en （デフォルト）"),
            El(2, "日本語 – ja"),
        };

        Assert.AreEqual(1, ElementSelectors.PickLocaleOption(options, "en").Index);
        Assert.AreEqual(0, ElementSelectors.PickLocaleOption(options, "en-GB").Index);
        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.PickLocaleOption(options, "fr"));
        CollectionAssert.AreEqual(new[] { "en-GB", "en", "ja" }, ElementSelectors.LocaleCodes(options).ToArray());
    }

    [TestMethod]
    public void CurrentLocaleCode_表示の最後の言語行を読む()
    {
        Assert.AreEqual("en", ElementSelectors.CurrentLocaleCode(El(0, "言語\n英語 – en （デフォルト）")));
        Assert.IsNull(ElementSelectors.CurrentLocaleCode(El(0, "言語")));
    }

    [TestMethod]
    public void PickButton_文言の完全一致で1つだけ選ぶ()
    {
        var buttons = new[]
        {
            El(0, "公開"),
            El(1, " 下書きとして保存する "),
            El(2, "審査のため送信"),
        };

        Assert.AreEqual(1, ElementSelectors.PickButton(buttons, DashboardText.SaveDraftButton).Index);
        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.PickButton(buttons, "削除"));
        Assert.ThrowsExactly<DashboardException>(() =>
            ElementSelectors.PickButton([El(0, "審査のため送信"), El(1, "審査のため送信")], DashboardText.SubmitButton));
    }

    [TestMethod]
    public void PickFieldByLabel_空白の有無を無視してラベルで特定する()
    {
        var inputs = new[]
        {
            El(0, label: "公式URL\n確認済みのサイト", tag: "input", type: "text"),
            El(1, label: "ホームページ URL", tag: "input", type: "text"),
            El(2, label: "サポート URL", tag: "input", type: "text"),
        };

        Assert.AreEqual(0, ElementSelectors.PickFieldByLabel(inputs, ListingField.OfficialUrl).Index);
        Assert.AreEqual(1, ElementSelectors.PickFieldByLabel(inputs, ListingField.HomepageUrl).Index);
        Assert.AreEqual(2, ElementSelectors.PickFieldByLabel(inputs, ListingField.SupportUrl).Index);
        Assert.ThrowsExactly<DashboardException>(() =>
            ElementSelectors.PickFieldByLabel([El(0, label: "サポート URL"), El(1, label: "サポート URL")], ListingField.SupportUrl));
    }

    [TestMethod]
    public void PickDescription_textareaが1つでなければ例外()
    {
        Assert.AreEqual(3, ElementSelectors.PickDescription([El(3, tag: "textarea")]).Index);
        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.PickDescription([]));
        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.PickDescription([El(0), El(1)]));
    }

    [TestMethod]
    public void EnsureWritable_テキスト欄でない要素や編集不可の欄は止める()
    {
        ElementSelectors.EnsureWritable(El(0, tag: "textarea"), ListingField.Description);
        ElementSelectors.EnsureWritable(El(0, tag: "input", type: "url"), ListingField.SupportUrl);

        Assert.ThrowsExactly<DashboardException>(() => ElementSelectors.EnsureWritable(El(0, tag: "input", type: "checkbox"), ListingField.SupportUrl));
        Assert.ThrowsExactly<WriteBlockedException>(() => ElementSelectors.EnsureWritable(El(0, tag: "textarea", readOnly: true), ListingField.Description));
        Assert.ThrowsExactly<WriteBlockedException>(() => ElementSelectors.EnsureWritable(El(0, tag: "input", disabled: true), ListingField.HomepageUrl));
    }

    [TestMethod]
    public void HasMessage_常設の案内通知を除いて判定する()
    {
        var messages = new[] { "ガイドやヒントが必要な場合は、ヘルプをご覧ください。保存されました", "" };
        Assert.IsFalse(ElementSelectors.HasMessage(messages, DashboardText.SavedMessage));
        Assert.IsTrue(ElementSelectors.HasMessage([.. messages, "アイテムが保存されました。"], DashboardText.SavedMessage));
    }

    [TestMethod]
    public void ParseStatus_ステータス行の値を読む()
    {
        Assert.AreEqual("審査待ち", ElementSelectors.ParseStatus("アイテム\nステータス: 審査待ち\nパッケージ"));
        Assert.AreEqual("公開済み", ElementSelectors.ParseStatus("ステータス：公開済み"));
        Assert.IsNull(ElementSelectors.ParseStatus("なし"));
    }
}
