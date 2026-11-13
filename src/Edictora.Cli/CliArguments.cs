using Edictora.Core.Config;

namespace Edictora.Cli;

/// <summary>サブコマンド</summary>
internal enum CliCommandKind
{
    /// <summary>使い方を表示</summary>
    Help,

    /// <summary>バージョンを表示</summary>
    Version,

    /// <summary>Chrome を開いて手動ログインを待つ</summary>
    Login,

    /// <summary>現在値を表示（読み取りのみ）</summary>
    Pull,

    /// <summary>正本との差分を表示（読み取りのみ・既定）</summary>
    Diff,

    /// <summary>差分のある欄を書き換えて下書き保存（--yes 必須）</summary>
    Push,

    /// <summary>審査に送信（--item 1 件と --yes 必須）</summary>
    Submit,
}

/// <summary>解釈済みのコマンドライン</summary>
internal sealed record CliCommand(
    CliCommandKind Kind,
    string? ConfigPath = null,
    string? ProfileDir = null,
    string? ChromePath = null,
    IReadOnlyList<string>? Items = null,
    IReadOnlyList<string>? Locales = null,
    bool Yes = false,
    TimeSpan? LoginWait = null)
{
    /// <summary>指定されたアイテム（未指定なら空）</summary>
    public IReadOnlyList<string> ItemIds => Items ?? [];

    /// <summary>指定されたロケール（未指定なら空）</summary>
    public IReadOnlyList<string> LocaleCodes => Locales ?? [];
}

/// <summary>引数の誤り（終了コード 2）</summary>
internal sealed class CliUsageException(string message) : Exception(message);

/// <summary>
/// コマンドライン引数の解釈。外部パーサを使わない小さな実装（NativeAOT とサプライチェーンの都合）。
/// </summary>
/// <remarks>
/// ここではブラウザも設定ファイルも触らず、形式の検証だけを行う。誤りはブラウザを開く前に <see cref="CliUsageException"/> で止める。
/// </remarks>
internal static class CliArguments
{
    /// <summary>コマンドごとに使えるオプション</summary>
    private static readonly Dictionary<CliCommandKind, string[]> AllowedOptions = new()
    {
        [CliCommandKind.Login] = ["--config", "--profile", "--chrome", "--wait"],
        [CliCommandKind.Pull] = ["--config", "--profile", "--chrome", "--item", "--locale"],
        [CliCommandKind.Diff] = ["--config", "--profile", "--chrome", "--item", "--locale"],
        [CliCommandKind.Push] = ["--config", "--profile", "--chrome", "--item", "--locale", "--yes"],
        [CliCommandKind.Submit] = ["--config", "--profile", "--chrome", "--item", "--yes"],
    };

    /// <summary>値を取るオプション</summary>
    private static readonly HashSet<string> ValueOptions = ["--config", "--profile", "--chrome", "--item", "--locale", "--wait"];

