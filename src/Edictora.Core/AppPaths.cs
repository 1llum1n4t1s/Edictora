namespace Edictora.Core;

/// <summary>
/// Edictora が使う既定の保存先を 1 か所にまとめたもの。
/// </summary>
/// <remarks>
/// - 利用者が書く設定（正本の対応づけ）は移動プロファイル側の %APPDATA%\Edictora に置く。
/// - Chrome プロファイル・ログ・診断情報は端末ローカルの %LOCALAPPDATA%\Edictora に置く。
///   Chrome プロファイルにはログイン情報（Cookie）が入るため、ローミングさせない。
/// - いずれもリポジトリの外。診断情報（画面の保存）にも個人の画面が写るので、リポジトリへは置かない。
/// </remarks>
public static class AppPaths
{
    /// <summary>アプリ名（保存先フォルダ名に使う）</summary>
    public const string AppName = "Edictora";

    /// <summary>%LOCALAPPDATA%\Edictora</summary>
    public static string LocalRoot { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), AppName);

    /// <summary>%APPDATA%\Edictora</summary>
    public static string RoamingRoot { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), AppName);

    /// <summary>既定の設定ファイル（%APPDATA%\Edictora\config.json）</summary>
    public static string DefaultConfigPath { get; } = Path.Combine(RoamingRoot, "config.json");

    /// <summary>UI の画面設定（%APPDATA%\Edictora\settings.json）</summary>
    public static string UiSettingsPath { get; } = Path.Combine(RoamingRoot, "settings.json");

    /// <summary>既定の Chrome プロファイル（%LOCALAPPDATA%\Edictora\chrome-profile）。秘密情報として扱う。</summary>
    public static string DefaultProfileDirectory { get; } = Path.Combine(LocalRoot, "chrome-profile");

    /// <summary>ログ（%LOCALAPPDATA%\Edictora\logs）</summary>
    public static string LogDirectory { get; } = Path.Combine(LocalRoot, "logs");

    /// <summary>失敗時の画面保存先（%LOCALAPPDATA%\Edictora\diagnostics）</summary>
    public static string DiagnosticsDirectory { get; } = Path.Combine(LocalRoot, "diagnostics");
}
