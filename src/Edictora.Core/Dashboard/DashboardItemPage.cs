using System.Text.Json;
using Edictora.Core.Cdp;
using Edictora.Core.Listing;
using Edictora.Core.Logging;

namespace Edictora.Core.Dashboard;

/// <summary>
/// アイテムの編集画面（ストアの掲載情報）を CDP で操作する <see cref="IListingPage"/> 実装。
/// </summary>
/// <remarks>
/// 2026-11-13 の検証（DESIGN.md）で確かめた操作手順をそのまま使う。
/// - 操作の前に毎回 Page.bringToFront と Emulation.setFocusEmulationEnabled を行う
/// - クリックは scrollIntoView → 矩形の中心へ実際のマウス入力（他要素に覆われていたら止める）
/// - 書き換えはフォーカス → 全選択 → Input.insertText。DOM の value への直接代入はしない
/// - 待ちは固定時間の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）
/// </remarks>
internal sealed class DashboardItemPage(CdpPage page, string publisherId) : IListingPage
{
    /// <summary>条件確認の間隔</summary>
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(300);

    private string? _itemId;

    public async Task OpenItemAsync(string itemId, CancellationToken ct)
    {
        _itemId = itemId;
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        try
        {
            await page.NavigateAsync(DashboardUrls.EditUrl(publisherId, itemId), ct).ConfigureAwait(false);
        }
        catch (CdpException ex)
        {
            throw new DashboardException($"アイテム {itemId} の編集画面を開けませんでした: {ex.Message}", ex);
        }

        var url = await page.GetUrlAsync(ct).ConfigureAwait(false);
        if (DashboardUrls.IsLoginPage(url)) throw new LoginRequiredException();
        await WaitForEditorReadyAsync(ct).ConfigureAwait(false);

        // 読み込み後に別画面（一覧など）へ移されていないか確認する。アイテム ID の誤りで起きる。
        url = await page.GetUrlAsync(ct).ConfigureAwait(false);
        if (!DashboardUrls.IsEditPageOf(url, itemId))
        {
            throw new DashboardException($"アイテム {itemId} の編集画面を開けませんでした（別の画面へ移りました）。アイテム ID とパブリッシャーを確認してください");
        }
    }

    public async Task ReloadAsync(CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        try
        {
            await page.ReloadAsync(ct).ConfigureAwait(false);
        }
        catch (CdpException ex)
        {
            throw new DashboardException($"編集画面を読み込み直せませんでした: {ex.Message}", ex);
        }
        await WaitForEditorReadyAsync(ct).ConfigureAwait(false);
    }

    /// <summary>SPA なので load 後も中身が遅れて出る。言語コンボボックスと説明欄が揃うまで待つ。</summary>
    private Task WaitForEditorReadyAsync(CancellationToken ct)
        => PollAsync(async () =>
        {
            var combos = await CollectAsync(PageScripts.Kinds.Combobox, ct).ConfigureAwait(false);
            var textareas = await CollectAsync(PageScripts.Kinds.Textarea, ct).ConfigureAwait(false);
            var hasLanguage = combos.Any(c => ElementSelectors.Contains(c.Label, DashboardText.LanguageLabel) ||
                                              ElementSelectors.Contains(c.Text, DashboardText.LanguageLabel));
            return hasLanguage && textareas.Count > 0 ? true : (bool?)null;
        }, TimeSpan.FromSeconds(45), $"アイテム {_itemId} の編集画面（言語の選択欄と説明欄）の表示", ct);

    public async Task<ItemPageState> GetStateAsync(CancellationToken ct)
    {
        var status = ElementSelectors.ParseStatus(await BodyTextAsync(ct).ConfigureAwait(false));
        var textareas = await CollectAsync(PageScripts.Kinds.Textarea, ct).ConfigureAwait(false);
        var buttons = await CollectAsync(PageScripts.Kinds.Button, ct).ConfigureAwait(false);
        var saves = buttons.Where(b => string.Equals(b.Text.Trim(), DashboardText.SaveDraftButton, StringComparison.Ordinal)).ToList();
        return new ItemPageState(
            status,
            textareas.Count == 1,
            textareas.Count == 1 && !textareas[0].Disabled && !textareas[0].ReadOnly,
            saves.Count == 1,
            saves.Count == 1 && !saves[0].Disabled);
    }

