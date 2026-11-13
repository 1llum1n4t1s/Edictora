// Edictora がダッシュボードのページ内で Runtime.evaluate するスクリプト（C# 版 PageScripts.cs の Prelude を移植したもの）。
//
// 役割: 要素の「収集」と「1 要素への単純な操作」だけを行う。
// - どの要素を選ぶかは Rust 側（crates/edictora/src/dashboard/selectors.rs）で判断する。ブラウザなしでテストするため。
// - 収集した要素はページ側の一時配列（store）に保持し、Rust からは種類と位置（index）で指す。
//   ページが描き直されて要素が消えていたら、操作時にエラーにする（別の要素を誤って操作しない）。
// - スクリプトには利用者の入力値を埋め込まない。書き込む文字列は CDP の Input.insertText で渡す（注入の心配がない）。
// - クラス名には依存しない。役割（role）・タグ・ラベル・文言だけを使う。
//
// 導入は 1 回だけ: window.__edictora が既にあれば作り直さない。
// store（収集した要素の配列）は Runtime.evaluate をまたいで同じオブジェクトに残る必要があるため。
//
// __EDICTORA_GUIDE__ は Rust 側（dashboard/text.rs の GUIDE_NOTICE_MARKER）が JSON 文字列に置き換える。
// 文言の正本を Rust 側 1 か所に保つため、ここには書かない。

declare const __EDICTORA_GUIDE__: string;

/** 要素の種類（Rust 側の page_scripts::kinds と同じ名前） */
type Kind = "combobox" | "option" | "button" | "textarea" | "textInput" | "dialogButton" | "dialogCheckbox";

/** 収集した要素 1 つの情報（Rust 側の ElementInfo に対応） */
interface ElementInfo {
  index: number;
  tag: string;
  text: string;
  label: string;
  disabled: boolean;
  readOnly: boolean;
  type: string;
  /** コンボボックスの開閉状態（"true"/"false"、属性が無ければ null）。一覧を閉じたかの判定に使う */
  expanded: string | null;
  checked: boolean | null;
}

/** クリック位置（要素の中心）と、その点に要素自身（または子孫）があるか */
interface Point {
  x: number;
  y: number;
  hit: boolean;
}

/** Rust から呼ぶ関数一式 */
interface EdictoraApi {
  collect(kind: Kind): ElementInfo[];
  scrollTo(kind: Kind, i: number): true;
  point(kind: Kind, i: number): Point;
  focusSelectAll(kind: Kind, i: number): boolean;
  value(kind: Kind, i: number): unknown;
  messages(): string[];
  dialogs(): string[];
  bodyText(): string;
}

/** 導入先（window に API を 1 つだけぶら下げる） */
const host = window as Window & { __edictora?: EdictoraApi };

/** 入力欄に共通する（あれば使う）プロパティ */
type FormLike = Element & {
  disabled?: boolean;
  readOnly?: boolean;
  type?: string;
  checked?: boolean;
  value?: unknown;
  labels?: NodeListOf<HTMLLabelElement> | null;
  focus?: () => void;
  select?: () => void;
  setSelectionRange?: (start: number, end: number) => void;
  selectionStart?: number | null;
  selectionEnd?: number | null;
};

