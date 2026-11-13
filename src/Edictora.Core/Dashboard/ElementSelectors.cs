using System.Text.RegularExpressions;
using Edictora.Core.Listing;

namespace Edictora.Core.Dashboard;

/// <summary>
/// ページから集めた要素 1 つの情報（JS 側で集め、C# 側で選ぶ）。
/// </summary>
/// <param name="Index">ページ側の一時配列での位置（クリック・入力のときに使う）</param>
/// <param name="Tag">タグ名（小文字）</param>
/// <param name="Text">innerText（前後空白を除いたもの）</param>
/// <param name="Label">ラベル候補を連結した文字列（aria-label・aria-labelledby・label 要素・その欄だけを含む最大の祖先の innerText）</param>
/// <param name="Disabled">disabled または aria-disabled="true"</param>
/// <param name="ReadOnly">readOnly または aria-readonly="true"</param>
/// <param name="Type">input の type（小文字。input 以外は空）</param>
/// <param name="Checked">チェックボックスの状態（チェックボックス以外は null）</param>
/// <param name="Expanded">aria-expanded の値（"true"/"false"。属性が無ければ null）。コンボボックスの開閉判定に使う</param>
/// <remarks>
/// 「どの要素を選ぶか」の判断はブラウザなしでテストできるよう、すべて C# 側の純粋関数にしている。
/// JS 側は候補を集めて情報を返すだけにとどめる。
/// </remarks>
public sealed record ElementInfo(
    int Index,
    string Tag,
    string Text,
    string Label,
    bool Disabled,
    bool ReadOnly,
    string Type = "",
    bool? Checked = null,
    string? Expanded = null);

/// <summary>言語の選択肢 1 つ（「日本語 – ja」「英語 – en （デフォルト）」）</summary>
/// <param name="Name">言語名</param>
/// <param name="Code">ロケールコード</param>
/// <param name="IsDefault">（デフォルト）表記があるか</param>
public sealed record LocaleOption(string Name, string Code, bool IsDefault);

/// <summary>
/// 集めた要素から目的の 1 つを選ぶ純粋関数群。見つからない・複数ある場合は <see cref="DashboardException"/>。
/// </summary>
/// <remarks>
/// 曖昧なときに「最初の 1 つ」を選ぶと、別の欄を書き換える事故につながる。
/// そのため、候補がちょうど 1 つに絞れないときは必ず止める。
/// </remarks>
public static partial class ElementSelectors
{
    /// <summary>
    /// 言語切替のコンボボックスを選ぶ。最初の combobox はパブリッシャー選択なので、ラベル「言語」で絞り込む。
    /// </summary>
    public static ElementInfo PickLanguageCombobox(IReadOnlyList<ElementInfo> comboboxes)
    {
        // 部分一致ではなく「ラベルの 1 行が完全一致」で選ぶ。
        // 実画面ではカテゴリ欄の親要素に「すべての言語用」という説明があり、部分一致だと
        // カテゴリ欄まで「言語」に当たって 2 件になった（2026-11-13 の実ダッシュボードで確認）。
        var matches = comboboxes
            .Where(c => HasLabelLine(c.Label, DashboardText.LanguageLabel) || HasLabelLine(c.Text, DashboardText.LanguageLabel))
            .ToList();
        return matches.Count switch
        {
            1 => matches[0],
            0 => throw new DashboardException($"「{DashboardText.LanguageLabel}」のコンボボックスが見つかりません（ダッシュボードが日本語表示か確認してください）"),
            _ => throw new DashboardException($"「{DashboardText.LanguageLabel}」のコンボボックスが {matches.Count} 個あり、特定できません"),
        };
    }

