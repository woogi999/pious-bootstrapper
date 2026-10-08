// Right-click and "…" menus for each kind of item.

import { app, type Menu } from "./state.svelte";
import { closeInstance, joinServer, launch, lastGame, openLaunch, play, run, serverHop } from "./api";
import { gameUrl } from "./format";
import type { Account, Game, Instance, PrivateServer, Recommendation, Uuid } from "./types";

type Items = Menu["items"];

function guard(title: string, body: string, confirm: string, action: () => void): () => void {
  return () => app.confirm(title, body, confirm, action);
}

export function gameMenu(game: Game): Items {
  const snap = app.snap!;
  const servers = snap.bootstrapper.servers.filter((s) => s.game_id === game.id);
  const best = [...servers].sort(
    (a, b) => Number(b.favorite) - Number(a.favorite) || (b.last_joined ?? "").localeCompare(a.last_joined ?? ""),
  )[0];
  const items: Items = [
    { icon: "play", label: "Play", action: () => play(game.id) },
    { icon: "launch", label: "Play with…", action: () => openLaunch(game.id) },
    { icon: "hop", label: "Join a random server", action: () => serverHop({ game: game.id }) },
    { icon: "server", label: "Browse servers…", action: () => (app.modal = { kind: "servers", game: game.id }) },
  ];
  if (!game.in_library) items.push({ icon: "add", label: "Add to library", action: () => run("add_to_library", { game: game.id }) });
  if (best) items.push({ icon: "server", label: "Join private server", action: () => joinServer(best.id) });
  items.push(
    "separator",
    {
      icon: "heart",
      label: game.favorite ? "Remove from favorites" : "Add to favorites",
      action: () => run("toggle_favorite", { game: game.id }),
    },
    { icon: "folder", label: "Add to collection…", action: () => (app.modal = { kind: "collection", game: game.id }) },
    { icon: "edit", label: "Edit launch configuration", action: () => editConfig(game.id) },
    { icon: "add", label: "Add private server", action: () => (app.modal = { kind: "server", editing: null, game: game.id }) },
    "separator",
    { icon: "info", label: "View details", action: () => app.navigate({ name: "game", id: game.id }) },
    { icon: "external", label: "Open on Roblox", action: () => run("open_url", { url: gameUrl(game.place_id) }) },
    { icon: "refresh", label: "Refresh details", action: () => run("refresh_game", { game: game.id }) },
    { icon: "launch", label: "Make a desktop shortcut", action: () => run("create_shortcut", { game: game.id }) },
    "separator",
    {
      icon: "remove",
      label: game.in_library ? "Remove from library" : "Remove from Recents",
      danger: true,
      action: guard(
        game.in_library ? "Remove game?" : "Remove from Recents?",
        `${game.name} and its saved private servers will be removed. You can add it again any time.`,
        "Remove",
        () => {
          if (app.page.name === "game" && app.page.id === game.id) app.back();
          run("remove_game", { game: game.id });
        },
      ),
    },
  );
  return items;
}

export function editConfig(game: Uuid) {
  const snap = app.snap!;
  const g = snap.bootstrapper.games.find((x) => x.id === game);
  if (!g) return;
  const config = snap.effective[game];
  app.modal = {
    kind: "launch",
    mode: "edit",
    game,
    pickGame: false,
    account: g.config.pin_account ? g.config.account : null,
    version: config.version,
    server: config.server,
  };
}

export function serverMenu(server: PrivateServer): Items {
  return [
    { icon: "arrow-right", label: "Join", action: () => joinServer(server.id) },
    { icon: "launch", label: "Join with…", action: () => openLaunch(server.game_id, { server: server.id }) },
    "separator",
    {
      icon: "star",
      label: server.favorite ? "Remove from favorites" : "Add to favorites",
      action: () => run("toggle_server_favorite", { server: server.id }),
    },
    { icon: "edit", label: "Edit", action: () => (app.modal = { kind: "server", editing: server, game: server.game_id }) },
    { icon: "copy", label: "Copy link", action: () => copy(server.link) },
    { icon: "game", label: "Go to game", action: () => app.navigate({ name: "game", id: server.game_id }) },
    "separator",
    {
      icon: "remove",
      label: "Remove",
      danger: true,
      action: guard("Remove private server?", `${server.name} will be removed from your library.`, "Remove", () =>
        run("remove_server", { server: server.id }),
      ),
    },
  ];
}

export function accountMenu(account: Account): Items {
  const label = account.alias ?? account.display_name;
  return [
    { icon: "launch", label: "Launch a game…", action: () => openLaunch(lastGame(), { account: account.id }) },
    { icon: "play", label: "Play as this account", action: () => run("set_active_account", { account: account.id }) },
    { icon: "crown", label: "Set as default", action: () => run("set_default_account", { account: account.id }) },
    { icon: "edit", label: "Rename", action: () => (app.modal = { kind: "rename", account: account.id }) },
    { icon: "sliders", label: "Roblox settings…", action: () => (app.modal = { kind: "roblox_settings", account: account.id }) },
    { icon: "sign-in", label: "Sign in again", action: () => (app.modal = { kind: "add_account", reauth: account.id }) },
    {
      icon: "external",
      label: "View Roblox profile",
      action: () => run("open_url", { url: `https://www.roblox.com/users/${account.user_id}/profile` }),
    },
    "separator",
    {
      icon: "remove",
      label: "Remove account",
      danger: true,
      action: guard(
        "Remove account?",
        `${label} (@${account.username}) will be signed out and removed from Pious. Your Roblox account itself is not affected.`,
        "Remove",
        () => run("remove_account", { account: account.id }),
      ),
    },
  ];
}

