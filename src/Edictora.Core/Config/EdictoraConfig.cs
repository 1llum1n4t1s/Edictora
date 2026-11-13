using Edictora.Core.Listing;

namespace Edictora.Core.Config;

/// <summary>設定ファイルの誤り（ブラウザを開く前に検出して止める）</summary>
public sealed class ConfigException(string message, Exception? inner = null) : Exception(message, inner);

/// <summary>検証済みの設定</summary>
/// <param name="SourcePath">読み込んだ設定ファイルの絶対パス（存在しない場合は想定パス）</param>
/// <param name="ProfileDirectory">Chrome プロファイルの絶対パス</param>
/// <param name="ChromePath">chrome.exe の指定（null なら自動検出）</param>
/// <param name="Items">対象アイテム</param>
public sealed record EdictoraConfig(
    string SourcePath,
    string ProfileDirectory,
    string? ChromePath,
    IReadOnlyList<ItemConfig> Items)
{
    /// <summary>アイテム ID で検索する（大文字小文字は区別しない）</summary>
    public ItemConfig? FindItem(string itemId)
        => Items.FirstOrDefault(i => string.Equals(i.ItemId, itemId, StringComparison.OrdinalIgnoreCase));
}

/// <summary>アイテム 1 件の設定（検証済み）</summary>
/// <param name="ItemId">32 文字のアイテム ID（小文字）</param>
/// <param name="DisplayName">表示名（未指定ならアイテム ID）</param>
/// <param name="BaseDirectory">相対パスの基準（絶対パス）</param>
/// <param name="Locales">ロケールごとの正本</param>
public sealed record ItemConfig(
    string ItemId,
    string DisplayName,
    string BaseDirectory,
    IReadOnlyList<LocaleConfig> Locales);

/// <summary>ロケール 1 件の正本（検証済み）</summary>
/// <param name="Code">ロケールコード（例: ja, en, pt-BR）。区切りは「-」に正規化済み</param>
/// <param name="Description">説明欄の取り出し元（null なら説明欄は対象外）</param>
/// <param name="Urls">URL 欄の正本（キーがない欄は対象外。空文字は「空欄が正本」）</param>
public sealed record LocaleConfig(
    string Code,
    TextSource? Description,
    IReadOnlyDictionary<ListingField, string> Urls)
{
    /// <summary>このロケールで対象にする欄（表示順）</summary>
    public IReadOnlyList<ListingField> Fields
        => [.. ListingFieldInfo.All.Where(f => f == ListingField.Description ? Description is not null : Urls.ContainsKey(f))];
}

/// <summary>説明文の取り出し元（検証済み）</summary>
/// <param name="FilePath">ファイルの絶対パス</param>
/// <param name="SectionStart">開始見出し行の正規表現（null ならファイル全体）</param>
/// <param name="SectionEnd">終端見出し行の正規表現（null ならファイル全体）</param>
public sealed record TextSource(string FilePath, string? SectionStart, string? SectionEnd);
