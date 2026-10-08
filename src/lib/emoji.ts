// Discord-style emoji shortcodes for text boxes: typing `:so` shows a list
// of matching emoji above the box (arrows choose, Enter or Tab picks, Esc
// closes), and a finished `:sob:` turns into 😭 by itself.

import table from "./emoji-shortcodes.json";

const ENTRIES = table as [string, string][];
const BY_NAME = new Map(ENTRIES);
const LIMIT = 8;

/** Emoji whose shortcode starts with `query`, then ones that contain it. */
export function searchEmoji(query: string, limit = LIMIT): [string, string][] {
  const q = query.toLowerCase();
  if (!q) return [];
  const starts: [string, string][] = [];
  const contains: [string, string][] = [];
  for (const entry of ENTRIES) {
    if (entry[0].startsWith(q)) starts.push(entry);
    else if (entry[0].includes(q)) contains.push(entry);
    if (starts.length >= limit) break;
  }
  return [...starts, ...contains].slice(0, limit);
}

/** The emoji for an exact shortcode, without colons. */
export function emojiFor(name: string): string | undefined {
  return BY_NAME.get(name.toLowerCase());
}

type Field = HTMLInputElement | HTMLTextAreaElement;

const STYLE = `
.emoji-list{position:fixed;z-index:9999;min-width:220px;max-width:320px;padding:4px;border-radius:10px;
  background:rgb(var(--panel,18 18 20)/0.97);border:1px solid rgb(var(--surface,255 255 255)/0.1);
  box-shadow:0 12px 32px rgb(0 0 0/0.45);backdrop-filter:blur(18px);font-size:13px;color:rgb(var(--text,237 237 239));
  animation:emoji-in 140ms cubic-bezier(.22,1,.36,1)}
.emoji-list .head{padding:4px 8px 6px;font-size:10.5px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;color:rgb(var(--faint,140 140 150))}
.emoji-list button{display:flex;align-items:center;gap:10px;width:100%;padding:5px 8px;border:none;border-radius:7px;background:none;color:inherit;font:inherit;text-align:left;cursor:pointer}
.emoji-list button.on{background:rgb(var(--surface,255 255 255)/0.1)}
.emoji-list .glyph{font-size:18px;width:22px;text-align:center}
.emoji-list .name{opacity:.85}
@keyframes emoji-in{from{opacity:0;transform:translateY(4px)}}`;

function ensureStyle() {
  if (document.getElementById("emoji-list-style")) return;
  const style = document.createElement("style");
  style.id = "emoji-list-style";
  style.textContent = STYLE;
  document.head.appendChild(style);
}

/** A Svelte action: `<textarea use:emojiShortcodes={on}>`. */
export function emojiShortcodes(field: Field, enabled: boolean = true) {
  let on = enabled;
  let list: HTMLDivElement | null = null;
  let matches: [string, string][] = [];
  let selected = 0;
  /** Where the `:` of the shortcode being typed is. */
  let start = -1;

  const close = () => {
    list?.remove();
    list = null;
    matches = [];
    start = -1;
  };

  /** Puts `emoji` in place of the text from `from` to the caret. */
  const replace = (from: number, emoji: string) => {
    const caret = field.selectionStart ?? field.value.length;
    field.value = field.value.slice(0, from) + emoji + field.value.slice(caret);
    const at = from + emoji.length;
    field.setSelectionRange(at, at);
    // Tell bound values (bind:value) about the change.
    field.dispatchEvent(new Event("input", { bubbles: true }));
  };

  const pick = (i: number) => {
    const match = matches[i];
    const from = start;
    close();
    if (match && from >= 0) replace(from, match[1]);
    field.focus();
  };

  const draw = () => {
    if (!matches.length) return close();
    ensureStyle();
    if (!list) {
      list = document.createElement("div");
      list.className = "emoji-list";
      document.body.appendChild(list);
      // Clicking the list doesn't take focus from the box.
      list.addEventListener("mousedown", (e) => e.preventDefault());
    }
    list.replaceChildren();
    const head = document.createElement("div");
    head.className = "head";
    head.textContent = "Emoji matching :" + field.value.slice(start + 1, field.selectionStart ?? undefined);
    list.appendChild(head);
    matches.forEach(([name, emoji], i) => {
      const button = document.createElement("button");
      button.type = "button";
      if (i === selected) button.className = "on";
      const glyph = document.createElement("span");
      glyph.className = "glyph";
      glyph.textContent = emoji;
      const label = document.createElement("span");
      label.className = "name";
      label.textContent = `:${name}:`;
      button.append(glyph, label);
      button.addEventListener("click", () => pick(i));
      list!.appendChild(button);
    });
    // Just above the box.
    const rect = field.getBoundingClientRect();
    const height = list.offsetHeight;
    list.style.left = `${Math.max(8, rect.left)}px`;
    list.style.top = `${Math.max(8, rect.top - height - 6)}px`;
  };

  const onInput = () => {
    if (!on) return;
    const caret = field.selectionStart ?? field.value.length;
    const before = field.value.slice(0, caret);
    // A finished :shortcode: becomes its emoji.
    const done = /:([a-z0-9_+-]{1,40}):$/i.exec(before);
    const emoji = done && emojiFor(done[1]);
    if (done && emoji) {
      close();
      replace(caret - done[0].length, emoji);
      return;
    }
    // A shortcode being typed: a colon (at the start or after a space)
    // and at least two characters.
    const typing = /(^|\s):([a-z0-9_+-]{2,40})$/i.exec(before);
    if (!typing) return close();
    start = caret - typing[2].length - 1;
    const found = searchEmoji(typing[2]);
    if (found.map((m) => m[0]).join() !== matches.map((m) => m[0]).join()) selected = 0;
    matches = found;
    draw();
  };

  const onKey = (event: Event) => {
    const e = event as KeyboardEvent;
    if (!list || !matches.length) return;
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
    };
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      selected = (selected + (e.key === "ArrowDown" ? 1 : matches.length - 1)) % matches.length;
      draw();
      handled();
    } else if ((e.key === "Enter" || e.key === "Tab") && !e.shiftKey) {
      handled();
      pick(selected);
    } else if (e.key === "Escape") {
      handled();
      close();
    }
  };

  field.addEventListener("input", onInput);
  // Capture, so picking with Enter never also sends the message.
  field.addEventListener("keydown", onKey, true);
  field.addEventListener("blur", close);
  return {
    update(next: boolean = true) {
      on = next;
      if (!on) close();
    },
    destroy() {
      close();
      field.removeEventListener("input", onInput);
      field.removeEventListener("keydown", onKey, true);
      field.removeEventListener("blur", close);
    },
  };
}
