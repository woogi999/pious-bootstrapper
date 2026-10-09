// Run with: node plugins/macros/test/engine.test.mjs
// Drives engine.js with a fake `pious` that records every call and keeps a
// virtual clock (sleep() advances it), so nothing here really waits.

import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

// engine.js is a classic script (it runs in a browser page), but the repo's
// package.json makes Node treat .js as ES modules, so evaluate it directly.
const source = readFileSync(new URL("../engine.js", import.meta.url), "utf8");
const shim = { exports: {} };
new Function("module", source)(shim);
const E = shim.exports;

let passed = 0;
const tests = [];
const test = (name, fn) => tests.push([name, fn]);

function fakeHost({ windows = [], storage = {} } = {}) {
  const log = [];
  let clock = 0;
  const handlers = {};
  const listeners = {};
  const files = { ...storage };
  const host = {
    log,
    handlers,
    files,
    get clock() {
      return clock;
    },
    now: () => clock,
    emit: (event, value) => (listeners[event] || []).forEach((f) => f(value)),
    windows,
    foreground: { window: 1, pid: 1, roblox: true },
    async call(method, ...args) {
      log.push([method, ...args]);
      switch (method) {
        case "sleep":
          clock += args[0];
          // Let other tasks (the test, events) run, like a real sleep would.
          await new Promise((r) => setImmediate(r));
          return null;
        case "storage.read":
          return files[args[0]] ?? null;
        case "storage.write":
          files[args[0]] = args[1];
          return null;
        case "windows.roblox":
          return host.windows;
        case "windows.foreground":
          return host.foreground;
        case "input.cursor":
          return { x: 10, y: 20 };
        case "input.windowPoint":
          return { x: 500, y: 400 };
        case "input.pixel":
          return host.pixel ? host.pixel(...args) : "#000000";
        case "input.recordStop":
          return host.recorded || [];
        case "hotkeys.set":
          return [];
        default:
          return null;
      }
    },
    on(event, fn) {
      (listeners[event] ||= []).push(fn);
      return () => {};
    },
    handle(name, fn) {
      handlers[name] = fn;
    },
    calls: (method) => log.filter((c) => c[0] === method),
    sleeps: () => log.filter((c) => c[0] === "sleep").reduce((n, c) => n + c[1], 0),
  };
  return host;
}

const engineFor = (host, extra = {}) => E.createEngine(host, { now: host.now, random: () => 0.5, ...extra });
const macro = (over) => E.defaultMacro({ name: "Test", target: "System", ...over });

// // AutoHotkey ----

test("AHK: reads a v1 script", () => {
  const script = "; farm\nF1::\nLoop, 3 {\n  Send, {e down}\n  Sleep, 120\n  Send {e up}\n  Click, 100, 200\n}\nSend ^c\nreturn";
  const out = E.ahkImport(script);
  assert.equal(out.hotkey, "F1");
  assert.equal(out.skipped, 0);
  assert.equal(out.steps[0].kind, "Loop");
  assert.equal(out.steps[0].times, 3);
  const s = out.steps[0].steps;
  assert.deepEqual(s[0], { kind: "Key", key: "KeyE", press: "Down", hold_ms: 0 });
  assert.deepEqual(s[1], { kind: "Wait", ms: 120, random_ms: 0 });
  assert.deepEqual(s[3], { kind: "Move", x: 100, y: 200, relative: false, duration_ms: 0 });
  assert.deepEqual(s[4], { kind: "Click", button: "Left", press: "Tap", count: 1 });
  assert.deepEqual(out.steps[1], { kind: "Key", key: "ControlLeft", press: "Down", hold_ms: 0 });
  assert.deepEqual(out.steps[2], { kind: "Key", key: "KeyC", press: "Tap", hold_ms: 30 });
  assert.deepEqual(out.steps[3], { kind: "Key", key: "ControlLeft", press: "Up", hold_ms: 0 });
});

test("AHK: reads v2 and keeps unknown lines as notes", () => {
  const out = E.ahkImport('^!a:: {\n    Send "hi{Enter}"\n    Sleep(50)\n    MsgBox "done"\n}');
  assert.equal(out.hotkey, "Control+Alt+KeyA");
  assert.deepEqual(out.steps[0], { kind: "Text", text: "hi", delay_ms: 0 });
  assert.deepEqual(out.steps[1], { kind: "Key", key: "Enter", press: "Tap", hold_ms: 30 });
  assert.deepEqual(out.steps[2], { kind: "Wait", ms: 50, random_ms: 0 });
  assert.equal(out.steps[3].kind, "Comment");
  assert.equal(out.skipped, 1);
});

