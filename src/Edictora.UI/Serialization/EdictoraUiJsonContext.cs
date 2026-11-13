using System.Text.Json.Serialization;
using Edictora.UI.Services;

namespace Edictora.UI.Serialization;

/// <summary>画面設定 (settings.json) 用 JSON シリアライズコンテキスト（NativeAOT / トリミング対応）。</summary>
[JsonSourceGenerationOptions(WriteIndented = true, PropertyNameCaseInsensitive = true)]
[JsonSerializable(typeof(AppSettings))]
internal sealed partial class EdictoraUiJsonContext : JsonSerializerContext;
