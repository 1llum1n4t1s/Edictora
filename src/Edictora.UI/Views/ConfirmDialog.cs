using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;

namespace Edictora.UI.Views;

/// <summary>
/// 「実行 / キャンセル」の確認ダイアログ。下書き保存と審査送信の前に必ず出す。
/// </summary>
/// <remarks>
/// 既定のボタンは「キャンセル」側にする（Enter の押し間違いで送信しないため）。
/// XAML を持たない小さな部品なので、コードだけで組み立てる。
/// </remarks>
public static class ConfirmDialog
{
    /// <summary>ダイアログを表示し、「実行」が押されたら true</summary>
    public static async Task<bool> ShowAsync(Window owner, string title, string message)
    {
        var result = false;
        var dialog = new Window
        {
            Title = title,
            Width = 560,
            SizeToContent = SizeToContent.Height,
            MaxHeight = 640,
            CanResize = false,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            ShowInTaskbar = false,
        };

        var ok = new Button { Content = title, MinWidth = 120 };
        ok.Classes.Add("accent");
        var cancel = new Button { Content = "キャンセル", MinWidth = 120, IsDefault = true, IsCancel = true };
        ok.Click += (_, _) =>
        {
            result = true;
            dialog.Close();
        };
        cancel.Click += (_, _) => dialog.Close();

        dialog.Content = new StackPanel
        {
            Margin = new Avalonia.Thickness(20),
            Spacing = 16,
            Children =
            {
                new ScrollViewer
                {
                    MaxHeight = 480,
                    Content = new SelectableTextBlock { Text = message, TextWrapping = TextWrapping.Wrap },
                },
                new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    HorizontalAlignment = HorizontalAlignment.Right,
                    Spacing = 8,
                    Children = { cancel, ok },
                },
            },
        };

        await dialog.ShowDialog(owner);
        return result;
    }
}
