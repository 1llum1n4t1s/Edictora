// 生成ファイル: page-scripts/src/edictora.ts から `bun run build` で作る。直接編集しない。
(() => {
  // src/edictora.ts
  var host = window;
  function createApi() {
    const GUIDE = __EDICTORA_GUIDE__;
    const visible = (el) => {
      if (!el || !el.isConnected)
        return false;
      if (el.closest('[aria-hidden="true"]'))
        return false;
      const r = el.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0)
        return false;
      const s = getComputedStyle(el);
      return s.visibility !== "hidden" && s.display !== "none";
    };
    const text = (el) => (el.innerText || el.textContent || "").replace(/\r\n/g, `
`).trim();
    const FIELD = "input, textarea, select, [role=combobox]";
    const labelOf = (el) => {
      const parts = [];
      const al = el.getAttribute("aria-label");
      if (al)
        parts.push(al);
      const lb = el.getAttribute("aria-labelledby");
      if (lb) {
        for (const id of lb.split(/\s+/)) {
          const t = document.getElementById(id);
          if (t)
            parts.push(text(t));
        }
      }
      const labels = el.labels;
      if (labels)
        for (const l of labels)
          parts.push(text(l));
      let cur = el;
      let best = null;
      while (cur.parentElement && cur.parentElement !== document.body) {
        const p = cur.parentElement;
        if (p.querySelectorAll(FIELD).length > 1)
          break;
        best = p;
        cur = p;
      }
      if (best)
        parts.push(text(best));
      return parts.join(`
`);
    };
    const allDialogs = () => [...document.querySelectorAll("[role=dialog], [role=alertdialog]")].filter((d) => visible(d) && text(d) && !(GUIDE !== null && text(d).includes(GUIDE)));
    const inDialog = (el) => !!el.closest("[role=dialog], [role=alertdialog]");
    const sources = {
      combobox: () => [...document.querySelectorAll("[role=combobox]")].filter(visible),
      option: () => [...document.querySelectorAll("[role=option]")].filter((o) => {
        if (!visible(o))
          return false;
        const list = o.closest("[role=listbox]");
        if (!list)
          return true;
        const r = list.getBoundingClientRect();
        return r.width > 0 && r.height > 0;
      }),
      button: () => [...document.querySelectorAll("button, [role=button]")].filter((b) => visible(b) && !inDialog(b)),
      textarea: () => [...document.querySelectorAll("textarea")].filter(visible),
      textInput: () => [...document.querySelectorAll("input")].filter((i) => visible(i) && ["", "text", "url"].includes((i.getAttribute("type") || "").toLowerCase())),
      dialogButton: () => {
        const d = allDialogs();
        return d.length === 1 ? [...d[0].querySelectorAll("button, [role=button]")].filter(visible) : [];
      },
      dialogCheckbox: () => {
        const d = allDialogs();
        return d.length === 1 ? [...d[0].querySelectorAll("input[type=checkbox], [role=checkbox]")] : [];
      },
      fileInput: () => [...document.querySelectorAll("input[type=file]")],
      slotImage: () => [],
      imageButton: () => []
    };
    const slotRoot = (input) => {
      let cur = input;
      let best = input;
      while (cur.parentElement && cur.parentElement !== document.body) {
        const p = cur.parentElement;
        if (p.querySelectorAll("input[type=file]").length > 1)
          break;
        best = p;
        cur = p;
      }
      return best;
    };
    const store = {};
    const slotSources = {
      slotImage: (root) => realImages(root),
      imageButton: (root) => [...root.querySelectorAll("[role=button][aria-label], button[aria-label]")]
    };
    const info = (el, i) => {
      const f = el;
      return {
        index: i,
        tag: el.tagName.toLowerCase(),
        text: text(el),
        label: labelOf(el),
        disabled: !!f.disabled || el.getAttribute("aria-disabled") === "true",
        readOnly: !!f.readOnly || el.getAttribute("aria-readonly") === "true",
        type: el.tagName === "INPUT" ? (el.getAttribute("type") || "").toLowerCase() : "",
        expanded: el.getAttribute("aria-expanded"),
        checked: f.type === "checkbox" ? !!f.checked : el.getAttribute("role") === "checkbox" ? el.getAttribute("aria-checked") === "true" : null
      };
    };
    const realImages = (root) => [...root.querySelectorAll("img")].filter((m) => (m.getAttribute("src") || "").trim() !== "");
    const fileInfo = (el, i) => {
      const root = slotRoot(el);
      return {
        index: i,
        tag: "input",
        text: text(root),
        label: "",
        disabled: !!el.disabled,
        readOnly: false,
        type: "file",
        expanded: null,
        checked: null,
        accept: el.getAttribute("accept") || "",
        images: realImages(root).map((m) => ({
          alt: m.getAttribute("alt") || "",
          src: m.currentSrc || m.getAttribute("src") || ""
        }))
      };
    };
    const slotInfo = (el, i) => {
      const r = el.getBoundingClientRect();
      return {
        index: i,
        tag: el.tagName.toLowerCase(),
        text: el.tagName === "IMG" ? "" : text(el),
        label: el.tagName === "IMG" ? el.getAttribute("alt") || "" : el.getAttribute("aria-label") || "",
        disabled: el.disabled === true || el.getAttribute("aria-disabled") === "true",
        readOnly: false,
        type: "",
        expanded: el.getAttribute("aria-expanded"),
        checked: null,
        width: Math.round(r.width),
        height: Math.round(r.height)
      };
    };
    const get = (kind, i) => {
      const el = (store[kind] || [])[i];
      if (!el || !el.isConnected)
        throw new Error("EDICTORA_ELEMENT_GONE:" + kind);
      return el;
    };
    return {
      collect: (kind) => {
        const els = sources[kind]();
        store[kind] = els;
        return kind === "fileInput" ? els.map(fileInfo) : els.map(info);
      },
      collectSlot: (kind, slotIndex) => {
        const input = (store.fileInput || [])[slotIndex];
        if (!input || !input.isConnected)
          throw new Error("EDICTORA_ELEMENT_GONE:fileInput");
        const pick = slotSources[kind];
        if (!pick)
          throw new Error("EDICTORA_ELEMENT_GONE:" + kind);
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
        const describe = (n) => {
          if (!n)
            return "__EDICTORA_OFFSCREEN__";
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
            height: Math.round(r.height)
          },
          viewport: { width: innerWidth, height: innerHeight }
        };
      },
      focusSelectAll: (kind, i) => {
        const el = get(kind, i);
        const length = typeof el.value === "string" ? el.value.length : 0;
        if (typeof el.focus === "function")
          el.focus();
        if (typeof el.setSelectionRange === "function")
          el.setSelectionRange(0, length);
        else if (typeof el.select === "function")
          el.select();
        return document.activeElement === el && el.selectionStart === 0 && el.selectionEnd === length;
      },
      value: (kind, i) => get(kind, i).value,
      messages: () => [...document.querySelectorAll("[role=alert], [role=status], [aria-live]")].map(text).filter(Boolean),
      dialogs: () => allDialogs().map(text),
      bodyText: () => document.body ? document.body.innerText : ""
    };
  }
  if (!host.__edictora) {
    host.__edictora = createApi();
  }
})();
