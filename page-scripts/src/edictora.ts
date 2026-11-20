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
  | "imageButton"
  | "listboxOption";

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
  /** listboxOption のみ: data-value 属性（無ければ空） */
  dataValue?: string;
  /** listboxOption のみ: aria-selected の値（"true"/"false"、属性が無ければ null） */
  selected?: string | null;
  /** listboxOption のみ: 所属する [role=listbox] のページ内の通し番号（属さなければ -1） */
  group?: number;
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
  /** 失敗時の診断用: 見えている要素の役割・名前・値・状態・矩形をたどったテキスト */
  snapshot(): string;
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
    // 一覧の選択肢は、閉じていて見えないものも含めてすべて集める（カテゴリの現在値を、一覧を開かずに読むため。
    // 2026-11-19 の実画面: 閉じた一覧は枠が 0×0 だが li[role=option] は全部 DOM にある）。どの一覧のものかは Rust 側で選ぶ
    listboxOption: () => [...document.querySelectorAll("[role=option]")],
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

  // listboxOption 用。ラベルは所属する一覧の aria-label / aria-labelledby（祖先の文字列は混ぜない）、
  // 枠の大きさは一覧のもの（開いたかの判定に使う）。選ぶ判断は Rust 側（selectors.rs のカテゴリの関数）
  const optionInfo = (el: Element, i: number): ElementInfo => {
    const lists = [...document.querySelectorAll("[role=listbox]")];
    const list = el.closest("[role=listbox]");
    const r = (list || el).getBoundingClientRect();
    const parts: string[] = [];
    if (list) {
      const al = list.getAttribute("aria-label");
      if (al) parts.push(al);
      const lb = list.getAttribute("aria-labelledby");
      if (lb) {
        for (const id of lb.split(/\s+/)) {
          const t = document.getElementById(id);
          if (t) parts.push(text(t));
        }
      }
    }
    return {
      index: i,
      tag: el.tagName.toLowerCase(),
      text: text(el),
      label: parts.join("\n"),
      disabled: el.getAttribute("aria-disabled") === "true",
      readOnly: false,
      type: "",
      expanded: null,
      checked: null,
      dataValue: el.getAttribute("data-value") || "",
      selected: el.getAttribute("aria-selected"),
      group: list ? lists.indexOf(list) : -1,
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

  // ---- 失敗時の診断用: ページのテキスト スナップショット ----
  // スクリーンショットの代わりに、AI が画像を見ずに読めるテキストで「今の画面」を返す。
  // 返すのは見えている要素の 役割・名前・値・状態・矩形 だけで、どれが原因かの判断は読み手（人・AI）に任せる。
  // 利用者の入力値は埋め込まない（読み出すだけ）。出力は ASCII の印と JSON 風の引用で 1 行 1 要素にする。
  // publisherId・アカウントの氏名・メールアドレスの伏せ字は Rust 側（logging::redact_diagnostics）が保存前に行う。
  const SNAPSHOT_MAX_CHARS = 1_000_000; // 全体の上限
  const SNAPSHOT_VALUE_MAX = 2000; // 1 つの値（名前・値・本文）の上限
  const SNAPSHOT_MAX_DEPTH = 80;
  const NAME_FROM_CONTENT = new Set([
    "button", "link", "heading", "option", "tab", "menuitem", "checkbox", "radio", "switch", "img",
  ]);
  const IMPLICIT_ROLE: Record<string, string> = {
    button: "button", textarea: "textbox", select: "combobox", dialog: "dialog", nav: "navigation",
    main: "main", header: "banner", footer: "contentinfo", aside: "complementary", form: "form",
    ul: "list", ol: "list", li: "listitem", table: "table", tr: "row", td: "cell", th: "columnheader",
    progress: "progressbar", option: "option", details: "group", article: "article", img: "img",
    h1: "heading", h2: "heading", h3: "heading", h4: "heading", h5: "heading", h6: "heading",
  };
  const INPUT_ROLE: Record<string, string> = {
    button: "button", submit: "button", reset: "button", image: "button", checkbox: "checkbox",
    radio: "radio", range: "slider", number: "spinbutton", file: "file-input",
  };
  const SKIP_TAGS = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "HEAD", "META", "LINK"]);

  /** 長い値は上限で切り、元の文字数を添える */
  const clip = (s: string): string =>
    s.length > SNAPSHOT_VALUE_MAX ? `${s.slice(0, SNAPSHOT_VALUE_MAX)}...(${s.length} chars)` : s;
  const oneLine = (s: string): string => s.replace(/\s+/g, " ").trim();
  const quoted = (s: string): string => JSON.stringify(clip(oneLine(s)));

  const roleOf = (el: Element): string => {
    const explicit = (el.getAttribute("role") || "").trim().split(/\s+/)[0];
    // 意味を持たない役割は付けず、暗黙の役割にも頼らず、子へ畳む
    if (explicit === "presentation" || explicit === "none" || explicit === "generic") return "";
    if (explicit) return explicit;
    const tag = el.tagName.toLowerCase();
    if (tag === "a") return el.hasAttribute("href") ? "link" : "";
    if (tag === "input") {
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "hidden") return "";
      return INPUT_ROLE[type] || (el.hasAttribute("list") ? "combobox" : "textbox");
    }
    if (tag === "select") {
      const s = el as HTMLSelectElement;
      return s.multiple || s.size > 1 ? "listbox" : "combobox";
    }
    if (tag === "section") return el.hasAttribute("aria-label") || el.hasAttribute("aria-labelledby") ? "region" : "";
    return IMPLICIT_ROLE[tag] || "";
  };

  const nameOf = (el: Element, role: string): string => {
    const al = el.getAttribute("aria-label");
    if (al && al.trim()) return al;
    const lb = el.getAttribute("aria-labelledby");
    if (lb) {
      const t = lb
        .split(/\s+/)
        .map((id) => document.getElementById(id))
        .filter((n): n is HTMLElement => !!n)
        .map(text)
        .join(" ");
      if (t.trim()) return t;
    }
    const labels = (el as FormLike).labels;
    if (labels && labels.length > 0) {
      const t = [...labels].map(text).join(" ");
      if (t.trim()) return t;
    }
    if (el.tagName === "IMG" && el.getAttribute("alt")) return el.getAttribute("alt") || "";
    if (NAME_FROM_CONTENT.has(role) && el.tagName !== "IMG") {
      const t = text(el);
      if (t) return t;
    }
    return el.getAttribute("placeholder") || el.getAttribute("title") || "";
  };

  /** 状態の印（値のあるものだけ）。disabled / readonly / required は真のときだけ出す */
  const statesOf = (el: Element): string[] => {
    const f = el as FormLike & { required?: boolean };
    const out: string[] = [];
    if (f.disabled || el.getAttribute("aria-disabled") === "true") out.push("disabled");
    if (f.readOnly || el.getAttribute("aria-readonly") === "true") out.push("readonly");
    if (f.required || el.getAttribute("aria-required") === "true") out.push("required");
    for (const a of ["expanded", "selected", "pressed", "current", "invalid", "haspopup", "busy"]) {
      const v = el.getAttribute("aria-" + a);
      if (v !== null) out.push(`${a}=${v}`);
    }
    if (f.type === "checkbox" || f.type === "radio") out.push(`checked=${!!f.checked}`);
    else if (el.hasAttribute("aria-checked")) out.push(`checked=${el.getAttribute("aria-checked")}`);
    if (document.activeElement === el) out.push("focused");
    const dv = el.getAttribute("data-value");
    if (dv) out.push(`data-value=${quoted(dv)}`);
    return out;
  };

  const valueOf = (el: Element): string | null => {
    const tag = el.tagName;
    if (tag === "TEXTAREA") return clip((el as HTMLTextAreaElement).value);
    if (tag === "INPUT") {
      const i = el as HTMLInputElement;
      const type = (i.getAttribute("type") || "text").toLowerCase();
      if (type === "password") return "(masked)"; // 文字数も出さない
      if (["checkbox", "radio", "button", "submit", "reset", "image", "file"].includes(type)) return null;
      return clip(i.value);
    }
    if (tag === "SELECT") {
      return [...(el as HTMLSelectElement).selectedOptions].map((o) => oneLine(o.textContent || "")).join(", ");
    }
    return null;
  };

  /**
   * 描画の状態。
   * - gone: display:none / visibility:hidden。部分木ごと省く
   * - contents: 枠を持たないが子は描画される（親の状態を引き継ぐ）
   * - empty: 0x0。自分の行は省くが子へは降りる（Edictora が非表示でも集める閉じた一覧の option や
   *   input[type=file] を残すため）。ただし option と file 入力だけは (hidden) 印つきで出す
   * - shown: 通常に表示されている
   */
  const renderState = (el: Element): "gone" | "contents" | "empty" | "shown" => {
    const s = getComputedStyle(el);
    if (s.display === "contents") return "contents";
    if (s.display === "none" || s.visibility === "hidden") return "gone";
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 ? "shown" : "empty";
  };

  const snapshot = (): string => {
    const lines: string[] = [];
    let size = 0;
    let truncated = false;
    const push = (depth: number, line: string): void => {
      if (truncated) return;
      if (size + line.length > SNAPSHOT_MAX_CHARS) {
        truncated = true;
        lines.push("... (truncated: output size limit reached)");
        return;
      }
      lines.push("  ".repeat(depth) + line);
      size += line.length + depth * 2 + 1;
    };

    // shown: 親までが描画されているか（0x0 の部分木の中の素の文字列は出さない）
    const walkChildren = (nodes: Iterable<Node>, depth: number, level: number, shown: boolean): void => {
      for (const n of nodes) {
        if (truncated) return;
        if (n.nodeType === Node.TEXT_NODE) {
          const t = oneLine(n.textContent || "");
          if (t && shown) push(depth, `text ${quoted(t)}`);
        } else if (n.nodeType === Node.ELEMENT_NODE) {
          walk(n as Element, depth, level + 1, shown);
        }
      }
    };

    const walk = (el: Element, depth: number, level: number, parentShown: boolean): void => {
      if (truncated || SKIP_TAGS.has(el.tagName)) return;
      if (level > SNAPSHOT_MAX_DEPTH) {
        push(depth, "... (truncated: depth limit reached)");
        return;
      }
      const state = renderState(el);
      if (state === "gone") return;
      const shown = state === "contents" ? parentShown : state === "shown";
      const tag = el.tagName.toLowerCase();
      if (tag === "svg") {
        // 名前のある SVG だけ画像として出し、中身はたどらない
        const label = el.getAttribute("aria-label");
        if (label && shown) push(depth, `img ${quoted(label)}`);
        return;
      }
      if (tag === "slot") {
        const assigned = (el as HTMLSlotElement).assignedNodes({ flatten: true });
        walkChildren(assigned.length > 0 ? assigned : el.childNodes, depth, level, shown);
        return;
      }
      if (tag === "iframe") {
        const frame = el as HTMLIFrameElement;
        let doc: Document | null = null;
        try {
          doc = frame.contentDocument;
        } catch {
          doc = null;
        }
        const fr = el.getBoundingClientRect();
        if (shown) {
          push(
            depth,
            `iframe ${quoted(el.getAttribute("title") || "")}${doc ? "" : " (not readable: cross-origin)"} ${rectOf(fr)}`,
          );
        }
        if (doc && doc.body) walk(doc.body, depth + 1, level + 1, shown);
        return;
      }
      const role = roleOf(el);
      // 内容で名前が決まる要素と、値を別に出す textarea / select は子をたどらない
      // （textarea の初期値の text が value と並ばないように）
      const leaf = NAME_FROM_CONTENT.has(role) || tag === "textarea" || tag === "select";
      // 0x0 の要素は option と file 入力だけ (hidden) 印つきで出す。それ以外は自分の行を省いて子へ降りる
      const hiddenKept = !shown && state === "empty" && (role === "option" || (tag === "input" && role === "file-input"));
      const printSelf = !!role && (shown || hiddenKept);
      const children = (): void => {
        if (leaf) return;
        const root = (el as HTMLElement).shadowRoot;
        walkChildren(root ? root.childNodes : el.childNodes, printSelf ? depth + 1 : depth, level, shown);
      };
      if (!printSelf) {
        // 意味の無い入れ子・0x0 の要素は子へ畳む（字下げも増やさない）
        children();
        return;
      }
      const parts: string[] = [role === "heading" ? headingRole(el) : role];
      const name = nameOf(el, role);
      if (name.trim()) parts.push(quoted(name));
      const value = valueOf(el);
      if (value !== null) parts.push(`value=${JSON.stringify(value)}`);
      const states = statesOf(el);
      if (hiddenKept) states.unshift("hidden");
      if (states.length > 0) parts.push(`{${states.join(" ")}}`);
      parts.push(rectOf(el.getBoundingClientRect()));
      push(depth, parts.join(" "));
      children();
    };

    const rectOf = (r: DOMRect): string =>
      `@${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)}`;

    const headingRole = (el: Element): string => {
      const level = el.getAttribute("aria-level") || (/^H[1-6]$/.test(el.tagName) ? el.tagName.slice(1) : "");
      return level ? `heading[${level}]` : "heading";
    };

    const rootEl = document.body || document.documentElement;
    // 例外でも途中までの行を残し、必ず文字列を返す（診断の取得自体で失敗を増やさない）
    try {
      if (rootEl) walk(rootEl, 0, 0, true);
    } catch (err) {
      lines.push(`... (snapshot error: ${err instanceof Error ? err.message : String(err)})`);
    }
    const header = [
      "# Edictora page snapshot (text)",
      `url: ${location.href}`,
      `title: ${JSON.stringify(clip(document.title || ""))}`,
      `lang: ${JSON.stringify(document.documentElement ? document.documentElement.lang : "")}`,
      `viewport: ${innerWidth}x${innerHeight}`,
      `readyState: ${document.readyState}`,
      "format: <role> \"<name>\" value=\"<value>\" {<state>...} @x,y,w,h  (indent = nesting; text \"...\" = plain text; invisible elements omitted except option / file-input marked {hidden})",
      "---",
    ];
    return header.join("\n") + "\n" + lines.join("\n") + "\n";
  };

  return {
    snapshot,
    collect: (kind) => {
      const els = sources[kind]();
      store[kind] = els;
      return kind === "fileInput" ? els.map(fileInfo) : kind === "listboxOption" ? els.map(optionInfo) : els.map(info);
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