export function versionMenu(hash: string): Items {
  const snap = app.snap!;
  const record = snap.bootstrapper.versions.find((v) => v.hash === hash);
  if (!record) return [];
  return [
    { icon: "play", label: "Launch a game…", action: () => openLaunch(lastGame(), { version: hash }) },
    { icon: "pin", label: "Set as default", action: () => run("set_default_version", { choice: { Specific: hash } }) },
    { icon: "folder", label: "Open folder", action: () => run("open_path", { path: record.path }) },
    { icon: "copy", label: "Copy build hash", action: () => copy(hash) },
    "separator",
    {
      icon: "remove",
      label: "Remove version",
      danger: true,
      action: guard(
        "Remove version?",
        record.source === "Pious"
          ? `${snap.version_titles[hash]} — its files will be deleted from disk.`
          : `${snap.version_titles[hash]} — it was installed by ${record.source === "Roblox" ? "the Roblox installer" : "a bootstrapper"}. Its files will be deleted; it may be reinstalled later.`,
        "Remove",
        () => run("remove_version", { hash }),
      ),
    },
  ];
}

/** A link that joins the same server a window is in: for anyone with a
 * browser (`web`), or straight into Roblox. `null` while the server isn't
 * known yet (or for a private server without a saved link). */
export function serverLink(instance: Instance, web = true): string | null {
  const snap = app.snap;
  if (!snap) return null;
  if (instance.server !== "Public") {
    return snap.bootstrapper.servers.find((s) => s.id === (instance.server as { Private: Uuid }).Private)?.link ?? null;
  }
  const place = snap.bootstrapper.games.find((g) => g.id === instance.game)?.place_id;
  if (!place || !instance.job) return null;
  return web
    ? `https://www.roblox.com/games/start?placeId=${place}&gameInstanceId=${instance.job}`
    : `roblox://experiences/start?placeId=${place}&gameInstanceId=${instance.job}`;
}

/** Copies the link to a window's server, or says why it can't. */
export function copyServerLink(instance: Instance, web = true) {
  const link = serverLink(instance, web);
  if (link) copy(link);
  else app.toast("caution", "Pious doesn't know this server yet. Try again once the game has finished joining.");
}

export function instanceMenu(instance: Instance): Items {
  const isPrivate = instance.server !== "Public";
  return [
    { icon: "focus", label: "Focus window", action: () => run("focus_instance", { instance: instance.id }) },
    { icon: "hop", label: "Server hop", action: () => serverHop({ instance: instance.id }) },
    { icon: "server", label: "Browse servers…", action: () => (app.modal = { kind: "servers", game: instance.game }) },
    "separator",
    { icon: "link", label: isPrivate ? "Copy private server link" : "Copy server link", action: () => copyServerLink(instance) },
    ...(isPrivate ? [] : [{ icon: "copy", label: "Copy Roblox link (opens the app)", action: () => copyServerLink(instance, false) }]),
    "separator",
    { icon: "power", label: "Close instance", danger: true, action: () => closeInstance(instance.id) },
  ];
}

export function recommendationMenu(rec: Recommendation): Items {
  return [
    { icon: "play", label: "Play", action: () => playRecommendation(rec.universe_id) },
    { icon: "info", label: "View details", action: () => openRecommendation(rec.universe_id) },
    { icon: "add", label: "Add to library", action: () => run("recommendation_game", { universe: rec.universe_id, add: true }) },
    "separator",
    { icon: "external", label: "Open on Roblox", action: () => run("open_url", { url: gameUrl(rec.place_id) }) },
  ];
}

/** Opens a recommended game without adding it to the library. */
export async function openRecommendation(universe: number) {
  const id = await run<Uuid | null>("recommendation_game", { universe, add: false });
  if (id) app.navigate({ name: "game", id });
}

export async function playRecommendation(universe: number) {
  const id = await run<Uuid | null>("recommendation_game", { universe, add: false });
  if (id) play(id);
}

export function addMenu(): Items {
  const snap = app.snap!;
  return [
    { icon: "game", label: "Add game", action: () => (app.modal = { kind: "add_game" }) },
    { icon: "users", label: "Join a player…", action: () => (app.modal = { kind: "join_player" }) },
    { icon: "link", label: "Join a private server link…", action: () => (app.modal = { kind: "join_link" }) },
    {
      icon: "server",
      label: "Add private server",
      action: () =>
        (app.modal = snap.bootstrapper.games.some((g) => g.in_library)
          ? { kind: "server", editing: null, game: app.page.name === "game" ? app.page.id : null }
          : { kind: "add_game" }),
    },
    { icon: "add-account", label: "Add account", action: () => (app.modal = { kind: "add_account", reauth: null }) },
    { icon: "download", label: "Install Roblox version", action: () => (app.modal = { kind: "install_version" }) },
  ];
}

export function copy(text: string) {
  navigator.clipboard.writeText(text).then(
    () => app.toast("positive", "Copied to clipboard"),
    () => app.toast("negative", "Couldn't copy that."),
  );
}

export { launch };