    public async Task<IReadOnlyList<string>> GetLocalesAsync(CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        var combo = ElementSelectors.PickLanguageCombobox(await CollectAsync(PageScripts.Kinds.Combobox, ct).ConfigureAwait(false));
        var current = ElementSelectors.CurrentLocaleCode(combo);
        await ClickAsync(PageScripts.Kinds.Combobox, combo.Index, "言語の選択欄", ct).ConfigureAwait(false);

        var options = await WaitForOptionsAsync(ct).ConfigureAwait(false);
        var codes = ElementSelectors.LocaleCodes(options);

        // 一覧を閉じる。Escape で閉じなければ、今の言語の選択肢を押し直して閉じる（言語は変わらない）。
        // 開閉は言語欄の aria-expanded で判定する。選択肢の要素は閉じてもページに残り、
        // パブリッシャー名の選択肢も常に表示されているため、「選択肢が 0 件になったか」では判定できない。
        await page.PressKeyAsync("Escape", 27, ct).ConfigureAwait(false);
        var closed = await TryPollAsync(async () => await IsLanguageListClosedAsync(ct).ConfigureAwait(false) ? true : (bool?)null,
            TimeSpan.FromSeconds(3), ct).ConfigureAwait(false);
        if (!closed && current is not null)
        {
            var again = await CollectAsync(PageScripts.Kinds.Option, ct).ConfigureAwait(false);
            var option = ElementSelectors.PickLocaleOption(again, current);
            await ClickAsync(PageScripts.Kinds.Option, option.Index, "言語の選択肢", ct).ConfigureAwait(false);
        }
        await PollAsync(async () => await IsLanguageListClosedAsync(ct).ConfigureAwait(false) ? true : (bool?)null,
            TimeSpan.FromSeconds(5), "言語の選択肢を閉じる", ct).ConfigureAwait(false);

        if (codes.Count == 0) throw new DashboardException("言語の選択肢からロケールを読み取れませんでした");
        return codes;
    }

    public async Task SwitchLocaleAsync(string code, CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        var combo = ElementSelectors.PickLanguageCombobox(await CollectAsync(PageScripts.Kinds.Combobox, ct).ConfigureAwait(false));
        if (string.Equals(ElementSelectors.CurrentLocaleCode(combo), code, StringComparison.OrdinalIgnoreCase)) return;

        await ClickAsync(PageScripts.Kinds.Combobox, combo.Index, "言語の選択欄", ct).ConfigureAwait(false);
        var options = await WaitForOptionsAsync(ct).ConfigureAwait(false);
        var option = ElementSelectors.PickLocaleOption(options, code);
        await ClickAsync(PageScripts.Kinds.Option, option.Index, $"言語の選択肢（{code}）", ct).ConfigureAwait(false);

        // 選択欄の表示が切り替わるまで待つ
        await PollAsync(async () =>
        {
            var c = ElementSelectors.PickLanguageCombobox(await CollectAsync(PageScripts.Kinds.Combobox, ct).ConfigureAwait(false));
            return string.Equals(ElementSelectors.CurrentLocaleCode(c), code, StringComparison.OrdinalIgnoreCase) ? true : (bool?)null;
        }, TimeSpan.FromSeconds(15), $"言語 {code} への切り替え", ct).ConfigureAwait(false);

        // 欄の中身が新しい言語のものに入れ替わり、落ち着くまで待つ（説明欄の値が 2 回続けて同じになるまで）
        string? previous = null;
        await PollAsync(async () =>
        {
            await Task.Delay(TimeSpan.FromMilliseconds(400), ct).ConfigureAwait(false);
            var textarea = ElementSelectors.PickDescription(await CollectAsync(PageScripts.Kinds.Textarea, ct).ConfigureAwait(false));
            var value = await ValueAsync(PageScripts.Kinds.Textarea, textarea.Index, ct).ConfigureAwait(false);
            var stable = previous is not null && string.Equals(previous, value, StringComparison.Ordinal);
            previous = value;
            return stable ? true : (bool?)null;
        }, TimeSpan.FromSeconds(15), $"言語 {code} の内容の表示", ct).ConfigureAwait(false);
    }

    /// <summary>
    /// 言語の一覧が開くまで待ち、表示中の選択肢を返す。
    /// パブリッシャー名の選択肢は常に表示されているので、「言語 – code」形式の選択肢が現れるまで待つ。
    /// </summary>
    private Task<IReadOnlyList<ElementInfo>> WaitForOptionsAsync(CancellationToken ct)
        => PollAsync(async () =>
        {
            var options = await CollectAsync(PageScripts.Kinds.Option, ct).ConfigureAwait(false);
            return options.Any(o => ElementSelectors.ParseLocaleOption(o.Text) is not null) ? options : null;
        }, TimeSpan.FromSeconds(10), "言語の選択肢の表示", ct);

