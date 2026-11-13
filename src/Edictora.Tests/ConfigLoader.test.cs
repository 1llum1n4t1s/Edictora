using Edictora.Core;
using Edictora.Core.Config;
using Edictora.Core.Listing;

namespace Edictora.Tests;

[TestClass]
public sealed class ConfigLoaderTests
{
    private const string ItemA = "abcdefghijklmnopabcdefghijklmnop";
    private const string ItemB = "ponmlkjihgfedcbaponmlkjihgfedcba";

    private string _dir = "";

    [TestInitialize]
    public void Setup()
    {
        // テスト出力フォルダの中に作業用フォルダを作る（利用者の TEMP は使わない）
        _dir = Path.Combine(AppContext.BaseDirectory, "work", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(_dir, "repo", "webstore"));
        File.WriteAllText(Path.Combine(_dir, "repo", "webstore", "listing.txt"), "■ 説明\n本文です\n■ カテゴリ\n");
        File.WriteAllText(Path.Combine(_dir, "repo", "webstore", "whole.txt"), "全体の説明\n");
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_dir)) Directory.Delete(_dir, recursive: true);
    }

    private string ConfigPath => Path.Combine(_dir, "config.json");

    private EdictoraConfig Parse(string json, string? profile = null) => ConfigLoader.Parse(json, ConfigPath, profile);

    [TestMethod]
    public void Parse_相対パスを設定ファイルとbaseDir基準で解決する()
    {
        var config = Parse($$"""
            {
              // コメントと末尾カンマを許す
              "profileDir": "profile",
              "items": [
                {
                  "itemId": "{{ItemA}}",
                  "name": "Replace Translator",
                  "baseDir": "repo",
                  "locales": {
                    "ja": {
                      "description": { "file": "webstore/listing.txt", "sectionStart": "^■ 説明", "sectionEnd": "^■ カテゴリ" },
                      "homepageUrl": "https://example.com/",
                      "supportUrl": "",
                    },
                    "en": { "description": { "file": "webstore/whole.txt" } }
                  }
                }
              ]
            }
            """);

        Assert.AreEqual(Path.Combine(_dir, "profile"), config.ProfileDirectory);
        var item = config.Items.Single();
        Assert.AreEqual(ItemA, item.ItemId);
        Assert.AreEqual("Replace Translator", item.DisplayName);
        Assert.AreEqual(Path.Combine(_dir, "repo"), item.BaseDirectory);

        var ja = item.Locales.Single(l => l.Code == "ja");
        Assert.AreEqual(Path.Combine(_dir, "repo", "webstore", "listing.txt"), ja.Description!.FilePath);
        CollectionAssert.AreEqual(new[] { ListingField.Description, ListingField.HomepageUrl, ListingField.SupportUrl }, ja.Fields.ToArray());
        // 空文字は「空欄が正本」として対象に含める
        Assert.AreEqual("", ja.Urls[ListingField.SupportUrl]);
        // 書いていない欄は対象外
        Assert.IsFalse(ja.Urls.ContainsKey(ListingField.OfficialUrl));
    }

    [TestMethod]
    public void Parse_profileDir省略時は既定のプロファイル()
    {
        var config = Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { "homepageUrl": "https://example.com/" } } } ] }""");

        Assert.AreEqual(AppPaths.DefaultProfileDirectory, config.ProfileDirectory);
        Assert.AreEqual(ItemA, config.Items[0].DisplayName);
    }

    [TestMethod]
    public void Parse_profile指定は設定ファイルより優先する()
    {
        var other = Path.Combine(_dir, "other-profile");
        var config = Parse("""{ "profileDir": "profile", "items": [] }""", other);

        Assert.AreEqual(other, config.ProfileDirectory);
    }

    [TestMethod]
    public void Parse_アイテムIDの形式が違えば例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() =>
            Parse("""{ "items": [ { "itemId": "short", "locales": { "ja": { "homepageUrl": "https://example.com/" } } } ] }"""));
        StringAssert.Contains(ex.Message, "itemId");
    }

    [TestMethod]
    public void Parse_アイテムIDは小文字に揃え重複を検出する()
    {
        var upper = ItemA.ToUpperInvariant();
        var ex = Assert.ThrowsExactly<ConfigException>(() => Parse($$"""
            { "items": [
              { "itemId": "{{ItemA}}", "locales": { "ja": { "homepageUrl": "https://example.com/" } } },
              { "itemId": "{{upper}}", "locales": { "ja": { "homepageUrl": "https://example.com/" } } }
            ] }
            """));
        StringAssert.Contains(ex.Message, "重複");
    }

    [TestMethod]
    public void Parse_未知のキーは綴り間違いとして例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() =>
            Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { "homepageURL": "https://example.com/" } } } ] }"""));
        StringAssert.Contains(ex.Message, "形式");
    }

    [TestMethod]
    public void Parse_欄が1つも無いロケールは例外()
    {
        Assert.ThrowsExactly<ConfigException>(() =>
            Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { } } } ] }"""));
    }

    [TestMethod]
    public void Parse_URLがhttpでなければ例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() =>
            Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { "supportUrl": "mailto:a@example.com" } } } ] }"""));
        StringAssert.Contains(ex.Message, "supportUrl");
    }

    [TestMethod]
    public void Parse_正本ファイルが無ければ例外()
    {
        var ex = Assert.ThrowsExactly<ConfigException>(() =>
            Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { "description": { "file": "missing.txt" } } } } ] }"""));
        StringAssert.Contains(ex.Message, "正本ファイルがありません");
    }

    [TestMethod]
    public void Parse_区間の片側だけ指定は例外()
    {
        Assert.ThrowsExactly<ConfigException>(() => Parse($$"""
            { "items": [ { "itemId": "{{ItemA}}", "baseDir": "repo",
              "locales": { "ja": { "description": { "file": "webstore/listing.txt", "sectionStart": "^■ 説明" } } } } ] }
            """));
    }

    [TestMethod]
    public void Parse_ロケールコードの区切りを正規化し重複を検出する()
    {
        var config = Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "pt_BR": { "homepageUrl": "https://example.com/" } } } ] }""");
        Assert.AreEqual("pt-BR", config.Items[0].Locales[0].Code);

        Assert.ThrowsExactly<ConfigException>(() =>
            Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "pt_BR": { "homepageUrl": "https://a.example/" }, "pt-BR": { "homepageUrl": "https://b.example/" } } } ] }"""));
    }

    [TestMethod]
    public void Load_設定ファイルが無ければrequireFileで分岐する()
    {
        var missing = Path.Combine(_dir, "none.json");

        Assert.ThrowsExactly<ConfigException>(() => ConfigLoader.Load(missing));
        var empty = ConfigLoader.Load(missing, requireFile: false);
        Assert.IsEmpty(empty.Items);
    }

    [TestMethod]
    public void Build_正本の値を取り出しロケールで絞り込める()
    {
        var config = Parse($$"""
            { "items": [ { "itemId": "{{ItemB}}", "baseDir": "repo", "locales": {
              "ja": { "description": { "file": "webstore/listing.txt", "sectionStart": "^■ 説明", "sectionEnd": "^■ カテゴリ" } },
              "en": { "description": { "file": "webstore/whole.txt" }, "officialUrl": "https://example.com/en" }
            } } ] }
            """);

        var expected = ExpectedListingBuilder.Build(config.Items[0]);
        Assert.AreEqual("本文です", expected.Locales.Single(l => l.Code == "ja").Values[ListingField.Description]);
        Assert.AreEqual("全体の説明", expected.Locales.Single(l => l.Code == "en").Values[ListingField.Description]);

        var onlyEn = expected.FilterLocales(["en"]);
        Assert.AreEqual("en", onlyEn.Locales.Single().Code);
        Assert.ThrowsExactly<ConfigException>(() => expected.FilterLocales(["fr"]));
    }

    [TestMethod]
    public void Build_説明が上限を超えたら例外()
    {
        File.WriteAllText(Path.Combine(_dir, "long.txt"), new string('あ', ExpectedListingBuilder.DescriptionMaxLength + 1));
        var config = Parse($$"""{ "items": [ { "itemId": "{{ItemA}}", "locales": { "ja": { "description": { "file": "long.txt" } } } } ] }""");

        Assert.ThrowsExactly<ConfigException>(() => ExpectedListingBuilder.Build(config.Items[0]));
    }
}
