// snapshot()（失敗時の診断用テキスト スナップショット）の振る舞いを、手書きの最小の偽 DOM で確かめる。
//
// - 本番で評価されるのは dist/edictora.js（Bun で src から生成）なので、dist を読んで評価する。
//   dist が古いときは先に `bun run build` が要る（CI では `bun run check` が最新かを確かめる）。
// - 依存は増やさない（happy-dom / jsdom は使わない）。偽 DOM はレイアウトを持たないので、
//   矩形・表示状態は要素ごとに指定する。ブラウザは起動しない。
// - この test/ は tsconfig の対象外（bun:test の型を入れないため）。typecheck は src だけを見る。

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

type Attrs = Record<string, string>;
interface Opts {
  rect?: [number, number, number, number];
  display?: string;
  visibility?: string;
  props?: Record<string, unknown>;
}

class FakeText {
  nodeType = 3;
  constructor(public textContent: string) {}
}

class FakeEl {
  nodeType = 1;
  childNodes: (FakeEl | FakeText)[] = [];
  shadowRoot: { childNodes: (FakeEl | FakeText)[] } | null = null;
  rect: [number, number, number, number];
  display: string;
  visibility: string;
  [key: string]: unknown;
  constructor(
    public tagName: string,
    public attrs: Attrs,
    opts: Opts,
    children: (FakeEl | FakeText | string)[],
  ) {
    this.rect = opts.rect ?? [0, 0, 100, 20];
    this.display = opts.display ?? "block";
    this.visibility = opts.visibility ?? "visible";
    // 実際の input / textarea / select が常に持つプロパティの既定値
    if (tagName === "INPUT" || tagName === "TEXTAREA") this.value = "";
    if (tagName === "SELECT") this.selectedOptions = [];
    Object.assign(this, opts.props ?? {});
    for (const c of children) this.childNodes.push(typeof c === "string" ? new FakeText(c) : c);
  }
  getAttribute(n: string): string | null {
    return n in this.attrs ? this.attrs[n] : null;
  }
  hasAttribute(n: string): boolean {
    return n in this.attrs;
  }
  getBoundingClientRect() {
    const [left, top, width, height] = this.rect;
    return { left, top, width, height };
  }
  get textContent(): string {
    return this.childNodes.map((c) => c.textContent).join("");
  }
  get innerText(): string {
    return this.textContent;
  }
}

const el = (tag: string, attrs: Attrs = {}, opts: Opts = {}, ...children: (FakeEl | FakeText | string)[]) =>
  new FakeEl(tag.toUpperCase(), attrs, opts, children);

const bundle = readFileSync(join(import.meta.dir, "..", "dist", "edictora.js"), "utf8").replace(
  "__EDICTORA_GUIDE__",
  "null",
);

interface PageOpts {
  active?: FakeEl;
  ids?: Record<string, FakeEl>;
}

/** 偽 DOM をグローバルに据えて dist を評価し、snapshot() の出力を返す（評価ごとに window を作り直す） */
function snapshotOf(body: FakeEl, page: PageOpts = {}): string {
  const g = globalThis as Record<string, unknown>;
  g.window = {};
  g.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 };
  g.innerWidth = 1280;
  g.innerHeight = 900;
  g.location = { href: "https://example.test/devconsole/PUB123/item/edit" };
  g.getComputedStyle = (e: FakeEl) => ({ display: e.display, visibility: e.visibility });
  g.document = {
    body,
    documentElement: { lang: "ja" },
    title: "Title",
    readyState: "complete",
    activeElement: page.active ?? null,
    getElementById: (id: string) => page.ids?.[id] ?? null,
  };
  (0, eval)(bundle);
  return (g.window as { __edictora: { snapshot(): string } }).__edictora.snapshot();
}

/** ヘッダー（--- まで）を除いた本体の行 */
function bodyLines(out: string): string[] {
  return out.split("\n").slice(out.split("\n").indexOf("---") + 1).filter((l) => l !== "");
}

const page = (...children: (FakeEl | FakeText | string)[]) => el("body", {}, { rect: [0, 0, 1280, 900] }, ...children);

describe("ヘッダー", () => {
  test("URL・タイトル・lang・ビューポート・readyState を先頭に出す", () => {
    const out = snapshotOf(page());
    expect(out).toContain("url: https://example.test/devconsole/PUB123/item/edit\n");
    expect(out).toContain('title: "Title"\n');
    expect(out).toContain('lang: "ja"\n');
    expect(out).toContain("viewport: 1280x900\n");
    expect(out).toContain("readyState: complete\n");
    expect(out.endsWith("\n")).toBe(true);
  });
});

