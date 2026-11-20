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
      checkbox: () => [...document.querySelectorAll("input[type=checkbox], [role=checkbox]")].filter((c) => visible(c) && !inDialog(c)),
      fileInput: () => [...document.querySelectorAll("input[type=file]")],
      slotImage: () => [],
      imageButton: () => [],
      listboxOption: () => [...document.querySelectorAll("[role=option]")]
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
    const optionInfo = (el, i) => {
      const lists = [...document.querySelectorAll("[role=listbox]")];
      const list = el.closest("[role=listbox]");
      const r = (list || el).getBoundingClientRect();
      const parts = [];
      if (list) {
        const al = list.getAttribute("aria-label");
        if (al)
          parts.push(al);
        const lb = list.getAttribute("aria-labelledby");
        if (lb) {
          for (const id of lb.split(/\s+/)) {
            const t = document.getElementById(id);
            if (t)
              parts.push(text(t));
          }
        }
      }
      return {
        index: i,
        tag: el.tagName.toLowerCase(),
        text: text(el),
        label: parts.join(`
`),
        disabled: el.getAttribute("aria-disabled") === "true",
        readOnly: false,
        type: "",
        expanded: null,
        checked: null,
        dataValue: el.getAttribute("data-value") || "",
        selected: el.getAttribute("aria-selected"),
        group: list ? lists.indexOf(list) : -1,
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
    const SNAPSHOT_MAX_CHARS = 1e6;
    const SNAPSHOT_VALUE_MAX = 2000;
    const SNAPSHOT_MAX_DEPTH = 80;
    const NAME_FROM_CONTENT = new Set([
      "button",
      "link",
      "heading",
      "option",
      "tab",
      "menuitem",
      "checkbox",
      "radio",
      "switch",
      "img"
    ]);
    const IMPLICIT_ROLE = {
      button: "button",
      textarea: "textbox",
      select: "combobox",
      dialog: "dialog",
      nav: "navigation",
      main: "main",
      header: "banner",
      footer: "contentinfo",
      aside: "complementary",
      form: "form",
      ul: "list",
      ol: "list",
      li: "listitem",
      table: "table",
      tr: "row",
      td: "cell",
      th: "columnheader",
      progress: "progressbar",
      option: "option",
      details: "group",
      article: "article",
      img: "img",
      h1: "heading",
      h2: "heading",
      h3: "heading",
      h4: "heading",
      h5: "heading",
      h6: "heading"
    };
    const INPUT_ROLE = {
      button: "button",
      submit: "button",
      reset: "button",
      image: "button",
      checkbox: "checkbox",
      radio: "radio",
      range: "slider",
      number: "spinbutton",
      file: "file-input"
    };
    const SKIP_TAGS = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "HEAD", "META", "LINK"]);
    const clip = (s) => s.length > SNAPSHOT_VALUE_MAX ? `${s.slice(0, SNAPSHOT_VALUE_MAX)}...(${s.length} chars)` : s;
    const oneLine = (s) => s.replace(/\s+/g, " ").trim();
    const quoted = (s) => JSON.stringify(clip(oneLine(s)));
    const roleOf = (el) => {
      const explicit = (el.getAttribute("role") || "").trim().split(/\s+/)[0];
      if (explicit === "presentation" || explicit === "none" || explicit === "generic")
        return "";
      if (explicit)
        return explicit;
      const tag = el.tagName.toLowerCase();
      if (tag === "a")
        return el.hasAttribute("href") ? "link" : "";
      if (tag === "input") {
        const type = (el.getAttribute("type") || "text").toLowerCase();
        if (type === "hidden")
          return "";
        return INPUT_ROLE[type] || (el.hasAttribute("list") ? "combobox" : "textbox");
      }
      if (tag === "select") {
        const s = el;
        return s.multiple || s.size > 1 ? "listbox" : "combobox";
      }
      if (tag === "section")
        return el.hasAttribute("aria-label") || el.hasAttribute("aria-labelledby") ? "region" : "";
      return IMPLICIT_ROLE[tag] || "";
    };
    const nameOf = (el, role) => {
      const al = el.getAttribute("aria-label");
      if (al && al.trim())
        return al;
      const lb = el.getAttribute("aria-labelledby");
      if (lb) {
        const t = lb.split(/\s+/).map((id) => document.getElementById(id)).filter((n) => !!n).map(text).join(" ");
        if (t.trim())
          return t;
      }
      const labels = el.labels;
      if (labels && labels.length > 0) {
        const t = [...labels].map(text).join(" ");
        if (t.trim())
          return t;
      }
      if (el.tagName === "IMG" && el.getAttribute("alt"))
        return el.getAttribute("alt") || "";
      if (NAME_FROM_CONTENT.has(role) && el.tagName !== "IMG") {
        const t = text(el);
        if (t)
          return t;
      }
      return el.getAttribute("placeholder") || el.getAttribute("title") || "";
    };
    const statesOf = (el) => {
      const f = el;
      const out = [];
      if (f.disabled || el.getAttribute("aria-disabled") === "true")
        out.push("disabled");
      if (f.readOnly || el.getAttribute("aria-readonly") === "true")
        out.push("readonly");
      if (f.required || el.getAttribute("aria-required") === "true")
        out.push("required");
      for (const a of ["expanded", "selected", "pressed", "current", "invalid", "haspopup", "busy"]) {
        const v = el.getAttribute("aria-" + a);
        if (v !== null)
          out.push(`${a}=${v}`);
      }
      if (f.type === "checkbox" || f.type === "radio")
        out.push(`checked=${!!f.checked}`);
      else if (el.hasAttribute("aria-checked"))
        out.push(`checked=${el.getAttribute("aria-checked")}`);
      if (document.activeElement === el)
        out.push("focused");
      const dv = el.getAttribute("data-value");
      if (dv)
        out.push(`data-value=${quoted(dv)}`);
      return out;
    };
    const valueOf = (el) => {
      const tag = el.tagName;
      if (tag === "TEXTAREA")
        return clip(el.value);
      if (tag === "INPUT") {
        const i = el;
        const type = (i.getAttribute("type") || "text").toLowerCase();
        if (type === "password")
          return "(masked)";
        if (["checkbox", "radio", "button", "submit", "reset", "image", "file"].includes(type))
          return null;
        return clip(i.value);
      }
      if (tag === "SELECT") {
        return [...el.selectedOptions].map((o) => oneLine(o.textContent || "")).join(", ");
      }
      return null;
    };
    const renderState = (el) => {
      const s = getComputedStyle(el);
      if (s.display === "contents")
        return "contents";
      if (s.display === "none" || s.visibility === "hidden")
        return "gone";
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0 ? "shown" : "empty";
    };
    const snapshot = () => {
      const lines = [];
      let size = 0;
      let truncated = false;
      const push = (depth, line) => {
        if (truncated)
          return;
        if (size + line.length > SNAPSHOT_MAX_CHARS) {
          truncated = true;
          lines.push("... (truncated: output size limit reached)");
          return;
        }
        lines.push("  ".repeat(depth) + line);
        size += line.length + depth * 2 + 1;
      };
      const walkChildren = (nodes, depth, level, shown) => {
        for (const n of nodes) {
          if (truncated)
            return;
          if (n.nodeType === Node.TEXT_NODE) {
            const t = oneLine(n.textContent || "");
            if (t && shown)
              push(depth, `text ${quoted(t)}`);
          } else if (n.nodeType === Node.ELEMENT_NODE) {
            walk(n, depth, level + 1, shown);
          }
        }
      };
      const walk = (el, depth, level, parentShown) => {
        if (truncated || SKIP_TAGS.has(el.tagName))
          return;
        if (level > SNAPSHOT_MAX_DEPTH) {
          push(depth, "... (truncated: depth limit reached)");
          return;
        }
        const state = renderState(el);
        if (state === "gone")
          return;
        const shown = state === "contents" ? parentShown : state === "shown";
        const tag = el.tagName.toLowerCase();
        if (tag === "svg") {
          const label = el.getAttribute("aria-label");
          if (label && shown)
            push(depth, `img ${quoted(label)}`);
          return;
        }
        if (tag === "slot") {
          const assigned = el.assignedNodes({ flatten: true });
          walkChildren(assigned.length > 0 ? assigned : el.childNodes, depth, level, shown);
          return;
        }
        if (tag === "iframe") {
          const frame = el;
          let doc = null;
          try {
            doc = frame.contentDocument;
          } catch {
            doc = null;
          }
          const fr = el.getBoundingClientRect();
          if (shown) {
            push(depth, `iframe ${quoted(el.getAttribute("title") || "")}${doc ? "" : " (not readable: cross-origin)"} ${rectOf(fr)}`);
          }
          if (doc && doc.body)
            walk(doc.body, depth + 1, level + 1, shown);
          return;
        }
        const role = roleOf(el);
        const leaf = NAME_FROM_CONTENT.has(role) || tag === "textarea" || tag === "select";
        const hiddenKept = !shown && state === "empty" && (role === "option" || tag === "input" && role === "file-input");
        const printSelf = !!role && (shown || hiddenKept);
        const children = () => {
          if (leaf)
            return;
          const root = el.shadowRoot;
          walkChildren(root ? root.childNodes : el.childNodes, printSelf ? depth + 1 : depth, level, shown);
        };
        if (!printSelf) {
          children();
          return;
        }
        const parts = [role === "heading" ? headingRole(el) : role];
        const name = nameOf(el, role);
        if (name.trim())
          parts.push(quoted(name));
        const value = valueOf(el);
        if (value !== null)
          parts.push(`value=${JSON.stringify(value)}`);
        const states = statesOf(el);
        if (hiddenKept)
          states.unshift("hidden");
        if (states.length > 0)
          parts.push(`{${states.join(" ")}}`);
        parts.push(rectOf(el.getBoundingClientRect()));
        push(depth, parts.join(" "));
        children();
      };
      const rectOf = (r) => `@${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)}`;
      const headingRole = (el) => {
        const level = el.getAttribute("aria-level") || (/^H[1-6]$/.test(el.tagName) ? el.tagName.slice(1) : "");
        return level ? `heading[${level}]` : "heading";
      };
      const rootEl = document.body || document.documentElement;
      try {
        if (rootEl)
          walk(rootEl, 0, 0, true);
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
        'format: <role> "<name>" value="<value>" {<state>...} @x,y,w,h  (indent = nesting; text "..." = plain text; invisible elements omitted except option / file-input marked {hidden})',
        "---"
      ];
      return header.join(`
`) + `
` + lines.join(`
`) + `
`;
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