    /// <summary>言語欄の aria-expanded が "false" か（一覧が閉じているか）</summary>
    private async Task<bool> IsLanguageListClosedAsync(CancellationToken ct)
    {
        var combo = ElementSelectors.PickLanguageCombobox(await CollectAsync(PageScripts.Kinds.Combobox, ct).ConfigureAwait(false));
        return string.Equals(combo.Expanded, "false", StringComparison.Ordinal);
    }

    public async Task<IReadOnlyDictionary<ListingField, string>> ReadFieldsAsync(IReadOnlyCollection<ListingField> fields, CancellationToken ct)
    {
        var result = new Dictionary<ListingField, string>();
        foreach (var field in fields)
        {
            var (kind, element) = await LocateAsync(field, ct).ConfigureAwait(false);
            result[field] = await ValueAsync(kind, element.Index, ct).ConfigureAwait(false);
        }
        return result;
    }

    public async Task WriteFieldAsync(ListingField field, string value, CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        var (kind, element) = await LocateAsync(field, ct).ConfigureAwait(false);
        ElementSelectors.EnsureWritable(element, field);

        await EvaluateAsync(PageScripts.ScrollTo(kind, element.Index), ct).ConfigureAwait(false);
        var selected = await EvaluateAsync(PageScripts.FocusSelectAll(kind, element.Index), ct).ConfigureAwait(false);
        if (selected.ValueKind != JsonValueKind.True)
        {
            throw new DashboardException($"{field.DisplayName()}の欄にフォーカスして全選択できませんでした");
        }

        if (value.Length == 0)
        {
            // Input.insertText("") では選択範囲が消えないため、Delete キーで消す
            await page.PressKeyAsync("Delete", 46, ct).ConfigureAwait(false);
        }
        else
        {
            await page.InsertTextAsync(value, ct).ConfigureAwait(false);
        }

        // 書き換え後の値が正本と完全一致するまで待つ。一致しなければ保存せずに止める。
        string? last = null;
        var ok = await TryPollAsync(async () =>
        {
            last = await ValueAsync(kind, element.Index, ct).ConfigureAwait(false);
            return string.Equals(last, value, StringComparison.Ordinal) ? true : (bool?)null;
        }, TimeSpan.FromSeconds(10), ct).ConfigureAwait(false);
        if (!ok)
        {
            var at = TextDiff.FirstDifferenceIndex(last ?? "", value);
            throw new DashboardException(
                $"{field.DisplayName()}を書き換えましたが、値が正本と一致しません（{at} 文字目から相違、現在 {last?.Length ?? 0} 文字 / 正本 {value.Length} 文字）。保存せずに止めます");
        }
    }

    public async Task SaveDraftAsync(CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);

        // 前回の「保存されました」が残っていると成功を誤認するので、消えるまで待つ
        var clean = await TryPollAsync(async () =>
            ElementSelectors.HasMessage(await MessagesAsync(ct).ConfigureAwait(false), DashboardText.SavedMessage) ? null : (bool?)true,
            TimeSpan.FromSeconds(10), ct).ConfigureAwait(false);
        if (!clean) throw new DashboardException("前回の保存メッセージが消えないため、保存結果を確かめられません。保存を止めます");

        var button = ElementSelectors.PickButton(await CollectAsync(PageScripts.Kinds.Button, ct).ConfigureAwait(false), DashboardText.SaveDraftButton);
        if (button.Disabled) throw new WriteBlockedException($"「{DashboardText.SaveDraftButton}」ボタンが押せない状態です");
        await ClickAsync(PageScripts.Kinds.Button, button.Index, $"「{DashboardText.SaveDraftButton}」ボタン", ct).ConfigureAwait(false);

