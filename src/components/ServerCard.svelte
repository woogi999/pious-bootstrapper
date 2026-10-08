<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { joinServer, run } from "../lib/api";
  import { serverMenu } from "../lib/menus";
  import { artworkKey, oneLine, relativeInline } from "../lib/format";
  import type { PrivateServer } from "../lib/types";
  import Artwork from "./Artwork.svelte";
  import Icon from "./Icon.svelte";

  let { server, row = false }: { server: PrivateServer; row?: boolean } = $props();
  const snap = $derived(app.snap!);
  const game = $derived(snap.bootstrapper.games.find((g) => g.id === server.game_id));
</script>

{#if row}
  <div
    class="card glass-base server-row"
    role="button"
    tabindex="0"
    onclick={() => joinServer(server.id)}
    onkeydown={(e) => e.key === "Enter" && joinServer(server.id)}
    oncontextmenu={(e) => app.openMenu(e, serverMenu(server))}
  >
    <Artwork path={game ? snap.images[artworkKey(game)] : null} name={game?.name ?? server.name} radius="var(--r-sm)" class="thumb" />
    <div class="col grow" style="gap: 0">
      <span class="item-title line">{server.name}</span>
      <span class="secondary line">{game?.name ?? "Unknown game"}{server.notes ? ` · ${oneLine(server.notes)}` : ""}</span>
    </div>
    <span class="secondary line" style="width: 130px; flex: none">{server.last_joined ? `Joined ${relativeInline(server.last_joined)}` : "Not joined yet"}</span>
    <button class="icon-btn" class:on={server.favorite} aria-label="Favorite" onclick={(e) => (e.stopPropagation(), run("toggle_server_favorite", { server: server.id }))}>
      <Icon name="star" size={13} />
    </button>
    <button class="btn primary small" onclick={(e) => (e.stopPropagation(), joinServer(server.id))}><Icon name="arrow-right" size={13} />Join</button>
    <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, serverMenu(server))}><Icon name="more" /></button>
  </div>
{:else}
<div
  class="card glass server"
  role="button"
  tabindex="0"
  onclick={() => joinServer(server.id)}
  onkeydown={(e) => e.key === "Enter" && joinServer(server.id)}
  oncontextmenu={(e) => app.openMenu(e, serverMenu(server))}
>
  <div class="art-wrap">
    <Artwork path={game ? snap.images[artworkKey(game)] : null} name={game?.name ?? server.name} radius="0" class="art" />
    <div class="fade"></div>
    <div class="over">
      <div class="row">
        <span class="art-chip"><Icon name="lock" size={11} />Private</span>
        <span class="spacer"></span>
        <button
          class="overlay-btn"
          class:on={server.favorite}
          aria-label="Favorite"
          onclick={(e) => (e.stopPropagation(), run("toggle_server_favorite", { server: server.id }))}
        >
          <Icon name="star" size={13} />
        </button>
      </div>
      <span class="name line">{server.name}</span>
    </div>
  </div>
  <div class="body">
    <div class="row" style="gap: 6px; color: rgb(var(--faint))">
      <Icon name="game" size={11} /><span class="meta line">{game?.name ?? "Unknown game"}</span>
    </div>
    <span class="secondary line">{server.notes ? oneLine(server.notes) : "No notes"}</span>
    <div class="row">
      <span class="secondary line grow">
        {server.last_joined ? `Joined ${relativeInline(server.last_joined)}` : "Not joined yet"}
      </span>
      <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, serverMenu(server))}><Icon name="more" /></button>
      <button class="btn primary small" onclick={(e) => (e.stopPropagation(), joinServer(server.id))}>
        <Icon name="arrow-right" size={13} />Join
      </button>
    </div>
  </div>
</div>
{/if}

<style>
  .server {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .art-wrap {
    position: relative;
    height: 92px;
  }
  .art-wrap :global(.art),
  .fade {
    position: absolute;
    inset: 0;
  }
  .fade {
    background: linear-gradient(transparent 35%, rgb(0 0 0 / 0.8));
  }
  .over {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 9px 10px;
  }
  .name {
    font-weight: 600;
    font-size: 15px;
    color: white;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }
  .server-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px 7px 7px;
    border-radius: var(--r-md);
  }
  .server-row :global(.thumb) {
    width: 64px;
    height: 36px;
    flex: none;
  }
</style>
