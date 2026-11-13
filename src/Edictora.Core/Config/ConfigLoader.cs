using System.Text.Json;
using System.Text.RegularExpressions;
using Edictora.Core.Listing;
using Edictora.Core.Serialization;

namespace Edictora.Core.Config;

/// <summary>
/// 設定ファイル（config.json）を読み込み、検証して <see cref="EdictoraConfig"/> にする。
/// </summary>
/// <remarks>
/// ブラウザを開く前に、設定の誤りをすべてここで検出して止める（途中まで書き換えてから設定ミスに気づく事態を避ける）。
/// 未知のキーはエラーにする。綴り間違いの欄が黙って「対象外」になり、更新漏れに気づけなくなるのを防ぐため。
/// </remarks>
public static partial class ConfigLoader
{
    /// <summary>
    /// 設定を読み込む。
    /// </summary>
    /// <param name="configPath">設定ファイルのパス（null なら既定の %APPDATA%\Edictora\config.json）</param>
    /// <param name="profileOverride">CLI の --profile 指定（設定ファイルより優先。相対パスはカレントディレクトリ基準）</param>
    /// <param name="chromeOverride">CLI の --chrome 指定（設定ファイルより優先）</param>
    /// <param name="requireFile">true なら設定ファイルが無いときにエラー。false なら空の設定で続行（login 用）</param>
    public static EdictoraConfig Load(string? configPath, string? profileOverride = null, string? chromeOverride = null, bool requireFile = true)
    {
        var path = Path.GetFullPath(Expand(configPath) ?? AppPaths.DefaultConfigPath);
        if (!File.Exists(path))
        {
            if (requireFile)
            {
                throw new ConfigException($"設定ファイルがありません: {path}（README の設定例を参考に作成してください）");
            }
            return new EdictoraConfig(path, ResolveProfile(profileOverride, null, Path.GetDirectoryName(path)!), Expand(chromeOverride), []);
        }

        string json;
        try
        {
            json = File.ReadAllText(path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new ConfigException($"設定ファイルを読めません: {path}（{ex.Message}）", ex);
        }

        return Parse(json, path, profileOverride, chromeOverride);
    }

    /// <summary>JSON 文字列から設定を作る（テスト用に公開）</summary>
    /// <param name="json">設定ファイルの内容</param>
    /// <param name="configPath">設定ファイルの絶対パス（相対パスの基準になる）</param>
    /// <param name="profileOverride">--profile 指定</param>
    /// <param name="chromeOverride">--chrome 指定</param>
    public static EdictoraConfig Parse(string json, string configPath, string? profileOverride = null, string? chromeOverride = null)
    {
        ConfigFileDto? dto;
        try
        {
            dto = JsonSerializer.Deserialize(json, EdictoraJsonContext.Default.ConfigFileDto);
        }
        catch (JsonException ex)
        {
            throw new ConfigException($"設定ファイルの形式が正しくありません: {configPath}（{ex.Message}）", ex);
        }
        if (dto is null) throw new ConfigException($"設定ファイルが空です: {configPath}");

        var configDir = Path.GetDirectoryName(Path.GetFullPath(configPath))!;
        var profile = ResolveProfile(profileOverride, dto.ProfileDir, configDir);
        var chrome = Expand(chromeOverride) ?? Expand(dto.ChromePath);

        var items = new List<ItemConfig>();
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var index = 0;
        foreach (var itemDto in dto.Items ?? [])
        {
            index++;
            var item = ParseItem(itemDto, index, configDir);
            if (!seen.Add(item.ItemId))
            {
                throw new ConfigException($"items[{index}]: アイテム ID が重複しています: {item.ItemId}");
            }
            items.Add(item);
        }

        return new EdictoraConfig(Path.GetFullPath(configPath), profile, chrome, items);
    }

    /// <summary>アイテム ID の形式か（CWS の ID は a〜p の 32 文字）</summary>
    public static bool IsValidItemId(string? value) => value is not null && ItemIdRegex().IsMatch(value);

    /// <summary>ロケールコードを比較用に正規化する（「_」を「-」に寄せる。大文字小文字はそのまま）</summary>
    public static string NormalizeLocaleCode(string code) => code.Trim().Replace('_', '-');

    /// <summary>ロケールコードの形式か（例: ja, en, pt-BR, es-419, zh-CN）</summary>
    public static bool IsValidLocaleCode(string? value) => value is not null && LocaleCodeRegex().IsMatch(value);

    private static ItemConfig ParseItem(ItemConfigDto dto, int index, string configDir)
    {
        var where = $"items[{index}]";
        var itemId = dto.ItemId?.Trim().ToLowerInvariant();
        if (!IsValidItemId(itemId))
        {
            throw new ConfigException($"{where}: itemId は 32 文字の CWS アイテム ID（a〜p）で指定してください: {dto.ItemId ?? "(未指定)"}");
        }
        where = $"{where} ({itemId})";

        var baseDir = string.IsNullOrWhiteSpace(dto.BaseDir)
            ? configDir
            : Path.GetFullPath(Path.Combine(configDir, Expand(dto.BaseDir)!));
        if (!Directory.Exists(baseDir))
        {
            throw new ConfigException($"{where}: baseDir がありません: {baseDir}");
        }

        if (dto.Locales is null || dto.Locales.Count == 0)
        {
            throw new ConfigException($"{where}: locales を 1 つ以上指定してください");
        }

        var locales = new List<LocaleConfig>();
        var seenCodes = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var (rawCode, localeDto) in dto.Locales)
        {
            var code = NormalizeLocaleCode(rawCode);
            if (!IsValidLocaleCode(code))
            {
                throw new ConfigException($"{where}: ロケールコードが正しくありません: {rawCode}");
            }
            if (!seenCodes.Add(code))
            {
                throw new ConfigException($"{where}: ロケールコードが重複しています: {rawCode}");
            }
            if (localeDto is null)
            {
                throw new ConfigException($"{where}.{code}: 内容が空です");
            }
            locales.Add(ParseLocale(localeDto, code, $"{where}.{code}", baseDir));
        }

        var name = string.IsNullOrWhiteSpace(dto.Name) ? itemId! : dto.Name.Trim();
        return new ItemConfig(itemId!, name, baseDir, locales);
    }

