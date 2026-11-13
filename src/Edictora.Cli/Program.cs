using System.Text;
using Edictora.Cli;
using Edictora.Core.Logging;

// Edictora CLI の入口。
// 日本語の出力を文字化けさせないよう、標準出力・標準エラーを UTF-8 にする。
Console.OutputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);

using var cts = new CancellationTokenSource();
Console.CancelKeyPress += (_, e) =>
{
    // 1 回目の Ctrl+C は中断要求として扱い、Chrome を正常終了させてから抜ける
    e.Cancel = true;
    cts.Cancel();
};

LoggerService.Initialize();
try
{
    var app = new CliApp(Console.Out, Console.Error);
    return await app.RunAsync(args, cts.Token);
}
finally
{
    LoggerService.Shutdown();
}