describe("役割", () => {
  test("主要タグの暗黙の役割と heading の段階", () => {
    const lines = bodyLines(
      snapshotOf(
        page(
          el("button", {}, {}, "Go"),
          el("a", { href: "/x" }, {}, "Link"),
          el("a", {}, {}, "no href"),
          el("h2", {}, {}, "Title2"),
          el("input", { type: "text", "aria-label": "T" }, { props: { value: "v" } }),
          el("input", { type: "checkbox", "aria-label": "C" }, { props: { checked: true, type: "checkbox" } }),
          el("input", { type: "hidden" }),
          el("select", { "aria-label": "S" }),
          el("nav", {}, {}, el("ul", {}, {}, el("li", {}, {}, el("span", {}, {}, "item")))),
        ),
      ),
    );
    expect(lines).toContain('button "Go" @0,0,100,20');
    expect(lines).toContain('link "Link" @0,0,100,20');
    expect(lines.some((l) => l.includes("no href") && !l.startsWith("link"))).toBe(true);
    expect(lines).toContain('heading[2] "Title2" @0,0,100,20');
    expect(lines.some((l) => l.startsWith('textbox "T" value="v"'))).toBe(true);
    expect(lines.some((l) => l.startsWith('checkbox "C"') && l.includes("checked=true"))).toBe(true);
    expect(lines.some((l) => l.includes("hidden") && l.startsWith("textbox"))).toBe(false);
    expect(lines.some((l) => l.startsWith('combobox "S"'))).toBe(true);
    // ナビゲーション > リスト > リスト項目 の字下げ
    expect(lines).toContain("navigation @0,0,100,20");
    expect(lines).toContain("  list @0,0,100,20");
    expect(lines).toContain("    listitem @0,0,100,20");
    expect(lines).toContain('      text "item"');
  });

  test("role 属性は暗黙の役割より優先し、presentation 系は子へ畳む", () => {
    const lines = bodyLines(
      snapshotOf(
        page(
          el("div", { role: "combobox", "aria-expanded": "false" }, {}, "Pick"),
          el("button", { role: "tab" }, {}, "T1"),
          el("div", { role: "presentation" }, {}, el("button", {}, {}, "inner")),
          el("div", { role: "none" }, {}, "plain"),
        ),
      ),
    );
    expect(lines[0]).toBe("combobox {expanded=false} @0,0,100,20");
    expect(lines[1]).toBe('  text "Pick"');
    expect(lines).toContain('tab "T1" @0,0,100,20');
    expect(lines).toContain('button "inner" @0,0,100,20');
    expect(lines).toContain('text "plain"');
    expect(lines.some((l) => l.startsWith("presentation") || l.startsWith("none"))).toBe(false);
  });

  test("名前の無い section は役割にしない（名前があれば region）", () => {
    const lines = bodyLines(
      snapshotOf(page(el("section", {}, {}, "a"), el("section", { "aria-label": "R" }, {}, "b"))),
    );
    expect(lines).toContain('text "a"');
    expect(lines).toContain('region "R" @0,0,100,20');
  });
});

describe("名前", () => {
  const ids = {
    lab: el("span", {}, {}, "By labelledby"),
  };

  test("aria-label > aria-labelledby > label > alt / placeholder / title > 内容", () => {
    const withLabels = el("input", { type: "text", "aria-label": "AL", "aria-labelledby": "lab" }, {
      props: { labels: [el("label", {}, {}, "LBL")] },
    });
    const onlyLabelledby = el("input", { type: "text", "aria-labelledby": "lab" }, { props: { labels: [el("label", {}, {}, "LBL")] } });
    const onlyLabel = el("input", { type: "text", placeholder: "PH" }, { props: { labels: [el("label", {}, {}, "LBL")] } });
    const placeholder = el("input", { type: "text", placeholder: "PH", title: "TT" });
    const title = el("input", { type: "text", title: "TT" });
    const content = el("button", {}, {}, "Content");
    const alt = el("img", { alt: "ALT", src: "a.png" });
    const lines = bodyLines(snapshotOf(page(withLabels, onlyLabelledby, onlyLabel, placeholder, title, content, alt), { ids }));
    expect(lines[0]).toStartWith('textbox "AL"');
    expect(lines[1]).toStartWith('textbox "By labelledby"');
    expect(lines[2]).toStartWith('textbox "LBL"');
    expect(lines[3]).toStartWith('textbox "PH"');
    expect(lines[4]).toStartWith('textbox "TT"');
    expect(lines[5]).toStartWith('button "Content"');
    expect(lines[6]).toStartWith('img "ALT"');
  });

  test("名前の空白は 1 つにまとめ、改行は出さない", () => {
    const lines = bodyLines(snapshotOf(page(el("button", {}, {}, "  a \n  b  "))));
    expect(lines[0]).toBe('button "a b" @0,0,100,20');
  });
});

