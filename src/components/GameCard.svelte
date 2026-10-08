<!-- A library card for a game. Every text line stays on one line, so long
     names and accounts never push the buttons out of shape. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { play, run } from "../lib/api";
  import { gameMenu } from "../lib/menus";
  import { accountName, artworkKey, relative } from "../lib/format";
  import type { Game } from "../lib/types";
  import Artwork from "./Artwork.svelte";
  import Icon from "./Icon.svelte";

  let { game, row = false }: { game: Game; row?: boolean } = $props();

  const snap = $derived(app.snap!);
  const running = $derived(snap.instances.filter((i) => i.game === game.id).length);
  const servers = $derived(snap.bootstrapper.servers.filter((s) => s.game_id === game.id).length);
  const account = $derived(accountName(snap, snap.effective[game.id]?.account ?? null));
</script>

{#snippet action()}
  {#if game.in_library}
    <button class="btn primary small" onclick={(e) => (e.stopPropagation(), play(game.id))}><Icon name="play" size={13} />Play</button>
  {:else}
    <button class="btn primary small" onclick={(e) => (e.stopPropagation(), run("add_to_library", { game: game.id }))}>
      <Icon name="add" size={13} />Add
    </button>
  {/if}
{/snippet}

{#if row}
  <div
    class="card glass-base game-row"
    role="button"
    tabindex="0"
    onclick={() => app.navigate({ name: "game", id: game.id })}
    onkeydown={(e) => e.key === "Enter" && app.navigate({ name: "game", id: game.id })}
    oncontextmenu={(e) => app.openMenu(e, gameMenu(game))}
  >
    <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="var(--r-sm)" class="thumb" />
    <div class="col grow" style="gap: 0">
      <span class="item-title line">{game.name}{#if running}<span class="badge strong running-badge">{running === 1 ? "Running" : `${running} running`}</span>{/if}</span>
      <span class="secondary line">{game.developer ?? "Roblox experience"}</span>
    </div>
    <span class="secondary line col-played">{game.last_played ? relative(game.last_played) : "Never played"}</span>
    <span class="secondary line col-account">{account}</span>
    <button
      class="icon-btn"
      class:on={game.favorite}
      aria-label="Favorite"
      title="Favorite"
      onclick={(e) => (e.stopPropagation(), run("toggle_favorite", { game: game.id }))}
    >
      <Icon name="heart" size={13} />
    </button>
    {@render action()}
    <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, gameMenu(game))}><Icon name="more" /></button>
  </div>
{:else}
<div
  class="card glass game"
  role="button"
  tabindex="0"
  onclick={() => app.navigate({ name: "game", id: game.id })}
  onkeydown={(e) => e.key === "Enter" && app.navigate({ name: "game", id: game.id })}
  oncontextmenu={(e) => app.openMenu(e, gameMenu(game))}
>
  <div class="art-wrap">
    <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="0" class="art" />
    <div class="over">
      {#if running}
        <span class="art-chip"><span class="dot"></span>{running === 1 ? "Running" : `${running} running`}</span>
      {/if}
      {#if !game.in_library}
        <span class="art-chip"><Icon name="history" size={11} />Recent</span>
      {/if}
      <span class="spacer"></span>
      <button
        class="overlay-btn"
        class:on={game.favorite}
        aria-label="Favorite"
        onclick={(e) => (e.stopPropagation(), run("toggle_favorite", { game: game.id }))}
      >
        <Icon name="heart" size={13} />
      </button>
    </div>
  </div>
  <div class="body">
    <div class="col" style="gap: 1px">
      <span class="item-title line">{game.name}</span>
      <span class="secondary line">{game.developer ?? "Roblox experience"}</span>
    </div>
    <div class="row info">
      <Icon name="clock" size={11} />
      <span class="secondary line grow">{game.last_played ? relative(game.last_played) : "Never played"}</span>
      {#if servers}<span class="badge"><Icon name="server" size={11} />{servers}</span>{/if}
    </div>
    <div class="row info">
      <Icon name="account" size={11} />
      <span class="secondary line grow">{account}</span>
    </div>
    <div class="row">
      {@render action()}
      <span class="spacer"></span>
      <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, gameMenu(game))}><Icon name="more" /></button>
    </div>
  </div>
</div>
{/if}

<style>
  .game {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .art-wrap {
    position: relative;
    aspect-ratio: 16 / 9;
  }
  .art-wrap :global(.art) {
    position: absolute;
    inset: 0;
  }
  .over {
    position: absolute;
    inset: 8px 8px auto 8px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(var(--text));
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }
  .info {
    gap: 5px;
    color: rgb(var(--faint));
    height: 18px;
  }
  .game-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px 7px 7px;
    border-radius: var(--r-md);
    --lift: -1px;
  }
  .game-row:hover {
    --grow: 1;
  }
  .game-row :global(.thumb) {
    width: 76px;
    height: 43px;
    flex: none;
  }
  .col-played {
    width: 120px;
    flex: none;
  }
  .col-account {
    width: 140px;
    flex: none;
  }
  .running-badge {
    margin-left: 8px;
    height: 18px;
    font-size: 10.5px;
  }
</style>