    private static LocaleConfig ParseLocale(LocaleConfigDto dto, string code, string where, string baseDir)
    {
        TextSource? description = null;
        if (dto.Description is not null)
        {
            var d = dto.Description;
            if (string.IsNullOrWhiteSpace(d.File))
            {
                throw new ConfigException($"{where}.description: file を指定してください");
            }
            if ((d.SectionStart is null) != (d.SectionEnd is null))
            {
                throw new ConfigException($"{where}.description: sectionStart と sectionEnd は両方指定してください");
            }
            if (d.SectionStart is not null) SectionExtractor.ValidatePattern(d.SectionStart, $"{where}.description.sectionStart");
            if (d.SectionEnd is not null) SectionExtractor.ValidatePattern(d.SectionEnd, $"{where}.description.sectionEnd");

            var file = Path.GetFullPath(Path.Combine(baseDir, Expand(d.File)!));
            if (!File.Exists(file))
            {
                throw new ConfigException($"{where}.description: 正本ファイルがありません: {file}");
            }
            description = new TextSource(file, d.SectionStart, d.SectionEnd);
        }

        var urls = new Dictionary<ListingField, string>();
        AddUrl(urls, ListingField.OfficialUrl, dto.OfficialUrl, where);
        AddUrl(urls, ListingField.HomepageUrl, dto.HomepageUrl, where);
        AddUrl(urls, ListingField.SupportUrl, dto.SupportUrl, where);

        if (description is null && urls.Count == 0)
        {
            throw new ConfigException($"{where}: description・homepageUrl・supportUrl・officialUrl のいずれかを指定してください");
        }

        return new LocaleConfig(code, description, urls);
    }

    private static void AddUrl(Dictionary<ListingField, string> urls, ListingField field, string? value, string where)
    {
        // キーなし（null）は対象外。空文字は「空欄が正本」として扱う。
        if (value is null) return;
        var trimmed = value.Trim();
        if (trimmed.Length > 0 &&
            (!Uri.TryCreate(trimmed, UriKind.Absolute, out var uri) || (uri.Scheme != Uri.UriSchemeHttps && uri.Scheme != Uri.UriSchemeHttp)))
        {
            throw new ConfigException($"{where}.{field.ConfigKey()}: http(s) の絶対 URL を指定してください: {value}");
        }
        urls[field] = trimmed;
    }

    private static string ResolveProfile(string? profileOverride, string? configured, string configDir)
    {
        // 優先順位: --profile（カレント基準） > 設定ファイル（設定ファイルの場所基準） > 既定
        if (!string.IsNullOrWhiteSpace(profileOverride)) return Path.GetFullPath(Expand(profileOverride)!);
        if (!string.IsNullOrWhiteSpace(configured)) return Path.GetFullPath(Path.Combine(configDir, Expand(configured)!));
        return AppPaths.DefaultProfileDirectory;
    }

    /// <summary>%LOCALAPPDATA% などの環境変数を展開する</summary>
    private static string? Expand(string? value)
        => string.IsNullOrWhiteSpace(value) ? null : Environment.ExpandEnvironmentVariables(value.Trim());

    [GeneratedRegex("^[a-p]{32}$", RegexOptions.CultureInvariant)]
    private static partial Regex ItemIdRegex();

    [GeneratedRegex("^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$", RegexOptions.CultureInvariant)]
    private static partial Regex LocaleCodeRegex();
}
