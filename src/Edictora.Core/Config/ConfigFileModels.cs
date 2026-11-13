namespace Edictora.Core.Config;

// 設定ファイル（config.json）をそのまま写した入力用の型。
// 値の検証・パス解決は ConfigLoader が行い、検証済みの EdictoraConfig に変換する。
// 「キーがない（null）」と「空文字」を区別するため、文字列はすべて null 許容にしている。
//   - URL 欄: キーなし → その欄は対象外 / "" → 空欄が正本（空にする）

/// <summary>config.json のルート</summary>
public sealed class ConfigFileDto
{
    /// <summary>Chrome プロファイルの場所（任意。相対パスは設定ファイルの場所が基準）</summary>
    public string? ProfileDir { get; set; }

    /// <summary>chrome.exe の場所（任意。省略時は自動検出）</summary>
    public string? ChromePath { get; set; }

    /// <summary>対象アイテムの一覧</summary>
    public List<ItemConfigDto>? Items { get; set; }
}

/// <summary>アイテム 1 件の設定</summary>
public sealed class ItemConfigDto
{
    /// <summary>CWS のアイテム ID（32 文字）</summary>
    public string? ItemId { get; set; }

    /// <summary>表示名（任意）</summary>
    public string? Name { get; set; }

    /// <summary>相対パスの基準（任意。相対なら設定ファイルの場所が基準）</summary>
    public string? BaseDir { get; set; }

    /// <summary>ロケールコード → そのロケールの正本</summary>
    public Dictionary<string, LocaleConfigDto>? Locales { get; set; }
}

/// <summary>ロケール 1 件の正本</summary>
public sealed class LocaleConfigDto
{
    /// <summary>説明欄の正本（任意）</summary>
    public TextSourceDto? Description { get; set; }

    /// <summary>ホームページ URL（任意）</summary>
    public string? HomepageUrl { get; set; }

    /// <summary>サポート URL（任意）</summary>
    public string? SupportUrl { get; set; }

    /// <summary>公式 URL（任意）</summary>
    public string? OfficialUrl { get; set; }
}

/// <summary>説明文の取り出し元</summary>
public sealed class TextSourceDto
{
    /// <summary>ファイルのパス（相対なら baseDir が基準）</summary>
    public string? File { get; set; }

    /// <summary>区間の開始見出し行の正規表現（任意。sectionEnd と組で指定）</summary>
    public string? SectionStart { get; set; }

    /// <summary>区間の終端見出し行の正規表現（任意。sectionStart と組で指定）</summary>
    public string? SectionEnd { get; set; }
}