    /// <summary>
    /// 選択肢の表記「&lt;言語名&gt; – &lt;code&gt;」を解釈する。形式が違えば null。
    /// </summary>
    /// <remarks>
    /// 区切りは EN DASH（U+2013）。念のため EM DASH・ハイフンも受け付ける。
    /// 後ろに「（デフォルト）」などが付くことがある。コードは完全一致で比べる（en が en-GB に当たらないように）。
    /// </remarks>
    public static LocaleOption? ParseLocaleOption(string? text)
    {
        if (string.IsNullOrWhiteSpace(text)) return null;
        if (text.Contains('\n'))
        {
            // 選択肢の innerText にアイコン等の行が混ざる場合に備え、解釈できた最後の行を使う
            LocaleOption? last = null;
            foreach (var line in text.Split('\n'))
            {
                if (!line.Contains('\n') && ParseLocaleOption(line) is { } parsed) last = parsed;
            }
            return last;
        }
        var m = LocaleOptionRegex().Match(text.Trim());
        if (!m.Success) return null;
        var code = m.Groups["code"].Value.Replace('_', '-');
        var rest = m.Groups["rest"].Value;
        return new LocaleOption(m.Groups["name"].Value.Trim(), code, rest.Contains("デフォルト", StringComparison.Ordinal));
    }

    /// <summary>
    /// コンボボックスの表示テキストから、いま選ばれているロケールを読む。
    /// 表示は「言語\n英語 – en （デフォルト）」のように複数行のことがあるので、最後に解釈できた行を使う。
    /// </summary>
    public static string? CurrentLocaleCode(ElementInfo combobox)
    {
        string? code = null;
        foreach (var line in combobox.Text.Split('\n'))
        {
            var parsed = ParseLocaleOption(line);
            if (parsed is not null) code = parsed.Code;
        }
        return code;
    }

    /// <summary>選択肢一覧からロケールコードの一覧を作る（解釈できない行は無視、重複は除く）</summary>
    public static IReadOnlyList<string> LocaleCodes(IReadOnlyList<ElementInfo> options)
        => [.. options.Select(o => ParseLocaleOption(o.Text)?.Code).OfType<string>().Distinct(StringComparer.OrdinalIgnoreCase)];

    /// <summary>コードが完全一致する選択肢を 1 つ選ぶ</summary>
    public static ElementInfo PickLocaleOption(IReadOnlyList<ElementInfo> options, string code)
    {
        var matches = options
            .Where(o => ParseLocaleOption(o.Text) is { } p && string.Equals(p.Code, code, StringComparison.OrdinalIgnoreCase))
            .ToList();
        return matches.Count switch
        {
            1 => matches[0],
            0 => throw new DashboardException(
                $"言語の選択肢にロケール {code} がありません（ある言語: {string.Join(", ", LocaleCodes(options))}）"),
            _ => throw new DashboardException($"言語の選択肢にロケール {code} が {matches.Count} 個あり、特定できません"),
        };
    }

    /// <summary>文言が完全一致するボタンを 1 つ選ぶ（前後の空白は無視）</summary>
    public static ElementInfo PickButton(IReadOnlyList<ElementInfo> buttons, string text)
    {
        var matches = buttons.Where(b => string.Equals(b.Text.Trim(), text, StringComparison.Ordinal)).ToList();
        return matches.Count switch
        {
            1 => matches[0],
            0 => throw new DashboardException($"「{text}」ボタンが見つかりません"),
            _ => throw new DashboardException($"「{text}」ボタンが {matches.Count} 個あり、特定できません"),
        };
    }

    /// <summary>
    /// ラベルで URL 欄を 1 つ選ぶ。空白の有無（「公式 URL」と「公式URL」）は区別しない。
    /// </summary>
    public static ElementInfo PickFieldByLabel(IReadOnlyList<ElementInfo> inputs, ListingField field)
    {
        var label = DashboardText.FieldLabel(field);
        // 言語欄と同じ理由で、ラベルの 1 行が完全一致するものだけを選ぶ（親要素の説明文に別の欄名が含まれても当たらない）。
        var matches = inputs.Where(i => HasLabelLine(i.Label, label)).ToList();
        return matches.Count switch
        {
            1 => matches[0],
            0 => throw new DashboardException($"「{label}」の入力欄が見つかりません"),
            _ => throw new DashboardException($"「{label}」の入力欄が {matches.Count} 個あり、特定できません"),
        };
    }

