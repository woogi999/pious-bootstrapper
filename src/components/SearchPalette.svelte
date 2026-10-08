<!-- Ctrl K: search games, servers, accounts, versions and pages. -->
<script lang="ts">
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";
  import { app, type Page, type TweaksTab } from "../lib/state.svelte";
  import { joinServer } from "../lib/api";
  import { accountLabel, artworkKey, avatarKey, who } from "../lib/format";
  import type { SearchHit } from "../lib/types";
  import Artwork from "./Artwork.svelte";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";

  type Hit = SearchHit;

  /** In the overlay: what picking a result does instead of navigating. */
  let { onpick }: { onpick?: (hit: Hit) => void } = $props();

  let query = $state("");
  let selected = $state(0);
  let input: HTMLInputElement;

  const snap = $derived(app.snap!);

  function score(haystack: string, needle: string): number | null {
    const h = haystack.toLowerCase();
    if (h === needle) return 0;
    if (h.startsWith(needle)) return 1;
    if (h.split(/\s+/).some((w) => w.startsWith(needle))) return 2;
    if (h.includes(needle)) return 3;
    return null;
  }
  const best = (...scores: (number | null)[]) => {
    const valid = scores.filter((s): s is number => s !== null);
    return valid.length ? Math.min(...valid) : null;
  };

  const results = $derived.by((): { recent: boolean; hits: Hit[] } => {
    const q = query.trim().toLowerCase();
    const library = snap.bootstrapper;
    if (!q) {
      const games = library.games.filter((g) => g.last_played).sort((a, b) => b.last_played!.localeCompare(a.last_played!));
      const hits: Hit[] = games.slice(0, 5).map((g) => ({ kind: "game", id: g.id }));
      const servers = library.servers.filter((s) => s.last_joined).sort((a, b) => b.last_joined!.localeCompare(a.last_joined!));
      hits.push(...servers.slice(0, 3).map((s) => ({ kind: "server" as const, id: s.id })));
      const accounts = [...library.accounts].sort((a, b) => (b.last_used ?? "").localeCompare(a.last_used ?? ""));
      hits.push(...accounts.slice(0, 3).map((a) => ({ kind: "account" as const, id: a.id })));
      if (!hits.length) hits.push(...library.games.slice(0, 6).map((g) => ({ kind: "game" as const, id: g.id })));
      return { recent: true, hits };
    }
    const scored: [number, number, Hit][] = [];
    for (const g of library.games) {
      const s = best(score(g.name, q), g.developer ? (score(g.developer, q) ?? 99) + 2 : null, score(String(g.place_id), q));
      if (s !== null) scored.push([s, 0, { kind: "game", id: g.id }]);
    }
    for (const sv of library.servers) {
      const game = library.games.find((g) => g.id === sv.game_id)?.name ?? "";
      const s = best(score(sv.name, q), game ? (score(game, q) ?? 99) + 2 : null, (score(sv.notes, q) ?? 99) + 3);
      if (s !== null && s < 99) scored.push([s, 1, { kind: "server", id: sv.id }]);
    }
    for (const a of library.accounts) {
      const s = best(score(a.username, q), score(a.display_name, q), a.alias ? score(a.alias, q) : null);
      if (s !== null) scored.push([s, 2, { kind: "account", id: a.id }]);
    }
    for (const v of library.versions) {
      const s = best(score(v.hash, q), score(snap.version_titles[v.hash] ?? "", q), v.version ? score(v.version, q) : null);
      if (s !== null) scored.push([s, 3, { kind: "version", hash: v.hash }]);
    }
    const pages: [Page["name"], string, string, TweaksTab?][] = [
      ["home", "Home", "home"],
      ["games", "Games library", "game"],
      ["servers", "Private servers", "server"],
      ["friends", "Friends and chat", "friends"],
      ["accounts", "Accounts", "accounts"],
      ["versions", "Roblox versions", "versions"],
      ["tweaks", "Tweaks, FastFlags and FPS unlock", "sliders"],
      ["tweaks", "Discord Rich Presence and appear online (Tweaks)", "chat", "presence"],
      ["tweaks", "FPS unlock, graphics and performance (Tweaks)", "lightning", "performance"],
      ["tweaks", "Mods: cursors, fonts, sounds, shift lock, Roblox icon (Tweaks)", "sparkles", "mods"],
      ["tweaks", "FastFlag editor (Tweaks)", "command-line", "flags"],
      ["tweaks", "In game: anti-AFK, rejoin, server location, emoji (Tweaks)", "clock", "ingame"],
      ["tweaks", "DNS and internet for Roblox (Tweaks)", "globe", "internet"],
      ["tweaks", "Compatibility, priority and cleanup (Tweaks)", "window", "system"],
      ["keybinds", "Game keybinds and key remaps", "keyboard"],
      ["running", "Instances: running Roblox windows", "running"],
      ["settings", "Settings", "settings"],
    ];
    for (const [page, name, icon, tab] of pages) {
      const s = score(name, q);
      if (s !== null) scored.push([s + 1, 4, { kind: "page", page, name, icon, tab }]);
    }
    scored.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    return { recent: false, hits: scored.slice(0, 24).map((x) => x[2]) };
  });

  $effect(() => {
    query;
    selected = 0;
  });

  $effect(() => input?.focus());

  function close() {
    app.searchOpen = false;
  }

  function open(hit: Hit) {
    close();
    if (onpick) return onpick(hit);
    switch (hit.kind) {
      case "game":
        return app.navigate({ name: "game", id: hit.id });
      case "server":
        return joinServer(hit.id);
      case "account":
        return app.navigate({ name: "accounts" });
      case "version":
        return app.navigate({ name: "versions" });
      case "page":
        if (hit.tab) app.tweaksTab = hit.tab as TweaksTab;
        return app.navigate({ name: hit.page } as Page);
    }
  }

  function keydown(event: KeyboardEvent) {
    const n = results.hits.length;
    if (event.key === "ArrowDown" && n) {
      event.preventDefault();
      selected = (selected + 1) % n;
    } else if (event.key === "ArrowUp" && n) {
      event.preventDefault();
      selected = (selected - 1 + n) % n;
    } else if (event.key === "Enter" && results.hits[selected]) {
      open(results.hits[selected]);
    } else if (event.key === "Escape") {
      close();
    }
  }