test("AHK: loops without braces cover one line", () => {
  let out = E.ahkImport("Loop 5\n    Click\nSend a");
  assert.equal(out.steps.length, 2);
  assert.equal(out.steps[0].kind, "Loop");
  assert.equal(out.steps[0].times, 5);
  assert.equal(out.steps[0].steps.length, 1);
  out = E.ahkImport("Loop, 2\n{\nClick\nClick\n}");
  assert.equal(out.steps[0].times, 2);
  assert.equal(out.steps[0].steps.length, 2);
});

test("AHK: export then import round-trips", () => {
  const m = macro({
    hotkey: "Shift+F2",
    steps: [
      { kind: "Key", key: "KeyW", press: "Down", hold_ms: 0 },
      { kind: "Wait", ms: 500, random_ms: 0 },
      { kind: "Key", key: "KeyW", press: "Up", hold_ms: 0 },
      { kind: "Click", button: "Right", press: "Tap", count: 2 },
    ],
  });
  const script = E.ahkExport(m);
  assert.match(script, /#Requires AutoHotkey v2\.0/);
  assert.match(script, /^\+F2:: \{$/m);
  const back = E.ahkImport(script);
  assert.equal(back.hotkey, "Shift+F2");
  assert.deepEqual(back.steps, m.steps);
});

test("AHK: export of repeat modes, loops and other steps re-imports", () => {
  const m = macro({
    repeat: "Times",
    times: 4,
    steps: [
      { kind: "Loop", times: 2, steps: [{ kind: "Key", key: "KeyE", press: "Tap", hold_ms: 80 }] },
      { kind: "Move", x: 5, y: 6, relative: false, duration_ms: 0 },
    ],
  });
  const script = E.ahkExport(m);
  assert.match(script, /Loop 4 \{/);
  const back = E.ahkImport(script);
  assert.equal(back.hotkey, "F8");
  assert.equal(back.steps[0].times, 4);
  const inner = back.steps[0].steps;
  assert.equal(inner[0].kind, "Loop");
  assert.deepEqual(inner[0].steps[0], { kind: "Key", key: "KeyE", press: "Down", hold_ms: 0 });
  assert.deepEqual(inner[1], { kind: "Move", x: 5, y: 6, relative: false, duration_ms: 0 });
  assert.match(E.ahkExport(macro({ steps: [{ kind: "Text", text: 'say "hi"\nok', delay_ms: 0 }] })), /SendText "say `"hi`"`nok"/);
});

// // Recording ----

test("recorded presses become taps", () => {
  const events = [
    { ms: 0, kind: "key", code: "KeyW", down: true },
    { ms: 120, kind: "key", code: "KeyW", down: true },
    { ms: 200, kind: "key", code: "KeyW", down: false },
    { ms: 700, kind: "button", button: "Left", down: true, x: 5, y: 6 },
    { ms: 760, kind: "button", button: "Left", down: false, x: 5, y: 6 },
  ];
  assert.deepEqual(E.eventsToSteps(events, [], false, true), [
    { kind: "Key", key: "KeyW", press: "Tap", hold_ms: 200 },
    { kind: "Wait", ms: 500, random_ms: 0 },
    { kind: "Move", x: 5, y: 6, relative: false, duration_ms: 0 },
    { kind: "Click", button: "Left", press: "Tap", count: 1 },
  ]);
});

test("recording: ignores the record hotkey, honors timing and moves, uses move events for click spots", () => {
  const events = [
    { ms: 0, kind: "key", code: "ControlLeft", down: true },
    { ms: 5, kind: "key", code: "KeyM", down: true },
    { ms: 10, kind: "move", x: 40, y: 50 },
    { ms: 15, kind: "move", x: 41, y: 51 },
    { ms: 300, kind: "button", button: "Right", down: true },
    { ms: 900, kind: "button", button: "Right", down: false },
    { ms: 1000, kind: "scroll", amount: -2, horizontal: false },
  ];
  const ignore = E.hotkeyCodes("Control+KeyM");
  const noTiming = E.eventsToSteps(events, ignore, false, false);
  assert.deepEqual(noTiming, [
    { kind: "Move", x: 41, y: 51, relative: false, duration_ms: 0 },
    { kind: "Click", button: "Right", press: "Tap", count: 1 },
    { kind: "Scroll", amount: -2, horizontal: false },
  ]);
  const withMoves = E.eventsToSteps(events, ignore, true, true);
  // Both close moves merge into one; no Move is added before the click.
  assert.equal(withMoves[0].kind, "Move");
  assert.equal(withMoves[0].x, 41);
  assert.equal(withMoves.filter((s) => s.kind === "Move").length, 1);
  assert.ok(withMoves.some((s) => s.kind === "Wait" && s.ms === 290));
});

// // Defaults and storage shape ----

test("defaults equal the Rust Default impls", () => {
  assert.deepEqual(E.defaultSettings(), { record_hotkey: "F7", stop_hotkey: "Shift+Escape", record_moves: false, record_timing: true });
  const a = E.defaultAutoclicker();
  assert.equal(a.hotkey, "F6");
  assert.equal(a.interval_ms, 100);
  assert.equal(a.hold_ms, 10);
  assert.equal(a.fixed, null);
  assert.equal(a.burst, 2);
  assert.equal(a.limit_value, 100);
  const d = E.normalizeData(null);
  assert.deepEqual(d.macros, []);
  assert.deepEqual(d.settings, E.defaultSettings());
  const m = E.normalizeData({ macros: [{ id: "x", name: "A", steps: [{ kind: "Wait", ms: 5 }] }] }).macros[0];
  assert.equal(m.times, 3);
  assert.equal(m.speed, 1);
  assert.equal(m.target, "Roblox");
  assert.equal(m.repeat, "Once");
});

// // Target resolution ----

test("targets: System, Roblox (last), AllRoblox, Accounts", async () => {
  const wins = [
    { window: 11, pid: 1, account: { id: "a1", username: "a" }, last: false },
    { window: 22, pid: 2, account: { id: "a2", username: "b" }, last: true },
    { window: 33, pid: 3, account: null, last: false },
  ];
  const host = fakeHost({ windows: wins });
  assert.deepEqual(await E.resolveTargets(host, "System", []), [null]);
  assert.deepEqual(await E.resolveTargets(host, "Roblox", []), [22]);
  assert.deepEqual(await E.resolveTargets(host, "AllRoblox", []), [11, 22, 33]);
  assert.deepEqual(await E.resolveTargets(host, "Accounts", ["a1"]), [11]);
  await assert.rejects(E.resolveTargets(host, "Accounts", []), /Pick which accounts/);
  await assert.rejects(E.resolveTargets(host, "Accounts", ["zz"]), /None of the picked/);
  host.windows = [{ window: 5, last: false }];
  assert.deepEqual(await E.resolveTargets(host, "Roblox", []), [5]);
  host.windows = [];
  await assert.rejects(E.resolveTargets(host, "Roblox", []), /No Roblox window/);
});

// // Playback ----

test("playback: key order, Loop expansion and sleeps at speed 2", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  const m = macro({
    speed: 2,
    steps: [
      { kind: "Key", key: "KeyW", press: "Down", hold_ms: 0 },
      { kind: "Wait", ms: 400, random_ms: 0 },
      { kind: "Key", key: "KeyW", press: "Up", hold_ms: 0 },
      { kind: "Loop", times: 3, steps: [{ kind: "Key", key: "KeyE", press: "Tap", hold_ms: 100 }, { kind: "Wait", ms: 60, random_ms: 0 }] },
    ],
  });
  eng.data = { macros: [m] };
  const started = await eng.toggleMacro(eng.data.macros[0]);
  assert.equal(started.started, true);
  await started.done;
  const keys = host.log.filter((c) => c[0] === "input.key").map((c) => `${c[1]}:${c[2] ? "down" : "up"}:${c[3]}`);
  assert.deepEqual(keys, [
    "KeyW:down:null",
    "KeyW:up:null",
    ...Array(3).fill(["KeyE:down:null", "KeyE:up:null"]).flat(),
  ]);
  // The wait of 400 and 3 x (tap 100 + wait 60), all at 2x speed.
  assert.equal(host.sleeps(), (400 + 3 * (100 + 60)) / 2);
  // Down comes before the sleep, up after it.
  const order = host.log.map((c) => c[0]).filter((c) => c === "sleep" || c === "input.key");
  assert.equal(order[0], "input.key");
  assert.equal(order[1], "sleep");
  assert.deepEqual(eng.statusInfo(), { running: [], clicking: false, recording: false });
});

test("playback: Times repeats, tap minimum hold, sleeps in chunks of at most 50 ms", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { macros: [macro({ repeat: "Times", times: 2, steps: [{ kind: "Wait", ms: 130, random_ms: 0 }, { kind: "Key", key: "Space", press: "Tap", hold_ms: 0 }] })] };
  await (await eng.toggleMacro(eng.data.macros[0])).done;
  assert.equal(host.sleeps(), 2 * (130 + 15));
  assert.ok(host.calls("sleep").every((c) => c[1] <= 50));
  assert.equal(host.calls("input.key").length, 4);
});

test("playback: random wait, text, click, scroll, move go to the right target", async () => {
  const host = fakeHost({ windows: [{ window: 7, last: true, account: null }, { window: 8, last: false, account: null }] });
  const eng = engineFor(host);
  eng.data = {
    macros: [
      macro({
        target: "AllRoblox",
        steps: [
          { kind: "Wait", ms: 100, random_ms: 100 }, // random() = 0.5 -> 150
          { kind: "Text", text: "a\u{1F600}", delay_ms: 10 },
          { kind: "Move", x: 30, y: 40, relative: false, duration_ms: 0 },
          { kind: "Click", button: "Left", press: "Tap", count: 1 },
          { kind: "Scroll", amount: 2, horizontal: false },
        ],
      }),
    ],
  };
  await (await eng.toggleMacro(eng.data.macros[0])).done;
  assert.deepEqual(host.calls("input.text").map((c) => [c[1], c[2]]), [["a", 7], ["a", 8], ["\u{1F600}", 7], ["\u{1F600}", 8]]);
  assert.deepEqual(host.calls("input.move").map((c) => c.slice(1)), [[30, 40, false, 7], [30, 40, false, 8]]);
  const clicks = host.calls("input.button").map((c) => c.slice(1));
  assert.deepEqual(clicks[0], ["Left", true, 7, { x: 30, y: 40 }]);
  assert.equal(clicks.length, 4);
  assert.deepEqual(host.calls("input.scroll").map((c) => c[3]), [7, 8]);
  assert.equal(host.sleeps(), 150 + 2 * 10 + 12);
});

test("playback: held keys and buttons are released when a macro stops", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = {
    macros: [
      macro({
        repeat: "UntilStopped",
        steps: [
          { kind: "Key", key: "KeyW", press: "Down", hold_ms: 0 },
          { kind: "Click", button: "Right", press: "Down", count: 1 },
          { kind: "Wait", ms: 100000, random_ms: 0 },
        ],
      }),
    ],
  };
  const r = await eng.toggleMacro(eng.data.macros[0]);
  for (let i = 0; i < 50 && !host.calls("sleep").length; i++) await new Promise((x) => setImmediate(x));
  // Running again stops it.
  const second = await eng.toggleMacro(eng.data.macros[0]);
  assert.equal(second.started, false);
  await r.done;
  const k = host.calls("input.key");
  assert.deepEqual(k.map((c) => c[2]), [true, false]);
  const b = host.calls("input.button");
  assert.deepEqual(b.map((c) => c[2]), [true, false]);
  assert.ok(host.clock < 200, `stopped promptly (clock ${host.clock})`);
});

test("playback: stop flag halts a long wait within 50 ms of virtual time", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { macros: [macro({ steps: [{ kind: "Wait", ms: 5000, random_ms: 0 }, { kind: "Key", key: "KeyX", press: "Tap", hold_ms: 10 }] })] };
  // Stop right after the third chunk of sleeping.
  const real = host.call.bind(host);
  let n = 0;
  host.call = async (m, ...a) => {
    const v = await real(m, ...a);
    if (m === "sleep" && ++n === 3) await eng.stopAll();
    return v;
  };
  const r = await eng.toggleMacro(eng.data.macros[0]);
  await r.done;
  assert.ok(host.clock <= 150 + 50, `clock ${host.clock}`);
  assert.equal(host.calls("input.key").length, 0);
});

