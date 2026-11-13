using Edictora.Core.Listing;

namespace Edictora.Core.Dashboard;

/// <summary>確認ダイアログ（「審査のため送信」を押した後）の内容</summary>
/// <param name="Text">ダイアログの文面</param>
/// <param name="AutoPublish">「審査の合格後に…自動的に公開する」チェックの状態（見つからなければ null）。ツールは変更しない</param>
public sealed record SubmitDialogInfo(string Text, bool? AutoPublish);

/// <summary>審査送信の結果</summary>
/// <param name="Submitted">送信を確定したか（確認で取りやめたら false）</param>
/// <param name="Dialog">確認ダイアログの内容</param>
/// <param name="Message">送信後の結果メッセージ</param>
/// <param name="StatusText">送信後のステータス</param>
public sealed record SubmitResult(bool Submitted, SubmitDialogInfo Dialog, string? Message, string? StatusText);

/// <summary>
/// アイテム 1 件の編集画面に対する操作。実装は CDP で画面を操作する <see cref="DashboardItemPage"/>。
/// </summary>
/// <remarks>
/// 手順の組み立て（差分判定・照合・中止判断）は <see cref="Services.ListingService"/> に置き、
/// この interface は「画面の 1 操作」だけを表す。ブラウザなしでも手順をテストできるようにするための境界。
/// </remarks>
public interface IListingPage
{
    /// <summary>アイテムの編集画面を開き、操作できる状態になるまで待つ</summary>
    Task OpenItemAsync(string itemId, CancellationToken ct);

    /// <summary>今の画面を読み込み直し、操作できる状態になるまで待つ（未保存の変更があれば例外）</summary>
    Task ReloadAsync(CancellationToken ct);

    /// <summary>書き込み可否の判断材料を読む</summary>
    Task<ItemPageState> GetStateAsync(CancellationToken ct);

    /// <summary>アイテムが持つロケールの一覧（言語の選択肢）</summary>
    Task<IReadOnlyList<string>> GetLocalesAsync(CancellationToken ct);

    /// <summary>編集中の言語を切り替える（すでにその言語なら何もしない）</summary>
    Task SwitchLocaleAsync(string code, CancellationToken ct);

    /// <summary>指定した欄の現在値を読む</summary>
    Task<IReadOnlyDictionary<ListingField, string>> ReadFieldsAsync(IReadOnlyCollection<ListingField> fields, CancellationToken ct);

    /// <summary>欄を書き換え、値が <paramref name="value"/> と完全一致することを確かめる（不一致なら例外）</summary>
    Task WriteFieldAsync(ListingField field, string value, CancellationToken ct);

    /// <summary>「下書きとして保存する」を押し、「保存されました」を確認する</summary>
    Task SaveDraftAsync(CancellationToken ct);

    /// <summary>
    /// 「審査のため送信」を押し、確認ダイアログを <paramref name="confirm"/> に渡す。
    /// true ならダイアログ内の「審査のため送信」、false なら「キャンセル」を押す。
    /// </summary>
    Task<SubmitResult> SubmitForReviewAsync(Func<SubmitDialogInfo, CancellationToken, Task<bool>> confirm, CancellationToken ct);

    /// <summary>失敗時の画面を診断用に保存する（失敗しても例外にしない）。保存先を返す。</summary>
    Task<string?> CaptureDiagnosticsAsync(string label, CancellationToken ct);
}
