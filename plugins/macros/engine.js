// Macros engine: runs in the background for as long as the plugin is on.
// It owns the data (macros.json), registers the hotkeys, plays macros,
// drives the auto-clicker and records new macros. The page (index.html)
// only shows the state it is sent and asks for changes by message.
//
// Everything that needs to be on time uses pious.call("sleep", ms). Input
// goes out through pious.call("input.*", ...). See docs/PLUGIN-API.md.
//
// The file also works under plain Node (module.exports) so that
// test/engine.test.mjs can drive it with a fake `pious`.

(function (root) {
  "use strict";

  // ── Defaults (same as the Rust Default impls this replaces) ───────────

  const uuid = () => {
    try {
      if (root.crypto && typeof root.crypto.randomUUID === "function") return root.crypto.randomUUID();
    } catch {
      /* fall through */
    }
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      return (c === "x" ? r : (r & 3) | 8).toString(16);
    });
  };

  function defaultMacro(over = {}) {
    return {
      id: uuid(),
      name: "New macro",
      hotkey: "",
      repeat: "Once",
      times: 3,
      speed: 1,
      game_only: false,
      target: "Roblox",
      accounts: [],
      steps: [],
      ...over,
    };
  }
  const defaultSettings = () => ({ record_hotkey: "F7", stop_hotkey: "Shift+Escape", record_moves: false, record_timing: true });
  const defaultAutoclicker = () => ({
    enabled: false,
    hotkey: "F6",
    mode: "Toggle",
    input: "Mouse",
    button: "Left",
    key: "KeyE",
    double: false,
    interval_ms: 100,
    random_ms: 0,
    hold_ms: 10,
    fixed: null,
    limit: "Unlimited",
    limit_value: 100,
    game_only: false,
    target: "Roblox",
    accounts: [],
    trigger: "Left",
    burst: 2,
  });

  const clone = (v) => JSON.parse(JSON.stringify(v));
  const isObj = (v) => v && typeof v === "object" && !Array.isArray(v);
  const num = (v, d) => (typeof v === "number" && Number.isFinite(v) ? v : d);
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

  /** Fills in what a stored macro is missing (serde's `default`s). */
  function normalizeMacro(raw) {
    const m = { ...defaultMacro(), ...(isObj(raw) ? raw : {}) };
    if (typeof m.id !== "string" || !m.id) m.id = uuid();
    m.name = String(m.name ?? "");
    m.hotkey = String(m.hotkey ?? "");
    m.times = num(m.times, 3);
    m.speed = num(m.speed, 1);
    m.game_only = !!m.game_only;
    m.accounts = Array.isArray(m.accounts) ? m.accounts : [];
    m.steps = Array.isArray(m.steps) ? m.steps.filter(isObj) : [];
    return m;
  }

  function normalizeData(raw) {
    const d = isObj(raw) ? raw : {};
    return {
      macros: Array.isArray(d.macros) ? d.macros.map(normalizeMacro) : [],
      settings: { ...defaultSettings(), ...(isObj(d.settings) ? d.settings : {}) },
      autoclicker: { ...defaultAutoclicker(), ...(isObj(d.autoclicker) ? d.autoclicker : {}) },
    };
  }

  /** Steps counted the way the page shows them (loops count their insides). */
  const countSteps = (steps) => steps.reduce((n, s) => n + 1 + (s.kind === "Loop" && Array.isArray(s.steps) ? countSteps(s.steps) : 0), 0);

  // ── Keys and colors ──────────────────────────────────────────────────

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
  /** "Control+Shift+KeyK" → "Ctrl + Shift + K". */
  function keyLabel(combo) {
    if (!combo) return "Not set";
    return String(combo)
      .split("+")
      .map((p) => KEY_NAMES[p] ?? p.replace(/^Key/, "").replace(/^Digit/, "").replace(/^Numpad/, "Num "))
      .join(" + ");
  }

  /** The key codes a hotkey like "Control+Shift+KeyM" is made of (either side of a modifier). */
  function hotkeyCodes(combo) {
    const out = [];
    for (const part of String(combo || "").split("+")) {
      if (!part) continue;
      if (part === "Control") out.push("ControlLeft", "ControlRight");
      else if (part === "Shift") out.push("ShiftLeft", "ShiftRight");
      else if (part === "Alt") out.push("AltLeft", "AltRight");
      else if (part === "Super" || part === "Meta") out.push("MetaLeft", "MetaRight");
      else out.push(part);
    }
    return out;
  }

  /** "#RRGGBB" → [r, g, b], or null. */
  function parseColor(text) {
    const hex = String(text ?? "").trim().replace(/^#+/, "");
    if (!/^[0-9a-fA-F]{6}$/.test(hex)) return null;
    const v = parseInt(hex, 16);
    return [(v >> 16) & 255, (v >> 8) & 255, v & 255];
  }

  const BUTTON_CODE = { Left: "MouseLeft", Right: "MouseRight", Middle: "MouseMiddle", Back: "MouseBack", Forward: "MouseForward" };

  // ── Recorded events → steps ──────────────────────────────────────────

  /**
   * Turns recorded input (`{ ms, kind, ... }` from input.recordStop) into
   * steps. `ignore` holds key codes to leave out (the record hotkey).
   * Button events may carry `x`/`y`; otherwise the last known pointer
   * position from the move events is used for the click's Move step.
   */
  function eventsToSteps(events, ignore, moves, timing) {
    const steps = [];
    let last = null;
    let lastMove = null;
    let pos = null;
    const down = new Set();
    const gap = (at) => {
      if (last !== null) {
        const ms = Math.trunc(at - last);
        if (timing && ms >= 10) steps.push({ kind: "Wait", ms, random_ms: 0 });
      }
      last = at;
    };
    for (const e of events || []) {
      if (!isObj(e)) continue;
      const at = num(e.ms, 0);
      if (e.kind === "key") {
        const code = e.code;
        if (!code || ignore.includes(code)) continue;
        const isDown = !!e.down;
        if (isDown && down.has(code)) continue; // key repeat while held
        if (isDown) down.add(code);
        else if (!down.has(code)) continue; // let go of a key pressed before recording
        else down.delete(code);
        gap(at);
        steps.push({ kind: "Key", key: code, press: isDown ? "Down" : "Up", hold_ms: 0 });
      } else if (e.kind === "button") {
        const isDown = !!e.down;
        const p = typeof e.x === "number" && typeof e.y === "number" ? { x: e.x, y: e.y } : pos;
        gap(at);
        if (isDown && !moves && p) steps.push({ kind: "Move", x: p.x, y: p.y, relative: false, duration_ms: 0 });
        steps.push({ kind: "Click", button: e.button || "Left", press: isDown ? "Down" : "Up", count: 1 });
      } else if (e.kind === "move") {
        pos = { x: e.x, y: e.y };
        if (!moves) continue;
        // About 60 points a second is plenty.
        if (lastMove !== null && at - lastMove < 16) {
          const s = steps[steps.length - 1];
          if (s && s.kind === "Move") {
            s.x = e.x;
            s.y = e.y;
            continue;
          }
        }
        lastMove = at;
        gap(at);
        steps.push({ kind: "Move", x: e.x, y: e.y, relative: false, duration_ms: 0 });
      } else if (e.kind === "scroll") {
        gap(at);
        steps.push({ kind: "Scroll", amount: e.amount, horizontal: !!e.horizontal });
      }
    }
    return simplify(steps);
  }

  /** Down + up of the same key or button with only a pause between becomes one tap. */
  function simplify(steps) {
    const out = [];
    for (const step of steps) {
      if (step.kind === "Key" && step.press === "Up") {
        const n = out.length;
        const lastStep = out[n - 1];
        let wait = 0;
        let at = -1;
        if (lastStep && lastStep.kind === "Wait" && n >= 2) {
          wait = lastStep.ms;
          at = n - 2;
        } else if (n >= 1) {
          at = n - 1;
        }
        if (at >= 0) {
          const d = out[at];
          if (d.kind === "Key" && d.press === "Down" && d.key === step.key) {
            out.length = at;
            out.push({ kind: "Key", key: step.key, press: "Tap", hold_ms: wait });
            continue;
          }
        }
        out.push(step);
      } else if (step.kind === "Click" && step.press === "Up") {
        const n = out.length;
        const lastStep = out[n - 1];
        let at = -1;
        if (lastStep && lastStep.kind === "Wait" && n >= 2 && lastStep.ms < 250) at = n - 2;
        else if (n >= 1) at = n - 1;
        if (at >= 0) {
          const d = out[at];
          if (d.kind === "Click" && d.press === "Down" && d.button === step.button) {
            out.length = at;
            out.push({ kind: "Click", button: step.button, press: "Tap", count: 1 });
            continue;
          }
        }
        out.push(step);
      } else {
        out.push(step);
      }
    }
    return out;
  }

  // ── AutoHotkey in and out ────────────────────────────────────────────

  const AHK_NAMED = {
    enter: "Enter",
    return: "Enter",
    space: "Space",
    tab: "Tab",
    esc: "Escape",
    escape: "Escape",
    backspace: "Backspace",
    bs: "Backspace",
    delete: "Delete",
    del: "Delete",
    insert: "Insert",
    ins: "Insert",
    home: "Home",
    end: "End",
    pgup: "PageUp",
    pgdn: "PageDown",
    up: "ArrowUp",
    down: "ArrowDown",
    left: "ArrowLeft",
    right: "ArrowRight",
    shift: "ShiftLeft",
    lshift: "ShiftLeft",
    rshift: "ShiftRight",
    ctrl: "ControlLeft",
    control: "ControlLeft",
    lctrl: "ControlLeft",
    lcontrol: "ControlLeft",
    rctrl: "ControlRight",
    rcontrol: "ControlRight",
    alt: "AltLeft",
    lalt: "AltLeft",
    ralt: "AltRight",
    lwin: "MetaLeft",
    win: "MetaLeft",
    rwin: "MetaRight",
    capslock: "CapsLock",
    numlock: "NumLock",
    scrolllock: "ScrollLock",
    printscreen: "PrintScreen",
    pause: "Pause",
    appskey: "ContextMenu",
    numpadenter: "NumpadEnter",
    numpadadd: "NumpadAdd",
    numpadsub: "NumpadSubtract",
    numpadmult: "NumpadMultiply",
    numpaddiv: "NumpadDivide",
    numpaddot: "NumpadDecimal",
    "-": "Minus",
    "=": "Equal",
    "[": "BracketLeft",
    "]": "BracketRight",
    "\\": "Backslash",
    ";": "Semicolon",
    "'": "Quote",
    "`": "Backquote",
    ",": "Comma",
    ".": "Period",
    "/": "Slash",
  };

  const asciiLower = (s) => s.replace(/[A-Z]/g, (c) => c.toLowerCase());
  const asciiUpper = (s) => s.replace(/[a-z]/g, (c) => c.toUpperCase());
  /** Rust's f64::round: halves go away from zero. */
  const roundAway = (x) => (x < 0 ? -Math.round(-x) : Math.round(x));
  const trimStartChars = (s, chars) => {
    let i = 0;
    while (i < s.length && chars.includes(s[i])) i++;
    return s.slice(i);
  };
  const trimEndChars = (s, chars) => {
    let i = s.length;
    while (i > 0 && chars.includes(s[i - 1])) i--;
    return s.slice(0, i);
  };
  const trimStartStr = (s, prefix) => {
    while (prefix && s.startsWith(prefix)) s = s.slice(prefix.length);
    return s;
  };

  /** AutoHotkey's key names → Pious's. */
  function ahkKeyCode(name) {
    const n = name.trim();
    const lower = asciiLower(n);
    if (Object.prototype.hasOwnProperty.call(AHK_NAMED, lower)) return AHK_NAMED[lower];
    if (n.length === 1) {
      if (/[A-Za-z]/.test(n)) return "Key" + asciiUpper(n);
      if (/[0-9]/.test(n)) return "Digit" + n;
    }
    let m = /^numpad(\d)$/.exec(lower);
    if (m) return "Numpad" + m[1];
    m = /^f(\d{1,3})$/.exec(lower);
    if (m) {
      const f = parseInt(m[1], 10);
      if (f >= 1 && f <= 24) return "F" + f;
    }
    return null;
  }

  const AHK_OUT = {
    Enter: "Enter",
    Space: "Space",
    Tab: "Tab",
    Escape: "Esc",
    Backspace: "Backspace",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PgUp",
    PageDown: "PgDn",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    ShiftLeft: "LShift",
    Shift: "LShift",
    ShiftRight: "RShift",
    ControlLeft: "LCtrl",
    Control: "LCtrl",
    ControlRight: "RCtrl",
    AltLeft: "LAlt",
    Alt: "LAlt",
    AltRight: "RAlt",
    MetaLeft: "LWin",
    Meta: "LWin",
    Super: "LWin",
    MetaRight: "RWin",
    CapsLock: "CapsLock",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Backquote: "`",
    Comma: ",",
    Period: ".",
    Slash: "/",
  };
  /** Pious's key names → AutoHotkey's. */
  function ahkKey(code) {
    if (Object.prototype.hasOwnProperty.call(AHK_OUT, code)) return AHK_OUT[code];
    if (code.startsWith("Key")) return asciiLower(code.slice(3));
    if (code.startsWith("Digit")) return code.slice(5);
    return code;
  }

  function ahkMouseButton(name) {
    switch (asciiLower(name.trim())) {
      case "l":
      case "left":
      case "lbutton":
        return "Left";
      case "r":
      case "right":
      case "rbutton":
        return "Right";
      case "m":
      case "middle":
      case "mbutton":
        return "Middle";
      case "x1":
      case "xbutton1":
        return "Back";
      case "x2":
      case "xbutton2":
        return "Forward";
      default:
        return null;
    }
  }

  /** A hotkey label's keys (`^!F1`) in Pious's format (`Control+Alt+F1`). */
  function hotkeyOfLabel(label) {
    let rest = trimStartChars(label.trim(), "~*$");
    const parts = [];
    while (rest.length) {
      const c = rest[0];
      const mod = c === "^" ? "Control" : c === "!" ? "Alt" : c === "+" ? "Shift" : c === "#" ? "Super" : null;
      if (!mod) break;
      parts.push(mod);
      rest = rest.slice(1);
    }
    const key = rest.split(/\s+/).filter(Boolean)[0];
    if (!key) return null;
    const code = ahkKeyCode(key);
    if (!code) return null;
    parts.push(code);
    return parts.join("+");
  }

  /** Arguments after a command, v1 (`Sleep, 100`) or v2 (`Sleep 100`, `Sleep(100)`). */
  function ahkArgs(text) {
    let rest = trimStartChars(text.trim(), ",").trim();
    if (rest.startsWith("(") && rest.endsWith(")") && rest.length >= 2) rest = rest.slice(1, -1);
    const out = [];
    let current = "";
    let quoted = false;
    for (const c of rest) {
      if (c === '"') quoted = !quoted;
      else if (c === "," && !quoted) {
        out.push(current.trim());
        current = "";
      } else current += c;
    }
    if (current.trim() !== "" || out.length) out.push(current.trim());
    return out;
  }

  function ahkNumber(text) {
    const t = text.trim().replace(/^"+|"+$/g, "");
    if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(t)) return null;
    return roundAway(Number(t));
  }

  /** `Send` text: `{Key}`, `{Key down}`, `{Key 3}`, modifiers `^!+#`, and plain characters. */
  function sendSteps(text, raw) {
    const steps = [];
    let plain = "";
    const flush = () => {
      if (plain) {
        steps.push({ kind: "Text", text: plain, delay_ms: 0 });
        plain = "";
      }
    };
    const tap = (key) => ({ kind: "Key", key, press: "Tap", hold_ms: 30 });
    const chars = Array.from(text);
    let held = [];
    const releaseHeld = () => {
      for (const key of held.reverse()) steps.push({ kind: "Key", key, press: "Up", hold_ms: 0 });
      held = [];
    };
    let i = 0;
    while (i < chars.length) {
      const c = chars[i];
      if (!raw && "^!+#".includes(c)) {
        flush();
        const modifier = c === "^" ? "ControlLeft" : c === "!" ? "AltLeft" : c === "+" ? "ShiftLeft" : "MetaLeft";
        steps.push({ kind: "Key", key: modifier, press: "Down", hold_ms: 0 });
        held.push(modifier);
        i++;
        continue;
      }
      if (!raw && c === "{") {
        let end = -1;
        for (let k = i + 1; k < chars.length; k++) {
          if (chars[k] === "}") {
            end = k;
            break;
          }
        }
        if (end >= 0) {
          // `{}}` sends a brace.
          if (end === i + 1 && chars[end + 1] === "}") end += 1;
          const inner = chars.slice(i + 1, end).join("");
          const words = inner.split(/\s+/).filter(Boolean);
          const name = words[0] ?? "";
          const arg = asciiLower(words[1] ?? "");
          const key = ahkKeyCode(name);
          if (key) {
            flush();
            if (arg === "down") steps.push({ kind: "Key", key, press: "Down", hold_ms: 0 });
            else if (arg === "up") steps.push({ kind: "Key", key, press: "Up", hold_ms: 0 });
            else {
              const n = /^\d+$/.test(arg) ? parseInt(arg, 10) : 1;
              const times = clamp(n, 1, 500);
              for (let t = 0; t < times; t++) steps.push(tap(key));
            }
          } else if (Array.from(name).length === 1) {
            // `{!}`, `{#}`, a literal character.
            plain += name;
          } else {
            flush();
            steps.push({ kind: "Comment", text: `AutoHotkey key Pious doesn't know: {${inner}}` });
          }
          i = end + 1;
          releaseHeld();
          continue;
        }
      }
      if (!held.length) {
        plain += c;
      } else {
        // A modifier applies to this one key.
        const key = ahkKeyCode(c);
        steps.push(key ? tap(key) : { kind: "Text", text: c, delay_ms: 0 });
        releaseHeld();
      }
      i++;
    }
    flush();
    releaseHeld();
    return steps;
  }

  /** `Click` and `MouseClick` arguments: a button, a spot, a count, down/up. */
  function clickSteps(parts, mouseClick) {
    const words = mouseClick ? parts.slice() : parts.flatMap((p) => p.split(/\s+/).filter(Boolean));
    let button = "Left";
    let press = "Tap";
    const numbers = [];
    let relative = false;
    for (const w of words) {
      const lower = asciiLower(w);
      const b = ahkMouseButton(lower);
      if (b) button = b;
      else if (lower === "down" || lower === "d") press = "Down";
      else if (lower === "up" || lower === "u") press = "Up";
      else if (lower === "rel" || lower === "relative" || (lower === "r" && mouseClick)) relative = true;
      else {
        const n = ahkNumber(w);
        if (n !== null) numbers.push(n);
      }
    }
    const steps = [];
    if (numbers.length >= 2) steps.push({ kind: "Move", x: numbers[0], y: numbers[1], relative, duration_ms: 0 });
    // One number is a count (`Click 2`); with a spot, it comes third.
    const count = numbers.length === 1 ? numbers[0] : (numbers[2] ?? 1);
    if (count > 0) steps.push({ kind: "Click", button, press, count: clamp(count, 1, 100) });
    return steps;
  }

  /** Reads an AutoHotkey script → `{ steps, hotkey, skipped }`. */
  function ahkImport(script) {
    let hotkey = null;
    let skipped = 0;
    // Each open `Loop`: [times, steps inside, braceless (covers only the next line)].
    const stack = [[0, [], false]];
    let inComment = false;
    const closeLoop = () => {
      const [times, inner] = stack.pop();
      stack[stack.length - 1][1].push({ kind: "Loop", times, steps: inner });
    };

    for (const rawLine of String(script).split(/\r\n|\n|\r/)) {
      let line = rawLine.trim();
      if (inComment) {
        if (line.startsWith("*/")) inComment = false;
        continue;
      }
      if (line.startsWith("/*")) {
        inComment = !line.includes("*/");
        continue;
      }
      if (line.startsWith(";")) continue;
      const semi = line.indexOf(" ;");
      if (semi >= 0) line = line.slice(0, semi).trimEnd();
      if (line === "" || (line.startsWith("#") && !line.includes("::"))) continue;

      // A hotkey label, maybe with its action on the same line.
      const sep = line.indexOf("::");
      if (sep >= 0) {
        const label = line.slice(0, sep);
        const action = line.slice(sep + 2);
        if ((label !== "" && !label.includes(" ")) || "^!+#~*$".includes(label[0] ?? "\0")) {
          if (hotkey === null) hotkey = hotkeyOfLabel(label);
          line = action.trim();
          if (line === "" || line === "{") continue;
        }
      }

      const lower = asciiLower(line);
      const at = line.search(/[, (\t]/);
      const command = at >= 0 ? lower.slice(0, at) : lower;
      const rest = at >= 0 ? line.slice(at) : "";
      const steps = stack[stack.length - 1][1];
      switch (command) {
        case "}":
          if (stack.length > 1) closeLoop();
          break;
        case "{":
          stack[stack.length - 1][2] = false;
          continue;
        case "return":
        case "exitapp":
        case "reload":
          break;
        case "loop": {
          const braced = rest.trimEnd().endsWith("{");
          const parts = ahkArgs(trimEndChars(rest.trimEnd(), "{"));
          const n = parts.length ? ahkNumber(parts[0]) : null;
          const times = clamp(n ?? 0, 0, 1000000);
          stack.push([times, [], !braced]);
          continue;
        }
        case "sleep": {
          const a = ahkArgs(rest);
          const n = a.length ? ahkNumber(a[0]) : null;
          steps.push({ kind: "Wait", ms: clamp(n ?? 0, 0, 86400000), random_ms: 0 });
          break;
        }
        case "send":
        case "sendinput":
        case "sendevent":
        case "sendplay":
        case "sendraw":
        case "sendtext": {
          let text = trimStartChars(rest.trim(), ",").trim();
          if (text.startsWith("(") && text.endsWith(")") && text.length >= 2) text = text.slice(1, -1);
          if (text.startsWith('"') && text.endsWith('"') && text.length >= 2) text = text.slice(1, -1);
          const raw = command === "sendraw" || command === "sendtext" || text.startsWith("{Raw}") || text.startsWith("{Text}");
          text = trimStartStr(trimStartStr(text, "{Raw}"), "{Text}");
          steps.push(...sendSteps(text.split("``").join("`").split("`n").join("\n"), raw));
          break;
        }
        case "click":
          steps.push(...clickSteps(ahkArgs(rest), false));
          break;
        case "mouseclick":
          steps.push(...clickSteps(ahkArgs(rest), true));
          break;
        case "mousemove": {
          const parts = ahkArgs(rest);
          const x = parts.length > 0 ? ahkNumber(parts[0]) : null;
          const y = parts.length > 1 ? ahkNumber(parts[1]) : null;
          const speed = (parts.length > 2 ? ahkNumber(parts[2]) : null) ?? 0;
          const relative = parts.length > 3 && parts[3].trim().toLowerCase() === "r";
          if (x !== null && y !== null) steps.push({ kind: "Move", x, y, relative, duration_ms: clamp(speed, 0, 100) * 10 });
          break;
        }
        case "winactivate":
        case "winwaitactive":
          if (lower.includes("roblox")) {
            steps.push({ kind: "FocusRoblox" });
            break;
          }
        // falls through (not Roblox): kept as a note
        default:
          if (
            ["setkeydelay", "setmousedelay", "sendmode", "setbatchlines", "coordmode", "settitlematchmode", "setdefaultmousespeed"].includes(command)
          ) {
            break;
          }
          if (command === "run") {
            const a = ahkArgs(rest);
            if (a.length && a[0]) steps.push({ kind: "Run", target: a[0].replace(/^"+|"+$/g, "") });
            break;
          }
          skipped++;
          steps.push({ kind: "Comment", text: `From AutoHotkey (not run): ${line}` });
      }
      // A loop without braces ends after its one line.
      while (stack.length > 1) {
        const top = stack[stack.length - 1];
        if (top[2] && top[1].length) closeLoop();
        else break;
      }
    }
    while (stack.length > 1) closeLoop();
    return { steps: stack.pop()[1], hotkey, skipped };
  }

  const AHK_BUTTON = { Left: "Left", Right: "Right", Middle: "Middle", Back: "X1", Forward: "X2" };

  /** A hotkey in Pious's format (`Control+Alt+F1`) as an AutoHotkey label. */
  function ahkHotkey(hotkey) {
    let out = "";
    let key = null;
    for (const part of String(hotkey).split("+")) {
      if (part === "Control") out += "^";
      else if (part === "Alt") out += "!";
      else if (part === "Shift") out += "+";
      else if (part === "Super" || part === "Meta") out += "#";
      else key = ahkKey(part);
    }
    return key === null ? null : out + key;
  }

  function writeSteps(steps, indent, out) {
    const pad = "    ".repeat(indent);
    let text = "";
    for (const step of steps) {
      let line;
      switch (step.kind) {
        case "Key": {
          const k = ahkKey(step.key);
          const press = step.press || "Tap";
          if (press === "Down") line = `Send "{${k} down}"`;
          else if (press === "Up") line = `Send "{${k} up}"`;
          else if ((step.hold_ms || 0) > 0) line = `Send "{${k} down}"\n${pad}Sleep ${step.hold_ms}\n${pad}Send "{${k} up}"`;
          else line = `Send "{${k}}"`;
          break;
        }
        case "Text":
          line = `SendText "${step.text.split("`").join("``").split('"').join('`"').split("\n").join("`n")}"`;
          break;
        case "Click": {
          const b = AHK_BUTTON[step.button || "Left"] || "Left";
          const press = step.press || "Tap";
          line = press === "Down" ? `Click "${b} Down"` : press === "Up" ? `Click "${b} Up"` : `Click "${b} ${step.count ?? 1}"`;
          break;
        }
        case "Move": {
          const speed = Math.min(Math.floor((step.duration_ms || 0) / 10), 100);
          line = step.relative ? `MouseMove ${step.x}, ${step.y}, ${speed}, "R"` : `MouseMove ${step.x}, ${step.y}, ${speed}`;
          break;
        }
        case "Scroll": {
          const name = !step.horizontal ? (step.amount > 0 ? "WheelUp" : "WheelDown") : step.amount > 0 ? "WheelRight" : "WheelLeft";
          line = `Click "${name} ${Math.abs(step.amount)}"`;
          break;
        }
        case "Wait":
          line = step.random_ms > 0 ? `Sleep ${step.ms} + Random(0, ${step.random_ms})` : `Sleep ${step.ms}`;
          break;
        case "Loop": {
          const inner = writeSteps(step.steps || [], indent + 1, "");
          const head = step.times === 0 ? "Loop" : `Loop ${step.times}`;
          line = `${head} {\n${inner}${pad}}`;
          break;
        }
        case "WaitPixel": {
          const bgr = String(step.color).replace(/^#+/, "");
          line = `while (PixelGetColor(${step.x}, ${step.y}) != "0x${bgr}")\n${pad}    Sleep 20`;
          break;
        }
        case "FocusRoblox":
          line = 'WinActivate "ahk_exe RobloxPlayerBeta.exe"';
          break;
        case "Run":
          line = `Run "${step.target.split('"').join('`"')}"`;
          break;
        case "Comment":
          line = `; ${step.text}`;
          break;
        default:
          continue;
      }
      text += pad + line + "\n";
    }
    return out + text;
  }

  /** Writes a macro as an AutoHotkey v2 script. */
  function ahkExport(m) {
    let out = `; ${m.name} — exported from Pious\n#Requires AutoHotkey v2.0\nSendMode "Input"\nCoordMode "Mouse", "Screen"\n\n`;
    const body = writeSteps(m.steps || [], 1, "");
    const indentMore = (t) =>
      t
        .split("\n")
        .filter((l, i, a) => !(i === a.length - 1 && l === ""))
        .map((l) => `    ${l}\n`)
        .join("");
    const label = (m.hotkey && ahkHotkey(m.hotkey)) || "F8";
    let repeat;
    switch (m.repeat) {
      case "Times":
        repeat = `    Loop ${Math.max(1, m.times | 0)} {\n${indentMore(body)}    }\n`;
        break;
      case "UntilStopped":
      case "WhileHeld":
        repeat = `    Loop {\n${indentMore(body)}    }\n`;
        break;
      default:
        repeat = body;
    }
    out += `${label}:: {\n${repeat}}\n`;
    return out;
  }

  // ── Targets ──────────────────────────────────────────────────────────

  /**
   * The windows input goes to: `[null]` for the real keyboard and mouse,
   * else a list of window numbers (from windows.roblox()).
   */
  async function resolveTargets(P, target, accounts) {
    if (target === "System") return [null];
    const wins = (await P.call("windows.roblox")) || [];
    if (target === "Accounts") {
      const ids = accounts || [];
      const windows = wins.filter((w) => w.account && ids.includes(w.account.id)).map((w) => w.window);
      if (!windows.length) {
        throw new Error(ids.length ? "None of the picked accounts has a Roblox window open." : "Pick which accounts' Roblox windows this goes to.");
      }
      return windows;
    }
    if (!wins.length) throw new Error("No Roblox window is open to send this to.");
    if (target === "AllRoblox") return wins.map((w) => w.window);
    // The Roblox window you used last, else any.
    return [(wins.find((w) => w.last) || wins[0]).window];
  }

  // ── The engine ───────────────────────────────────────────────────────

  function createEngine(P, opts = {}) {
    const now = opts.now || (() => Date.now());
    const random = opts.random || Math.random;
    const call = (method, ...args) => P.call(method, ...args);

    let data = normalizeData(null);
    /** macro id → control block `{ stop, ops }` */
    const running = new Map();
    let clicker = null; // control block of the running auto-clicker
    let recording = null; // { since }
    let lastHotkeySig = null;
    let lastHotkeyProblems = "";
    let watching = false;
    let chain = Promise.resolve(); // serializes host-side registrations
    let saveChain = Promise.resolve();
    let lastError = { text: "", at: 0 };

    // ── Reporting ──
    const message = (e) => String((e && e.message) || e || "Something went wrong").slice(0, 190);
    function toast(text) {
      try {
        return Promise.resolve(call("toast", text)).catch(() => {});
      } catch {
        return Promise.resolve();
      }
    }
    function hud(kind, text) {
      try {
        return Promise.resolve(call("hud", kind, text)).catch(() => {});
      } catch {
        return Promise.resolve();
      }
    }
    /** Tells the user about an error (not the same one over and over). */
    function report(e) {
      const text = message(e);
      const t = now();
      if (lastError.text === text && t - lastError.at < 3000) return;
      lastError = { text, at: t };
      toast(text);
    }

    // ── Timing ──
    const jitter = (ms) => (ms > 0 ? random() * ms : 0);

    /** Waits (in chunks of at most 50 ms, so a stop is quick). False when stopped. */
    async function wait(ms, ctl) {
      ctl.ops++;
      const end = now() + Math.max(0, ms);
      for (;;) {
        if (ctl.stop) return false;
        const left = end - now();
        if (left <= 0) return true;
        await call("sleep", Math.max(1, Math.round(Math.min(50, left))));
      }
    }

    // ── Sending input ──

    /** Sends a playback's input to its targets and remembers what is held down. */
    class Out {
      constructor(targets, ctl) {
        this.targets = targets;
        this.ctl = ctl;
        this.system = targets.length === 1 && targets[0] === null;
        this.at = null; // pointer for background windows (screen coordinates)
        this.keys = [];
        this.buttons = [];
      }
      each(method, ...args) {
        this.ctl.ops++;
        return Promise.all(this.targets.map((t) => call(method, ...this.withTarget(method, t, args))));
      }
      withTarget(method, t, args) {
        switch (method) {
          case "input.key":
            return [args[0], args[1], t];
          case "input.text":
            return [args[0], t];
          case "input.button":
            return [args[0], args[1], t, this.system ? null : args[2]];
          case "input.move":
            return [args[0], args[1], args[2], t];
          case "input.scroll":
            return [args[0], args[1], t, this.system ? null : args[2]];
          default:
            return args;
        }
      }
      async key(code, down) {
        if (down) {
          if (!this.keys.includes(code)) this.keys.push(code);
        } else this.keys = this.keys.filter((k) => k !== code);
        await this.each("input.key", code, down);
      }
      async char(c) {
        await this.each("input.text", c);
      }
      async button(button, down) {
        if (down) this.buttons.push(button);
        else {
          const i = this.buttons.indexOf(button);
          if (i >= 0) this.buttons.splice(i, 1);
        }
        await this.each("input.button", button, down, this.at);
      }
      async moveTo(x, y) {
        if (!this.system) this.at = { x, y };
        await this.each("input.move", x, y, false);
      }
      /** A relative move. In the background this moves the pointer Pious keeps for the window. */
      async moveBy(dx, dy) {
        if (this.system) await this.each("input.move", dx, dy, true);
        else {
          const c = await this.cursor();
          await this.moveTo(c.x + dx, c.y + dy);
        }
      }
      async scroll(notches, horizontal) {
        await this.each("input.scroll", notches, horizontal, this.at);
      }
      async cursor() {
        this.ctl.ops++;
        if (this.system) {
          const c = await call("input.cursor");
          return { x: c.x, y: c.y };
        }
        if (this.at) return this.at;
        const p = await call("input.windowPoint", this.targets[0], 0.5, 0.5);
        return { x: p.x, y: p.y };
      }
      async pixel(x, y) {
        this.ctl.ops++;
        return call("input.pixel", x, y, this.system ? null : this.targets[0]);
      }
      /** Lets go of everything still held down. */
      async release() {
        const keys = this.keys;
        const buttons = this.buttons;
        this.keys = [];
        this.buttons = [];
        for (const k of keys) await this.each("input.key", k, false).catch(() => {});
        for (const b of buttons) await this.each("input.button", b, false, this.at).catch(() => {});
      }
    }

    // ── Playback ──

    /** Plays steps once. Returns false when stopped part-way. Anything held is let go before returning. */
    async function play(steps, speed, ctl, out) {
      const sp = num(speed, 1);
      const scale = 1 / clamp(sp, 0.1, 20);
      try {
        return await runSteps(steps, scale, ctl, out);
      } finally {
        await out.release();
      }
    }

    async function runSteps(steps, scale, ctl, out) {
      for (const step of steps) {
        if (ctl.stop) return false;
        let ok = true;
        switch (step.kind) {
          case "Key": {
            const press = step.press || "Tap";
            if (press === "Down") await out.key(step.key, true);
            else if (press === "Up") await out.key(step.key, false);
            else {
              await out.key(step.key, true);
              ok = await wait(Math.max(num(step.hold_ms, 0), 15) * scale, ctl);
              await out.key(step.key, false);
            }
            break;
          }
          case "Text": {
            for (const c of String(step.text ?? "")) {
              await out.char(c);
              if (!(await wait(Math.max(num(step.delay_ms, 0), 4) * scale, ctl))) {
                ok = false;
                break;
              }
            }
            break;
          }
          case "Click": {
            const button = step.button || "Left";
            const press = step.press || "Tap";
            if (press === "Down") await out.button(button, true);
            else if (press === "Up") await out.button(button, false);
            else {
              const count = Math.max(num(step.count, 1), 1);
              for (let i = 0; i < count; i++) {
                if (i > 0 && !(await wait(40 * scale, ctl))) {
                  ok = false;
                  break;
                }
                await out.button(button, true);
                await call("sleep", 12);
                await out.button(button, false);
              }
            }
            break;
          }
          case "Move": {
            const dur = num(step.duration_ms, 0);
            if (!dur) {
              if (step.relative) await out.moveBy(step.x, step.y);
              else await out.moveTo(step.x, step.y);
            } else ok = await glide(out, step.x, step.y, !!step.relative, dur * scale, ctl);
            break;
          }
          case "Scroll":
            await out.scroll(step.amount, !!step.horizontal);
            break;
          case "Wait":
            ok = await wait((num(step.ms, 0) + jitter(num(step.random_ms, 0))) * scale, ctl);
            break;
          case "Loop": {
            const times = num(step.times, 0);
            const inner = Array.isArray(step.steps) ? step.steps : [];
            let round = 0;
            while (times === 0 || round < times) {
              round++;
              const before = ctl.ops;
              const started = now();
              if (!(await runSteps(inner, scale, ctl, out))) {
                ok = false;
                break;
              }
              // An empty or instant loop shouldn't spin the CPU.
              const instant = inner.length === 0 || ctl.ops === before || (times === 0 && now() - started < 1);
              if (instant && !(await wait(50, ctl))) {
                ok = false;
                break;
              }
            }
            break;
          }
          case "WaitPixel": {
            const want = parseColor(step.color) || [0, 0, 0];
            const tol = num(step.tolerance, 0);
            const timeout = num(step.timeout_ms, 0);
            const start = now();
            for (;;) {
              const got = parseColor(await out.pixel(step.x, step.y));
              if (got && got.every((v, i) => Math.abs(v - want[i]) <= tol)) break;
              if (timeout > 0 && now() - start >= timeout) break;
              if (!(await wait(25, ctl))) {
                ok = false;
                break;
              }
            }
            break;
          }
          case "FocusRoblox": {
            const wins = (await call("windows.roblox")) || [];
            const w = wins.find((x) => x.last) || wins[0];
            if (w) await call("windows.focus", w.window);
            ok = await wait(80, ctl);
            break;
          }
          case "Run": {
            const target = String(step.target ?? "").trim();
            if (target) {
              try {
                // A program, file or web address (the "run" permission).
                await call("system.open", target);
              } catch (e) {
                report(e);
              }
            }
            break;
          }
          default: // Comment and anything unknown
            break;
        }
        if (!ok) return false;
      }
      return true;
    }

    async function glide(out, x, y, relative, ms, ctl) {
      const s = await out.cursor();
      const tx = relative ? s.x + x : x;
      const ty = relative ? s.y + y : y;
      const frames = Math.max(Math.ceil(ms / 8), 1);
      let lx = s.x;
      let ly = s.y;
      for (let i = 1; i <= frames; i++) {
        const t = i / frames;
        const e = t * t * (3 - 2 * t);
        const nx = s.x + roundAway((tx - s.x) * e);
        const ny = s.y + roundAway((ty - s.y) * e);
        if (relative) await out.moveBy(nx - lx, ny - ly);
        else await out.moveTo(nx, ny);
        lx = nx;
        ly = ny;
        if (!(await wait(ms / frames, ctl))) return false;
      }
      return true;
    }

    // ── Running macros ──

    const findMacro = (id) => data.macros.find((m) => m.id === id);

    /**
     * Starts a macro, or stops it if it is running. Resolves once it has
     * started (or stopped); `done` settles when playback ends.
     */
    async function toggleMacro(m) {
      const cur = running.get(m.id);
      if (cur) {
        cur.stop = true;
        return { started: false, done: Promise.resolve() };
      }
      if (!m.steps.length) throw new Error(`${m.name} has no steps yet.`);
      const copy = clone(m);
      const ctl = { stop: false, ops: 0 };
      running.set(m.id, ctl);
      busyChanged();
      let targets;
      try {
        targets = await resolveTargets(P, copy.target || "Roblox", copy.accounts);
      } catch (e) {
        running.delete(m.id);
        busyChanged();
        throw e;
      }
      const done = (async () => {
        try {
          let round = 0;
          for (;;) {
            if (ctl.stop) break;
            round++;
            const out = new Out(targets, ctl);
            if (!(await play(copy.steps, copy.speed, ctl, out))) break;
            const more = copy.repeat === "Once" ? false : copy.repeat === "Times" ? round < Math.max(num(copy.times, 1), 1) : true;
            if (!more || ctl.stop) break;
          }
        } catch (e) {
          report(e);
        } finally {
          if (running.get(m.id) === ctl) running.delete(m.id);
          busyChanged();
        }
      })();
      return { started: true, done };
    }

    // ── The auto-clicker ──

    const seenPresses = new Map(); // "MouseLeft" → clicks you made
    const physical = new Set(); // codes held down right now

    async function clickOnce(s, ctl, out) {
      if (Array.isArray(s.fixed) && s.fixed.length === 2) await out.moveTo(s.fixed[0], s.fixed[1]);
      const presses = s.double ? 2 : 1;
      for (let i = 0; i < presses; i++) {
        if (i > 0 && !(await wait(30, ctl))) return;
        if (s.input === "Key") {
          await out.key(s.key, true);
          await wait(Math.max(s.hold_ms, 10), ctl);
          await out.key(s.key, false);
        } else {
          await out.button(s.button, true);
          if (s.hold_ms > 0) await wait(s.hold_ms, ctl);
          await out.button(s.button, false);
        }
      }
    }

    async function foregroundIsRoblox() {
      try {
        const f = await call("windows.foreground");
        return !!(f && f.roblox);
      } catch {
        return false;
      }
    }

    /** Clicks until stopped or the limit is reached. Returns the clicks made. */
    async function autoclick(s, ctl, out) {
      const startAt = now();
      let clicks = 0;
      const trigger = BUTTON_CODE[s.trigger] || "MouseLeft";
      let seen = seenPresses.get(trigger) || 0;
      let owed = 0; // extra clicks still owed for your clicks (OnClick)
      const allowed = async () => !s.game_only || (await foregroundIsRoblox());
      try {
        for (;;) {
          if (ctl.stop) break;
          if (s.limit === "Clicks" && clicks >= s.limit_value) break;
          if (s.limit === "Seconds" && Math.floor((now() - startAt) / 1000) >= s.limit_value) break;
          let go;
          if (s.mode === "MouseHeld") go = physical.has(trigger) && (await allowed());
          else if (s.mode === "OnClick") {
            const cur = seenPresses.get(trigger) || 0;
            if (cur > seen && (await allowed())) owed += (cur - seen) * clamp(s.burst, 1, 50);
            seen = cur;
            if (owed > 0) {
              owed--;
              go = true;
            } else go = false;
          } else go = true;
          if (!go) {
            // Waiting for you: check often, so it starts right away.
            if (!(await wait(4, ctl))) break;
            continue;
          }
          // Your own click comes first.
          if (s.mode === "OnClick" && !(await wait(clamp(s.interval_ms, 10, 60), ctl))) break;
          await clickOnce(s, ctl, out);
          clicks++;
          const w = Math.max(s.interval_ms, 1) + jitter(s.random_ms) - s.hold_ms;
          if (!(await wait(Math.max(w, 1), ctl))) break;
        }
      } finally {
        await out.release();
      }
      return clicks;
    }

    async function startClicker() {
      if (clicker) return null;
      const s = clone(data.autoclicker);
      const ctl = { stop: false, ops: 0 };
      clicker = ctl;
      busyChanged();
      let targets;
      try {
        targets = await resolveTargets(P, s.target || "Roblox", s.accounts);
      } catch (e) {
        clicker = null;
        busyChanged();
        throw e;
      }
      if (s.mode === "MouseHeld" || s.mode === "OnClick") {
        hud("autoclick", `Auto-clicker on · ${s.mode === "MouseHeld" ? "Hold the mouse to click" : "Your clicks get extra clicks"}`);
      }
      const done = (async () => {
        try {
          await autoclick(s, ctl, new Out(targets, ctl));
        } catch (e) {
          report(e);
        } finally {
          if (clicker === ctl) clicker = null;
          busyChanged();
        }
      })();
      return { done };
    }

    /** Starts clicking, or stops if it already is. */
    async function toggleClicker() {
      if (clicker) {
        clicker.stop = true;
        return null;
      }
      return startClicker();
    }

    // ── Recording ──

    async function toggleRecording() {
      if (recording) return finishRecording();
      try {
        await call("input.recordStart");
      } catch (e) {
        toast(message(e));
        return null;
      }
      recording = { since: now() };
      busyChanged();
      hud("macro", `Recording a macro · ${keyLabel(data.settings.record_hotkey)} to stop`);
      return null;
    }

    /** Stops recording and saves the macro. Returns its name, or null. */
    async function finishRecording() {
      if (!recording) return null;
      recording = null;
      let events = [];
      try {
        const r = await call("input.recordStop");
        events = Array.isArray(r) ? r : r && Array.isArray(r.events) ? r.events : [];
      } catch (e) {
        report(e);
      }
      busyChanged();
      const s = data.settings;
      const steps = eventsToSteps(events, hotkeyCodes(s.record_hotkey), !!s.record_moves, !!s.record_timing);
      if (!steps.length) {
        toast("Nothing was recorded.");
        return null;
      }
      const name = `Recorded macro ${data.macros.length + 1}`;
      data.macros.push(defaultMacro({ name, steps }));
      changed();
      hud("macro", `Saved ${name}`);
      toast(`Saved ${name}. Find it in Macros.`);
      return name;
    }

    /** Stops every macro and the auto-clicker (and a macro recording). */
    async function stopAll() {
      for (const ctl of running.values()) ctl.stop = true;
      if (clicker) clicker.stop = true;
      if (recording) await finishRecording();
    }

    // ── Status, hotkeys, watching ──

    const busy = () => running.size > 0 || !!clicker || !!recording;

    function statusInfo() {
      return { running: [...running.keys()].sort(), clicking: !!clicker, recording: !!recording };
    }

    function hotkeyList() {
      const out = [];
      for (const m of data.macros) if (m.hotkey) out.push({ id: "m:" + m.id, keys: m.hotkey, gameOnly: !!m.game_only });
      const a = data.autoclicker;
      if (a.enabled && a.hotkey) out.push({ id: "clicker", keys: a.hotkey, gameOnly: !!a.game_only });
      if (data.settings.record_hotkey) out.push({ id: "record", keys: data.settings.record_hotkey, gameOnly: false });
      if (busy() && data.settings.stop_hotkey) out.push({ id: "stop", keys: data.settings.stop_hotkey, gameOnly: false });
      return out;
    }

    function syncHotkeys() {
      chain = chain
        .then(async () => {
          const list = hotkeyList();
          const sig = JSON.stringify(list);
          if (sig === lastHotkeySig) return;
          lastHotkeySig = sig;
          let problems;
          try {
            problems = await call("hotkeys.set", list);
          } catch (e) {
            lastHotkeySig = null;
            throw e;
          }
          const text = Array.isArray(problems) ? problems.join(" ") : "";
          if (text && text !== lastHotkeyProblems) toast(text);
          lastHotkeyProblems = text;
        })
        .catch(report);
      return chain;
    }

    function syncWatch() {
      chain = chain
        .then(async () => {
          const want = !!clicker && (data.autoclicker.mode === "MouseHeld" || data.autoclicker.mode === "OnClick");
          if (want === watching) return;
          watching = want;
          if (!want) physical.clear();
          try {
            await call("input.watch", want);
          } catch (e) {
            watching = !want;
            throw e;
          }
        })
        .catch(report);
      return chain;
    }

    function syncStatus() {
      let text = null;
      if (recording) text = "Recording a macro";
      else if (running.size) text = "Macro running";
      else if (clicker) text = "Auto-clicking";
      return Promise.resolve(call("status", text ? { text, active: true } : null)).catch(() => {});
    }

    function stateMessage() {
      return { type: "state", macros: data.macros, settings: data.settings, autoclicker: data.autoclicker, ...statusInfo() };
    }
    function broadcast() {
      return Promise.resolve(call("send", stateMessage())).catch(() => {});
    }

    /** Running state changed. */
    function busyChanged() {
      syncStatus();
      syncHotkeys();
      syncWatch();
      broadcast();
    }

    /** The data changed. */
    function changed() {
      persist();
      syncHotkeys();
      syncWatch();
      broadcast();
    }

    function persist() {
      const text = JSON.stringify({ macros: data.macros, settings: data.settings, autoclicker: data.autoclicker }, null, 2);
      saveChain = saveChain.then(() => call("storage.write", "macros.json", text)).catch(report);
      return saveChain;
    }

    // ── Events ──

    async function safe(fn) {
      try {
        return await fn();
      } catch (e) {
        report(e);
        return null;
      }
    }

    function onHotkey(ev) {
      if (!ev || typeof ev.id !== "string") return Promise.resolve();
      return safe(async () => {
        const id = ev.id;
        if (!ev.pressed) {
          // Let go: stops what runs only while the key is held.
          if (id.startsWith("m:")) {
            const mid = id.slice(2);
            const m = findMacro(mid);
            if (m && m.repeat === "WhileHeld" && running.has(mid)) running.get(mid).stop = true;
          } else if (id === "clicker" && data.autoclicker.mode === "Hold" && clicker) {
            clicker.stop = true;
          }
          return;
        }
        if (id.startsWith("m:")) {
          const m = findMacro(id.slice(2));
          if (!m) return;
          await toggleMacro(m);
        } else if (id === "clicker") {
          await toggleClicker();
        } else if (id === "record") {
          await toggleRecording();
        } else if (id === "stop") {
          await stopAll();
        }
      });
    }

    function onInput(ev) {
      if (!ev || typeof ev.code !== "string") return;
      if (ev.down) {
        if (!physical.has(ev.code)) {
          physical.add(ev.code);
          if (ev.code.startsWith("Mouse")) seenPresses.set(ev.code, (seenPresses.get(ev.code) || 0) + 1);
        }
      } else physical.delete(ev.code);
    }

    const send = (msg) => Promise.resolve(call("send", msg)).catch(() => {});

    /** Messages from the page: it asks for state and sends edits. */
    function onMessage(msg) {
      if (!isObj(msg)) return Promise.resolve();
      return safe(async () => {
        switch (msg.type) {
          case "get":
            await broadcast();
            break;
          case "saveMacro": {
            const m = normalizeMacro(msg.macro);
            const i = data.macros.findIndex((x) => x.id === m.id);
            if (i >= 0) data.macros[i] = m;
            else data.macros.push(m);
            changed();
            break;
          }
          case "deleteMacro": {
            const ctl = running.get(msg.id);
            if (ctl) ctl.stop = true;
            data.macros = data.macros.filter((m) => m.id !== msg.id);
            changed();
            break;
          }
          case "settings":
            if (isObj(msg.patch)) {
              data.settings = { ...data.settings, ...msg.patch };
              changed();
            }
            break;
          case "autoclicker":
            if (isObj(msg.patch)) {
              data.autoclicker = { ...data.autoclicker, ...msg.patch };
              changed();
            }
            break;
          case "run": {
            const m = findMacro(msg.id);
            if (!m) throw new Error("That macro isn't there anymore.");
            await toggleMacro(m);
            break;
          }
          case "stopAll":
            await stopAll();
            break;
          case "toggleRecord":
            await toggleRecording();
            break;
          case "toggleClicker":
            await toggleClicker();
            break;
          case "importAhk": {
            const imported = ahkImport(String(msg.text ?? ""));
            if (!imported.steps.length) {
              await send({ type: "imported", error: "There was nothing Pious could turn into steps in that script." });
              break;
            }
            const m = defaultMacro({ name: String(msg.name || "AutoHotkey macro"), hotkey: imported.hotkey || "", steps: imported.steps });
            data.macros.push(m);
            changed();
            await send({ type: "imported", id: m.id, skipped: imported.skipped });
            break;
          }
          case "exportAhk": {
            const m = findMacro(msg.id);
            if (!m) throw new Error("That macro isn't there anymore.");
            await send({ type: "ahk", id: m.id, name: m.name, text: ahkExport(m) });
            break;
          }
        }
      });
    }

    // ── Requests Pious sends (docs/PLUGIN-API.md, feature "macros") ──

    const arg0 = (a) => (Array.isArray(a) ? a[0] : a) ?? {};
    const handlers = {
      list: () => data.macros.map((m) => ({ id: m.id, name: m.name, hotkey: m.hotkey, steps: countSteps(m.steps), running: running.has(m.id) })),
      run: async (a) => {
        const q = arg0(a);
        let m;
        if (q.id) m = findMacro(q.id);
        else if (q.name) {
          const n = String(q.name).trim().toLowerCase();
          m = data.macros.find((x) => x.name.toLowerCase() === n);
        }
        if (!m) throw new Error(q.name ? `No macro called "${q.name}".` : "That macro isn't there anymore.");
        const r = await toggleMacro(m);
        return r.started ? `Started ${m.name}` : `Stopped ${m.name}`;
      },
      stop: async () => {
        await stopAll();
        return "Stopped";
      },
      status: () => statusInfo(),
    };

    // ── Start ──

    async function load() {
      let text = null;
      try {
        text = await call("storage.read", "macros.json");
      } catch (e) {
        report(e);
      }
      let raw = null;
      if (text) {
        try {
          raw = JSON.parse(text);
        } catch (e) {
          report(new Error("macros.json couldn't be read, so Macros started empty."));
        }
      }
      data = normalizeData(raw);
    }

    async function start() {
      P.on("hotkey", (ev) => void onHotkey(ev));
      P.on("input", (ev) => onInput(ev));
      P.on("message", (msg) => void onMessage(msg));
      for (const [name, fn] of Object.entries(handlers)) P.handle(name, (a) => fn(a));
      await load();
      await syncHotkeys();
      await syncStatus();
      await broadcast();
    }

    return {
      start,
      handlers,
      onHotkey,
      onInput,
      onMessage,
      toggleMacro,
      toggleClicker,
      toggleRecording,
      finishRecording,
      stopAll,
      statusInfo,
      hotkeyList,
      load,
      get data() {
        return data;
      },
      set data(v) {
        data = normalizeData(v);
      },
      flush: async () => {
        await chain;
        await saveChain;
      },
    };
  }

  const api = {
    createEngine,
    defaultMacro,
    defaultSettings,
    defaultAutoclicker,
    normalizeData,
    normalizeMacro,
    eventsToSteps,
    simplify,
    ahkImport,
    ahkExport,
    resolveTargets,
    parseColor,
    keyLabel,
    hotkeyCodes,
    countSteps,
  };

  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  } else if (typeof pious !== "undefined") {
    const engine = createEngine(pious);
    engine.start().catch((e) => {
      try {
        pious.call("toast", String((e && e.message) || e).slice(0, 190)).catch(() => {});
      } catch {
        /* nothing more to do */
      }
    });
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
