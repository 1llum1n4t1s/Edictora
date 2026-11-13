using System.Text.RegularExpressions;
using SuperLightLogger;

namespace Edictora.Core.Logging;

/// <summary>ログレベル</summary>
public enum LogLevel
{
    /// <summary>デバッグレベル</summary>
    Debug,

    /// <summary>情報レベル</summary>
    Info,

    /// <summary>警告レベル</summary>
    Warning,

    /// <summary>エラーレベル</summary>
    Error
}

/// <summary>
/// SuperLightLogger を使用したログ出力クラス。
/// %LOCALAPPDATA%\Edictora\logs にローテーション出力する。
/// </summary>
/// <remarks>
/// ダッシュボードの URL には publisherId（パブリッシャーの識別子）が含まれる。
/// publisherId はコード・設定・ログに残さない方針なので、ログへ書く直前に
/// <see cref="Redact"/> で必ず伏せる（呼び出し側の書き忘れをここで吸収する）。
/// </remarks>
public static partial class LoggerService
{
    /// <summary>ロガーインスタンス</summary>
    private static ILog? _logger;

    /// <summary>初期化済みフラグ（スレッドセーフ: 0=未, 1=済）</summary>
    private static int _isConfigured;

    /// <summary>ログ出力ディレクトリ</summary>
    private static readonly string LogDirectory = AppPaths.LogDirectory;

    /// <summary>ログファイル名のプレフィックス</summary>
    private const string FilePrefix = "Edictora";

    /// <summary>ログ保持日数</summary>
    private const int RetentionDays = 7;

    /// <summary>最小ログレベル</summary>
    private static readonly LogLevel MinLogLevel =
#if DEBUG
        LogLevel.Debug;
#else
        LogLevel.Info;
#endif

    /// <summary>ロガーを初期化する（多重呼び出しは無視）</summary>
    public static void Initialize()
    {
        if (Interlocked.CompareExchange(ref _isConfigured, 1, 0) != 0) return;

        try
        {
            Directory.CreateDirectory(LogDirectory);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Trace.WriteLine(
                $"LoggerService.Initialize: ログディレクトリ '{LogDirectory}' の作成に失敗。ファイルログを無効化します: {ex.Message}");
            return;
        }

        var minLevel =
#if DEBUG
            "Debug";
#else
            "Information";
#endif

        LogManager.Configure(builder =>
        {
            builder.SetMinimumLevel(minLevel);
            builder.AddSuperLightFile(opt =>
            {
                opt.FileName = Path.Combine(LogDirectory, $"{FilePrefix}_${{shortdate}}.log");
                opt.Layout = "${longdate} [${level:uppercase=true}] ${message}";
                opt.ArchiveAboveSize = 10 * 1024 * 1024;
                opt.ArchiveFileName = Path.Combine(LogDirectory, $"{FilePrefix}_${{shortdate}}_{{#}}.log");
                opt.ArchiveNumbering = ArchiveNumbering.Sequence;
                opt.MaxArchiveFiles = 10;
                opt.Encoding = System.Text.Encoding.UTF8;
                opt.MinLevelName = minLevel;
            });
        });

        _logger = LogManager.GetLogger(FilePrefix);
        CleanupOldLogFiles(LogDirectory);
        Log("Logger initialized", LogLevel.Debug);
    }

    /// <summary>アプリケーション終了時に呼び出してログをフラッシュする</summary>
    public static void Shutdown()
    {
        LogManager.Shutdown();
        Interlocked.Exchange(ref _isConfigured, 0);
    }

    /// <summary>ログ出力ディレクトリのパスを取得する</summary>
    public static string GetLogDirectory() => LogDirectory;

    /// <summary>ログを出力する（publisherId は自動で伏せる）</summary>
    public static void Log(string message, LogLevel level = LogLevel.Info)
    {
        if (level < MinLogLevel) return;
        if (Volatile.Read(ref _isConfigured) == 0) Initialize();

        var safe = Redact(message);
        switch (level)
        {
            case LogLevel.Debug: _logger?.Debug(safe); break;
            case LogLevel.Info: _logger?.Info(safe); break;
            case LogLevel.Warning: _logger?.Warn(safe); break;
            case LogLevel.Error: _logger?.Error(safe); break;
            default: _logger?.Info(safe); break;
        }
    }

    /// <summary>例外情報を含むログを出力する（常に Error レベル）</summary>
    /// <remarks>
    /// 例外のメッセージやスタックにも URL が混ざりうるため、ToString() した全文を伏せてから書く。
    /// </remarks>
    public static void LogException(string message, Exception exception)
    {
        if (Volatile.Read(ref _isConfigured) == 0) Initialize();
        _logger?.Error(Redact($"{message}{Environment.NewLine}{exception}"));
    }

    /// <summary>
    /// 文字列中のダッシュボード URL から publisherId を伏せる。
    /// 「devconsole/」の直後のパス要素を &lt;publisher&gt; に置き換える。
    /// </summary>
    public static string Redact(string text)
    {
        if (string.IsNullOrEmpty(text)) return text;
        return DevconsolePathRegex().Replace(text, "${prefix}<publisher>");
    }

    [GeneratedRegex(@"(?<prefix>devconsole/)(?!<publisher>)[^/\s?#""'<>]+", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    private static partial Regex DevconsolePathRegex();

    /// <summary>保持期間を超えた古いログファイルを削除する（自アプリのログファイルだけが対象）</summary>
    internal static void CleanupOldLogFiles(string directory)
    {
        try
        {
            var cutoffDate = DateTime.Now.Date.AddDays(-RetentionDays);
            foreach (var file in Directory.GetFiles(directory, $"{FilePrefix}_*.log"))
            {
                // ${shortdate} は "2026-07-18" 形式で展開される
                var fileName = Path.GetFileNameWithoutExtension(file);
                var parts = fileName.Split('_');
                if (parts.Length >= 2 &&
                    DateTime.TryParseExact(parts[1], "yyyy-MM-dd", null, System.Globalization.DateTimeStyles.None, out var fileDate) &&
                    fileDate < cutoffDate)
                {
                    try { File.Delete(file); } catch { /* 使用中なら次回に回す */ }
                }
            }
        }
        catch (Exception ex)
        {
            Log($"ログファイルのクリーンアップ中にエラー: {ex.Message}", LogLevel.Warning);
        }
    }
}