test("playback: WhileHeld stops when the hotkey is released; hotkey toggles; empty macro errors", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  const m = macro({ repeat: "WhileHeld", hotkey: "KeyZ", steps: [{ kind: "Key", key: "KeyQ", press: "Tap", hold_ms: 20 }, { kind: "Wait", ms: 20, random_ms: 0 }] });
  const empty = macro({ name: "Empty" });
  eng.data = { macros: [m, empty] };
  const id = eng.data.macros[0].id;
  await eng.onHotkey({ id: "m:" + id, pressed: true });
  assert.deepEqual(eng.statusInfo().running, [id]);
  // Let it loop a few rounds, then let go of the key.
  const real = host.call.bind(host);
  let sleeps = 0;
  host.call = async (mth, ...a) => {
    const v = await real(mth, ...a);
    if (mth === "sleep" && ++sleeps === 10) await eng.onHotkey({ id: "m:" + id, pressed: false });
    return v;
  };
  for (let i = 0; i < 200 && eng.statusInfo().running.length; i++) await new Promise((r) => setImmediate(r));
  assert.deepEqual(eng.statusInfo().running, []);
  assert.ok(host.calls("input.key").length >= 4);
  await assert.rejects(eng.toggleMacro(eng.data.macros[1]), /no steps/);
});

