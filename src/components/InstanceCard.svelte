<!-- A running Roblox window: full card on the Instances page, or a slim Home tile. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { closeInstance, run, serverHop } from "../lib/api";
  import { copyServerLink, instanceMenu, serverLink } from "../lib/menus";
  import { accountLabel, artworkKey, avatarKey, runtime, serverLabel, versionTitle, who } from "../lib/format";
  import type { Instance } from "../lib/types";
  import Artwork from "./Artwork.svelte";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";

  let { instance, variant = "card" }: { instance: Instance; variant?: "card" | "tile" } = $props();

  const snap = $derived(app.snap!);
  const game = $derived(snap.bootstrapper.games.find((g) => g.id === instance.game));
  const account = $derived(snap.bootstrapper.accounts.find((a) => a.id === instance.account));
  const name = $derived(account ? accountLabel(account) : "Browser account");
  const hopping = $derived(snap.hopping.includes(`instance:${instance.id}`));

  // The running time ticks once a second, without touching anything else.
  let now = $state(Date.now());
  const timer = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(timer));
</script>

{#if variant === "tile"}
  <div
    class="card glass tile"
    role="button"
    tabindex="0"
    onclick={() => run("focus_instance", { instance: instance.id })}
    onkeydown={(e) => e.key === "Enter" && run("focus_instance", { instance: instance.id })}
    oncontextmenu={(e) => app.openMenu(e, instanceMenu(instance))}
  >
    <Avatar path={account ? snap.images[avatarKey(account)] : null} {name} size={28} />
    <div class="col grow" style="gap: 1px">
      <span class="item-title line">{game?.name ?? "Roblox"}</span>
      <span class="secondary line">{name} · {runtime(instance.started, now)}</span>
    </div>
    <span class:pulse={instance.status === "launching"}><Icon name={instance.status === "launching" ? "clock" : "running"} size={13} /></span>
  </div>
{:else}
  <div class="glass-elevated instance" role="listitem" oncontextmenu={(e) => app.openMenu(e, instanceMenu(instance))}>
    <Artwork path={game ? snap.images[artworkKey(game)] : null} name={game?.name ?? "?"} radius="var(--r-sm)" class="thumb" />
    <div class="col grow" style="gap: 4px">
      <div class="row" style="gap: 6px">
        <Avatar path={account ? snap.images[avatarKey(account)] : null} {name} size={20} />
        <span style="font-weight: 600; font-size: 12.5px">{name}</span>
        {#if account}<span class="secondary">@{who(account.username)}</span>{/if}
      </div>
      <span class="line title">{game?.name ?? "Unknown game"}</span>
      <div class="row details">
        <span><Icon name={instance.server === "Public" ? "globe" : "server"} size={12} />{serverLabel(snap, instance.server)}</span>
        <span><Icon name="versions" size={12} />{versionTitle(snap, instance.version)}</span>
        <span><Icon name="clock" size={12} />{runtime(instance.started, now)}</span>
        {#if instance.location}<span title="Where this server is"><Icon name="location" size={12} />{instance.location}</span>{/if}
      </div>
    </div>
    <div class="col side">
      {#if instance.status === "launching"}
        <span class="badge accent pulse"><Icon name="clock" size={11} />Launching</span>
      {:else if instance.status === "running"}
        <span class="badge strong"><Icon name="running" size={11} />Running</span>
      {:else}
        <span class="badge"><Icon name="info" size={11} />Started (not tracked)</span>
      {/if}
      <div class="row" style="gap: 6px">
        {#if instance.status === "untracked"}
          <button class="btn tertiary small" onclick={() => run("close_instance", { instance: instance.id })}>
            <Icon name="close" size={12} />Dismiss
          </button>
        {:else}
          <button
            class="btn small"
            disabled={instance.status !== "running" || hopping || !instance.account}
            onclick={() => serverHop({ instance: instance.id })}
          >
            <Icon name="hop" size={12} />{hopping ? "Hopping…" : "Server hop"}
          </button>
          <button
            class="btn small"
            disabled={!serverLink(instance)}
            title={serverLink(instance) ? "Copy a link that joins this exact server" : "Waiting for the game to join a server"}
            onclick={() => copyServerLink(instance)}
          >
            <Icon name="link" size={12} />Copy link
          </button>
          <button class="btn small" disabled={instance.status !== "running"} onclick={() => run("focus_instance", { instance: instance.id })}>
            <Icon name="focus" size={12} />Focus
          </button>
          <button class="btn small danger" disabled={instance.status !== "running"} onclick={() => closeInstance(instance.id)}>
            <Icon name="power" size={12} />Close
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
  }
  .instance {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px;
    animation: rise 260ms var(--ease) both;
  }
  .instance :global(.thumb) {
    width: 128px;
    height: 72px;
    flex: none;
  }
  .title {
    font-weight: 600;
    font-size: 15px;
  }
  .details {
    gap: 14px;
  }
  .details span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: rgb(var(--muted));
    white-space: nowrap;
  }
  .details :global(.icon) {
    color: rgb(var(--faint));
  }
  .side {
    align-items: flex-end;
    gap: 10px;
  }
</style>