    /// <summary>引数を解釈する。誤りは <see cref="CliUsageException"/>。</summary>
    public static CliCommand Parse(IReadOnlyList<string> args)
    {
        if (args.Any(a => a is "--help" or "-h" or "/?" or "help")) return new CliCommand(CliCommandKind.Help);
        if (args.Any(a => a is "--version" or "-v" or "version")) return new CliCommand(CliCommandKind.Version);

        var index = 0;
        var kind = CliCommandKind.Diff; // 既定は差分表示（読み取りのみ）
        if (args.Count > 0 && !args[0].StartsWith('-'))
        {
            kind = args[0].ToLowerInvariant() switch
            {
                "login" => CliCommandKind.Login,
                "pull" => CliCommandKind.Pull,
                "diff" => CliCommandKind.Diff,
                "push" => CliCommandKind.Push,
                "submit" => CliCommandKind.Submit,
                _ => throw new CliUsageException($"不明なコマンドです: {args[0]}（--help で使い方を表示）"),
            };
            index = 1;
        }

        var commandName = kind.ToString().ToLowerInvariant();
        string? config = null, profile = null, chrome = null;
        TimeSpan? wait = null;
        var items = new List<string>();
        var locales = new List<string>();
        var yes = false;

        for (; index < args.Count; index++)
        {
            var arg = args[index];
            if (!arg.StartsWith("--", StringComparison.Ordinal))
            {
                throw new CliUsageException($"余分な引数です: {arg}");
            }

            // --name=value 形式にも対応する
            string name;
            string? value = null;
            var eq = arg.IndexOf('=');
            if (eq > 0)
            {
                name = arg[..eq];
                value = arg[(eq + 1)..];
            }
            else
            {
                name = arg;
            }
            name = name.ToLowerInvariant();

            if (!AllowedOptions[kind].Contains(name))
            {
                throw new CliUsageException($"{commandName} では {name} は使えません（--help で使い方を表示）");
            }

            if (ValueOptions.Contains(name))
            {
                if (value is null)
                {
                    if (index + 1 >= args.Count || args[index + 1].StartsWith("--", StringComparison.Ordinal))
                    {
                        throw new CliUsageException($"{name} の値がありません");
                    }
                    value = args[++index];
                }
                if (string.IsNullOrWhiteSpace(value)) throw new CliUsageException($"{name} の値が空です");
            }
            else if (value is not null)
            {
                throw new CliUsageException($"{name} は値を取りません");
            }

            switch (name)
            {
                case "--config": config = SetOnce(config, value!, name); break;
                case "--profile": profile = SetOnce(profile, value!, name); break;
                case "--chrome": chrome = SetOnce(chrome, value!, name); break;
                case "--item":
                    var id = value!.Trim().ToLowerInvariant();
                    if (!ConfigLoader.IsValidItemId(id)) throw new CliUsageException($"--item は 32 文字の CWS アイテム ID で指定してください: {value}");
                    if (!items.Contains(id)) items.Add(id);
                    break;
                case "--locale":
                    var code = ConfigLoader.NormalizeLocaleCode(value!);
                    if (!ConfigLoader.IsValidLocaleCode(code)) throw new CliUsageException($"--locale のロケールコードが正しくありません: {value}");
                    if (!locales.Contains(code, StringComparer.OrdinalIgnoreCase)) locales.Add(code);
                    break;
                case "--wait":
                    if (!int.TryParse(value, out var minutes) || minutes is < 1 or > 120)
                    {
                        throw new CliUsageException("--wait は 1〜120 の分数で指定してください");
                    }
                    wait = TimeSpan.FromMinutes(minutes);
                    break;
                case "--yes": yes = true; break;
            }
        }

        if (kind == CliCommandKind.Submit)
        {
            // 審査送信は取り返しがつかないので、複数アイテムの一括送信はさせない
            if (items.Count == 0) throw new CliUsageException("submit には --item <アイテム ID> を 1 件指定してください");
            if (items.Count > 1) throw new CliUsageException("submit は 1 件ずつ実行してください（--item は 1 つだけ指定できます）");
            if (!yes) throw new CliUsageException("submit は審査に送信します。実行するには --yes を付けてください");
        }

        return new CliCommand(kind, config, profile, chrome, items, locales, yes, wait);
    }

    private static string SetOnce(string? current, string value, string name)
        => current is null ? value : throw new CliUsageException($"{name} が 2 回指定されています");

    /// <summary>使い方</summary>
    public const string HelpText = """
        Edictora — Chrome Web Store の掲載情報を正本ファイルと一括で比較・更新するツール

        使い方:
          edictora login   [--config <file>] [--profile <dir>] [--chrome <exe>] [--wait <分>]
          edictora pull    [--item <id>]... [--locale <code>]... [共通オプション]
          edictora diff    [--item <id>]... [--locale <code>]... [共通オプション]   （既定のコマンド）
          edictora push    [--item <id>]... [--locale <code>]... --yes [共通オプション]
          edictora submit  --item <id> --yes [共通オプション]

        コマンド:
          login    専用プロファイルの Chrome を開き、手動ログインしてダッシュボードに着くまで待ちます
          pull     ダッシュボードの現在値を表示します（読み取りのみ）
          diff     正本との差分を表示します（読み取りのみ）
          push     差分のある欄だけを書き換えて下書き保存し、読み直して照合します。--yes が無ければ差分を表示して止まります
          submit   1 件のアイテムを審査に送信します（「自動的に公開する」の設定は変更しません）

        共通オプション:
          --config <file>   設定ファイル（既定: %APPDATA%\Edictora\config.json）
          --profile <dir>   Chrome プロファイル（既定: 設定ファイルの profileDir、なければ %LOCALAPPDATA%\Edictora\chrome-profile）
          --chrome <exe>    chrome.exe の場所（既定: 設定ファイルの chromePath、なければ自動検出）
          --item <id>       対象アイテム（複数可。省略時は設定ファイルの全アイテム）
          --locale <code>   対象ロケール（複数可。例: ja, en）
          --wait <分>       login で待つ時間（既定 10 分）
          --help            この説明を表示
          --version         バージョンを表示

        終了コード:
          0  成功（差分なし）
          1  実行時エラー（画面操作の失敗・照合の不一致など）
          2  引数・設定の誤り
          3  差分あり（diff、または --yes なしの push）
          4  書き込みを中止（審査待ち・状態を判定できない）
          5  ログインが必要（login を実行してください）
          130 中断（Ctrl+C）
        """;
}
