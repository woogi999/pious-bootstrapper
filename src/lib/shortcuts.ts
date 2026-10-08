// Keyboard shortcuts inside Pious (the main window and the overlay). Each
// can be changed in Settings → Keyboard; global, in-game hotkeys live
// there too and run in Rust.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { app, NAV, PAGE_TITLES, type Page } from "./state.svelte";
import { launch, lastGame, play, setPreferences } from "./api";
import { fromEvent } from "./keys";

export interface Shortcut {
  id: string;
  label: string;
  /** The default keys, as `fromEvent` spells them. */
  keys: string;
}

const PAGE_KEYS = ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0"];

/** Every shortcut, in the order Settings lists them. */
export const SHORTCUTS: Shortcut[] = [
  { id: "search", label: "Search everything", keys: "Control+KeyK" },
  { id: "settings", label: "Settings", keys: "Control+Comma" },
  { id: "sidebar", label: "Collapse the sidebar", keys: "Control+KeyB" },
  { id: "play_last", label: "Play your last game", keys: "Control+KeyP" },
  { id: "add_game", label: "Add a game", keys: "Control+KeyN" },
  { id: "add_account", label: "Add an account", keys: "Control+Shift+KeyN" },
  { id: "join_player", label: "Join a player", keys: "Control+KeyJ" },
  { id: "join_link", label: "Join a private server link", keys: "Control+KeyL" },
  { id: "chat", label: "Open chat", keys: "Control+Shift+KeyM" },
  { id: "refresh", label: "Refresh the page", keys: "Control+KeyR" },
  { id: "pin", label: "Keep on top", keys: "Control+KeyT" },
  { id: "record", label: "Start or stop recording", keys: "Control+Shift+KeyR" },
  { id: "streamer", label: "Streamer mode", keys: "Control+Shift+KeyS" },
  { id: "stop_macros", label: "Stop macros and the auto-clicker", keys: "Control+Shift+KeyX" },
  { id: "fullscreen", label: "Fullscreen", keys: "F11" },
  { id: "back", label: "Back", keys: "Alt+ArrowLeft" },
  ...NAV.map((page, i) => ({ id: `page:${page}`, label: `Go to ${PAGE_TITLES[page]}`, keys: PAGE_KEYS[i] ? `Control+${PAGE_KEYS[i]}` : "" })),
  // A plugin that starts off, so it has no key until one is set.
  { id: "page:macros", label: "Go to Macros", keys: "" },
];

/** Keys that always work and can't be changed. */
export const FIXED: [string, string][] = [
  ["Esc", "Close menus and dialogs"],
  ["/", "Search (when not typing)"],
];

/** The keys a shortcut uses now ("" = turned off). */
export function keysFor(id: string): string {
  const changed = app.snap?.bootstrapper.preferences.shortcuts[id];
  return changed ?? SHORTCUTS.find((s) => s.id === id)?.keys ?? "";
}

const typingIn = (target: EventTarget | null) =>
  target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable);

export async function toggleFullscreen() {
  try {
    app.fullscreen = await invoke<boolean>("toggle_fullscreen");
  } catch {
    const win = getCurrentWindow();
    app.fullscreen = !(await win.isFullscreen());
    await win.setFullscreen(app.fullscreen);
  }
}