test("playback: WaitPixel waits for the color, with a timeout", async () => {
  const host = fakeHost();
  let reads = 0;
  host.pixel = () => (++reads >= 3 ? "#102030" : "#000000");
  const eng = engineFor(host);
  eng.data = {
    macros: [
      macro({
        steps: [
          { kind: "WaitPixel", x: 1, y: 2, color: "#112233", tolerance: 3, timeout_ms: 0 },
          { kind: "WaitPixel", x: 1, y: 2, color: "#FFFFFF", tolerance: 0, timeout_ms: 100 },
          { kind: "Comment", text: "x" },
        ],
      }),
    ],
  };
  await (await eng.toggleMacro(eng.data.macros[0])).done;
  assert.equal(reads >= 3, true);
  assert.ok(host.clock >= 100 && host.clock < 200, `clock ${host.clock}`);
});

test("playback: Move glides in frames and ends on the target", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { macros: [macro({ steps: [{ kind: "Move", x: 110, y: 20, relative: false, duration_ms: 80 }] })] };
  await (await eng.toggleMacro(eng.data.macros[0])).done;
  const moves = host.calls("input.move");
  assert.equal(moves.length, 10);
  assert.deepEqual(moves[moves.length - 1].slice(1), [110, 20, false, null]);
  assert.equal(host.sleeps(), 80);
});