function createApi(): EdictoraApi {
  const GUIDE = __EDICTORA_GUIDE__;

  const visible = (el: Element | null): boolean => {
    if (!el || !el.isConnected) return false;
    if (el.closest('[aria-hidden="true"]')) return false;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return false;
    const s = getComputedStyle(el);
    return s.visibility !== "hidden" && s.display !== "none";
  };

  const text = (el: Element): string =>
    ((el as HTMLElement).innerText || el.textContent || "").replace(/\r\n/g, "\n").trim();

  const FIELD = "input, textarea, select, [role=combobox]";

  // ラベル候補を集める: aria-label / aria-labelledby / label 要素 / その欄だけを含む最大の祖先の文字列
  const labelOf = (el: Element): string => {
    const parts: string[] = [];
    const al = el.getAttribute("aria-label");
    if (al) parts.push(al);
    const lb = el.getAttribute("aria-labelledby");
    if (lb) {
      for (const id of lb.split(/\s+/)) {
        const t = document.getElementById(id);
        if (t) parts.push(text(t));
      }
    }
    const labels = (el as FormLike).labels;
    if (labels) for (const l of labels) parts.push(text(l));
    let cur: Element = el;
    let best: Element | null = null;
    while (cur.parentElement && cur.parentElement !== document.body) {
      const p = cur.parentElement;
      if (p.querySelectorAll(FIELD).length > 1) break;
      best = p;
      cur = p;
    }
    if (best) parts.push(text(best));
    return parts.join("\n");
  };

  const allDialogs = (): Element[] =>
    [...document.querySelectorAll("[role=dialog], [role=alertdialog]")].filter(
      (d) => visible(d) && text(d) && !text(d).includes(GUIDE),
    );

  const inDialog = (el: Element): boolean => !!el.closest("[role=dialog], [role=alertdialog]");

  const sources: Record<Kind, () => Element[]> = {
    combobox: () => [...document.querySelectorAll("[role=combobox]")].filter(visible),
    // 閉じた一覧（listbox）は要素が残ったまま枠が 0×0 になるだけなので、枠に大きさがある一覧の選択肢だけを数える。
    // （2026-11-13 の実画面: 閉じても [role=option] が表示扱いで残り、「選択肢が消えたら閉じた」と判定できなかった）
    option: () =>
      [...document.querySelectorAll("[role=option]")].filter((o) => {
        if (!visible(o)) return false;
        const list = o.closest("[role=listbox]");
        if (!list) return true;
        const r = list.getBoundingClientRect();
        return r.width > 0 && r.height > 0;
      }),
    button: () => [...document.querySelectorAll("button, [role=button]")].filter((b) => visible(b) && !inDialog(b)),
    textarea: () => [...document.querySelectorAll("textarea")].filter(visible),
    textInput: () =>
      [...document.querySelectorAll("input")].filter(
        (i) => visible(i) && ["", "text", "url"].includes((i.getAttribute("type") || "").toLowerCase()),
      ),
    dialogButton: () => {
      const d = allDialogs();
      return d.length === 1 ? [...d[0].querySelectorAll("button, [role=button]")].filter(visible) : [];
    },
    dialogCheckbox: () => {
      const d = allDialogs();
      return d.length === 1 ? [...d[0].querySelectorAll("input[type=checkbox], [role=checkbox]")] : [];
    },
  };

  const store: Partial<Record<Kind, Element[]>> = {};

  const info = (el: Element, i: number): ElementInfo => {
    const f = el as FormLike;
    return {
      index: i,
      tag: el.tagName.toLowerCase(),
      text: text(el),
      label: labelOf(el),
      disabled: !!f.disabled || el.getAttribute("aria-disabled") === "true",
      readOnly: !!f.readOnly || el.getAttribute("aria-readonly") === "true",
      type: el.tagName === "INPUT" ? (el.getAttribute("type") || "").toLowerCase() : "",
      expanded: el.getAttribute("aria-expanded"),
      checked:
        f.type === "checkbox"
          ? !!f.checked
          : el.getAttribute("role") === "checkbox"
            ? el.getAttribute("aria-checked") === "true"
            : null,
    };
  };

  const get = (kind: Kind, i: number): FormLike => {
    const el = (store[kind] || [])[i];
    if (!el || !el.isConnected) throw new Error("要素が画面から消えました: " + kind);
    return el as FormLike;
  };

  return {
    collect: (kind) => {
      const els = sources[kind]();
      store[kind] = els;
      return els.map(info);
    },
    scrollTo: (kind, i) => {
      get(kind, i).scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
      return true;
    },
    point: (kind, i) => {
      const el = get(kind, i);
      const r = el.getBoundingClientRect();
      const x = r.left + r.width / 2;
      const y = r.top + r.height / 2;
      const hit = document.elementFromPoint(x, y);
      return { x, y, hit: !!hit && (hit === el || el.contains(hit)) };
    },
    focusSelectAll: (kind, i) => {
      const el = get(kind, i);
      const length = typeof el.value === "string" ? el.value.length : 0;
      if (typeof el.focus === "function") el.focus();
      if (typeof el.setSelectionRange === "function") el.setSelectionRange(0, length);
      else if (typeof el.select === "function") el.select();
      return document.activeElement === el && el.selectionStart === 0 && el.selectionEnd === length;
    },
    value: (kind, i) => get(kind, i).value,
    messages: () => [...document.querySelectorAll("[role=alert], [role=status], [aria-live]")].map(text).filter(Boolean),
    dialogs: () => allDialogs().map(text),
    bodyText: () => (document.body ? document.body.innerText : ""),
  };
}

// 既に導入済みなら作り直さない（store を評価間で保つ）
if (!host.__edictora) {
  host.__edictora = createApi();
}
