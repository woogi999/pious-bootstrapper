// The bridge between a plugin's frame (its page, or its engine in the
// hidden plugin-host window) and Pious. The frame only posts messages; this
// side knows which plugin it is, checks what the plugin may do, and passes
// calls to Rust (`plugin_call`) or handles the few that belong to the
// interface. Events from Rust ("plugin-event") are passed on to the frame.
// See docs/PLUGIN-API.md.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { app, type Page } from "./state.svelte";
import { handleLaunch, play, run } from "./api";
import { icon } from "./icons";
import type { LaunchOutcome } from "./types";

export interface BridgedPlugin {
  id: string;
  name: string;
  permissions: string[];
}

/** Methods the interface answers itself, and the permission each needs. */
const LOCAL: Record<string, string> = {
  snapshot: "read",
  launch: "launch",
  join: "launch",
  toast: "notify",
  navigate: "navigate",
  runMacro: "macros",
  openLink: "links",
  "app.invoke": "full",
  // Open to every plugin ("" = no permission needed).
  "ui.icons": "",
};

/** The address of a plugin's page or engine. Unlike `convertFileSrc`, which
 *  encodes the whole path as one segment, the folders stay folders, so the
 *  page's own relative files (`pious-plugin.js`, `ui.js`, `style.css`) load
 *  from its folder instead of the protocol's root. */
export function pageSrc(path: string): string {
  return convertFileSrc("") + path.split(/[\\/]/).map(encodeURIComponent).join("/");
}

/** What a plugin with "read" sees: no tokens, no file paths. */
export function pluginView() {
  const s = app.snap;
  if (!s) return null;
  return {
    games: s.bootstrapper.games.filter((g) => g.in_library).map((g) => ({ id: g.id, name: g.name, place_id: g.place_id, favorite: g.favorite, last_played: g.last_played })),
    accounts: s.bootstrapper.accounts.map((a) => ({ id: a.id, username: a.username, display_name: a.display_name, nickname: a.alias })),
    friends: s.friends.list.map((f) => ({ id: f.id, username: f.username, display_name: f.display_name, status: f.status, playing: f.location, place_id: f.place_id })),
    running: s.instances.map((i) => ({ id: i.id, game: s.bootstrapper.games.find((g) => g.id === i.game)?.name ?? null, started: i.started })),
  };
}

/** Every variable Pious's look is made of (app.css), as the window has them
 *  now, so pious-ui.css in a plugin page matches the user's theme. */
const THEME_VARS = [
  "--bg", "--surface", "--text", "--muted", "--faint", "--accent", "--accent-hover", "--accent-pressed", "--accent-ink",
  "--panel", "--rim", "--glass", "--radius", "--font-scale", "--ui-font", "--accent-2",
];

function themeVars() {
  const style = getComputedStyle(document.documentElement);
  const vars: Record<string, string> = {};
  for (const name of THEME_VARS) {
    const value = style.getPropertyValue(name).trim();
    if (value) vars[name] = value;
  }
  return vars;
}

/**
 * Connects one frame. `visible`: a page in the main window (dialogs and
 * toasts show there); false for an engine in the hidden host window.
 * Returns a function that disconnects it.
 */
export function bridge(plugin: () => BridgedPlugin | undefined, side: "page" | "engine", frame: () => HTMLIFrameElement | undefined) {
  let subscribed = false;
  // The theme last sent, so a change of theme reaches open pages too.
  let theme = "";
  const post = (message: unknown) => frame()?.contentWindow?.postMessage(message, "*");
  const sendTheme = () => {
    const vars = themeVars();
    theme = JSON.stringify(vars);
    post({ pious: "theme", vars });
  };
  const visible = side === "page";

  const toast = (text: string) => {
    const p = plugin();
    const message = `${p?.name ?? "A plugin"}: ${text.slice(0, 200)}`;
    if (visible) app.toast("neutral", message);
    else emitTo("main", "toast", { tone: "neutral", message }).catch(() => {});
  };

  async function local(p: BridgedPlugin, method: string, args: unknown[]): Promise<unknown> {
    switch (method) {
      case "snapshot":
        return pluginView();
      case "launch":
        if (visible) await play(String(args[0]));
        else return run<LaunchOutcome>("play", { game: String(args[0]), force: true });
        return true;
      case "join": {
        const once = (force: boolean) => run<LaunchOutcome>("join_player", { place: Number(args[0]), job: null, account: null, name: null, force });
        if (visible) handleLaunch(await once(false), () => once(true));
        else return once(true);
        return true;
      }
      case "toast":
        toast(String(args[0]));
        return true;
      case "navigate":
        if (visible) app.navigate({ name: String(args[0]) } as Page);
        else {
          await invoke("show_main");
          await emitTo("main", "plugin-navigate", String(args[0]));
        }
        return true;
      case "runMacro":
        return invoke("plugin_request", { feature: "macros", method: "run", args: { name: String(args[0]) } });
      case "openLink":
        await invoke("open_url", { url: String(args[0]) });
        return true;
      case "app.invoke":
        // Full access: any of Pious's own commands.
        return invoke(String(args[0]), (args[1] ?? {}) as Record<string, unknown>);
      case "ui.icons": {
        const names = Array.isArray(args[0]) ? args[0].map(String).slice(0, 200) : [];
        return Object.fromEntries(names.map((name) => [name, icon(name)]));
      }
    }
    throw new Error(`${p.name} can't use ${method} here.`);
  }

  async function handle(method: string, args: unknown[]): Promise<unknown> {
    const p = plugin();
    if (!p) throw new Error("This plugin is off.");
    if (method in LOCAL) {
      const needed = LOCAL[method];
      if (needed && !p.permissions.includes(needed) && !p.permissions.includes("full")) {
        throw new Error(`${p.name} didn't ask for the ${needed} permission.`);
      }
      return local(p, method, args);
    }
    return invoke("plugin_call", { plugin: p.id, side, method, args });
  }

  function onmessage(event: MessageEvent) {
    const f = frame();
    if (!f || event.source !== f.contentWindow) return;
    const m = event.data;
    if (!m || typeof m !== "object") return;
    if (m.pious === "ready") {
      sendTheme();
    } else if (m.pious === "call") {
      handle(String(m.method), Array.isArray(m.args) ? m.args : [])
        .then((value) => post({ pious: "reply", id: m.id, value: value ?? null }))
        .catch((e) => post({ pious: "reply", id: m.id, error: String(e?.message ?? e) }));
    } else if (m.pious === "subscribe" && m.event === "snapshot" && plugin()?.permissions.some((x) => x === "read" || x === "full")) {
      subscribed = true;
      post({ pious: "event", event: "snapshot", value: pluginView() });
    }
  }

  window.addEventListener("message", onmessage);
  const off = listen<{ plugin: string; to: string; event: string; value: unknown }>("plugin-event", (e) => {
    const { plugin: id, to, event, value } = e.payload;
    if (id === plugin()?.id && (to === side || to === "both")) post({ pious: "event", event, value });
  });
  return {
    /** The app's state changed: plugins following it get the new summary. */
    snapshotChanged() {
      if (subscribed) post({ pious: "event", event: "snapshot", value: pluginView() });
      if (theme && JSON.stringify(themeVars()) !== theme) sendTheme();
    },
    disconnect() {
      window.removeEventListener("message", onmessage);
      off.then((f) => f());
    },
  };
}
