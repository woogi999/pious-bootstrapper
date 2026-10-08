// Calls into the Rust backend, plus the launch flow that turns a backend
// answer ("ask first", "sign in again") into the right dialog.

import { invoke } from "@tauri-apps/api/core";
import { app } from "./state.svelte";
import type { LaunchOutcome, LaunchPlan, Uuid } from "./types";

export const call = invoke;

/** Runs a command, showing any error as a toast instead of throwing. */
export async function run<T>(command: string, args?: Record<string, unknown>): Promise<T | undefined> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    app.toast("negative", String(error));
    return undefined;
  }
}

/** Runs whenever a launch actually starts, including after a "Join
 * anyway?" question. The overlay uses it to get out of the way only once
 * nothing is left to answer. */
export const launchHooks: { started: (() => void) | null } = { started: null };

/** Follows up on what the backend said about a launch. `retry` launches
 * again, skipping the checks the user just confirmed. */
export function handleLaunch(outcome: LaunchOutcome | undefined, retry?: () => Promise<LaunchOutcome | undefined>) {
  if (!outcome) return;
  switch (outcome.kind) {
    case "started":
      launchHooks.started?.();
      break;
    case "confirm":
      if (retry) app.confirm(outcome.title, outcome.body, outcome.confirm, async () => handleLaunch(await retry()));
      break;
    case "sign_in": {
      const account = app.snap?.bootstrapper.accounts.find((a) => a.id === outcome.account);
      app.toast("caution", `${account ? label(account) : "That account"} needs to sign in again before launching.`);
      app.modal = { kind: "add_account", reauth: outcome.account };
      break;
    }
    case "add_account":
      app.toast("caution", "Add an account before launching a game.");
      app.modal = { kind: "add_account", reauth: null };
      break;
  }
}

function label(a: { alias: string | null; display_name: string }) {
  return a.alias ?? a.display_name;
}

export async function launch(plan: LaunchPlan) {
  const once = (force: boolean) => run<LaunchOutcome>("launch", { plan: { ...plan, force } });
  handleLaunch(await once(false), () => once(true));
}

/** Play with the game's setup. */
export async function play(game: Uuid) {
  const once = (force: boolean) => run<LaunchOutcome>("play", { game, force });
  handleLaunch(await once(false), () => once(true));
}

export async function joinServer(server: Uuid) {
  const once = (force: boolean) => run<LaunchOutcome>("join_server", { server, force });
  handleLaunch(await once(false), () => once(true));
}

export async function serverHop(target: { game?: Uuid; instance?: Uuid }) {
  const once = (force: boolean) => run<LaunchOutcome>("server_hop", { game: target.game ?? null, instance: target.instance ?? null, force });
  handleLaunch(await once(false), () => once(true));
}

/** Opens the "Play with…" dialog, prefilled from the game's setup. */
export function openLaunch(game: Uuid | null, opts: { account?: Uuid; version?: string; server?: Uuid } = {}) {
  const snap = app.snap;
  if (!snap) return;
  if (!snap.bootstrapper.accounts.length) {
    app.toast("caution", "Add an account before launching a game.");
    app.modal = { kind: "add_account", reauth: null };
    return;
  }
  const config = game ? snap.effective[game] : null;
  app.modal = {
    kind: "launch",
    mode: "launch",
    game,
    pickGame: !game || !!opts.account || !!opts.version,
    account: opts.account ?? config?.account ?? snap.active_account,
    version: opts.version ? { Specific: opts.version } : (config?.version ?? "Default"),
    server: opts.server ? { Private: opts.server } : (config?.server ?? "Public"),
  };
}

/** The most recently played game, to prefill launch dialogs. */
export function lastGame(): Uuid | null {
  const games = app.snap?.bootstrapper.games.filter((g) => g.last_played) ?? [];
  games.sort((a, b) => (b.last_played! > a.last_played! ? 1 : -1));
  return games[0]?.id ?? null;
}

/** Asks before closing a running instance when the user wants that. */
export function closeInstance(id: Uuid) {
  const snap = app.snap;
  const instance = snap?.instances.find((i) => i.id === id);
  const go = () => run("close_instance", { instance: id });
  if (!snap || !instance || !snap.bootstrapper.preferences.confirm_close || instance.status !== "running") {
    go();
    return;
  }
  const game = snap.bootstrapper.games.find((g) => g.id === instance.game)?.name ?? "Roblox";
  const account = snap.bootstrapper.accounts.find((a) => a.id === instance.account);
  app.confirm(
    "Close instance?",
    `${game} running as ${account ? label(account) : "your browser's account"} will be closed. Unsaved progress may be lost.`,
    "Close",
    go,
  );
}

export function closeAll() {
  const snap = app.snap;
  if (!snap?.instances.length) return;
  const go = () => run("close_all");
  if (snap.bootstrapper.preferences.confirm_close) {
    app.confirm("Close all instances?", `All ${snap.instances.length} running Roblox instances will be closed.`, "Close all", go);
  } else {
    go();
  }
}

export function setPreferences(patch: Record<string, unknown>) {
  return run("update_preferences", { patch });
}