// // Hotkeys, status, requests ----

test("hotkeys: macros, clicker only when enabled, record always, stop only while busy", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = {
    macros: [macro({ hotkey: "F9", game_only: true, steps: [{ kind: "Wait", ms: 100000, random_ms: 0 }] }), macro({ name: "NoKey" })],
    autoclicker: { enabled: false },
  };
  const ids = (l) => l.map((h) => h.id.split(":")[0]);
  assert.deepEqual(ids(eng.hotkeyList()), ["m", "record"]);
  assert.equal(eng.hotkeyList()[0].gameOnly, true);
  assert.equal(eng.hotkeyList()[1].gameOnly, false);
  eng.data = { macros: eng.data.macros, autoclicker: { enabled: true, game_only: true } };
  assert.deepEqual(ids(eng.hotkeyList()), ["m", "clicker", "record"]);
  const r = await eng.toggleMacro(eng.data.macros[0]);
  assert.deepEqual(ids(eng.hotkeyList()), ["m", "clicker", "record", "stop"]);
  await eng.flush();
  const last = host.calls("hotkeys.set").pop();
  assert.equal(last[1].at(-1).id, "stop");
  const status = host.calls("status").pop();
  assert.deepEqual(status[1], { text: "Macro running", active: true });
  await eng.onHotkey({ id: "stop", pressed: true });
  await r.done;
  await eng.flush();
  assert.equal(host.calls("status").pop()[1], null);
  assert.deepEqual(ids(host.calls("hotkeys.set").pop()[1]), ["m", "clicker", "record"]);
});

