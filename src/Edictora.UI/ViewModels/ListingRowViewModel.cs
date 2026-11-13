using CommunityToolkit.Mvvm.ComponentModel;
using Edictora.Core.Listing;

namespace Edictora.UI.ViewModels;

/// <summary>欄 1 つの左右比較（現在値 | 正本）</summary>
/// <param name="Label">欄の名前</param>
/// <param name="StatusLabel">状態の表示</param>
/// <param name="Current">ダッシュボードの現在値（未取得なら案内文）</param>
/// <param name="Expected">正本</param>
/// <param name="IsDifferent">差分ありか</param>
public sealed record FieldComparisonViewModel(string Label, string StatusLabel, string Current, string Expected, bool IsDifferent);

/// <summary>
/// 一覧の 1 行（アイテム × ロケール）。比較結果を表示用に持つだけで、操作ロジックは持たない。
/// </summary>
public sealed partial class ListingRowViewModel : ObservableObject
{
    public ListingRowViewModel(ExpectedItem item, ExpectedLocale locale)
    {
        ItemId = item.ItemId;
        ItemName = item.DisplayName;
        Locale = locale.Code;
        Apply(ListingComparer.Compare(item with { Locales = [locale] }, null)[0]);
    }

    /// <summary>アイテム ID</summary>
    public string ItemId { get; }

    /// <summary>表示名</summary>
    public string ItemName { get; }

    /// <summary>ロケール</summary>
    public string Locale { get; }

    /// <summary>下書き保存の対象として選ばれているか</summary>
    [ObservableProperty]
    public partial bool IsChecked { get; set; }

    /// <summary>行の状態</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(StatusLabel), nameof(IsMatch), nameof(IsDifferent), nameof(IsError), nameof(IsNotFetched))]
    public partial ComparisonStatus Status { get; set; }

    /// <summary>エラーの内容（なければ空）</summary>
    [ObservableProperty]
    public partial string ErrorText { get; set; } = "";

    /// <summary>欄ごとの比較</summary>
    [ObservableProperty]
    public partial IReadOnlyList<FieldComparisonViewModel> Fields { get; set; } = [];

    /// <summary>状態の表示名</summary>
    public string StatusLabel => Status.DisplayName();

    public bool IsMatch => Status == ComparisonStatus.Match;
    public bool IsDifferent => Status == ComparisonStatus.Different;
    public bool IsError => Status == ComparisonStatus.Error;
    public bool IsNotFetched => Status == ComparisonStatus.NotFetched;

    /// <summary>比較結果を反映する</summary>
    public void Apply(LocaleComparison comparison)
    {
        Status = comparison.Status;
        ErrorText = comparison.Error ?? "";
        Fields = [.. comparison.Fields.Select(f => new FieldComparisonViewModel(
            f.Field == ListingField.Description ? $"{f.Field.DisplayName()}（正本 {f.Expected.Length:N0} 文字）" : f.Field.DisplayName(),
            f.Status.DisplayName(),
            f.Current ?? "（未取得）",
            f.Expected,
            f.Status == ComparisonStatus.Different))];
        // 一致した行は保存対象から外す（誤って同じ内容を保存しない）
        if (Status == ComparisonStatus.Match) IsChecked = false;
    }

    /// <summary>エラーを反映する（欄の比較は未取得に戻す）</summary>
    public void ApplyError(string message)
    {
        Status = ComparisonStatus.Error;
        ErrorText = message;
    }
}
