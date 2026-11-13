using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Edictora.Core.Logging;
using Edictora.UI.Services;
using Edictora.UI.ViewModels;
using Edictora.UI.Views;
using Microsoft.Extensions.DependencyInjection;

namespace Edictora.UI;

public sealed partial class App : Application
{
    public override void Initialize() => AvaloniaXamlLoader.Load(this);

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // 未捕捉例外は最低限ログに残す
            AppDomain.CurrentDomain.UnhandledException += (_, e) =>
                LoggerService.LogException("未捕捉例外 (AppDomain)", e.ExceptionObject as Exception ?? new InvalidOperationException("unknown"));
            TaskScheduler.UnobservedTaskException += (_, e) =>
            {
                LoggerService.LogException("未捕捉例外 (Task)", e.Exception);
                e.SetObserved();
            };

            var services = new ServiceCollection();
            services.AddSingleton<SettingsService>();
            services.AddSingleton<MainWindowViewModel>();
            var provider = services.BuildServiceProvider();

            var viewModel = provider.GetRequiredService<MainWindowViewModel>();
            desktop.MainWindow = new MainWindow(viewModel);

            // 終了時に、自分で起動した Chrome を正常終了させる（プロファイルは残す）。
            // Core 側は ConfigureAwait(false) なので、UI スレッドで待っても固まらない。上限を設けて待つ。
            desktop.Exit += (_, _) =>
            {
                try
                {
                    Task.Run(async () => await viewModel.DisposeAsync()).Wait(TimeSpan.FromSeconds(30));
                }
                catch (Exception ex)
                {
                    LoggerService.LogException("終了時の後始末に失敗", ex);
                }
            };
        }

        base.OnFrameworkInitializationCompleted();
    }
}
