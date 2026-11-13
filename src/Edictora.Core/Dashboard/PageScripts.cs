using System.Text.Json;

namespace Edictora.Core.Dashboard;

/// <summary>
/// ページ内で実行するスクリプト。要素の「収集」と「1 要素への単純な操作」だけを行う。
/// </summary>
/// <remarks>
/// - どの要素を選ぶかは C# 側（<see cref="ElementSelectors"/>）で判断する。
/// - 収集した要素はページ側の一時配列に保持し、C# からは種類と位置（index）で指す。
///   ページが描き直されて要素が消えていたら、操作時にエラーにする（別の要素を誤って操作しない）。
/// - スクリプトには利用者の入力値を埋め込まない。書き込む文字列は CDP の Input.insertText で渡す。
///   そのためスクリプト注入の心配がない。
/// - クラス名には依存しない。役割（role）・タグ・ラベル・文言だけを使う。
/// </remarks>
internal static class PageScripts
{
    /// <summary>要素の種類（JS 側の収集関数名）</summary>
    internal static class Kinds
    {
        public const string Combobox = "combobox";
        public const string Option = "option";
        public const string Button = "button";
        public const string Textarea = "textarea";
        public const string TextInput = "textInput";
        public const string DialogButton = "dialogButton";
        public const string DialogCheckbox = "dialogCheckbox";
    }

    /// <summary>ヘルパーを定義し E に束縛する前置き（既に定義済みなら使い回す）</summary>
    private static readonly string Prelude = """
        const E = window.__edictora || (window.__edictora = (() => {
          const GUIDE = __GUIDE__;
          const visible = (el) => {
            if (!el || !el.isConnected) return false;
            if (el.closest('[aria-hidden="true"]')) return false;
            const r = el.getBoundingClientRect();
            if (r.width <= 0 || r.height <= 0) return false;
            const s = getComputedStyle(el);
            return s.visibility !== 'hidden' && s.display !== 'none';
          };
          const text = (el) => (el.innerText || el.textContent || '').replace(/\r\n/g, '\n').trim();
          const FIELD = 'input, textarea, select, [role=combobox]';
          // ラベル候補を集める: aria-label / aria-labelledby / label 要素 / その欄だけを含む最大の祖先の文字列
          const labelOf = (el) => {
            const parts = [];
            const al = el.getAttribute('aria-label'); if (al) parts.push(al);
            const lb = el.getAttribute('aria-labelledby');
            if (lb) for (const id of lb.split(/\s+/)) { const t = document.getElementById(id); if (t) parts.push(text(t)); }
            if (el.labels) for (const l of el.labels) parts.push(text(l));
            let cur = el, best = null;
            while (cur.parentElement && cur.parentElement !== document.body) {
              const p = cur.parentElement;
              if (p.querySelectorAll(FIELD).length > 1) break;
              best = p; cur = p;
            }
            if (best) parts.push(text(best));
            return parts.join('\n');
          };
          const allDialogs = () => [...document.querySelectorAll('[role=dialog], [role=alertdialog]')]
            .filter((d) => visible(d) && text(d) && !text(d).includes(GUIDE));
          const inDialog = (el) => !!el.closest('[role=dialog], [role=alertdialog]');
          const sources = {
            combobox: () => [...document.querySelectorAll('[role=combobox]')].filter(visible),
            // 閉じた一覧（listbox）は要素が残ったまま枠が 0×0 になるだけなので、枠に大きさがある一覧の選択肢だけを数える。
            // （2026-11-13 の実画面: 閉じても [role=option] が表示扱いで残り、「選択肢が消えたら閉じた」と判定できなかった）
            option: () => [...document.querySelectorAll('[role=option]')].filter((o) => {
              if (!visible(o)) return false;
              const list = o.closest('[role=listbox]');
              if (!list) return true;
              const r = list.getBoundingClientRect();
              return r.width > 0 && r.height > 0;
            }),
            button: () => [...document.querySelectorAll('button, [role=button]')].filter((b) => visible(b) && !inDialog(b)),
            textarea: () => [...document.querySelectorAll('textarea')].filter(visible),
            textInput: () => [...document.querySelectorAll('input')]
              .filter((i) => visible(i) && ['', 'text', 'url'].includes((i.getAttribute('type') || '').toLowerCase())),
            dialogButton: () => { const d = allDialogs(); return d.length === 1 ? [...d[0].querySelectorAll('button, [role=button]')].filter(visible) : []; },
            dialogCheckbox: () => { const d = allDialogs(); return d.length === 1 ? [...d[0].querySelectorAll('input[type=checkbox], [role=checkbox]')] : []; },
          };
          const store = {};
          const info = (el, i) => ({
            index: i,
            tag: el.tagName.toLowerCase(),
            text: text(el),
            label: labelOf(el),
            disabled: !!el.disabled || el.getAttribute('aria-disabled') === 'true',
            readOnly: !!el.readOnly || el.getAttribute('aria-readonly') === 'true',
            type: el.tagName === 'INPUT' ? (el.getAttribute('type') || '').toLowerCase() : '',
            // コンボボックスの開閉状態（"true"/"false"、属性が無ければ null）。一覧を閉じたかの判定に使う。
            expanded: el.getAttribute('aria-expanded'),
            checked: el.type === 'checkbox' ? !!el.checked
              : (el.getAttribute('role') === 'checkbox' ? el.getAttribute('aria-checked') === 'true' : null),
          });
          const get = (kind, i) => {
            const el = (store[kind] || [])[i];
            if (!el || !el.isConnected) throw new Error('要素が画面から消えました: ' + kind);
            return el;
          };
          return {
            collect: (kind) => { const els = sources[kind](); store[kind] = els; return els.map(info); },
            scrollTo: (kind, i) => { get(kind, i).scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' }); return true; },
            point: (kind, i) => {
              const el = get(kind, i);
              const r = el.getBoundingClientRect();
              const x = r.left + r.width / 2, y = r.top + r.height / 2;
              const hit = document.elementFromPoint(x, y);
              return { x, y, hit: !!hit && (hit === el || el.contains(hit)) };
            },
            focusSelectAll: (kind, i) => {
              const el = get(kind, i);
              el.focus();
              if (typeof el.setSelectionRange === 'function') el.setSelectionRange(0, el.value.length);
              else if (typeof el.select === 'function') el.select();
              return document.activeElement === el && el.selectionStart === 0 && el.selectionEnd === el.value.length;
            },
            value: (kind, i) => get(kind, i).value,
            messages: () => [...document.querySelectorAll('[role=alert], [role=status], [aria-live]')].map(text).filter(Boolean),
            dialogs: () => allDialogs().map(text),
            bodyText: () => (document.body ? document.body.innerText : ''),
            ready: () => document.readyState,
          };
        })());
        """.Replace("__GUIDE__", JsonSerializer.Serialize(DashboardText.GuideNoticeMarker, Serialization.EdictoraJsonContext.Default.String));