</script>

<button class="scrim" aria-label="Close search" onclick={close} transition:fade|global={{ duration: 200 }}></button>
<div class="place">
<div
  class="palette panel"
  in:fly|global={{ y: -14, duration: 260, easing: cubicOut }}
  out:scale|global={{ start: 0.97, duration: 160, opacity: 0 }}
>
  <div class="input-row">
    <Icon name="search" size={17} />
    <input bind:this={input} bind:value={query} placeholder="Search Pious" onkeydown={keydown} />
    <span class="kbd">Esc</span>
  </div>
  <hr class="divider" />
  <div class="list">
    {#if !results.hits.length}
      <div class="meta empty">
        {query.trim() ? "No results. Try another name, place ID or build hash." : "Start typing to search games, servers, accounts and versions."}
      </div>
    {:else}
      {#if results.recent}<div class="label heading">Recent</div>{/if}
      {#each results.hits as hit, i (i)}
        <button class="hit" class:on={i === selected} onmouseenter={() => (selected = i)} onclick={() => open(hit)}>
          {#if hit.kind === "game"}
            {@const game = snap.bootstrapper.games.find((g) => g.id === hit.id)!}
            <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="var(--r-sm)" class="thumb" />
            <span class="col grow"><span class="item-title line">{game.name}</span><span class="secondary line">{game.developer ?? "Roblox experience"}</span></span>
            <span class="badge">Game</span>
          {:else if hit.kind === "server"}
            {@const server = snap.bootstrapper.servers.find((s) => s.id === hit.id)!}
            <span class="thumb-icon"><span class="tile-icon" style="width: 32px; height: 32px"><Icon name="server" size={16} /></span></span>
            <span class="col grow"><span class="item-title line">{server.name}</span><span class="secondary line">{snap.bootstrapper.games.find((g) => g.id === server.game_id)?.name ?? ""}</span></span>
            <span class="badge">Private Server</span>
          {:else if hit.kind === "account"}
            {@const account = snap.bootstrapper.accounts.find((a) => a.id === hit.id)!}
            <span class="thumb-icon"><Avatar path={snap.images[avatarKey(account)]} name={accountLabel(account)} size={32} /></span>
            <span class="col grow"><span class="item-title line">{accountLabel(account)}</span><span class="secondary line">@{who(account.username)}</span></span>
            <span class="badge">Account</span>
          {:else if hit.kind === "version"}
            <span class="thumb-icon"><span class="tile-icon" style="width: 32px; height: 32px"><Icon name="versions" size={16} /></span></span>
            <span class="col grow"><span class="item-title line">{snap.version_titles[hit.hash]}</span><span class="secondary line">{hit.hash}</span></span>
            <span class="badge">Roblox Version</span>
          {:else}
            <span class="thumb-icon"><span class="tile-icon" style="width: 32px; height: 32px"><Icon name={hit.icon} size={16} /></span></span>
            <span class="col grow"><span class="item-title line">{hit.name}</span><span class="secondary">Section</span></span>
            <span class="badge">Go to</span>
          {/if}
        </button>
      {/each}
    {/if}
  </div>
  <hr class="divider" />
  <div class="row hints">
    <span class="kbd">↑↓</span><span class="secondary">navigate</span>
    <span class="kbd">Enter</span><span class="secondary">open</span>
    <span class="kbd">Esc</span><span class="secondary">close</span>
  </div>
</div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 110;
    border: none;
    background: rgb(0 0 0 / 0.42);
    /* Blurs the app behind the palette (Settings → Search blur). */
    backdrop-filter: blur(calc(var(--search-blur) * 22px)) saturate(1.1);
  }
  .place {
    position: fixed;
    inset: 84px 0 auto 0;
    z-index: 111;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .palette {
    pointer-events: auto;
    width: 600px;
    max-height: 520px;
    display: flex;
    flex-direction: column;
    border-radius: var(--r-xl);
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    color: rgb(var(--muted));
  }
  .input-row input {
    flex: 1;
    border: none;
    background: none;
    outline: none;
    color: rgb(var(--text));
    font: inherit;
    font-size: 16px;
  }
  .list {
    overflow-y: auto;
    padding: 6px;
    min-height: 0;
  }
  .empty {
    padding: 16px 12px;
  }
  .heading {
    padding: 6px 10px;
  }
  .hit {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    border-radius: var(--r-md);
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .hit.on {
    background: rgb(var(--surface) / 0.08);
  }
  .hit :global(.thumb) {
    width: 56px;
    height: 32px;
    flex: none;
  }
  .thumb-icon {
    display: grid;
    place-items: center;
    width: 56px;
    flex: none;
  }
  .hints {
    gap: 6px;
    padding: 9px 16px;
  }
  .hints .secondary {
    margin-right: 10px;
  }
</style>