    /// <summary>説明欄（textarea）を 1 つ選ぶ</summary>
    public static ElementInfo PickDescription(IReadOnlyList<ElementInfo> textareas)
        => textareas.Count switch
        {
            1 => textareas[0],
            0 => throw new DashboardException("説明欄（textarea）が見つかりません"),
            _ => throw new DashboardException($"textarea が {textareas.Count} 個あり、説明欄を特定できません"),
        };

    /// <summary>書き込み先として使える入力欄か確かめる（不可なら例外）</summary>
    public static void EnsureWritable(ElementInfo element, ListingField field)
    {
        var ok = field == ListingField.Description
            ? element.Tag == "textarea"
            : element.Tag == "input" && element.Type is "" or "text" or "url";
        if (!ok)
        {
            throw new DashboardException($"{field.DisplayName()}の欄がテキスト入力欄ではありません（{element.Tag} {element.Type}）。画面構成が想定と違うため止めます");
        }
        if (element.Disabled || element.ReadOnly)
        {
            throw new WriteBlockedException($"{field.DisplayName()}の欄が編集できない状態です");
        }
    }

    /// <summary>常設の案内通知を除いた結果メッセージの一覧</summary>
    public static IReadOnlyList<string> ResultMessages(IEnumerable<string> messages)
        => [.. messages.Select(m => m.Trim()).Where(m => m.Length > 0 && !m.Contains(DashboardText.GuideNoticeMarker, StringComparison.Ordinal))];

    /// <summary>結果メッセージの中に <paramref name="keyword"/> を含むものがあるか</summary>
    public static bool HasMessage(IEnumerable<string> messages, string keyword)
        => ResultMessages(messages).Any(m => m.Contains(keyword, StringComparison.Ordinal));

    /// <summary>ページ本文から「ステータス: …」の値を取り出す。見つからなければ null。</summary>
    public static string? ParseStatus(string? bodyText)
    {
        if (string.IsNullOrEmpty(bodyText)) return null;
        var m = StatusRegex().Match(bodyText);
        return m.Success ? m.Groups["status"].Value.Trim() : null;
    }

    /// <summary>空白を無視した部分一致</summary>
    /// <summary>
    /// 改行区切りのラベル文字列のどれか 1 行が、<paramref name="label"/> と完全一致するか（空白の有無は無視）。
    /// </summary>
    /// <remarks>
    /// ラベル候補には「その欄だけを含む最大の祖先」の文字列も入るため、部分一致だと説明文中の語に誤って当たる。
    /// 実画面ではラベルが単独の行（aria-labelledby の要素や aria-label）として現れるので、行単位の完全一致で十分に特定できる。
    /// </remarks>
    internal static bool HasLabelLine(string? labels, string label)
    {
        if (labels is null) return false;
        var target = RemoveWhiteSpace(label);
        return labels.Split('\n').Any(line => string.Equals(RemoveWhiteSpace(line), target, StringComparison.Ordinal));
    }

    internal static bool Contains(string? haystack, string needle)
        => haystack is not null && RemoveWhiteSpace(haystack).Contains(RemoveWhiteSpace(needle), StringComparison.Ordinal);

    private static string RemoveWhiteSpace(string s) => string.Concat(s.Where(c => !char.IsWhiteSpace(c)));

    [GeneratedRegex(@"^(?<name>.+?)\s*[–—-]\s*(?<code>[A-Za-z]{2,3}(?:[-_][A-Za-z0-9]{2,8})*)(?<rest>(?:\s|（|\().*)?$", RegexOptions.CultureInvariant)]
    private static partial Regex LocaleOptionRegex();

    [GeneratedRegex(@"ステータス\s*[:：]\s*(?<status>[^\n]+)", RegexOptions.CultureInvariant)]
    private static partial Regex StatusRegex();
}