/** Runs what a shortcut (or the logo, or a plugin) asks for. */
export function runAction(id: string, embedded = false): boolean {
  const snap = app.snap;
  if (!snap) return false;
  const free = !app.modal && !app.searchOpen;
  if (id.startsWith("page:")) {
    if (!free) return false;
    app.navigate({ name: id.slice(5) } as Page);
    return true;
  }
  if (id.startsWith("macro:")) {
    invoke("run_macro", { id: id.slice(6) }).catch((e) => app.toast("caution", String(e)));
    return true;
  }
  switch (id) {
    case "search":
      if (app.modal) return false;
      app.searchOpen = true;
      return true;
    case "home":
      if (!free) return false;
      app.navigate({ name: "home" });
      return true;
    case "settings":
      if (!free) return false;
      app.navigate({ name: "settings" });
      return true;
    case "sidebar":
      if (app.modal) return false;
      setPreferences({ sidebar_collapsed: !snap.bootstrapper.preferences.sidebar_collapsed });
      return true;
    case "play_last": {
      if (!free) return false;
      const game = lastGame();
      if (game) play(game);
      else app.modal = { kind: "add_game" };
      return true;
    }
    case "add_game":
      if (!free) return false;
      app.modal = { kind: "add_game" };
      return true;
    case "add_account":
      if (!free) return false;
      app.modal = { kind: "add_account", reauth: null };
      return true;
    case "join_player":
      if (!free) return false;
      app.modal = { kind: "join_player" };
      return true;
    case "join_link":
      if (!free) return false;
      app.modal = { kind: "join_link" };
      return true;
    case "chat":
      invoke("open_chat_window", { account: snap.friends.account, user: null });
      return true;
    case "refresh":
      if (!free) return false;
      refreshPage();
      return true;
    case "pin":
      if (embedded) return false;
      setPreferences({ pinned: !snap.bootstrapper.preferences.pinned });
      return true;
    case "record":
      invoke("toggle_recording");
      return true;
    case "streamer": {
      const on = !snap.bootstrapper.preferences.streamer_mode;
      setPreferences({ streamer_mode: on });
      app.toast("neutral", on ? "Streamer mode is on: names are hidden" : "Streamer mode is off");
      return true;
    }
    case "stop_macros":
      invoke("stop_automation");
      return true;
    case "fullscreen":
      if (embedded) return false;
      toggleFullscreen();
      return true;
    case "back":
      if (!free) return false;
      app.back();
      return true;
    case "none":
      return true;
    default:
      return false;
  }
}

/** Handles a key press; returns whether it was a shortcut. `embedded`
 * leaves out what only makes sense in the main window. */
export function handleShortcut(event: KeyboardEvent, embedded = false): boolean {
  const done = () => {
    event.preventDefault();
    return true;
  };
  if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    if (app.menu) app.menu = null;
    else if (app.searchOpen) app.searchOpen = false;
    else if (app.modal) app.modal = null;
    else if (app.page.name === "game") app.back();
    else return false;
    return done();
  }
  if (!app.snap) return false;
  // A hotkey box is listening for keys.
  if (document.querySelector(".hotkey.listening")) return false;
  const typing = typingIn(event.target);
  if (event.key === "/" && !typing && !app.modal && !app.searchOpen) {
    app.searchOpen = true;
    return done();
  }
  const combo = fromEvent(event);
  if (!combo) return false;
  // Ctrl F searches too, unless it's been given to something else.
  if (combo === "Control+KeyF" && !SHORTCUTS.some((s) => keysFor(s.id) === combo)) {
    return runAction("search", embedded) ? done() : false;
  }
  // Plain keys (no Ctrl or Alt) don't fire while typing.
  const plain = !event.ctrlKey && !event.altKey && !event.metaKey && !/^F\d+$/.test(event.code);
  if (typing && plain) return false;
  const match = SHORTCUTS.find((s) => keysFor(s.id) === combo);
  if (!match) return false;
  return runAction(match.id, embedded) ? done() : false;
}

function refreshPage() {
  switch (app.page.name) {
    case "friends":
      invoke("refresh_friends", { account: app.snap?.friends.account ?? null });
      break;
    case "versions":
      invoke("scan_versions").catch(() => {});
      invoke("refresh_bootstrappers");
      invoke("load_builds");
      break;
    case "home":
      invoke("refresh_recommendations").catch(() => {});
      invoke("refresh_friends", { account: app.snap?.friends.account ?? null });
      if (app.snap?.bootstrapper.preferences.news_on_home) invoke("refresh_news", { force: true }).catch(() => {});
      break;
    case "news":
      invoke("refresh_news", { force: true }).catch(() => {});
      break;
    default:
      break;
  }
}

export { launch };