test("requests: list / run / stop / status", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  await eng.start();
  const loop = { kind: "Loop", times: 2, steps: [{ kind: "Wait", ms: 10, random_ms: 0 }] };
  eng.data = { macros: [macro({ name: "Farm", hotkey: "F3", steps: [loop, { kind: "Wait", ms: 100000, random_ms: 0 }] }), macro({ name: "Empty" })] };
  const id = eng.data.macros[0].id;
  const H = host.handlers;
  assert.deepEqual(await H.list(), [
    { id, name: "Farm", hotkey: "F3", steps: 3, running: false },
    { id: eng.data.macros[1].id, name: "Empty", hotkey: "", steps: 0, running: false },
  ]);
  assert.match(await H.run({ name: "fArM" }), /Started Farm/);
  assert.deepEqual(await H.status(), { running: [id], clicking: false, recording: false });
  assert.equal((await H.list())[0].running, true);
  assert.match(await H.run({ id }), /Stopped Farm/);
  for (let i = 0; i < 200 && (await H.status()).running.length; i++) await new Promise((r) => setImmediate(r));
  assert.deepEqual((await H.status()).running, []);
  await assert.rejects(H.run({ name: "nope" }), /No macro called/);
  await assert.rejects(H.run({ name: "Empty" }), /no steps/);
  assert.match(await H.run({ name: "Farm" }), /Started/);
  assert.equal(await H.stop(), "Stopped");
  for (let i = 0; i < 200 && (await H.status()).running.length; i++) await new Promise((r) => setImmediate(r));
  assert.deepEqual((await H.status()).running, []);
});

test("start: loads macros.json, saves edits from the page and broadcasts", async () => {
  const stored = JSON.stringify({ macros: [{ id: "m1", name: "Old", steps: [{ kind: "Key", key: "KeyA" }] }], settings: { record_hotkey: "F10" } });
  const host = fakeHost({ storage: { "macros.json": stored } });
  const eng = engineFor(host);
  await eng.start();
  assert.equal(eng.data.macros[0].name, "Old");
  assert.equal(eng.data.settings.record_hotkey, "F10");
  assert.equal(eng.data.settings.stop_hotkey, "Shift+Escape");
  const sets = host.calls("hotkeys.set");
  assert.equal(sets.length, 1);
  assert.equal(sets[0][1][0].keys, "F10");
  host.emit("message", { type: "saveMacro", macro: { id: "m2", name: "New", hotkey: "F4", steps: [] } });
  host.emit("message", { type: "settings", patch: { record_moves: true } });
  await new Promise((r) => setImmediate(r));
  await eng.flush();
  const saved = JSON.parse(host.files["macros.json"]);
  assert.equal(saved.macros.length, 2);
  assert.equal(saved.settings.record_moves, true);
  assert.deepEqual(Object.keys(saved).sort(), ["autoclicker", "macros", "settings"]);
  const state = host.calls("send").pop()[1];
  assert.equal(state.type, "state");
  assert.equal(state.macros.length, 2);
  // Export and import by message.
  host.emit("message", { type: "exportAhk", id: "m1" });
  host.emit("message", { type: "importAhk", text: "F5::\nSleep 100\nSend a", name: "Imp" });
  await new Promise((r) => setImmediate(r));
  await eng.flush();
  const sent = host.calls("send").map((c) => c[1]);
  assert.ok(sent.some((m) => m.type === "ahk" && /Send "\{a\}"/.test(m.text)));
  const imported = sent.find((m) => m.type === "imported");
  assert.equal(imported.skipped, 0);
  assert.equal(eng.data.macros.find((m) => m.name === "Imp").hotkey, "F5");
});

// // Auto-clicker ----

test("clicker: Toggle mode clicks at the interval and stops at the limit", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { autoclicker: { target: "System", interval_ms: 100, hold_ms: 10, limit: "Clicks", limit_value: 3, double: true, fixed: [5, 6] } };
  const r = await eng.toggleClicker();
  await r.done;
  const b = host.calls("input.button").map((c) => c.slice(1, 4));
  assert.equal(b.length, 12); // 3 clicks x 2 presses x (down, up)
  assert.deepEqual(b[0], ["Left", true, null]);
  assert.equal(host.calls("input.move").length, 3);
  // Each click: 10 hold, 30 gap, 10 hold, then (100 - 10) wait.
  assert.equal(host.sleeps(), 3 * (10 + 30 + 10 + 90));
  assert.equal(eng.statusInfo().clicking, false);
});

test("clicker: key input, seconds limit, stops promptly", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { autoclicker: { target: "System", input: "Key", key: "KeyF", interval_ms: 200, limit: "Seconds", limit_value: 1 } };
  const r = await eng.toggleClicker();
  await r.done;
  assert.ok(host.calls("input.key").length >= 4);
  assert.ok(host.calls("input.key").every((c) => c[1] === "KeyF"));
  assert.ok(host.clock >= 1000 && host.clock < 1400, `clock ${host.clock}`);
  const r2 = await eng.toggleClicker();
  await eng.toggleClicker(); // stops
  await r2.done;
});

