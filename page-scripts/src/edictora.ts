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
// __EDICTORA_GUIDE__ は Rust 側（dashboard/text.rs の文言セットの guide_notice_marker）が JSON に置き換える。
// 文言の正本を Rust 側 1 か所に保つため、ここには書かない。案内通知の語が無い画面の言語では null になり、除外しない
// （空文字だと includes("") が常に真になり、すべてのダイアログが除外されてしまうため null で表す）。
//
// このスクリプトは利用者に見せる出力の言語を知らない。利用者向けの文になる例外・値は固定の印だけを返し、
// Rust 側（dashboard/selectors.rs の ELEMENT_GONE_MARKER・OFFSCREEN_COVER）で日本語か英語の文に直す。

declare const __EDICTORA_GUIDE__: string | null;

/** 要素の種類（Rust 側の page_scripts::kinds と同じ名前） */
type Kind =
  | "combobox"
  | "option"
  | "button"
  | "textarea"
  | "textInput"
  | "dialogButton"
  | "dialogCheckbox"
  | "checkbox"
  | "fileInput"
  | "slotImage"
  | "imageButton";

/** 画像 1 枚の情報（alt と src。Rust 側の PageImage に対応） */
interface ImageInfo {
  alt: string;
  src: string;
}

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
  /** fileInput のみ: accept 属性 */
  accept?: string;
  /** fileInput のみ: その入力だけを含む最大の祖先の中の img（画面の並び順） */
  images?: ImageInfo[];
  /** slotImage / imageButton のみ: 枠の大きさ（ホバーで現れるボタンが出たかの判定に使う） */
  width?: number;
  height?: number;
}

/**
 * クリック位置（要素の中心）と、その点に要素自身（または子孫）があるか。
 * 覆われていたときの原因調査用に、中心にあった要素の説明・要素の枠・画面の大きさも返す
 * （エラー文だけで原因を絞れるようにし、--show-browser で表示して確かめる必要をなくすため）
 */
interface Point {
  x: number;
  y: number;
  hit: boolean;
  /** 中心にあった別の要素の説明（role または タグ名と、ラベル・文言の先頭）。覆われていなければ null */
  cover: string | null;
  /** 要素の枠（ビューポート座標、整数に丸める） */
  rect: { left: number; top: number; width: number; height: number };
  /** ビューポートの大きさ */
  viewport: { width: number; height: number };
}