        IReadOnlyList<string> messages = [];
        var saved = await TryPollAsync(async () =>
        {
            messages = await MessagesAsync(ct).ConfigureAwait(false);
            return ElementSelectors.HasMessage(messages, DashboardText.SavedMessage) ? true : (bool?)null;
        }, TimeSpan.FromSeconds(30), ct).ConfigureAwait(false);
        if (!saved)
        {
            var shown = ElementSelectors.ResultMessages(messages);
            throw new DashboardException(
                $"下書き保存の完了メッセージ（{DashboardText.SavedMessage}）を確認できませんでした" +
                (shown.Count > 0 ? $"。表示: {string.Join(" / ", shown)}" : ""));
        }
        LoggerService.Log($"下書き保存を確認: {_itemId}", LogLevel.Info);
    }

    public async Task<SubmitResult> SubmitForReviewAsync(Func<SubmitDialogInfo, CancellationToken, Task<bool>> confirm, CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        if ((await DialogsAsync(ct).ConfigureAwait(false)).Count > 0)
        {
            throw new DashboardException("別のダイアログが開いているため、審査送信を始めません");
        }

        var button = ElementSelectors.PickButton(await CollectAsync(PageScripts.Kinds.Button, ct).ConfigureAwait(false), DashboardText.SubmitButton);
        if (button.Disabled) throw new WriteBlockedException($"「{DashboardText.SubmitButton}」ボタンが押せない状態です");
        await ClickAsync(PageScripts.Kinds.Button, button.Index, $"「{DashboardText.SubmitButton}」ボタン", ct).ConfigureAwait(false);

        // 確認ダイアログがちょうど 1 つ出るまで待つ
        var dialogText = await PollAsync(async () =>
        {
            var dialogs = await DialogsAsync(ct).ConfigureAwait(false);
            if (dialogs.Count > 1) throw new DashboardException($"ダイアログが {dialogs.Count} 個開いており、確認ダイアログを特定できません");
            return dialogs.Count == 1 ? dialogs[0] : null;
        }, TimeSpan.FromSeconds(15), "審査送信の確認ダイアログの表示", ct).ConfigureAwait(false);

        // 自動公開チェックは読むだけで変更しない
        var checks = await CollectAsync(PageScripts.Kinds.DialogCheckbox, ct).ConfigureAwait(false);
        var autoPublish = checks.FirstOrDefault(c => ElementSelectors.Contains(c.Label, DashboardText.AutoPublishMarker))?.Checked
            ?? (checks.Count == 1 ? checks[0].Checked : null);
        var info = new SubmitDialogInfo(dialogText, autoPublish);

        var dialogButtons = await CollectAsync(PageScripts.Kinds.DialogButton, ct).ConfigureAwait(false);
        if (!await confirm(info, ct).ConfigureAwait(false))
        {
            var cancel = ElementSelectors.PickButton(dialogButtons, DashboardText.CancelButton);
            await ClickAsync(PageScripts.Kinds.DialogButton, cancel.Index, "確認ダイアログの「キャンセル」", ct).ConfigureAwait(false);
            return new SubmitResult(false, info, null, ElementSelectors.ParseStatus(await BodyTextAsync(ct).ConfigureAwait(false)));
        }

        var ok = ElementSelectors.PickButton(dialogButtons, DashboardText.SubmitButton);
        if (ok.Disabled) throw new WriteBlockedException("確認ダイアログの送信ボタンが押せない状態です");
        await ClickAsync(PageScripts.Kinds.DialogButton, ok.Index, $"確認ダイアログの「{DashboardText.SubmitButton}」", ct).ConfigureAwait(false);

        IReadOnlyList<string> messages = [];
        var sent = await TryPollAsync(async () =>
        {
            messages = await MessagesAsync(ct).ConfigureAwait(false);
            return ElementSelectors.HasMessage(messages, DashboardText.SubmittedMessage) ? true : (bool?)null;
        }, TimeSpan.FromSeconds(45), ct).ConfigureAwait(false);

        var resultMessages = ElementSelectors.ResultMessages(messages);
        var status = ElementSelectors.ParseStatus(await BodyTextAsync(ct).ConfigureAwait(false));
        if (!sent)
        {
            throw new DashboardException(
                $"送信完了のメッセージ（{DashboardText.SubmittedMessage}）を確認できませんでした。ステータス: {status ?? "不明"}" +
                (resultMessages.Count > 0 ? $"。表示: {string.Join(" / ", resultMessages)}" : ""));
        }
        var message = resultMessages.FirstOrDefault(m => m.Contains(DashboardText.SubmittedMessage, StringComparison.Ordinal));
        LoggerService.Log($"審査送信を確認: {_itemId} ステータス={status}", LogLevel.Info);
        return new SubmitResult(true, info, message, status);
    }

    public async Task<string?> CaptureDiagnosticsAsync(string label, CancellationToken ct)
    {
        try
        {
            Directory.CreateDirectory(AppPaths.DiagnosticsDirectory);
            var safeLabel = string.Concat(label.Select(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_' ? c : '_'));
            var path = Path.Combine(AppPaths.DiagnosticsDirectory, $"{DateTime.Now:yyyyMMdd-HHmmss}-{safeLabel}.png");
            var png = await page.CaptureScreenshotAsync(ct).ConfigureAwait(false);
            await File.WriteAllBytesAsync(path, png, ct).ConfigureAwait(false);
            return path;
        }
        catch (Exception ex)
        {
            LoggerService.Log($"診断用の画面保存に失敗: {ex.Message}", LogLevel.Warning);
            return null;
        }
    }

    // ───────── 下請け ─────────

    /// <summary>欄の要素を特定する（説明は textarea、URL はラベル）</summary>
    private async Task<(string Kind, ElementInfo Element)> LocateAsync(ListingField field, CancellationToken ct)
    {
        if (field == ListingField.Description)
        {
            var textareas = await CollectAsync(PageScripts.Kinds.Textarea, ct).ConfigureAwait(false);
            return (PageScripts.Kinds.Textarea, ElementSelectors.PickDescription(textareas));
        }
        var inputs = await CollectAsync(PageScripts.Kinds.TextInput, ct).ConfigureAwait(false);
        return (PageScripts.Kinds.TextInput, ElementSelectors.PickFieldByLabel(inputs, field));
    }

    /// <summary>要素を画面内へスクロールし、中心を実際のマウス入力でクリックする</summary>
    private async Task ClickAsync(string kind, int index, string what, CancellationToken ct)
    {
        await page.BringToFrontAsync(ct).ConfigureAwait(false);
        await EvaluateAsync(PageScripts.ScrollTo(kind, index), ct).ConfigureAwait(false);
        await Task.Delay(TimeSpan.FromMilliseconds(150), ct).ConfigureAwait(false);

        // スクロール後の矩形を読み直す。中心が別の要素に覆われていたら押さない。
        var point = await EvaluateAsync(PageScripts.Point(kind, index), ct).ConfigureAwait(false);
        if (point.ValueKind != JsonValueKind.Object) throw new DashboardException($"{what}の位置を取得できませんでした");
        if (!point.GetProperty("hit").GetBoolean())
        {
            throw new DashboardException($"{what}が他の要素に覆われていて押せません");
        }
        await page.ClickAtAsync(point.GetProperty("x").GetDouble(), point.GetProperty("y").GetDouble(), ct).ConfigureAwait(false);
    }

    private async Task<IReadOnlyList<ElementInfo>> CollectAsync(string kind, CancellationToken ct)
        => PageScripts.ParseElements(await EvaluateAsync(PageScripts.Collect(kind), ct).ConfigureAwait(false));

    private async Task<string> ValueAsync(string kind, int index, CancellationToken ct)
    {
        var v = await EvaluateAsync(PageScripts.Value(kind, index), ct).ConfigureAwait(false);
        return v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
    }

    private async Task<IReadOnlyList<string>> MessagesAsync(CancellationToken ct)
        => PageScripts.ParseStrings(await EvaluateAsync(PageScripts.Messages(), ct).ConfigureAwait(false));

    private async Task<IReadOnlyList<string>> DialogsAsync(CancellationToken ct)
        => PageScripts.ParseStrings(await EvaluateAsync(PageScripts.Dialogs(), ct).ConfigureAwait(false));

    private async Task<string> BodyTextAsync(CancellationToken ct)
    {
        var v = await EvaluateAsync(PageScripts.BodyText(), ct).ConfigureAwait(false);
        return v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
    }

    private async Task<JsonElement> EvaluateAsync(string expression, CancellationToken ct)
    {
        try
        {
            return await page.EvaluateAsync(expression, ct).ConfigureAwait(false);
        }
        catch (JavaScriptException ex)
        {
            throw new DashboardException(ex.Message, ex);
        }
    }

    /// <summary>条件が満たされる（null 以外を返す）まで繰り返し確かめる。上限を超えたら例外。</summary>
    private static async Task<T> PollAsync<T>(Func<Task<T?>> probe, TimeSpan timeout, string what, CancellationToken ct) where T : class
    {
        var deadline = DateTime.UtcNow + timeout;
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            if (await probe().ConfigureAwait(false) is { } value) return value;
            if (DateTime.UtcNow >= deadline)
            {
                throw new DashboardException($"{what}を {timeout.TotalSeconds:0} 秒待ちましたが確認できませんでした");
            }
            await Task.Delay(PollInterval, ct).ConfigureAwait(false);
        }
    }

    private static async Task PollAsync(Func<Task<bool?>> probe, TimeSpan timeout, string what, CancellationToken ct)
    {
        if (!await TryPollAsync(probe, timeout, ct).ConfigureAwait(false))
        {
            throw new DashboardException($"{what}を {timeout.TotalSeconds:0} 秒待ちましたが確認できませんでした");
        }
    }

    /// <summary>条件が true になるまで確かめる。上限を超えたら false。</summary>
    private static async Task<bool> TryPollAsync(Func<Task<bool?>> probe, TimeSpan timeout, CancellationToken ct)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            if (await probe().ConfigureAwait(false) == true) return true;
            if (DateTime.UtcNow >= deadline) return false;
            await Task.Delay(PollInterval, ct).ConfigureAwait(false);
        }
    }
}