    /// <summary>E.&lt;call&gt; を評価する式を作る</summary>
    private static string Call(string call) => $"(() => {{ {Prelude} return E.{call}; }})()";

    private static string Quote(string kind) => JsonSerializer.Serialize(kind, Serialization.EdictoraJsonContext.Default.String);

    public static string Collect(string kind) => Call($"collect({Quote(kind)})");

    public static string ScrollTo(string kind, int index) => Call($"scrollTo({Quote(kind)}, {index})");

    public static string Point(string kind, int index) => Call($"point({Quote(kind)}, {index})");

    public static string FocusSelectAll(string kind, int index) => Call($"focusSelectAll({Quote(kind)}, {index})");

    public static string Value(string kind, int index) => Call($"value({Quote(kind)}, {index})");

    public static string Messages() => Call("messages()");

    public static string Dialogs() => Call("dialogs()");

    public static string BodyText() => Call("bodyText()");

    /// <summary>収集結果（JSON 配列）を <see cref="ElementInfo"/> に変換する</summary>
    public static IReadOnlyList<ElementInfo> ParseElements(JsonElement array)
    {
        if (array.ValueKind != JsonValueKind.Array) return [];
        var list = new List<ElementInfo>();
        foreach (var e in array.EnumerateArray())
        {
            list.Add(new ElementInfo(
                e.GetProperty("index").GetInt32(),
                Str(e, "tag"),
                Str(e, "text"),
                Str(e, "label"),
                Bool(e, "disabled"),
                Bool(e, "readOnly"),
                Str(e, "type"),
                e.TryGetProperty("checked", out var c) && c.ValueKind is JsonValueKind.True or JsonValueKind.False ? c.GetBoolean() : null,
                e.TryGetProperty("expanded", out var x) && x.ValueKind == JsonValueKind.String ? x.GetString() : null));
        }
        return list;
    }

    /// <summary>文字列配列（JSON）を変換する</summary>
    public static IReadOnlyList<string> ParseStrings(JsonElement array)
        => array.ValueKind == JsonValueKind.Array
            ? [.. array.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.String ? x.GetString() ?? "" : "")]
            : [];

    private static string Str(JsonElement e, string name)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";

    private static bool Bool(JsonElement e, string name)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.True;
}
