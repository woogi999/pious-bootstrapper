<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { joinServer, openLaunch, play, run, serverHop } from "../lib/api";
  import { editConfig, gameMenu, serverMenu } from "../lib/menus";
  import {
    accountLabel,
    accountName,
    artworkKey,
    avatarKey,
    gameUrl,
    oneLine,
    relative,
    relativeInline,
    sameServer,
    serverLabel,
    versionLabel,
    versionTitle,
  } from "../lib/format";
  import type { Uuid } from "../lib/types";
  import Artwork from "../components/Artwork.svelte";
  import Avatar from "../components/Avatar.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  let { id }: { id: Uuid } = $props();

  const snap = $derived(app.snap!);
  const game = $derived(snap.bootstrapper.games.find((g) => g.id === id));
  const config = $derived(snap.effective[id]);
  const running = $derived(snap.instances.filter((i) => i.game === id).length);
  const hopping = $derived(snap.hopping.includes(`game:${id}`));
  const servers = $derived(
    snap.bootstrapper.servers
      .filter((s) => s.game_id === id)
      .sort((a, b) => Number(b.favorite) - Number(a.favorite) || (b.last_joined ?? "").localeCompare(a.last_joined ?? "")),
  );
  const activity = $derived(snap.bootstrapper.activity.filter((a) => a.game_id === id).slice(0, 6));
  const account = $derived(config?.account ? snap.bootstrapper.accounts.find((a) => a.id === config.account) : null);
</script>