describe("値", () => {
  test("textarea は値の改行を \\n で出し、子の初期テキストは出さない", () => {
    const lines = bodyLines(
      snapshotOf(page(el("textarea", { "aria-label": "D" }, { props: { value: "L1\nL2" } }, "initial text"))),
    );
    expect(lines).toEqual(['textbox "D" value="L1\\nL2" @0,0,100,20']);
  });

  test("select は選択中の選択肢の文字列を出し、子の option はたどらない", () => {
    const sel = el("select", { "aria-label": "S" }, {
      props: { selectedOptions: [el("option", {}, {}, "Two"), el("option", {}, {}, "Three")] },
    }, el("option", {}, {}, "One"));
    const lines = bodyLines(snapshotOf(page(sel)));
    expect(lines).toEqual(['combobox "S" value="Two, Three" @0,0,100,20']);
  });

  test("password は (masked) だけで、文字数も出さない", () => {
    const lines = bodyLines(snapshotOf(page(el("input", { type: "password" }, { props: { value: "secret!" } }))));
    expect(lines).toEqual(['textbox value="(masked)" @0,0,100,20']);
    expect(lines.join("\n")).not.toContain("secret");
    expect(lines.join("\n")).not.toContain("chars");
  });

  test("1 値は 2,000 文字で切り、元の文字数を添える", () => {
    const long = "a".repeat(3000);
    const out = snapshotOf(page(el("textarea", { "aria-label": "D" }, { props: { value: long } })));
    expect(out).toContain(`${"a".repeat(2000)}...(3000 chars)`);
    expect(out).not.toContain("a".repeat(2001));
    const textOut = snapshotOf(page("b".repeat(2500)));
    expect(textOut).toContain("...(2500 chars)");
  });
});

describe("状態", () => {
  test("disabled・readonly・required・aria-*・checked・focused・data-value", () => {
    const btn = el("button", { "aria-expanded": "true", "aria-pressed": "false" }, { props: { disabled: true } }, "B");
    const opt = el("div", { role: "option", "aria-selected": "true", "data-value": "tools" }, {}, "Tools");
    const chk = el("div", { role: "checkbox", "aria-checked": "mixed" }, {}, "X");
    const input = el("input", { type: "text", "aria-label": "I", "aria-invalid": "true", "aria-required": "true" }, {
      props: { readOnly: true, value: "" },
    });
    const lines = bodyLines(snapshotOf(page(btn, opt, chk, input), { active: input }));
    expect(lines[0]).toBe('button "B" {disabled expanded=true pressed=false} @0,0,100,20');
    expect(lines[1]).toBe('option "Tools" {selected=true data-value="tools"} @0,0,100,20');
    expect(lines[2]).toBe('checkbox "X" {checked=mixed} @0,0,100,20');
    expect(lines[3]).toBe('textbox "I" value="" {readonly required invalid=true focused} @0,0,100,20');
  });

  test("aria-disabled だけでも disabled になる", () => {
    const lines = bodyLines(snapshotOf(page(el("div", { role: "button", "aria-disabled": "true" }, {}, "D"))));
    expect(lines[0]).toContain("{disabled}");
  });
});

