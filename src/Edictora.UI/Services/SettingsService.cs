using System.Text.Json;
using Edictora.Core;
using Edictora.Core.Logging;
using Edictora.UI.Serialization;

namespace Edictora.UI.Services;

/// <summary>画面の設定（settings.json に永続化）。正本の対応づけは config.json 側に持つ。</summary>
public sealed class AppSettings
{
    /// <summary>最後に開いた設定ファイル（null なら既定の %APPDATA%\Edictora\config.json）</summary>
    public string? ConfigPath { get; set; }
}

/// <summary>
/// %APPDATA%\Edictora\settings.json への画面設定の永続化。
/// UI スレッドからのみ触る前提の素朴な実装。
/// </summary>
public sealed class SettingsService
{
    private readonly string _settingsPath = AppPaths.UiSettingsPath;

    /// <summary>現在の設定</summary>
    public AppSettings Current { get; private set; } = new();

    public SettingsService()
    {
        Load();
    }

    /// <summary>settings.json から読み込む（破損時は既定値で継続）</summary>
    private void Load()
    {
        try
        {
            if (File.Exists(_settingsPath))
            {
                Current = JsonSerializer.Deserialize(File.ReadAllText(_settingsPath), EdictoraUiJsonContext.Default.AppSettings) ?? new AppSettings();
            }
        }
        catch (Exception ex)
        {
            LoggerService.Log($"画面設定の読み込みに失敗（既定値で継続）: {ex.Message}", LogLevel.Warning);
            Current = new AppSettings();
        }
    }

    /// <summary>設定を変更して保存する</summary>
    public void MutateAndSave(Action<AppSettings> mutate)
    {
        mutate(Current);
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_settingsPath)!);
            File.WriteAllText(_settingsPath, JsonSerializer.Serialize(Current, EdictoraUiJsonContext.Default.AppSettings));
        }
        catch (Exception ex)
        {
            LoggerService.Log($"画面設定の保存に失敗: {ex.Message}", LogLevel.Warning);
        }
    }
}
