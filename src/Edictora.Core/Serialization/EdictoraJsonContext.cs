using System.Text.Json;
using System.Text.Json.Serialization;
using Edictora.Core.Config;

namespace Edictora.Core.Serialization;

/// <summary>
/// 設定ファイル（config.json）用の JSON シリアライズコンテキスト（NativeAOT / トリミング対応のソース生成）。
/// </summary>
/// <remarks>
/// - コメントと末尾カンマを許す（人が手で書く設定なので）。
/// - 未知のキーはエラーにする（綴り間違いの欄が黙って対象外になるのを防ぐ）。
/// </remarks>
[JsonSourceGenerationOptions(
    WriteIndented = true,
    PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase,
    PropertyNameCaseInsensitive = false,
    ReadCommentHandling = JsonCommentHandling.Skip,
    AllowTrailingCommas = true,
    UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow)]
[JsonSerializable(typeof(ConfigFileDto))]
[JsonSerializable(typeof(string))]
internal sealed partial class EdictoraJsonContext : JsonSerializerContext;
