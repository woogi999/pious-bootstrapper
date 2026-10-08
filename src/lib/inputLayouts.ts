// The input overlay's layouts and how keys are labeled.

import type { InputOverlay } from "./types";

/** A key in a layout: its name ("KeyW"), or "" for an empty gap. */
export interface LayoutKey {
  code: string;
  /** Width in key widths. */
  width: number;
}

const WIDTHS: Record<string, number> = {
  Space: 3.2,
  ShiftLeft: 1.6,
  ShiftRight: 1.9,
  ControlLeft: 1.3,
  ControlRight: 1.3,
  AltLeft: 1.2,
  Tab: 1.4,
  CapsLock: 1.7,
  Enter: 1.9,
  Backspace: 1.9,
};

const row = (codes: string): LayoutKey[] =>
  codes
    .split(" ")
    .filter((c) => c.length)
    .map((c) => (c === "_" ? { code: "", width: 1 } : { code: c, width: WIDTHS[c] ?? 1 }));

export const PRESETS: Record<Exclude<InputOverlay["layout"], "custom">, { label: string; rows: LayoutKey[][] }> = {
  streamer: {
    label: "Streamer",
    rows: [
      row("Backquote Digit1 Digit2 Digit3 Digit4 Digit5"),
      row("Tab KeyQ KeyW KeyE KeyR KeyT"),
      row("_ KeyA KeyS KeyD KeyF KeyG"),
      row("ShiftLeft KeyZ KeyX KeyC KeyV KeyB"),
      row("ControlLeft _ AltLeft Space"),
    ],
  },
  wasd: {
    label: "WASD",
    rows: [row("_ KeyW _"), row("KeyA KeyS KeyD"), row("ShiftLeft Space")],
  },
  fps: {
    label: "Shooter",
    rows: [row("Digit1 Digit2 Digit3 Digit4"), row("Tab KeyQ KeyW KeyE KeyR"), row("ShiftLeft KeyA KeyS KeyD KeyF"), row("ControlLeft Space")],
  },
  keyboard: {
    label: "Whole keyboard",
    rows: [
      row("Escape Digit1 Digit2 Digit3 Digit4 Digit5 Digit6 Digit7 Digit8 Digit9 Digit0 Backspace"),
      row("Tab KeyQ KeyW KeyE KeyR KeyT KeyY KeyU KeyI KeyO KeyP"),
      row("CapsLock KeyA KeyS KeyD KeyF KeyG KeyH KeyJ KeyK KeyL Enter"),
      row("ShiftLeft KeyZ KeyX KeyC KeyV KeyB KeyN KeyM ShiftRight"),
      row("ControlLeft AltLeft Space"),
    ],
  },
  mouse: { label: "Only the mouse", rows: [] },
  none: { label: "No keys", rows: [] },
};

/** Layouts where every key is one size, apart from the space bar. */
const SAME_SIZE = new Set(["streamer"]);

/** The rows the overlay draws. */
export function layoutRows(o: InputOverlay): LayoutKey[][] {
  if (o.layout === "custom") return o.rows.map((r) => r.map((code) => ({ code, width: WIDTHS[code] ?? 1 })));
  const rows = PRESETS[o.layout]?.rows ?? PRESETS.streamer.rows;
  return SAME_SIZE.has(o.layout) ? rows.map((r) => r.map((k) => ({ ...k, width: k.code === "Space" ? 3 : 1 }))) : rows;
}

const NAMES: Record<string, string> = {
  Space: "Space",
  ShiftLeft: "Shift",
  ShiftRight: "Shift",
  ControlLeft: "Ctrl",
  ControlRight: "Ctrl",
  AltLeft: "Alt",
  AltRight: "Alt",
  MetaLeft: "Win",
  Tab: "Tab",
  CapsLock: "Caps",
  Escape: "Esc",
  Enter: "Enter",
  Backspace: "⌫",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  MouseLeft: "LMB",
  MouseRight: "RMB",
  MouseMiddle: "MMB",
  MouseBack: "M4",
  MouseForward: "M5",
  WheelUp: "Wheel ↑",
  WheelDown: "Wheel ↓",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Backquote: "~",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

/** "KeyW" → "W", "Digit1" → "1", "MouseBack" → "M4". */
export function keyName(code: string): string {
  if (NAMES[code]) return NAMES[code];
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `Num ${code.slice(6)}`;
  return code;
}

/** Every key and button a remap or a custom layout can use. */
export const ALL_CODES: string[] = [
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("").map((c) => `Key${c}`),
  ..."1234567890".split("").map((d) => `Digit${d}`),
  ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
  "Space",
  "Enter",
  "Tab",
  "Escape",
  "Backspace",
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "CapsLock",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Insert",
  "Delete",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Minus",
  "Equal",
  "BracketLeft",
  "BracketRight",
  "Backslash",
  "Semicolon",
  "Quote",
  "Backquote",
  "Comma",
  "Period",
  "Slash",
  ..."0123456789".split("").map((d) => `Numpad${d}`),
  "MouseLeft",
  "MouseRight",
  "MouseMiddle",
  "MouseBack",
  "MouseForward",
];
