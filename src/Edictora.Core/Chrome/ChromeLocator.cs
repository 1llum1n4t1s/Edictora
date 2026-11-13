using Microsoft.Win32;

namespace Edictora.Core.Chrome;

/// <summary>インストール済み Chrome (chrome.exe) の場所を特定する</summary>
public static class ChromeLocator
{
    /// <summary>
    /// chrome.exe のフルパスを探す。見つからなければ null。
    /// 探索順: App Paths レジストリ → Program Files → LocalAppData。
    /// </summary>
    public static string? FindChrome()
    {
        // 1) App Paths レジストリ（標準インストールで最も確実）
        foreach (var hive in new[] { Registry.LocalMachine, Registry.CurrentUser })
        {
            try
            {
                using var key = hive.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe");
                if (key?.GetValue(null) is string path && File.Exists(path)) return path;
            }
            catch
            {
                // レジストリアクセス失敗は次の候補へ
            }
        }

        // 2) 既知のインストールパス
        var candidates = new[]
        {
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), @"Google\Chrome\Application\chrome.exe"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), @"Google\Chrome\Application\chrome.exe"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Google\Chrome\Application\chrome.exe"),
        };
        return candidates.FirstOrDefault(File.Exists);
    }
}