/** Rust から呼ぶ関数一式 */
interface EdictoraApi {
  collect(kind: Kind): ElementInfo[];
  collectSlot(kind: Kind, slotIndex: number): ElementInfo[];
  element(kind: Kind, i: number): Element;
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
      (d) => visible(d) && text(d) && !(GUIDE !== null && text(d).includes(GUIDE)),
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
    // ページ本体のチェックボックス（ダイアログの外）。「プライバシー」タブのデータ使用の 9 種類と 3 つの宣誓チェック。
    // 実画面では input 自体が 48x48・不透明度 0 で、クリックはその input に届く（覆われていない）
    checkbox: () =>
      [...document.querySelectorAll("input[type=checkbox], [role=checkbox]")].filter((c) => visible(c) && !inDialog(c)),
    fileInput: () => [...document.querySelectorAll("input[type=file]")],
    // slotImage / imageButton は collectSlot で集める（単独では空）
    slotImage: () => [],
    imageButton: () => [],
  };

  // 画像アセットの入力（input[type=file]）。非表示でも集める（実画面では 0 幅のものがある）。
  // その入力だけを含む最大の祖先が「1 つのスロット」で、見出し・寸法・画像・ボタンはその中にある。
  const slotRoot = (input: Element): Element => {
    let cur: Element = input;
    let best: Element = input;
    while (cur.parentElement && cur.parentElement !== document.body) {
      const p = cur.parentElement;
      if (p.querySelectorAll("input[type=file]").length > 1) break;
      best = p;
      cur = p;
    }
    return best;
  };

  const store: Partial<Record<Kind, Element[]>> = {};

  // slotImage / imageButton は、直前の collect("fileInput") で集めたスロットの中だけから集める
  const slotSources: Partial<Record<Kind, (root: Element) => Element[]>> = {
    slotImage: (root) => realImages(root),
    imageButton: (root) => [...root.querySelectorAll("[role=button][aria-label], button[aria-label]")],
  };

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

  // スロットの中の実画像。src 属性が空の img は数えない。
  // 実画面（2026-11-15）: 画像を削除したスロットに、alt だけを持つ src が空の img が残る（枚数 0 のスロットを 1 枚と数えない）
  const realImages = (root: Element): Element[] =>
    [...root.querySelectorAll("img")].filter((m) => (m.getAttribute("src") || "").trim() !== "");

  // fileInput 用: スロットの見出し・寸法・画像を、その入力を含む最小の枠（slotRoot）から読む
  const fileInfo = (el: Element, i: number): ElementInfo => {
    const root = slotRoot(el);
    return {
      index: i,
      tag: "input",
      // 見出し・寸法の表記・案内文（行ごとに改行で区切られる）
      text: text(root),
      label: "",
      disabled: !!(el as FormLike).disabled,
      readOnly: false,
      type: "file",
      expanded: null,
      checked: null,
      accept: el.getAttribute("accept") || "",
      images: realImages(root).map((m) => ({
        alt: m.getAttribute("alt") || "",
        src: (m as HTMLImageElement).currentSrc || m.getAttribute("src") || "",
      })),
    };
  };

  // slotImage / imageButton 用。ボタンのラベルは aria-label そのものだけにする
  // （祖先の文字列を混ぜない。「画像を削除 <alt>」の完全一致で選ぶため）
  const slotInfo = (el: Element, i: number): ElementInfo => {
    const r = el.getBoundingClientRect();
    return {
      index: i,
      tag: el.tagName.toLowerCase(),
      text: el.tagName === "IMG" ? "" : text(el),
      label: el.tagName === "IMG" ? el.getAttribute("alt") || "" : el.getAttribute("aria-label") || "",
      disabled: (el as FormLike).disabled === true || el.getAttribute("aria-disabled") === "true",
      readOnly: false,
      type: "",
      expanded: el.getAttribute("aria-expanded"),
      checked: null,
      width: Math.round(r.width),
      height: Math.round(r.height),
    };
  };

  const get = (kind: Kind, i: number): FormLike => {
    const el = (store[kind] || [])[i];
    // 文は Rust 側で作る（selectors.rs の ELEMENT_GONE_MARKER と同じ印）
    if (!el || !el.isConnected) throw new Error("EDICTORA_ELEMENT_GONE:" + kind);
    return el as FormLike;
  };

  return {
    collect: (kind) => {
      const els = sources[kind]();
      store[kind] = els;
      return kind === "fileInput" ? els.map(fileInfo) : els.map(info);
    },
    collectSlot: (kind, slotIndex) => {
      const input = (store.fileInput || [])[slotIndex];
      if (!input || !input.isConnected) throw new Error("EDICTORA_ELEMENT_GONE:fileInput");
      const pick = slotSources[kind];
      if (!pick) throw new Error("EDICTORA_ELEMENT_GONE:" + kind);
      const els = pick(slotRoot(input));
      store[kind] = els;
      return els.map(slotInfo);
    },
    element: (kind, i) => get(kind, i),
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
      const ok = !!hit && (hit === el || el.contains(hit));
      // 覆っている要素の説明。クラス名は画面の更新で変わるので使わず、役割・ラベル・文言だけにする。
      // 文言は 1 行にまとめて先頭 40 文字まで（長い本文を出さない）
      const describe = (n: Element | null): string | null => {
        // 文は Rust 側で作る（selectors.rs の OFFSCREEN_COVER と同じ印）
        if (!n) return "__EDICTORA_OFFSCREEN__";
        const role = n.getAttribute("role");
        const kind = role ? `${n.tagName.toLowerCase()}[role=${role}]` : n.tagName.toLowerCase();
        const words = (n.getAttribute("aria-label") || text(n)).replace(/\s+/g, " ").trim().slice(0, 40);
        return words ? `${kind}「${words}」` : kind;
      };
      return {
        x,
        y,
        hit: ok,
        cover: ok ? null : describe(hit),
        rect: {
          left: Math.round(r.left),
          top: Math.round(r.top),
          width: Math.round(r.width),
          height: Math.round(r.height),
        },
        viewport: { width: innerWidth, height: innerHeight },
      };
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
