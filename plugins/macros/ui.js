// Macros page. The engine (engine.js) owns the data and everything that
// runs; this page shows the state the engine sends and sends back edits.
(() => {
  "use strict";

  // ── Small helpers ────────────────────────────────────────────────────

  const $app = document.getElementById("app");
  const clone = (v) => JSON.parse(JSON.stringify(v));
  const call = (m, ...a) => pious.call(m, ...a);
  const send = (msg) => call("send", msg).catch(() => {});
  const uuid = () =>
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
          const r = (Math.random() * 16) | 0;
          return (c === "x" ? r : (r & 3) | 8).toString(16);
        });
  const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

  function store(key, value) {
    try {
      if (value === undefined) return localStorage.getItem("macros." + key);
      localStorage.setItem("macros." + key, value);
    } catch {
      return null;
    }
  }

  // Like replaceChildren, but leaves out empty children (null would show as the text "null").
  const fill = (el, ...kids) => el.replaceChildren(...kids.flat(Infinity).filter((k) => k !== null && k !== undefined && k !== false));

  function h(tag, props, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) {
      if (v === undefined || v === null || v === false) continue;
      if (k === "class") el.className = v;
      else if (k === "style") el.style.cssText = v;
      else if (k.startsWith("on")) el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === "value" || k === "checked" || k === "disabled" || k === "selected") el[k] = v;
      else el.setAttribute(k, v === true ? "" : v);
    }
    const add = (kid) => {
      if (kid === null || kid === undefined || kid === false) return;
      if (Array.isArray(kid)) kid.forEach(add);
      else el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
    };
    kids.forEach(add);
    return el;
  }

  // Pious's own icons, fetched once at startup (see start-up at the end).
  const ICON_NAMES = [
    "play", "stop", "add", "remove", "record", "download", "copy", "more", "close", "up", "down", "target", "edit",
    "keyboard", "text", "autoclick", "sort", "clock", "refresh", "eyedropper", "focus", "external", "macros",
    "warning", "tip", "grid", "list", "check",
  ];
  let ICONS = {};
  function icon(name, size) {
    const el = h("span", { class: "icon" });
    el.innerHTML = ICONS[name] || "";
    if (size) el.style.width = el.style.height = size + "px";
    return el;
  }
  const btn = (label, opts = {}) =>
    h(
      "button",
      { class: "btn " + (opts.class || ""), type: "button", title: opts.title, onclick: opts.onclick, disabled: opts.disabled },
      opts.icon ? icon(opts.icon) : null,
      label,
    );
  const iconBtn = (name, label, onclick, disabled) =>
    h("button", { class: "icon-btn", type: "button", title: label, "aria-label": label, onclick, disabled }, icon(name, 14));
  // ── Keys ─────────────────────────────────────────────────────────────

  const KEY_NAMES = {
    Control: "Ctrl",
    Super: "Win",
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
    PageUp: "Page Up",
    PageDown: "Page Down",
    Escape: "Esc",
  };
  const keyLabel = (combo) =>
    !combo
      ? "Not set"
      : String(combo)
          .split("+")
          .map((p) => KEY_NAMES[p] ?? p.replace(/^Key/, "").replace(/^Digit/, "").replace(/^Numpad/, "Num "))
          .join(" + ");

  const isModifierEvent = (e) => /^(Control|Shift|Alt|Meta|OS)/.test(e.code) || ["Control", "Shift", "Alt", "Meta", "OS"].includes(e.key);
  function comboFromEvent(e) {
    if (isModifierEvent(e)) return null;
    const parts = [];
    if (e.ctrlKey) parts.push("Control");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    if (e.metaKey) parts.push("Super");
    parts.push(e.code);
    return parts.join("+");
  }

  /** Click, then press the keys you want (in combination). Esc cancels, Backspace clears. */
  function hotkeyInput(value, onchange, { clearable = false, width = "170px" } = {}) {
    let listening = false;
    const label = h("span", { class: "line" }, keyLabel(value));
    const button = h("button", { type: "button", class: "hotkey input", style: `width:${width}`, title: "Click to change" }, label);
    const stop = () => {
      listening = false;
      window.removeEventListener("keydown", onkey, true);
      button.classList.remove("listening");
      fill(button, label);
    };
    function onkey(e) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey) return stop();
      if (e.key === "Backspace" && clearable && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        onchange("");
        label.textContent = keyLabel("");
        return stop();
      }
      const combo = comboFromEvent(e);
      if (!combo) {
        const held = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win"].filter(Boolean).join(" + ");
        fill(button, h("span", { class: "pulse-dot" }), h("span", { class: "line" }, held ? `${held} + …` : "Press keys…"));
        return;
      }
      onchange(combo);
      label.textContent = keyLabel(combo);
      stop();
    }
    button.addEventListener("click", () => {
      if (listening) return stop();
      listening = true;
      button.classList.add("listening");
      fill(button, h("span", { class: "pulse-dot" }), h("span", { class: "line" }, "Press keys…"));
      window.addEventListener("keydown", onkey, true);
    });
    button.addEventListener("blur", () => listening && stop());
    return button;
  }

  /** One key (not a combination): click, then press it. */
  function keyPicker(value, onchange, width = "120px") {
    let listening = false;
    const text = () => keyLabel(value) === "Not set" ? "Pick a key" : keyLabel(value);
    const button = h("button", { type: "button", class: "hotkey input", style: `width:${width}`, title: "Click, then press a key" }, text());
    const stop = () => {
      listening = false;
      window.removeEventListener("keydown", onkey, true);
      button.classList.remove("listening");
      button.textContent = text();
    };
    function onkey(e) {
      e.preventDefault();
      e.stopPropagation();
      value = e.code;
      onchange(value);
      stop();
    }
    button.addEventListener("click", () => {
      if (listening) return stop();
      listening = true;
      button.classList.add("listening");
      fill(button, h("span", { class: "pulse-dot" }), "Press a key…");
      window.addEventListener("keydown", onkey, true);
    });
    button.addEventListener("blur", () => listening && stop());
    return button;
  }

  /**
   * Picks a point on the screen: while the pointer is moved there, the spot
   * (and color) under it is read live; Enter or the button takes it right
   * away, otherwise it is taken after 3 seconds (the page loses the keyboard
   * once you go to the game). Esc cancels.
   */
  function pointPicker(label, onpick) {
    let picking = false;
    let timer = 0;
    let poll = 0;
    let left = 0;
    let reading = null;
    const live = h("span", { class: "picker-live" });
    const button = h("button", { type: "button", class: "btn small", title: "Move the pointer to the spot within 3 seconds" });
    const idle = () => {
      button.classList.remove("counting");
      fill(button, icon("target"), label);
    };

    async function read() {
      try {
        const c = await call("input.cursor");
        let color = null;
        try {
          color = await call("input.pixel", c.x, c.y, null);
        } catch {
          /* no color */
        }
        reading = { x: c.x, y: c.y, color };
        live.textContent = `${c.x}, ${c.y}${color ? " " + color : ""}`;
      } catch {
        /* ignore */
      }
    }
    function end() {
      picking = false;
      clearInterval(timer);
      clearInterval(poll);
      window.removeEventListener("keydown", onkey, true);
      live.textContent = "";
      idle();
    }
    async function take() {
      await read();
      const r = reading;
      end();
      if (r) onpick(r.x, r.y, r.color);
    }
    function onkey(e) {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        end();
      } else if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        e.stopPropagation();
        take();
      }
    }
    function paint() {
      button.classList.add("counting");
      fill(button, h("span", { class: "pp-n" }, String(left)), "Move the pointer there");
    }
    button.addEventListener("click", () => {
      if (picking) return take();
      picking = true;
      left = 3;
      reading = null;
      paint();
      window.addEventListener("keydown", onkey, true);
      poll = setInterval(() => (button.isConnected ? read() : end()), 100);
      timer = setInterval(() => {
        if (!button.isConnected) return end();
        left--;
        if (left <= 0) take();
        else paint();
      }, 1000);
      read();
    });
    idle();
    return h("span", { class: "row" }, button, live);
  }

  // ── Form controls ────────────────────────────────────────────────────

  function select(options, value, onchange, width) {
    const el = h(
      "select",
      { class: "input select", style: `width:${width || "100%"}` },
      options.map((o) => h("option", { value: String(o.value), selected: String(o.value) === String(value) }, o.label)),
    );
    el.addEventListener("change", () => {
      const o = options.find((x) => String(x.value) === el.value);
      if (o) onchange(o.value);
    });
    return el;
  }
  function numInput(value, onchange, { min, max, cls = "num", step } = {}) {
    const el = h("input", { class: "input " + cls, type: "number", value: String(value ?? 0), min, max, step });
    el.addEventListener("change", () => {
      let n = Number(el.value);
      if (!Number.isFinite(n)) n = 0;
      n = Math.round(n);
      if (min !== undefined) n = Math.max(min, n);
      if (max !== undefined) n = Math.min(max, n);
      el.value = String(n);
      onchange(n);
    });
    return el;
  }
  function intInput(value, onchange) {
    // Free numbers (coordinates, scroll), not clamped.
    return numInput(value, onchange, {});
  }
  function toggle(on, onchange) {
    let state = !!on;
    const el = h("button", { type: "button", class: "switch" + (state ? " on" : ""), role: "switch", "aria-checked": String(state) }, h("span"));
    el.addEventListener("click", () => {
      state = !state;
      el.classList.toggle("on", state);
      el.setAttribute("aria-checked", String(state));
      onchange(state);
    });
    return el;
  }
  function textInput(value, onchange, placeholder, cls = "") {
    const el = h("input", { class: "input " + cls, type: "text", value: value ?? "", placeholder });
    el.addEventListener("change", () => onchange(el.value));
    return el;
  }
  const unit = (...kids) => h("label", { class: "unit" }, ...kids);
  function settingRow(title, desc, control) {
    return h("div", { class: "setting-row" }, h("div", { class: "text" }, h("span", { class: "title" }, title), desc ? h("span", { class: "description" }, desc) : null), control);
  }
  /** A labeled group: a small caption over a card of rows split by dividers. */
  function group(label, ...rows) {
    const kids = [];
    for (const r of rows.flat(Infinity)) {
      if (r === null || r === undefined || r === false) continue;
      if (kids.length) kids.push(h("hr", { class: "divider" }));
      kids.push(r);
    }
    return h("section", { class: "group" }, label ? h("span", { class: "label" }, label) : null, h("div", { class: "glass list" }, kids));
  }
  const field = (label, control, hint) => h("div", { class: "field" }, h("span", { class: "label" }, label), control, hint ? h("span", { class: "secondary" }, hint) : null);
  /** A slider with its value beside it; click the value to type one. */
  function slider(value, { min, max, step, digits = 2, unit = "×" }, onchange) {
    const range = h("input", { type: "range", min: String(min), max: String(max), step: String(step), value: String(value), style: "width:100%" });
    const shown = (v) => `${Number(v).toFixed(digits)}${unit}`;
    const val = h("button", { type: "button", class: "value number", title: "Click to type a value" }, shown(value));
    range.addEventListener("input", () => {
      const v = Math.round(Number(range.value) * 100) / 100;
      val.textContent = shown(v);
      onchange(v);
    });
    val.addEventListener("click", () => {
      const box = h("input", { class: "input number", value: Number(range.value).toFixed(digits) });
      let done = false;
      const commit = () => {
        if (done) return;
        done = true;
        const typed = Number(String(box.value).replace(",", ".").replace(/[^\d.\-]/g, ""));
        if (Number.isFinite(typed) && String(box.value).trim() !== "") {
          const v = Math.round(Math.min(max, Math.max(min, typed)) * 100) / 100;
          range.value = String(v);
          val.textContent = shown(v);
          onchange(v);
        }
        box.replaceWith(val);
      };
      box.addEventListener("blur", commit);
      box.addEventListener("keydown", (e) => {
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") {
          e.stopPropagation();
          done = true;
          box.replaceWith(val);
        }
      });
      val.replaceWith(box);
      box.focus();
      if (box.select) box.select();
    });
    return h("div", { class: "slider" }, range, val);
  }

  // ── Popover menu ─────────────────────────────────────────────────────

  let openMenu = null;
  function closeMenu() {
    if (openMenu) openMenu.remove();
    openMenu = null;
  }
  function popMenu(anchor, items) {
    closeMenu();
    const menu = h(
      "div",
      { class: "menu panel" },
      items.map((it) =>
        it === "-"
          ? h("hr", { class: "divider" })
          : h(
              "button",
              {
                type: "button",
                class: it.danger ? "danger" : "",
                onclick: (e) => {
                  e.stopPropagation();
                  closeMenu();
                  it.action();
                },
              },
              it.icon ? icon(it.icon) : null,
              it.label,
            ),
      ),
    );
    document.body.append(menu);
    const r = anchor.getBoundingClientRect();
    const w = menu.offsetWidth;
    const hgt = menu.offsetHeight;
    menu.style.left = Math.max(8, Math.min(window.innerWidth - w - 8, r.right - w)) + "px";
    const below = r.bottom + 4 + hgt <= window.innerHeight;
    menu.style.top = Math.max(8, below ? r.bottom + 4 : r.top - hgt - 4) + "px";
    openMenu = menu;
  }
  document.addEventListener("click", (e) => {
    if (openMenu && !openMenu.contains(e.target)) closeMenu();
  });

  // ── State ────────────────────────────────────────────────────────────

  const S = { loaded: false, macros: [], settings: {}, autoclicker: {}, running: [], clicking: false, recording: false };
  const ui = {
    tab: "macros",
    editing: null,
    view: store("view") === "Grid" ? "Grid" : "List",
    sort: ["name", "steps", "added"].includes(store("sort")) ? store("sort") : "name",
    accounts: [],
    importOpen: false,
    exported: null,
    notice: null, // { text, error }
    confirmDelete: null,
  };
  let draft = null;
  let saveTimer = 0;

  const TARGETS = [
    { value: "Roblox", label: "The Roblox window you used last" },
    { value: "AllRoblox", label: "Every Roblox window" },
    { value: "Accounts", label: "Windows of accounts I pick" },
    { value: "System", label: "Whatever's in front" },
  ];
  const targetText = (t) =>
    t === "System"
      ? "Like a real keyboard and mouse: your pointer moves, and it goes to whatever window is in front."
      : t === "Accounts"
        ? "Only the Roblox windows of the accounts you pick below, in the background, so each instance can run its own macro."
        : t === "AllRoblox"
          ? "Every Roblox window at once, in the background. Your own mouse and keyboard stay free."
          : 'Straight to Roblox, even when it\'s behind other windows. Your own mouse and keyboard stay free, so you can keep using your PC. (Turning the camera with the mouse needs "Whatever\'s in front".)';

  const countSteps = (steps) => steps.reduce((n, s) => n + 1 + (s.kind === "Loop" ? countSteps(s.steps || []) : 0), 0);
  const repeatText = (m) =>
    m.repeat === "Once" ? "Runs once" : m.repeat === "Times" ? `Runs ${m.times} times` : m.repeat === "WhileHeld" ? "Runs while the key is held" : "Runs until stopped";
  const isRunning = (id) => S.running.includes(id);
  const accountName = (a) => a.nickname || a.display_name || a.username || a.id;

  function say(text, error = false) {
    ui.notice = { text, error };
    renderNotice();
    clearTimeout(say.t);
    say.t = setTimeout(() => {
      ui.notice = null;
      renderNotice();
    }, 6000);
  }

  // ── Messages with the engine ─────────────────────────────────────────

  function onMessage(msg) {
    if (!msg || typeof msg !== "object") return;
    if (msg.type === "state") {
      S.loaded = true;
      for (const k of ["macros", "settings", "autoclicker", "running", "clicking", "recording"]) S[k] = msg[k];
      if (draft && saveTimer) {
        const i = S.macros.findIndex((m) => m.id === draft.id);
        if (i >= 0) S.macros[i] = clone(draft);
      }
      renderAll();
    } else if (msg.type === "imported") {
      ui.importOpen = false;
      if (msg.error) say(msg.error, true);
      else {
        ui.editing = msg.id;
        ui.tab = "macros";
        draft = null;
        say(msg.skipped ? `Imported. ${plural(msg.skipped, "line")} Pious can't run stayed in as notes.` : "Imported your AutoHotkey script as a macro.");
        pious.call("toast", "Imported your AutoHotkey script as a macro.").catch(() => {});
      }
      renderAll();
    } else if (msg.type === "ahk") {
      ui.exported = { name: msg.name, text: msg.text };
      renderExport();
    }
  }

  // ── Macro editing (draft, saved a moment after each change) ──────────

  function openEditor(id) {
    flush();
    ui.editing = id;
    draft = null;
    ui.exported = null;
    syncDraft();
    renderAll();
  }
  function syncDraft() {
    const found = S.macros.find((m) => m.id === ui.editing);
    if (!found) {
      draft = null;
      if (ui.editing) ui.editing = null;
      return;
    }
    if (!draft || draft.id !== found.id) draft = clone(found);
  }
  function touch() {
    if (!draft) return;
    const i = S.macros.findIndex((m) => m.id === draft.id);
    if (i >= 0) S.macros[i] = clone(draft);
    renderList();
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 350);
  }
  function flush() {
    if (!saveTimer) return;
    clearTimeout(saveTimer);
    saveTimer = 0;
    if (draft) send({ type: "saveMacro", macro: clone(draft) });
  }
  function change(patch) {
    if (!draft) return;
    Object.assign(draft, patch);
    touch();
  }

  function createMacro() {
    flush();
    const m = {
      id: uuid(),
      name: `Macro ${S.macros.length + 1}`,
      hotkey: "",
      repeat: "Once",
      times: 3,
      speed: 1,
      game_only: false,
      target: "Roblox",
      accounts: [],
      steps: [],
    };
    S.macros.push(m);
    send({ type: "saveMacro", macro: m });
    openEditor(m.id);
  }
  function duplicateMacro(m) {
    flush();
    const copy = { ...clone(m), id: uuid(), name: `${m.name} copy`, hotkey: "" };
    S.macros.push(copy);
    send({ type: "saveMacro", macro: copy });
    renderAll();
  }
  function deleteMacro(m) {
    if (ui.confirmDelete !== m.id) {
      ui.confirmDelete = m.id;
      say(`Delete ${m.name}? Choose Delete again to confirm. Its steps and hotkey are gone for good.`, true);
      clearTimeout(deleteMacro.t);
      deleteMacro.t = setTimeout(() => (ui.confirmDelete = null), 6000);
      return;
    }
    ui.confirmDelete = null;
    if (ui.editing === m.id) {
      ui.editing = null;
      draft = null;
      clearTimeout(saveTimer);
      saveTimer = 0;
    }
    S.macros = S.macros.filter((x) => x.id !== m.id);
    send({ type: "deleteMacro", id: m.id });
    ui.notice = null;
    renderAll();
  }
  function runMacro(m) {
    flush();
    send({ type: "run", id: m.id });
  }
  function exportMacro(m) {
    flush();
    send({ type: "exportAhk", id: m.id });
  }
  const setSetting = (patch) => {
    Object.assign(S.settings, patch);
    send({ type: "settings", patch });
  };
  const setClicker = (patch) => {
    Object.assign(S.autoclicker, patch);
    send({ type: "autoclicker", patch });
  };

  // ── Steps ────────────────────────────────────────────────────────────

  const PRESS = [
    { value: "Tap", label: "Press" },
    { value: "Down", label: "Hold down" },
    { value: "Up", label: "Let go" },
  ];
  const BUTTONS = [
    { value: "Left", label: "Left button" },
    { value: "Right", label: "Right button" },
    { value: "Middle", label: "Middle button" },
    { value: "Back", label: "Back button" },
    { value: "Forward", label: "Forward button" },
  ];
  const KINDS = [
    { kind: "Key", icon: "keyboard", label: "Press a key", make: () => ({ kind: "Key", key: "KeyE", press: "Tap", hold_ms: 50 }) },
    { kind: "Text", icon: "text", label: "Type text", make: () => ({ kind: "Text", text: "", delay_ms: 20 }) },
    { kind: "Click", icon: "autoclick", label: "Click", make: () => ({ kind: "Click", button: "Left", press: "Tap", count: 1 }) },
    { kind: "Move", icon: "target", label: "Move the pointer", make: () => ({ kind: "Move", x: 0, y: 0, relative: false, duration_ms: 0 }) },
    { kind: "Scroll", icon: "sort", label: "Scroll", make: () => ({ kind: "Scroll", amount: -3, horizontal: false }) },
    { kind: "Wait", icon: "clock", label: "Wait", make: () => ({ kind: "Wait", ms: 500, random_ms: 0 }) },
    { kind: "Loop", icon: "refresh", label: "Repeat steps", make: () => ({ kind: "Loop", times: 5, steps: [] }) },
    { kind: "WaitPixel", icon: "eyedropper", label: "Wait for a color", make: () => ({ kind: "WaitPixel", x: 0, y: 0, color: "#FFFFFF", tolerance: 10, timeout_ms: 0 }) },
    { kind: "FocusRoblox", icon: "focus", label: "Bring Roblox forward", make: () => ({ kind: "FocusRoblox" }) },
    { kind: "Run", icon: "external", label: "Open a program or link", make: () => ({ kind: "Run", target: "" }) },
    { kind: "Comment", icon: "edit", label: "Note", make: () => ({ kind: "Comment", text: "" }) },
  ];
  const kindMeta = (kind) => KINDS.find((k) => k.kind === kind) || KINDS[0];

  /** The list of steps (a macro's, or a loop's), edited in place. */
  function stepList(steps, depth, rebuild) {
    const box = h("div", { class: "steps" + (depth ? " nested" : "") });
    const mutate = () => touch();
    const structural = () => {
      touch();
      rebuild();
    };
    steps.forEach((step, i) => {
      const set = (patch) => {
        Object.assign(step, patch);
        mutate();
      };
      let body;
      switch (step.kind) {
        case "Key":
          body = [
            select(PRESS, step.press || "Tap", (press) => {
              set({ press });
              rebuild();
            }, "120px"),
            keyPicker(step.key, (key) => set({ key })),
            (step.press || "Tap") === "Tap" ? unit("for", numInput(step.hold_ms, (v) => set({ hold_ms: v }), { min: 0 }), "ms") : null,
          ];
          break;
        case "Text":
          body = [textInput(step.text, (text) => set({ text }), "Text to type", "grow"), unit(numInput(step.delay_ms, (v) => set({ delay_ms: v }), { min: 0 }), "ms per letter")];
          break;
        case "Click":
          body = [
            select(PRESS, step.press || "Tap", (press) => {
              set({ press });
              rebuild();
            }, "120px"),
            select(BUTTONS, step.button || "Left", (button) => set({ button }), "150px"),
            (step.press || "Tap") === "Tap" ? unit(numInput(step.count, (v) => set({ count: v }), { min: 1 }), "time(s)") : null,
          ];
          break;
        case "Move":
          body = [
            select(
              [
                { value: false, label: "To a spot" },
                { value: true, label: "By an amount" },
              ],
              !!step.relative,
              (relative) => {
                set({ relative });
                rebuild();
              },
              "140px",
            ),
            unit("X", intInput(step.x, (x) => set({ x }))),
            unit("Y", intInput(step.y, (y) => set({ y }))),
            step.relative
              ? null
              : pointPicker("Pick a spot", (x, y) => {
                  set({ x, y });
                  rebuild();
                }),
            unit("over", numInput(step.duration_ms, (v) => set({ duration_ms: v }), { min: 0 }), "ms"),
          ];
          break;
        case "Scroll":
          body = [
            unit(intInput(step.amount, (amount) => set({ amount })), "notches"),
            h("span", { class: "secondary" }, "(negative scrolls down)"),
            unit("Sideways", toggle(step.horizontal, (horizontal) => set({ horizontal }))),
          ];
          break;
        case "Wait":
          body = [unit(numInput(step.ms, (v) => set({ ms: v }), { min: 0 }), "ms"), unit("plus up to", numInput(step.random_ms, (v) => set({ random_ms: v }), { min: 0 }), "ms at random")];
          break;
        case "Loop":
        {
          const hint = h("span", { class: "secondary" }, step.times === 0 ? "(until stopped)" : "(0 = until stopped)");
          body = [
            unit("Repeat", numInput(step.times, (v) => {
              set({ times: v });
              hint.textContent = v === 0 ? "(until stopped)" : "(0 = until stopped)";
            }, { min: 0 }), "times"),
            hint,
          ];
        }
          break;
        case "WaitPixel": {
          const swatch = h("span", { class: "swatch", style: `background:${step.color}` });
          body = [
            unit("X", intInput(step.x, (x) => set({ x }))),
            unit("Y", intInput(step.y, (y) => set({ y }))),
            unit(
              "is",
              swatch,
              textInput(step.color, (color) => {
                set({ color: color.trim() });
                swatch.style.background = color.trim();
              }, "#RRGGBB", "hex"),
            ),
            pointPicker("Pick spot and color", (x, y, color) => {
              set({ x, y, color: color || step.color });
              rebuild();
            }),
            unit("±", numInput(step.tolerance, (v) => set({ tolerance: v }), { min: 0, max: 255 })),
            unit("give up after", numInput(step.timeout_ms, (v) => set({ timeout_ms: v }), { min: 0 }), "ms"),
          ];
          break;
        }
        case "FocusRoblox":
          body = [h("span", { class: "meta" }, "Brings a Roblox window to the front.")];
          break;
        case "Run":
          body = [textInput(step.target, (target) => set({ target }), "A program, file or https:// link", "grow")];
          break;
        default:
          body = [textInput(step.text, (text) => set({ text }), "A note for yourself", "grow note")];
      }
      const move = (by) => {
        const j = i + by;
        if (j < 0 || j >= steps.length) return;
        [steps[i], steps[j]] = [steps[j], steps[i]];
        structural();
      };
      box.append(
        h(
          "div",
          { class: "step glass-base" },
          h("span", { class: "n" }, String(i + 1)),
          h("span", { class: "kind", title: kindMeta(step.kind).label }, icon(kindMeta(step.kind).icon, 14)),
          h("div", { class: "body" }, body),
          h(
            "div",
            { class: "tools" },
            iconBtn("up", "Move up", () => move(-1), i === 0),
            iconBtn("down", "Move down", () => move(1), i === steps.length - 1),
            iconBtn("copy", "Duplicate", () => {
              steps.splice(i + 1, 0, clone(step));
              structural();
            }),
            iconBtn("remove", "Remove", () => {
              steps.splice(i, 1);
              structural();
            }),
          ),
          step.kind === "Loop"
            ? h("div", { class: "inner" }, stepList((step.steps = Array.isArray(step.steps) ? step.steps : []), depth + 1, () => {
                rebuild();
              }))
            : null,
        ),
      );
    });
    const add = h("button", { type: "button", class: "add" }, icon("add", 14), depth ? "Add a step inside" : "Add a step");
    add.addEventListener("click", (e) => {
      e.stopPropagation();
      popMenu(
        add,
        KINDS.filter((k) => depth < 3 || k.kind !== "Loop").map((k) => ({
          icon: k.icon,
          label: k.label,
          action: () => {
            steps.push(k.make());
            structural();
          },
        })),
      );
    });
    box.append(add);
    return box;
  }

  // ── Rendering ────────────────────────────────────────────────────────

  const regions = {};
  function region(name, cls) {
    return (regions[name] = h("div", { class: cls || "" }));
  }
  const hasFocus = (el) => !!el && el.contains(document.activeElement) && /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName);

  function buildSkeleton() {
    fill(
      $app,
      region("header", "page-header"),
      h(
        "div",
        { class: "notice caution", role: "note" },
        icon("warning"),
        h(
          "div",
          {},
          h("b", {}, "Ban risk. "),
          "Roblox and many games treat macros and auto-clickers as automation, which can get an account kicked or banned (above all for AFK farming). Check a game's rules before using them there. You use them at your own risk; Pious isn't responsible for bans or lost items.",
        ),
      ),
      region("notice"),
      region("importer"),
      region("exporter"),
      region("tabs", "row tabs-row"),
      region("body", "col page-body"),
    );
  }

  let iconsReady = false;
  function renderAll() {
    if (!S.loaded || !iconsReady) return;
    if (!regions.header) buildSkeleton();
    syncDraft();
    renderHeader();
    renderNotice();
    renderImporter();
    renderExport();
    renderTabs();
    renderBody();
  }

  function renderHeader() {
    const rec = S.recording;
    fill(regions.header,
      h("div", { class: "col grow", style: "gap:2px" }, h("h1", { class: "page-title" }, "Macros"), h("span", { class: "meta" }, "Repeat what you do with one key, or let Pious click for you")),
      h(
        "button",
        { type: "button", class: "btn" + (rec ? " recording" : ""), onclick: () => send({ type: "toggleRecord" }) },
        rec ? [h("span", { class: "rec-dot" }), "Stop recording"] : [icon("record"), "Record"],
        h("span", { class: "kbd" }, keyLabel(S.settings.record_hotkey)),
      ),
      btn("Import AutoHotkey", {
        icon: "download",
        title: "Turn an AutoHotkey script (.ahk) into a macro",
        onclick: () => {
          ui.importOpen = !ui.importOpen;
          renderImporter();
        },
      }),
      S.running.length || S.clicking ? btn("Stop all", { icon: "stop", class: "danger", onclick: () => send({ type: "stopAll" }) }) : null,
    );
  }

  function renderNotice() {
    if (!regions.notice) return;
    fill(
      regions.notice,
      ui.notice ? h("div", { class: "notice" + (ui.notice.error ? " caution" : ""), role: "status" }, icon(ui.notice.error ? "warning" : "check"), h("div", {}, ui.notice.text)) : null,
    );
  }

  function renderImporter() {
    if (!regions.importer) return;
    // Only when it opens or closes, so typing isn't wiped by state updates.
    if (ui.importOpen === !!regions.importer.firstChild) return;
    if (!ui.importOpen) return fill(regions.importer);
    const text = h("textarea", { class: "input", rows: "7", placeholder: "Paste an AutoHotkey script here, or choose a file…", spellcheck: "false" });
    let name = "AutoHotkey macro";
    const file = h("input", { type: "file", accept: ".ahk,.ah2,.txt,text/plain" });
    file.addEventListener("change", async () => {
      const f = file.files && file.files[0];
      if (!f) return;
      if (f.size > 1024 * 1024) return say("That file is too big to be a macro.", true);
      text.value = (await f.text()).replace(/^﻿/, "");
      name = f.name.replace(/\.[^.]*$/, "") || name;
    });
    fill(regions.importer,      h(
        "section",
        { class: "panel col", style: "gap:10px" },
        h("div", { class: "row" }, h("span", { class: "section-title grow" }, "Import an AutoHotkey script"), iconBtn("close", "Close", () => ((ui.importOpen = false), renderImporter()))),
        h("span", { class: "secondary" }, "Reads Send, Sleep, Click, MouseMove, Loop and more (AutoHotkey v1 and v2). Anything else stays in the macro as a note."),
        text,
        h(
          "div",
          { class: "row" },
          file,
          h("span", { class: "spacer" }),
          btn("Import", {
            class: "primary",
            icon: "download",
            onclick: () => {
              if (!text.value.trim()) return say("Paste a script or choose a file first.", true);
              send({ type: "importAhk", text: text.value, name });
            },
          }),
        ),
      ),
    );
  }

  let exportShown = null;
  function renderExport() {
    if (!regions.exporter) return;
    const e = ui.exported;
    if (!e) {
      exportShown = null;
      return fill(regions.exporter);
    }
    if (exportShown === e) return;
    exportShown = e;
    const safe = e.name.replace(/[^\p{L}\p{N} -]/gu, "_").trim() || "macro";
    const area = h("textarea", { class: "input", rows: "8", readonly: true, spellcheck: "false" });
    area.value = e.text;
    const url = (() => {
      try {
        return URL.createObjectURL(new Blob([e.text], { type: "text/plain;charset=utf-8" }));
      } catch {
        return null;
      }
    })();
    fill(regions.exporter,      h(
        "section",
        { class: "panel col", style: "gap:10px" },
        h("div", { class: "row" }, h("span", { class: "section-title grow" }, `${e.name} as an AutoHotkey v2 script`), iconBtn("close", "Close", () => ((ui.exported = null), renderExport()))),
        area,
        h(
          "div",
          { class: "row" },
          url ? h("a", { class: "btn primary", href: url, download: `${safe}.ahk` }, icon("download"), `Download ${safe}.ahk`) : null,
          btn("Copy", {
            icon: "copy",
            onclick: async () => {
              try {
                await navigator.clipboard.writeText(e.text);
                say("Copied.");
              } catch {
                area.select();
                say("Select the text and copy it (Ctrl+C).");
              }
            },
          }),
          h("span", { class: "secondary" }, "Save it as a .ahk file and run it with AutoHotkey v2."),
        ),
      ),
    );
  }

  function renderTabs() {
    fill(regions.tabs,      h("button", { type: "button", class: "chip" + (ui.tab === "macros" ? " on" : ""), onclick: () => switchTab("macros") }, icon("macros"), `Macros · ${S.macros.length}`),
      h("button", { type: "button", class: "chip" + (ui.tab === "clicker" ? " on" : ""), onclick: () => switchTab("clicker") }, icon("autoclick"), "Auto-clicker", S.clicking ? h("span", { class: "live" }) : null),
    );
  }
  function switchTab(tab) {
    flush();
    ui.tab = tab;
    renderTabs();
    bodyKey = "";
    renderBody();
  }

  let bodyKey = "";
  function renderBody() {
    if (ui.tab === "macros") renderMacrosTab();
    else renderClickerTab();
  }

  /** A first-visit tip, shown until dismissed ("Got it"). */
  function tip(id, text) {
    if (store("tip." + id) === "1") return null;
    const el = h(
      "div",
      { class: "tip glass-base" },
      h("span", { class: "bulb" }, icon("tip", 15)),
      h("span", { class: "text grow" }, text),
      h("button", {
        type: "button",
        class: "btn small tertiary",
        onclick: () => {
          store("tip." + id, "1");
          el.remove();
        },
      }, "Got it"),
    );
    return el;
  }

  // The Macros tab: list + settings on the left, the editor on the right.
  let listEl = null;
  let editorEl = null;
  let recSettingsEl = null;
  let layoutEl = null;
  let liveRun = null;

  function renderMacrosTab() {
    const key = "macros:" + (draft ? draft.id : "list");
    if (bodyKey !== key || !layoutEl || !layoutEl.isConnected) {
      bodyKey = key;
      listEl = h("div", { class: "col", style: "gap:10px;min-width:0" });
      recSettingsEl = h("div", { class: "col" });
      editorEl = draft ? h("aside", { class: "editor panel" }) : null;
      layoutEl = h("div", { class: "layout" + (draft ? " editing" : "") }, h("div", { class: "col", style: "gap:18px;min-width:0" }, listEl, recSettingsEl), editorEl);
      fill(
        regions.body,
        tip("macros", `Press Record (${keyLabel(S.settings.record_hotkey)}) and do something: your keys, clicks and timing become a macro. Or build one step by step. Give it a hotkey to run it anywhere, even in game. ${keyLabel(S.settings.stop_hotkey)} stops everything.`),
        layoutEl,
      );
      renderListShell();
      renderRecSettings();
      if (draft) renderEditor();
    } else {
      if (!hasFocus(recSettingsEl)) renderRecSettings();
      if (draft && editorEl && !hasFocus(editorEl) && !saveTimer && editorStale()) renderEditor();
    }
    renderList();
    updateLive();
  }

  function editorStale() {
    const found = S.macros.find((m) => m.id === draft.id);
    if (!found) return false;
    if (JSON.stringify(found) !== JSON.stringify(draft)) {
      draft = clone(found);
      return true;
    }
    return false;
  }

  function renderListShell() {
    const sortSel = select(
      [
        { value: "name", label: "Sort: Name" },
        { value: "steps", label: "Sort: Most steps" },
        { value: "added", label: "Sort: Added" },
      ],
      ui.sort,
      (v) => {
        ui.sort = v;
        store("sort", v);
        renderList();
      },
      "180px",
    );
    sortSel.title = "Sort";
    const viewBtn = (mode, label, name) =>
      h(
        "button",
        {
          type: "button",
          class: "chip" + (ui.view === mode ? " on" : ""),
          "aria-label": label,
          title: label,
          onclick: () => {
            ui.view = mode;
            store("view", mode);
            for (const b of viewBox.childNodes) b.classList.remove("on");
            btnFor[mode].classList.add("on");
            renderList();
          },
        },
        icon(name),
      );
    const btnFor = {};
    const viewBox = h("div", { class: "toggle", role: "radiogroup", "aria-label": "View" });
    btnFor.Grid = viewBtn("Grid", "Cards", "grid");
    btnFor.List = viewBtn("List", "List", "list");
    fill(viewBox, btnFor.Grid, btnFor.List);
    fill(
      listEl,
      h("div", { class: "row" }, btn("New macro", { icon: "add", class: "primary", onclick: createMacro }), h("span", { class: "spacer" }), sortSel, viewBox),
      h("div", { id: "macro-items" }),
    );
  }
  function renderList() {
    if (!listEl || ui.tab !== "macros") return;
    const holder = listEl.querySelector("#macro-items");
    if (!holder) return;
    if (!S.macros.length) {
      fill(holder,        h(
          "div",
          { class: "glass-base empty-state" },
          h("span", { class: "tile-icon" }, icon("macros", 22)),
          h("div", { class: "col", style: "align-items:center;gap:4px" }, h("span", { class: "title" }, "No macros yet"), h("span", { class: "body" }, `Record one with ${keyLabel(S.settings.record_hotkey)}, or build one step by step.`)),
          h("div", { class: "row" }, btn("Record", { icon: "record", onclick: () => send({ type: "toggleRecord" }) }), btn("New macro", { icon: "add", class: "primary", onclick: createMacro })),
        ),
      );
      return;
    }
    const order = S.macros.map((m, i) => ({ m, i }));
    order.sort((a, b) => (ui.sort === "steps" ? countSteps(b.m.steps) - countSteps(a.m.steps) : ui.sort === "added" ? a.i - b.i : a.m.name.localeCompare(b.m.name)));
    const asGrid = ui.view === "Grid" && !draft;
    fill(holder,      h(
        "div",
        { class: asGrid ? "grid" : "list-rows" },
        order.map(({ m }) => {
          const running = isRunning(m.id);
          const more = h("button", { type: "button", class: "icon-btn", title: "More", "aria-label": "More" }, icon("more"));
          const menu = (e) => {
            e.stopPropagation();
            e.preventDefault();
            popMenu(more, [
              { icon: running ? "stop" : "play", label: running ? "Stop" : "Run", action: () => runMacro(m) },
              { icon: "edit", label: "Edit", action: () => openEditor(m.id) },
              { icon: "copy", label: "Duplicate", action: () => duplicateMacro(m) },
              { icon: "download", label: "Save as AutoHotkey", action: () => exportMacro(m) },
              "-",
              { icon: "remove", label: ui.confirmDelete === m.id ? "Really delete" : "Delete", danger: true, action: () => deleteMacro(m) },
            ]);
          };
          more.addEventListener("click", menu);
          const row = h(
            "div",
            {
              class: "macro glass-base" + (asGrid ? " card" : "") + (ui.editing === m.id ? " on" : "") + (running ? " running" : ""),
              role: "button",
              tabindex: "0",
              onclick: () => (ui.editing === m.id ? closeEditor() : openEditor(m.id)),
              onkeydown: (e) => e.key === "Enter" && e.target === e.currentTarget && openEditor(m.id),
            },
            h("span", { class: "badge-icon" }, icon("macros", 16)),
            h("div", { class: "col grow", style: "gap:1px" }, h("span", { class: "item-title line" }, m.name), h("span", { class: "secondary line" }, `${plural(countSteps(m.steps), "step")} · ${repeatText(m)}`)),
            m.hotkey ? h("span", { class: "kbd" }, keyLabel(m.hotkey)) : null,
            h(
              "button",
              {
                type: "button",
                class: "btn small " + (running ? "danger" : "primary"),
                onclick: (e) => {
                  e.stopPropagation();
                  runMacro(m);
                },
              },
              icon(running ? "stop" : "play"),
              running ? "Stop" : "Run",
            ),
            more,
          );
          row.addEventListener("contextmenu", menu);
          return row;
        }),
      ),
    );
  }

  function closeEditor() {
    flush();
    ui.editing = null;
    draft = null;
    renderAll();
  }

  // Whether Macros' panel shows in Pious's in-game overlay (null until known,
  // or when this Pious can't show plugin panels).
  let overlayShown = null;
  async function loadOverlayShown() {
    try {
      overlayShown = await pious.ui.overlay.shown();
    } catch {
      overlayShown = null;
    }
    if (recSettingsEl && recSettingsEl.isConnected) renderRecSettings();
  }
  async function setOverlayShown(on) {
    overlayShown = on;
    renderRecSettings();
    try {
      await pious.ui.overlay.show(on);
    } catch (e) {
      overlayShown = !on;
      renderRecSettings();
      say(String(e?.message ?? e), true);
    }
  }

  function renderRecSettings() {
    fill(
      recSettingsEl,
      group(
        "Recording",
        settingRow("Record hotkey", "Starts and stops recording a new macro, anywhere.", hotkeyInput(S.settings.record_hotkey, (record_hotkey) => setSetting({ record_hotkey }), { clearable: true })),
        settingRow("Stop hotkey", "Stops every macro and the auto-clicker.", hotkeyInput(S.settings.stop_hotkey, (stop_hotkey) => setSetting({ stop_hotkey }), { clearable: true })),
        settingRow("Keep the timing", "Wait between actions as long as you did.", toggle(S.settings.record_timing, (record_timing) => setSetting({ record_timing }))),
        settingRow("Record pointer movement", "Every move, not just where clicks happen. Makes bigger macros.", toggle(S.settings.record_moves, (record_moves) => setSetting({ record_moves }))),
      ),
      overlayShown === null
        ? null
        : group(
            "In-game overlay",
            settingRow(
              "Show in the in-game overlay",
              "A Macros panel in Pious's in-game overlay: run or stop your macros and the auto-clicker without leaving the game.",
              toggle(overlayShown, setOverlayShown),
            ),
          ),
    );
  }
  let runBtn = null;
  function renderEditor() {
    if (!editorEl || !draft) return;
    const d = draft;

    const accountsHolder = h("div");
    const paintAccounts = () => {
      fill(accountsHolder, d.target === "Accounts" ? accountTargets(d.accounts || [], (accounts) => change({ accounts })) : null);
    };
    const targetHint = h("span", { class: "secondary" }, targetText(d.target || "Roblox"));
    const stepsHolder = h("div", { class: "col step-scroll", style: "gap:6px" });
    const rebuildSteps = () => fill(stepsHolder, stepList(d.steps, 0, rebuildSteps));
    rebuildSteps();
    runBtn = h("button", { type: "button", class: "btn small primary", onclick: () => runMacro(d) });
    const timesField = h("div");
    const paintTimes = () =>
      fill(timesField, d.repeat === "Times" ? field("Times", numInput(d.times, (times) => change({ times }), { min: 1, cls: "" })) : null);
    paintTimes();
    paintAccounts();

    fill(editorEl,      h(
        "div",
        { class: "row", style: "flex-wrap:nowrap" },
        textInput(d.name, (name) => change({ name: name.trim() || "Macro" }), "Name", "title grow"),
        iconBtn("close", "Close", closeEditor),
      ),
      h(
        "div",
        { class: "options" },
        field("Hotkey", hotkeyInput(d.hotkey, (hotkey) => change({ hotkey }), { clearable: true, width: "100%" })),
        field(
          "Repeat",
          select(
            [
              { value: "Once", label: "Once" },
              { value: "Times", label: "A number of times" },
              { value: "UntilStopped", label: "Until stopped" },
              { value: "WhileHeld", label: "While the hotkey is held" },
            ],
            d.repeat,
            (repeat) => {
              change({ repeat });
              paintTimes();
            },
          ),
        ),
        timesField,
        field("Speed", slider(d.speed, { min: 0.25, max: 4, step: 0.05 }, (speed) => change({ speed }))),
      ),
      h("div", { class: "row" }, h("span", { class: "meta grow" }, "Hotkey only while playing Roblox"), toggle(d.game_only, (game_only) => change({ game_only }))),
      h(
        "div",
        { class: "field" },
        h("span", { class: "label" }, "Send keys and clicks to"),
        select(TARGETS, d.target || "Roblox", (target) => {
          change({ target });
          targetHint.textContent = targetText(target);
          paintAccounts();
        }),
        targetHint,
      ),
      accountsHolder,
      btn("Save as AutoHotkey", { class: "small tertiary", icon: "download", onclick: () => exportMacro(d) }),
      h("hr", { class: "divider" }),
      h("div", { class: "row" }, h("span", { class: "section-title grow" }, "Steps"), runBtn),
      stepsHolder,
    );
    updateLive();
  }

  function accountTargets(value, onchange) {
    const list = ui.accounts;
    if (!list.length) return h("div", { class: "targets" }, h("span", { class: "meta" }, "Add an account first."));
    return h(
      "div",
      { class: "targets" },
      list.map((a) => {
        const chip = h("button", { type: "button", class: "chip" + (value.includes(a.id) ? " on" : "") }, accountName(a));
        chip.addEventListener("click", () => {
          const next = value.filter((x) => x !== a.id);
          if (!value.includes(a.id)) next.push(a.id);
          value = next;
          chip.className = "chip" + (next.includes(a.id) ? " on" : "");
          onchange(next);
        });
        return chip;
      }),
    );
  }
  /** Bits that change with the running state without rebuilding the editor. */
  function updateLive() {
    if (runBtn && draft && runBtn.isConnected) {
      const on = isRunning(draft.id);
      runBtn.className = "btn small " + (on ? "danger" : "primary");
      fill(runBtn, icon(on ? "stop" : "play"), on ? "Stop" : "Try it");
    }
  }

  // ── Auto-clicker tab ─────────────────────────────────────────────────

  let clickerEl = null;
  let clickerSig = "";
  const MODE_HELP = {
    Toggle: "Press the hotkey to start clicking, and again to stop.",
    Hold: "Clicks only while you hold the hotkey down.",
    MouseHeld: "Press the hotkey to turn it on. Then it clicks rapidly whenever you hold your mouse button down, and stops when you let go.",
    OnClick: "Press the hotkey to turn it on. Then every click you make is followed by extra clicks, like a burst.",
  };
  const BUTTONS_CLICK = [
    { value: "Left", label: "Left button" },
    { value: "Right", label: "Right button" },
    { value: "Middle", label: "Middle button" },
  ];

  function renderClickerTab() {
    const c = S.autoclicker;
    const sig = JSON.stringify([c, S.clicking, ui.accounts.length]);
    if (bodyKey !== "clicker" || !clickerEl || !clickerEl.isConnected) {
      bodyKey = "clicker";
      clickerEl = h("div", { class: "col", style: "gap:18px" });
      fill(regions.body, clickerEl);
      clickerSig = "";
    }
    if (sig === clickerSig) return;
    if (hasFocus(clickerEl) && clickerSig) {
      // Don't pull the field out from under the typist; just update the live button.
      paintGo();
      return;
    }
    clickerSig = sig;
    const modeText =
      c.mode === "Hold"
        ? `Hold ${keyLabel(c.hotkey)} to click`
        : c.mode === "MouseHeld"
          ? `Press ${keyLabel(c.hotkey)}, then hold the mouse to click`
          : c.mode === "OnClick"
            ? `Press ${keyLabel(c.hotkey)}, then each click gets ${c.burst ?? 2} more`
            : `Press ${keyLabel(c.hotkey)} to click`;
    const cps = Math.round((1000 / Math.max(1, c.interval_ms + c.random_ms / 2)) * 10) / 10;
    const go = h("button", { type: "button", class: "go", id: "go", onclick: () => send({ type: "toggleClicker" }) });

    fill(
      clickerEl,
      tip("autoclicker", `Turn it on, then press ${keyLabel(c.hotkey)} in any app or game to start and stop clicking.`),
      h(
        "div",
        { class: "clicker" },
        h(
          "section",
          { class: "panel col hero-panel" },
          h("div", { class: "row hero" }, h("div", { class: "col grow", style: "gap:2px" }, h("span", { class: "section-title" }, "Auto-clicker"), h("span", { class: "meta" }, c.enabled ? modeText : "Off: its hotkey does nothing")), toggle(c.enabled, (enabled) => setClicker({ enabled }))),
          go,
          h("span", { class: "secondary center" }, `About ${cps} clicks a second`),
        ),
        group(
          "What it does",
          settingRow("Hotkey", "Starts and stops it.", hotkeyInput(c.hotkey, (hotkey) => setClicker({ hotkey }))),
          settingRow(
            "Mode",
            MODE_HELP[c.mode],
            select(
              [
                { value: "Toggle", label: "Press to start and stop" },
                { value: "Hold", label: "Click while the hotkey is held" },
                { value: "MouseHeld", label: "Click while I hold the mouse" },
                { value: "OnClick", label: "Add clicks to each of mine" },
              ],
              c.mode,
              (mode) => setClicker({ mode }),
              "250px",
            ),
          ),
          c.mode === "MouseHeld" || c.mode === "OnClick"
            ? settingRow(
                "Your mouse button",
                c.mode === "MouseHeld" ? "The button you hold down." : "The button whose clicks get extra ones.",
                select(
                  [...BUTTONS_CLICK, { value: "Back", label: "Back side button" }, { value: "Forward", label: "Forward side button" }],
                  c.trigger ?? "Left",
                  (trigger) => setClicker({ trigger }),
                  "190px",
                ),
              )
            : null,
          c.mode === "OnClick"
            ? settingRow("Extra clicks", "How many more clicks each of yours gets.", unit(numInput(c.burst ?? 2, (burst) => setClicker({ burst }), { min: 1, max: 50 }), "more"))
            : null,
          settingRow(
            "Click with",
            "A mouse button, or press a key over and over.",
            h(
              "div",
              { class: "row" },
              select(
                [
                  { value: "Mouse", label: "Mouse" },
                  { value: "Key", label: "A key" },
                ],
                c.input,
                (input) => setClicker({ input }),
                "110px",
              ),
              c.input === "Mouse" ? select(BUTTONS_CLICK, c.button, (button) => setClicker({ button }), "150px") : keyPicker(c.key, (key) => setClicker({ key })),
            ),
          ),
          settingRow("Double click", "Two clicks each time.", toggle(c.double, (double) => setClicker({ double }))),
        ),
        group(
          "Timing",
          settingRow("Every", "Time between clicks.", unit(numInput(c.interval_ms, (v) => setClicker({ interval_ms: v }), { min: 1 }), "ms")),
          settingRow("Random extra", "Up to this much longer each time, so it looks less robotic.", unit(numInput(c.random_ms, (v) => setClicker({ random_ms: v }), { min: 0 }), "ms")),
          settingRow("Hold each click", "How long the button stays down.", unit(numInput(c.hold_ms, (v) => setClicker({ hold_ms: v }), { min: 0 }), "ms")),
          settingRow(
            "Stop after",
            "Or keep going until you stop it.",
            h(
              "div",
              { class: "row" },
              select(
                [
                  { value: "Unlimited", label: "Never" },
                  { value: "Clicks", label: "A number of clicks" },
                  { value: "Seconds", label: "A number of seconds" },
                ],
                c.limit,
                (limit) => setClicker({ limit }),
                "190px",
              ),
              c.limit !== "Unlimited" ? numInput(c.limit_value, (v) => setClicker({ limit_value: v }), { min: 1 }) : null,
            ),
          ),
        ),
        group(
          "Where",
          settingRow("Send clicks to", targetText(c.target || "Roblox"), select(TARGETS, c.target || "Roblox", (target) => setClicker({ target }), "240px")),
          c.target === "Accounts" ? h("div", { style: "padding:10px 0" }, accountTargets(c.accounts || [], (accounts) => setClicker({ accounts }))) : null,
          settingRow(
            "Click at",
            c.fixed ? `Always at ${c.fixed[0]}, ${c.fixed[1]}.` : "Wherever the pointer is.",
            h(
              "div",
              { class: "row" },
              c.fixed ? btn("Use the pointer", { class: "small tertiary", onclick: () => setClicker({ fixed: null }) }) : null,
              pointPicker(c.fixed ? "Pick again" : "Pick a spot", (x, y) => setClicker({ fixed: [x, y] })),
            ),
          ),
          settingRow("Only while playing", "The hotkey works only while a Roblox window is in front.", toggle(c.game_only, (game_only) => setClicker({ game_only }))),
        ),
      ),
    );
    paintGo();
  }

  function paintGo() {
    const go = document.getElementById("go");
    if (!go) return;
    go.className = "go" + (S.clicking ? " on" : "");
    fill(go, h("span", { class: "ring" }), icon(S.clicking ? "stop" : "autoclick", 20), S.clicking ? "Stop clicking" : "Start now");
  }

  // ── Start ────────────────────────────────────────────────────────────

  // Fetch Pious's icons once, before the first render; fall back to none.
  Promise.resolve()
    .then(() => pious.ui.icons(ICON_NAMES))
    .then((got) => {
      if (got && typeof got === "object") ICONS = got;
    })
    .catch(() => {})
    .then(() => {
      iconsReady = true;
      renderAll();
    });

  pious.on("message", onMessage);
  try {
    pious.on("snapshot", (snap) => {
      if (snap && Array.isArray(snap.accounts)) {
        const before = ui.accounts.length;
        ui.accounts = snap.accounts;
        if (before !== ui.accounts.length && S.loaded && !saveTimer) {
          bodyKey = "";
          renderAll();
        }
      }
    });
  } catch {
    /* no read permission: accounts stay empty */
  }
  call("snapshot")
    .then((snap) => {
      if (snap && Array.isArray(snap.accounts)) {
        ui.accounts = snap.accounts;
        if (S.loaded && !saveTimer) {
          bodyKey = "";
          renderAll();
        }
      }
    })
    .catch(() => {});

  loadOverlayShown();

  // Ask until the engine answers (it may still be starting).
  let asks = 0;
  const ask = () => {
    if (S.loaded) return;
    send({ type: "get" });
    if (++asks < 30) setTimeout(ask, 1000);
    else {
      const boot = document.getElementById("boot");
      if (boot) boot.textContent = "Macros isn't running. Turn the plugin off and on again in Settings → Plugins.";
    }
  };
  ask();
})();