describe("表示状態", () => {
  test("display:none と visibility:hidden は部分木ごと省く", () => {
    const lines = bodyLines(
      snapshotOf(
        page(
          el("div", {}, { display: "none" }, el("button", {}, {}, "gone1")),
          el("div", {}, { visibility: "hidden" }, el("button", {}, {}, "gone2")),
          el("button", {}, {}, "kept"),
        ),
      ),
    );
    expect(lines).toEqual(['button "kept" @0,0,100,20']);
  });

  test("0x0 は自分の行を省いて子へ降り、素の文字列は出さない", () => {
    const lines = bodyLines(
      snapshotOf(
        page(
          el("div", { role: "listbox", "aria-label": "L" }, { rect: [0, 0, 0, 0] }, "hidden text", el("button", {}, { rect: [5, 6, 30, 10] }, "visible child")),
        ),
      ),
    );
    expect(lines).toEqual(['button "visible child" @5,6,30,10']);
  });

  test("0x0 の option と input[type=file] は {hidden} つきで残す", () => {
    const lines = bodyLines(
      snapshotOf(
        page(
          el("ul", { role: "listbox", "aria-label": "Category" }, { rect: [0, 0, 0, 0] },
            el("li", { role: "option", "data-value": "tools", "aria-selected": "true" }, { rect: [0, 0, 0, 0] }, "Tools"),
            el("li", { role: "none" }, { rect: [0, 0, 0, 0] }, el("button", {}, { rect: [0, 0, 0, 0] }, "zero button")),
          ),
          el("input", { type: "file" }, { rect: [0, 0, 0, 0] }),
        ),
      ),
    );
    expect(lines).toEqual([
      'option "Tools" {hidden selected=true data-value="tools"} @0,0,0,0',
      "file-input {hidden} @0,0,0,0",
    ]);
  });

  test("display:contents は枠を持たないが子を描画する", () => {
    const lines = bodyLines(
      snapshotOf(page(el("div", {}, { display: "contents", rect: [0, 0, 0, 0] }, el("button", {}, {}, "in contents")))),
    );
    expect(lines).toEqual(['button "in contents" @0,0,100,20']);
  });

  test("script / style / svg（名前なし）は出さず、名前のある svg は img にする", () => {
    const lines = bodyLines(
      snapshotOf(
        page(el("script", {}, {}, "alert(1)"), el("style", {}, {}, "a{}"), el("svg", {}), el("svg", { "aria-label": "Logo" })),
      ),
    );
    expect(lines).toEqual(['img "Logo"']);
  });
});

describe("shadowRoot・iframe", () => {
  test("open な shadowRoot の中をたどる", () => {
    const host = el("div", {}, {}, el("button", {}, {}, "light"));
    host.shadowRoot = { childNodes: [el("button", {}, {}, "shadow")] };
    expect(bodyLines(snapshotOf(page(host)))).toEqual(['button "shadow" @0,0,100,20']);
  });

  test("同一オリジンの iframe は行（矩形つき）を出して中をたどる", () => {
    const frame = el("iframe", { title: "Inner" }, { rect: [1, 2, 300, 200] });
    frame.contentDocument = { body: el("body", {}, { rect: [0, 0, 300, 200] }, el("button", {}, {}, "in frame")) };
    expect(bodyLines(snapshotOf(page(frame)))).toEqual([
      'iframe "Inner" @1,2,300,200',
      '  button "in frame" @0,0,100,20',
    ]);
  });

  test("cross-origin の iframe（contentDocument が例外）は印と矩形だけ", () => {
    const frame = el("iframe", { title: "Ext" }, { rect: [1, 2, 300, 200] });
    Object.defineProperty(frame, "contentDocument", {
      get() {
        throw new Error("SecurityError");
      },
    });
    expect(bodyLines(snapshotOf(page(frame)))).toEqual(['iframe "Ext" (not readable: cross-origin) @1,2,300,200']);
  });
});

describe("上限と例外", () => {
  test("全体が 1,000,000 文字を超えたら切って印を付ける", () => {
    const many = Array.from({ length: 40000 }, (_, i) => el("button", {}, {}, `button number ${i}`));
    const out = snapshotOf(page(...many));
    expect(out).toContain("... (truncated: output size limit reached)");
    expect(out.length).toBeLessThan(1_100_000);
    expect(out).toContain("button number 0");
    expect(out).not.toContain("button number 39999");
  });

  test("深さが上限（80）を超えたら切って印を付ける", () => {
    let node: FakeEl = el("button", {}, {}, "deep");
    for (let i = 0; i < 90; i++) node = el("div", {}, {}, node);
    const out = snapshotOf(page(node));
    expect(out).toContain("... (truncated: depth limit reached)");
    expect(out).not.toContain('button "deep"');
  });

  test("途中で例外が出ても、それまでの行に印を足して文字列を返す", () => {
    const bad = el("div", {}, {});
    bad.getBoundingClientRect = () => {
      throw new Error("boom");
    };
    const out = snapshotOf(page(el("button", {}, {}, "before"), bad, el("button", {}, {}, "after")));
    expect(typeof out).toBe("string");
    expect(out).toContain('button "before"');
    expect(out).toContain("... (snapshot error: boom)");
    expect(out).not.toContain('button "after"');
  });
});
