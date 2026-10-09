<!-- Add a game: search Roblox by name, or paste a link or place ID. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { call, run } from "../lib/api";
  import type { GameInfo, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";

  type Found = {
    universe_id: number;
    place_id: number;
    name: string;
    creator: string;
    description: string | null;
    players: number;
    up_votes: number;
    down_votes: number;
    icon: string | null;
  };

  let input = $state("");
  let lookup = $state<{ kind: "idle" } | { kind: "loading" } | { kind: "found"; info: GameInfo } | { kind: "failed"; error: string }>({
    kind: "idle",
  });
  let results = $state<Found[]>([]);
  let searching = $state(false);
  let searchError = $state<string | null>(null);
  let chosen = $state<number | null>(null);

  /** A link or a place ID is looked up; anything else is a search. */
  const direct = $derived(/^\s*\d+\s*$/.test(input) || /roblox\.com|ro\.blox|^\s*https?:/i.test(input));
  const inLibrary = $derived(new Set(app.snap!.bootstrapper.games.filter((g) => g.in_library).map((g) => g.place_id)));

  // Search a moment after typing stops; only the newest answer counts.
  let timer: ReturnType<typeof setTimeout> | undefined;
  let asked = 0;
  $effect(() => {
    const query = input.trim();
    clearTimeout(timer);
    chosen = null;
    if (direct || query.length < 2) {
      results = [];
      searching = false;
      searchError = null;
      return;
    }
    searching = true;
    const ticket = ++asked;
    timer = setTimeout(async () => {
      try {
        const found = await call<Found[]>("search_games", { query });
        if (ticket === asked) {
          results = found;
          searchError = null;
        }
      } catch (e) {
        if (ticket === asked) searchError = String(e);
      } finally {
        if (ticket === asked) searching = false;
      }
    }, 350);
  });

  async function look() {
    if (!input.trim() || lookup.kind === "loading") return;
    lookup = { kind: "loading" };
    try {
      lookup = { kind: "found", info: await call<GameInfo>("look_up_game", { input }) };
    } catch (error) {
      lookup = { kind: "failed", error: String(error) };
    }
  }

  async function addInfo(info: GameInfo) {
    const id = await run<Uuid>("add_game", { info });
    app.modal = null;
    if (id) app.navigate({ name: "game", id });
  }

  function infoOf(g: Found): GameInfo {
    return { place_id: g.place_id, universe_id: g.universe_id, name: g.name, developer: g.creator || null, description: g.description };
  }

  async function save() {
    if (direct) {
      if (lookup.kind !== "found") return look();
      return addInfo(lookup.info);
    }
    const game = results.find((g) => g.place_id === chosen) ?? results[0];
    if (game) addInfo(infoOf(game));
  }

  const compact = (n: number) => Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 }).format(n);
  const rating = (g: Found) => (g.up_votes + g.down_votes > 0 ? Math.round((g.up_votes / (g.up_votes + g.down_votes)) * 100) : null);
  const canAdd = $derived(direct ? lookup.kind === "found" : results.length > 0);
</script>

<ModalFrame title="Add Game" subtitle="Search Roblox, or paste a game's link or place ID." width={560}>
  <div class="field">
    <span class="label">Game</span>
    <div class="row">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        autofocus
        placeholder="Search games, or paste a link or place ID"
        bind:value={input}
        oninput={() => (lookup = { kind: "idle" })}
        onkeydown={(e) => e.key === "Enter" && save()}
      />
      {#if direct}
        <button class="btn" disabled={!input.trim() || lookup.kind === "loading"} onclick={look}><Icon name="search" />Look up</button>
      {/if}
    </div>
  </div>

  {#if direct}
    {#if lookup.kind === "loading"}
      <div class="notice"><Icon name="clock" />Looking up the game on Roblox…</div>
    {:else if lookup.kind === "failed"}
      <div class="notice negative"><Icon name="warning" />{lookup.error}</div>
    {:else if lookup.kind === "found"}
      <div class="glass found">
        <span class="tile-icon" style="width: 44px; height: 44px"><Icon name="game" size={20} /></span>
        <div class="col grow">
          <span class="item-title">{lookup.info.name}</span>
          <span class="meta">{lookup.info.developer ?? "Roblox"} · Place {lookup.info.place_id}</span>
        </div>
        <Icon name="success" size={18} />
      </div>
    {:else}
      <span class="secondary">Press Enter or Look up.</span>
    {/if}
  {:else if input.trim().length < 2}
    <span class="secondary">Type a game's name to search Roblox, or paste its link from roblox.com.</span>
  {:else if searchError}
    <div class="notice negative"><Icon name="warning" />{searchError}</div>
  {:else if searching && !results.length}
    <div class="notice"><Icon name="clock" />Searching Roblox…</div>
  {:else if !results.length}
    <span class="secondary">No games found for “{input.trim()}”.</span>
  {:else}
    <div class="results" class:dim={searching}>
      {#each results as g, i (g.universe_id)}
        <button
          class="result"
          class:on={chosen === g.place_id || (chosen === null && i === 0)}
          onclick={() => (chosen = g.place_id)}
          ondblclick={() => addInfo(infoOf(g))}
          title="Double-click to add"
        >
          {#if g.icon}
            <img class="thumb" src={g.icon} alt="" loading="lazy" />
          {:else}
            <span class="tile-icon thumb"><Icon name="game" size={18} /></span>
          {/if}
          <span class="col grow">
            <span class="item-title line">{g.name}</span>
            <span class="meta line">
              {#if g.creator}{g.creator} · {/if}{compact(g.players)} playing{#if rating(g) !== null}&nbsp;· {rating(g)}% liked{/if}
            </span>
          </span>
          {#if inLibrary.has(g.place_id)}<span class="badge">In library</span>{/if}
        </button>
      {/each}
    </div>
  {/if}

  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    <button class="btn primary" disabled={!canAdd} onclick={save}><Icon name="add" />Add to Library</button>
  {/snippet}
</ModalFrame>

<style>
  .found {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
  }
  .results {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 340px;
    overflow-x: hidden;
    overflow-y: auto;
    margin: 0 -4px;
    padding: 0 4px;
    transition: opacity var(--fast);
  }
  .results.dim {
    opacity: 0.6;
  }
  .result {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    background: none;
    text-align: left;
    cursor: pointer;
    animation: rise 220ms var(--ease) both;
    transition: background var(--fast), border-color var(--fast);
  }
  .result:hover {
    background: rgb(var(--surface) / calc(0.05 * var(--glass)));
  }
  .result.on {
    background: rgb(var(--surface) / calc(0.08 * var(--glass)));
    border-color: rgb(var(--surface) / calc(0.12 * var(--glass)));
  }
  .thumb {
    width: 44px;
    height: 44px;
    border-radius: var(--r-sm);
    flex: none;
    object-fit: cover;
  }
</style>
