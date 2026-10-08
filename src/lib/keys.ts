// Key combinations, as Tauri's global shortcuts spell them
// ("Control+Shift+KeyK", "Home", "F12"), and how they read to people.

const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "OS"]);

/** The combination a key press makes, or null for a lone modifier. */
export function fromEvent(event: KeyboardEvent): string | null {
  if (MODIFIERS.has(event.key) || event.code.startsWith("Control") || event.code.startsWith("Shift") || event.code.startsWith("Alt") || event.code.startsWith("Meta")) {
    return null;
  }
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  parts.push(event.code);
  return parts.join("+");
}

const NAMES: Record<string, string> = {
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
export function keyLabel(combo: string): string {
  if (!combo) return "Not set";
  return combo
    .split("+")
    .map((part) => NAMES[part] ?? part.replace(/^Key/, "").replace(/^Digit/, "").replace(/^Numpad/, "Num "))
    .join(" + ");
}

/** Whether a key press matches a combination. */
export function matches(event: KeyboardEvent, combo: string): boolean {
  return !!combo && fromEvent(event) === combo;
}
