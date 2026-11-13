using System.Collections.Specialized;
using Avalonia.Controls;
using Avalonia.Platform.Storage;
using Edictora.UI.ViewModels;

namespace Edictora.UI.Views;

public sealed partial class MainWindow : Window
{
    /// <summary>XAML ローダー・デザイナー用（ViewModel は後から渡す）</summary>
    public MainWindow()
    {
        InitializeComponent();
    }

    /// <summary>アプリからの生成用</summary>
    public MainWindow(MainWindowViewModel viewModel) : this()
    {
        DataContext = viewModel;

        // ダイアログ表示は View の責務なのでデリゲート注入で VM から分離する
        viewModel.ConfirmAsync = (title, message) => ConfirmDialog.ShowAsync(this, title, message);
        viewModel.PickConfigFileAsync = PickConfigFileAsync;

        // ログ追記時に末尾へ自動スクロール
        viewModel.LogLines.CollectionChanged += OnLogLinesChanged;
    }

    private void OnLogLinesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        if (e.Action == NotifyCollectionChangedAction.Add)
        {
            LogScroll.ScrollToEnd();
        }
    }

    private async Task<string?> PickConfigFileAsync()
    {
        var result = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "設定ファイル（config.json）を選択",
            AllowMultiple = false,
            FileTypeFilter = [new FilePickerFileType("JSON") { Patterns = ["*.json"] }],
        });
        return result.Count > 0 ? result[0].Path.LocalPath : null;
    }
}
