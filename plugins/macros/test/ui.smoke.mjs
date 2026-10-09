// Run with: node plugins/macros/test/ui.smoke.mjs
// Loads ui.js against a tiny fake DOM and a fake `pious`, feeds it engine
// state and checks that every screen renders without throwing.

import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

class Node_ {
  constructor() {
    this.childNodes = [];
    this.parentNode = null;
  }
  get isConnected() {
    let n = this;
    while (n) {
      if (n === doc.body) return true;
      n = n.parentNode;
    }
    return false;
  }
  append(...kids) {
    for (const k of kids) {
      const node = typeof k === "string" ? new Text_(k) : k;
      if (node.parentNode) node.parentNode.childNodes = node.parentNode.childNodes.filter((c) => c !== node);
      node.parentNode = this;
      this.childNodes.push(node);
    }
  }
  replaceChildren(...kids) {
    for (const c of this.childNodes) c.parentNode = null;
    this.childNodes = [];
    this.append(...kids.filter((k) => k !== null && k !== undefined));
  }
  remove() {
    if (this.parentNode) this.parentNode.childNodes = this.parentNode.childNodes.filter((c) => c !== this);
    this.parentNode = null;
  }
  contains(n) {
    for (; n; n = n.parentNode) if (n === this) return true;
    return false;
  }
  get textContent() {
    return this.childNodes.map((c) => c.textContent).join("");
  }
  set textContent(v) {
    this.replaceChildren(String(v));
  }
}
class Text_ extends Node_ {
  constructor(t) {
    super();
    this.data = t;
  }
  get textContent() {
    return this.data;
  }
  set textContent(v) {
    this.data = String(v);
  }
}
class El extends Node_ {
  constructor(tag) {
    super();
    this.tagName = tag.toUpperCase();
    this.attrs = {};
    this.listeners = {};
    this.style = { cssText: "" };
    this.className = "";
    this.classList = {
      add: (c) => (this.className = [...new Set((this.className + " " + c).trim().split(/\s+/))].join(" ")),
      remove: (c) => (this.className = this.className.split(/\s+/).filter((x) => x !== c).join(" ")),
    };
    this.value = "";
  }
  setAttribute(k, v) {
    this.attrs[k] = v;
  }
  addEventListener(t, f) {
    (this.listeners[t] ||= []).push(f);
  }
  removeEventListener() {}
  dispatch(t, e = {}) {
    for (const f of this.listeners[t] || []) f({ stopPropagation() {}, preventDefault() {}, target: this, currentTarget: this, ...e });
  }
  getBoundingClientRect() {
    return { left: 0, right: 100, top: 0, bottom: 20 };
  }
  get offsetWidth() {
    return 100;
  }
  get offsetHeight() {
    return 100;
  }
  select() {}
  find(pred, out = []) {
    for (const c of this.childNodes) {
      if (c instanceof El) {
        if (pred(c)) out.push(c);
        c.find(pred, out);
      }
    }
    return out;
  }
  querySelector(sel) {
    const id = sel.replace(/^#/, "");
    return this.find((e) => e.attrs.id === id || e.id === id)[0] || null;
  }
}
const doc = {
  body: new El("body"),
  activeElement: null,
  createElement: (t) => new El(t),
  createElementNS: (_, t) => new El(t),
  createTextNode: (t) => new Text_(String(t)),
  getElementById: (id) => doc.body.find((e) => e.attrs.id === id)[0] || null,
  addEventListener() {},
};
const app = new El("div");
app.attrs.id = "app";
doc.body.append(app);
const boot = new El("p");
boot.attrs.id = "boot";
app.append(boot);

const sent = [];
const listeners = {};
const pious = {
  ui: { icons: async (names) => Object.fromEntries(names.map((n) => [n, `<svg data-icon="${n}"></svg>`])) },
  async call(m, ...a) {
    if (m === "send") sent.push(a[0]);
    if (m === "snapshot") return { accounts: [{ id: "acc1", username: "bob", display_name: "Bob" }] };
    if (m === "input.cursor") return { x: 3, y: 4 };
    if (m === "input.pixel") return "#112233";
    return null;
  },
  on(ev, f) {
    (listeners[ev] ||= []).push(f);
    return () => {};
  },
};
Object.assign(globalThis, {
  document: doc,
  Node: Node_,
  pious,
  window: { innerWidth: 1000, innerHeight: 800, addEventListener() {}, removeEventListener() {} },
});

const fire = (msg) => listeners.message.forEach((f) => f(msg));
const wait = () => new Promise((r) => setTimeout(r, 5));

const src = readFileSync(new URL("../ui.js", import.meta.url), "utf8");
new Function(src)();
await wait();
assert.ok(sent.some((m) => m.type === "get"), "asks for state");

const steps = [
  { kind: "Key", key: "KeyW", press: "Tap", hold_ms: 50 },
  { kind: "Text", text: "hi", delay_ms: 20 },
  { kind: "Click", button: "Left", press: "Tap", count: 1 },
  { kind: "Move", x: 1, y: 2, relative: false, duration_ms: 0 },
  { kind: "Scroll", amount: -3, horizontal: false },
  { kind: "Wait", ms: 500, random_ms: 0 },
  { kind: "Loop", times: 2, steps: [{ kind: "Wait", ms: 5, random_ms: 0 }] },
  { kind: "WaitPixel", x: 0, y: 0, color: "#FFFFFF", tolerance: 10, timeout_ms: 0 },
  { kind: "FocusRoblox" },
  { kind: "Run", target: "https://example.com" },
  { kind: "Comment", text: "note" },
];
const macro = { id: "m1", name: "One", hotkey: "F3", repeat: "Times", times: 3, speed: 1, game_only: false, target: "Accounts", accounts: ["acc1"], steps };
const state = {
  type: "state",
  macros: [macro],
  settings: { record_hotkey: "F7", stop_hotkey: "Shift+Escape", record_moves: false, record_timing: true },
  autoclicker: { enabled: true, hotkey: "F6", mode: "OnClick", input: "Mouse", button: "Left", key: "KeyE", double: false, interval_ms: 100, random_ms: 0, hold_ms: 10, fixed: [5, 6], limit: "Clicks", limit_value: 10, game_only: false, target: "Accounts", accounts: [], trigger: "Left", burst: 2 },
  running: [],
  clicking: false,
  recording: false,
};
fire(state);
assert.ok(app.textContent.includes("One"), "list shows the macro");
assert.ok(app.textContent.includes("Runs 3 times"));

// Open the editor by clicking the row.
const row = app.find((e) => e.className.split(" ").includes("macro"))[0];
row.dispatch("click");
await wait();
assert.ok(app.textContent.includes("Steps"), "editor open");
assert.ok(app.textContent.includes("Add a step"));
assert.ok(app.textContent.includes("Bring Roblox forward") || app.textContent.includes("Brings a Roblox window"));
assert.ok(app.textContent.includes("Bob"), "accounts listed");

// Editing a field saves (debounced) via saveMacro.
const name = app.find((e) => e.tagName === "INPUT" && e.className.includes("title"))[0];
name.value = "Renamed";
name.dispatch("change");
await new Promise((r) => setTimeout(r, 450));
const saved = sent.filter((m) => m.type === "saveMacro").pop();
assert.equal(saved.macro.name, "Renamed");

// Running state updates without throwing.
fire({ ...state, macros: [{ ...macro, name: "Renamed" }], running: ["m1"], recording: true });
assert.ok(app.textContent.includes("Stop recording"));
assert.ok(app.textContent.includes("Stop all"));

// Clicker tab.
const tab = app.find((e) => e.className.includes("chip") && e.textContent.includes("Auto-clicker"))[0];
tab.dispatch("click");
assert.ok(app.textContent.includes("Extra clicks"), "OnClick settings");
assert.ok(app.textContent.includes("Always at 5, 6"));
fire({ ...state, clicking: true });
assert.ok(app.textContent.includes("Stop clicking"));

// Export and import panels.
app.find((e) => e.tagName === "BUTTON" && e.textContent.includes("Import AutoHotkey"))[0].dispatch("click");
assert.ok(app.textContent.includes("Import an AutoHotkey script"));
fire({ type: "ahk", id: "m1", name: "Renamed", text: "F3:: {\n}\n" });
assert.ok(app.textContent.includes("as an AutoHotkey v2 script"));
fire({ type: "imported", error: "nope" });
assert.ok(app.textContent.includes("nope"));

console.log("ui smoke test: ok");
process.exit(0);