test("clicker: MouseHeld clicks only while the trigger is held and watches input", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { autoclicker: { target: "System", mode: "MouseHeld", trigger: "Left", interval_ms: 50, hold_ms: 5 } };
  const r = await eng.toggleClicker();
  await eng.flush();
  assert.equal(host.calls("input.watch").pop()[1], true);
  // Nothing held yet: only polling sleeps.
  for (let i = 0; i < 20; i++) await new Promise((x) => setImmediate(x));
  assert.equal(host.calls("input.button").length, 0);
  eng.onInput({ code: "MouseLeft", down: true });
  for (let i = 0; i < 60 && host.calls("input.button").length < 4; i++) await new Promise((x) => setImmediate(x));
  assert.ok(host.calls("input.button").length >= 4);
  eng.onInput({ code: "MouseLeft", down: false });
  await new Promise((x) => setImmediate(x));
  await eng.toggleClicker();
  await r.done;
  await eng.flush();
  assert.equal(host.calls("input.watch").pop()[1], false);
});

test("clicker: OnClick adds a burst per press; game_only gates it", async () => {
  const host = fakeHost();
  host.foreground = { window: 1, pid: 1, roblox: false };
  const eng = engineFor(host);
  eng.data = { autoclicker: { target: "System", mode: "OnClick", trigger: "Left", burst: 3, interval_ms: 20, hold_ms: 0, game_only: true } };
  const r = await eng.toggleClicker();
  eng.onInput({ code: "MouseLeft", down: true });
  eng.onInput({ code: "MouseLeft", down: false });
  for (let i = 0; i < 40; i++) await new Promise((x) => setImmediate(x));
  assert.equal(host.calls("input.button").length, 0, "not in Roblox: no extra clicks");
  host.foreground = { window: 1, pid: 1, roblox: true };
  eng.onInput({ code: "MouseLeft", down: true });
  eng.onInput({ code: "MouseLeft", down: false });
  for (let i = 0; i < 100 && host.calls("input.button").length < 6; i++) await new Promise((x) => setImmediate(x));
  assert.equal(host.calls("input.button").length, 6); // 3 clicks x (down, up)
  await eng.toggleClicker();
  await r.done;
});

// // Recording flow ----

test("recording: toggles, saves 'Recorded macro N' and announces it", async () => {
  const host = fakeHost();
  const eng = engineFor(host);
  eng.data = { macros: [macro()] };
  host.recorded = [
    { ms: 0, kind: "key", code: "F7", down: false },
    { ms: 10, kind: "key", code: "KeyA", down: true },
    { ms: 60, kind: "key", code: "KeyA", down: false },
    { ms: 80, kind: "key", code: "F7", down: true },
  ];
  await eng.onHotkey({ id: "record", pressed: true });
  assert.equal(eng.statusInfo().recording, true);
  assert.equal(host.calls("input.recordStart").length, 1);
  assert.deepEqual(host.calls("status").pop()[1], { text: "Recording a macro", active: true });
  await eng.onHotkey({ id: "record", pressed: true });
  assert.equal(eng.statusInfo().recording, false);
  assert.equal(eng.data.macros.length, 2);
  assert.equal(eng.data.macros[1].name, "Recorded macro 2");
  assert.deepEqual(eng.data.macros[1].steps, [{ kind: "Key", key: "KeyA", press: "Tap", hold_ms: 50 }]);
  assert.ok(host.calls("hud").some((c) => c[1] === "macro" && /Saved Recorded macro 2/.test(c[2])));
  // An empty recording is reported, nothing saved.
  host.recorded = [];
  await eng.onHotkey({ id: "record", pressed: true });
  await eng.onHotkey({ id: "stop", pressed: true });
  assert.equal(eng.data.macros.length, 2);
  assert.ok(host.calls("toast").some((c) => /Nothing was recorded/.test(c[1])));
});

// // Run ----

let failed = 0;
for (const [name, fn] of tests) {
  try {
    await fn();
    passed++;
    console.log(`ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${name}\n${e && e.stack ? e.stack : e}`);
  }
}
console.log(`\n${passed} passed, ${failed} failed (${tests.length} tests)`);
process.exit(failed ? 1 : 0);


