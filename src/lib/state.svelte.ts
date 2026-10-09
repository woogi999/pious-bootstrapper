// The window's state: the latest snapshot from the backend (read-only
// here), plus what only the interface cares about (page, dialogs, menus,
// search, toasts).

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot, Tone, Uuid, GameInfo, PrivateServer, VersionChoice, ServerChoice } from "./types";

export type Page =
  | { name: "home" }
  | { name: "games" }
  | { name: "game"; id: Uuid }
  | { name: "servers" }
  | { name: "accounts" }
  | { name: "friends" }
  | { name: "versions" }
  | { name: "tweaks" }
  | { name: "keybinds" }
  | { name: "macros" }
  | { name: "news" }
  | { name: "running" }
  | { name: "settings" }
  | { name: "help"; doc?: string }
  | { name: "plugin"; id: string };

/** The pages, in the order Ctrl 1–9 and 0 open them (later ones have no key until one is set). */
export const NAV = ["home", "games", "servers", "friends", "accounts", "versions", "tweaks", "news", "running", "settings", "keybinds"] as const;

/** The ID of the Macros plugin (macros and the auto-clicker, in plugins/macros). */
export const MACROS_PLUGIN = "pious.macros";

/** Whether macros are on: the Macros plugin is in the plugins folder and turned on. */
export function macrosOn(): boolean {
  return !!app.snap?.plugins.some((p) => p.provides === "macros" && p.enabled);
}

export const PAGE_TITLES: Record<string, string> = {
  home: "Home",
  games: "Games",
  game: "Games",
  servers: "Private Servers",
  friends: "Friends",
  accounts: "Accounts",
  versions: "Versions",
  tweaks: "Tweaks",
  keybinds: "Keybinds",
  macros: "Macros",
  news: "News",
  running: "Instances",
  settings: "Settings",
  help: "Help",
  plugin: "Plugin",
};

export type SettingsTab = "general" | "appearance" | "overlay" | "recording" | "keyboard" | "plugins" | "about";
export type TweaksTab = "launching" | "presence" | "performance" | "mods" | "flags" | "ingame" | "internet" | "system";

export type Modal =
  | { kind: "add_game" }
  | { kind: "server"; editing: PrivateServer | null; game: Uuid | null }
  | { kind: "add_account"; reauth: Uuid | null }
  | {
      kind: "launch";
      mode: "launch" | "edit";
      game: Uuid | null;
      pickGame: boolean;
      account: Uuid | null;
      version: VersionChoice;
      server: ServerChoice;
    }
  | { kind: "rename"; account: Uuid }
  | { kind: "collection"; game: Uuid }
  | { kind: "install_version" }
  | { kind: "join_player" }
  | { kind: "join_link" }
  | { kind: "servers"; game: Uuid }
  | { kind: "roblox_settings"; account: Uuid | null }
  | { kind: "welcome" }
  | { kind: "whats_new" }
  | { kind: "confirm"; title: string; body: string; confirm: string; action: () => unknown };

export interface MenuItem {
  icon: string;
  label: string;
  action: () => void;
  danger?: boolean;
}
export type Menu = { x: number; y: number; items: (MenuItem | "separator")[] };

export interface ToastItem {
  id: number;
  tone: Tone;
  message: string;
  leaving: boolean;
}

class AppState {
  /** The latest snapshot. Replaced wholesale, never mutated. */
  snap = $state.raw<Snapshot | null>(null);

  page = $state<Page>({ name: "home" });
  history: Page[] = [];
  modal = $state<Modal | null>(null);
  menu = $state<Menu | null>(null);
  searchOpen = $state(false);
  toasts = $state<ToastItem[]>([]);
  maximized = $state(false);
  fullscreen = $state(false);
  /** Bumped on each navigation so lists can replay their entrance. */
  pageKey = $state(0);

  /** Games-page filters (kept while moving between pages). */
  gamesQuery = $state("");
  gamesFilter = $state<"all" | "favorites" | "recent" | `collection:${string}`>("all");
  gamesSort = $state<"recent" | "name" | "added" | "played">("recent");
  serversQuery = $state("");
  serversFavorites = $state(false);
  settingsTab = $state<SettingsTab>("general");
  tweaksTab = $state<TweaksTab>("launching");
  /** The chat open on the Friends page: (conversation, friend's user ID). */
  chat = $state<{ conversation: string; user: number } | null>(null);

  private nextToast = 0;

  navigate(page: Page) {
    if (JSON.stringify(page) === JSON.stringify(this.page)) return;
    this.history.push(this.page);
    if (this.history.length > 32) this.history.shift();
    this.go(page);
  }

  back() {
    this.go(this.history.pop() ?? { name: "home" });
  }

  private go(page: Page) {
    this.menu = null;
    this.page = page;
    this.pageKey++;
    document.getElementById("page-scroll")?.scrollTo({ top: 0 });
    if (page.name === "friends") invoke("refresh_friends", { account: this.snap?.friends.account ?? null });
    if (page.name === "news") invoke("refresh_news", { force: false });
    if (page.name === "versions" || page.name === "settings") {
      invoke("refresh_bootstrappers");
      if (page.name === "versions" && this.snap && !this.snap.builds.length && !this.snap.builds_error) {
        invoke("load_builds");
      }
    }
  }

  toast(tone: Tone, message: string) {
    const id = ++this.nextToast;
    this.toasts.push({ id, tone, message, leaving: false });
    const visible = this.toasts.filter((t) => !t.leaving);
    if (visible.length > 4) this.dismiss(visible[0].id);
    setTimeout(() => this.dismiss(id), 5000);
  }

  dismiss(id: number) {
    const toast = this.toasts.find((t) => t.id === id);
    if (!toast || toast.leaving) return;
    toast.leaving = true;
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), 220);
  }

  openMenu(event: MouseEvent, items: Menu["items"]) {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.max(8, Math.min(event.clientX, window.innerWidth - 230));
    const y = Math.max(8, Math.min(event.clientY, window.innerHeight - 24 - items.length * 32));
    this.menu = { x, y, items };
  }

  confirm(title: string, body: string, confirm: string, action: () => unknown) {
    this.modal = { kind: "confirm", title, body, confirm, action };
  }
}

export const app = new AppState();

/** Grid or list for a page. */
export function viewOf(page: string): "Grid" | "List" {
  return app.snap?.bootstrapper.preferences.views[page] ?? (page === "versions" || page === "running" ? "List" : "Grid");
}

/** How a page is sorted (`fallback` until the user picks). */
export function sortOf(page: string, fallback: string): string {
  return app.snap?.bootstrapper.preferences.sorts[page] ?? fallback;
}

/** Starts listening to the backend. */
export async function connect() {
  // A snapshot event can overtake the reply below; the backend doesn't
  // resend an unchanged snapshot, so the older reply must not replace it.
  let live = false;
  await listen<Snapshot>("snapshot", (event) => {
    live = true;
    app.snap = event.payload;
  });
  await listen<{ tone: Tone; message: string }>("toast", (event) => app.toast(event.payload.tone, event.payload.message));
  const first = await invoke<Snapshot>("get_snapshot");
  if (!live) app.snap = first;
}

export type { GameInfo };