{#if !game || !config}
  <EmptyState icon="game" title="This game is no longer in your library" body="It may have been removed.">
    <button class="btn primary" onclick={() => app.navigate({ name: "games" })}><Icon name="library" />Back to Games</button>
  </EmptyState>
{:else}
  <div class="page">
    <div class="glass-elevated banner">
      <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="0" class="art" />
      <div class="shade"></div>
      <div class="content">
        <div class="row" style="gap: 6px; flex-wrap: wrap">
          <span class="art-chip"><Icon name="hash" size={11} />Place {game.place_id}</span>
          {#if !game.in_library}<span class="art-chip"><Icon name="history" size={11} />Recent · not in your library</span>{/if}
          {#if game.collection}<span class="art-chip"><Icon name="folder" size={11} />{game.collection}</span>{/if}
          {#if running}<span class="art-chip">{running === 1 ? "Running" : `${running} running`}</span>{/if}
        </div>
        <div class="col" style="gap: 2px">
          <h1 class="display line">{game.name}</h1>
          {#if game.developer}<span class="meta" style="color: rgb(255 255 255 / 0.78)">by {game.developer}</span>{/if}
        </div>
        <div class="row actions">
          <button class="btn primary hero" onclick={() => play(id)}><Icon name="play" />PLAY</button>
          <button class="btn" onclick={() => openLaunch(id)}><Icon name="launch" />Play with…</button>
          <button class="btn" disabled={hopping} onclick={() => serverHop({ game: id })}>
            <Icon name="hop" />{hopping ? "Finding a server…" : "Random server"}
          </button>
          <button class="btn" onclick={() => (app.modal = { kind: "servers", game: id })}><Icon name="server" />Servers</button>
          {#if game.in_library}
            <button class="btn" onclick={() => (app.modal = { kind: "server", editing: null, game: id })}><Icon name="server" />Add Private Server</button>
          {:else}
            <button class="btn" onclick={() => run("add_to_library", { game: id })}><Icon name="add" />Add to library</button>
          {/if}
          <button class="overlay-btn big" class:on={game.favorite} aria-label="Favorite" onclick={() => run("toggle_favorite", { game: id })}>
            <Icon name="heart" size={14} />
          </button>
          <button class="overlay-btn big" aria-label="More" onclick={(e) => app.openMenu(e, gameMenu(game))}><Icon name="more" size={14} /></button>
        </div>
        <span class="summary">
          Plays as {accountName(snap, config.account)} · {versionLabel(snap, config.version)} · {serverLabel(snap, config.server)}
        </span>
      </div>
    </div>

    <div class="columns">
      <div class="main">
        <section class="section">
          <h2 class="section-title">Overview</h2>
          <div class="glass-base text selectable">{game.description ?? "No description available."}</div>
        </section>

        <section class="section">
          <div class="section-header">
            <div class="col grow" style="gap: 1px">
              <h2 class="section-title">Private servers</h2>
              <span class="meta">
                {servers.length === 0 ? "Saved servers for this game" : servers.length === 1 ? "1 saved server" : `${servers.length} saved servers`}
              </span>
            </div>
            {#if servers.length}
              <button class="btn small" onclick={() => (app.modal = { kind: "server", editing: null, game: id })}><Icon name="add" />Add</button>
            {/if}
          </div>
          {#if !servers.length}
            <EmptyState icon="server" title="No private servers yet" body="Save a private server to join it instantly later.">
              <button class="btn primary" onclick={() => (app.modal = { kind: "server", editing: null, game: id })}>
                <Icon name="add" />Add Private Server
              </button>
            </EmptyState>
          {:else}
            <div class="col" style="gap: 6px">
              {#each servers as server (server.id)}
                <div class="glass-base server" role="listitem" oncontextmenu={(e) => app.openMenu(e, serverMenu(server))}>
                  <span class="tile-icon" style="width: 32px; height: 32px"><Icon name="server" size={15} /></span>
                  <div class="col grow" style="gap: 2px">
                    <div class="row" style="gap: 6px">
                      <span class="item-title line">{server.name}</span>
                      {#if server.favorite}<Icon name="star" size={11} />{/if}
                      {#if sameServer(config.server, { Private: server.id })}<span class="badge"><Icon name="check" size={11} />In launch config</span>{/if}
                    </div>
                    <span class="secondary line">
                      {[server.notes ? oneLine(server.notes) : null, server.last_joined ? `Last joined ${relativeInline(server.last_joined)}` : "Not joined yet"]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </div>
                  <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, serverMenu(server))}><Icon name="more" /></button>
                  <button class="btn primary small" onclick={() => joinServer(server.id)}><Icon name="arrow-right" size={12} />Join</button>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <section class="section">
          <h2 class="section-title">Recent activity</h2>
          {#if !activity.length}
            <div class="glass-base text meta">Your launches of this game will appear here.</div>
          {:else}
            <div class="glass-base list">
              {#each activity as entry, i (`${entry.at}-${i}`)}
                {@const who = snap.bootstrapper.accounts.find((a) => a.id === entry.account_id)}
                <div class="row entry">
                  <Avatar path={who ? snap.images[avatarKey(who)] : null} name={who ? accountLabel(who) : "?"} size={24} />
                  <div class="col grow" style="gap: 1px">
                    <span>Played as {who ? accountLabel(who) : "your browser account"}</span>
                    <span class="secondary line">{serverLabel(snap, entry.server)} · {versionTitle(snap, entry.version)}</span>
                  </div>
                  <span class="secondary">{relative(entry.at)}</span>
                </div>
              {/each}
            </div>
          {/if}
        </section>
      </div>

      <aside class="side">
        <div class="glass-elevated box">
          <div class="row">
            <h3 class="section-title grow" style="font-size: 14px">Launch configuration</h3>
            <button class="icon-btn" aria-label="Edit" onclick={() => editConfig(id)}><Icon name="edit" /></button>
          </div>
          <div class="meta-row">
            <Icon name="account" size={14} /><span class="meta">Account</span>
            {#if account}
              <span class="row value" style="gap: 6px">
                <Avatar path={snap.images[avatarKey(account)]} name={accountLabel(account)} size={18} />
                <span class="line">{accountLabel(account)}</span>
                {#if !game.config.pin_account}<span class="secondary">· sidebar</span>{/if}
              </span>
            {:else}
              <span class="value">Add an account</span>
            {/if}
          </div>
          <div class="meta-row"><Icon name="versions" size={14} /><span class="meta">Version</span><span class="value line">{versionLabel(snap, config.version)}</span></div>
          <div class="meta-row">
            <Icon name={config.server === "Public" ? "globe" : "server"} size={14} /><span class="meta">Server</span>
            <span class="value line">{serverLabel(snap, config.server)}</span>
          </div>
          {#if !snap.default_version && !snap.latest_installed}
            <span class="secondary">No managed version found — Roblox will launch with the system installation.</span>
          {/if}
        </div>
        <div class="glass box">
          <div class="stat"><Icon name="play" size={14} /><span class="meta grow">Times played</span><span>{game.play_count}</span></div>
          <div class="stat"><Icon name="clock" size={14} /><span class="meta grow">Last played</span><span>{game.last_played ? relative(game.last_played) : "Never"}</span></div>
          <div class="stat"><Icon name="library" size={14} /><span class="meta grow">Added</span><span>{relative(game.added_at)}</span></div>
          <hr class="divider" />
          <div class="row" style="gap: 2px">
            <button class="btn tertiary small" onclick={() => run("open_url", { url: gameUrl(game.place_id) })}><Icon name="external" />Roblox page</button>
            <button class="btn tertiary small" onclick={() => run("refresh_game", { game: id })}><Icon name="refresh" />Refresh</button>
          </div>
        </div>
      </aside>
    </div>
  </div>
{/if}

<style>
  .banner {
    position: relative;
    height: 250px;
    overflow: hidden;
  }
  .banner :global(.art),
  .shade {
    position: absolute;
    inset: 0;
  }
  .shade {
    background:
      linear-gradient(90deg, rgb(0 0 0 / 0.8), rgb(0 0 0 / 0.45) 38%, transparent 70%),
      linear-gradient(transparent 25%, rgb(0 0 0 / 0.88));
  }
  .content {
    position: absolute;
    inset: auto 0 0 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 22px;
    max-width: 880px;
    color: white;
  }
  .actions {
    gap: 8px;
  }
  .actions .btn:not(.primary) {
    background: rgb(255 255 255 / 0.1);
    border-color: rgb(255 255 255 / 0.14);
    color: white;
  }
  .overlay-btn.big {
    width: 34px;
    height: 34px;
  }
  .summary {
    font-size: 12px;
    color: rgb(255 255 255 / 0.72);
  }
  .columns {
    display: flex;
    gap: 18px;
    align-items: flex-start;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .side {
    width: 300px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .text {
    padding: 14px;
    color: rgb(var(--muted));
    line-height: 1.5;
    white-space: pre-line;
    max-height: 220px;
    overflow-y: auto;
  }
  .server {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
  }
  .list {
    padding: 4px;
  }
  .entry {
    gap: 10px;
    padding: 6px 10px;
  }
  .box {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
  }
  .meta-row {
    display: grid;
    grid-template-columns: 20px 70px minmax(0, 1fr);
    align-items: center;
    color: rgb(var(--faint));
  }
  .value {
    color: rgb(var(--text));
    min-width: 0;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 10px;
    color: rgb(var(--faint));
  }
  .stat > span:last-child {
    color: rgb(var(--text));
  }
</style>
