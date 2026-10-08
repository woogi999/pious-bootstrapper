<!-- An account as a card (`variant="card"`), a list row (`"row"`) or a slim
     Home tile (`"tile"`). Clicking picks it as the account Play uses. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { lastGame, openLaunch, run } from "../lib/api";
  import { accountMenu } from "../lib/menus";
  import { accountLabel, avatarKey, relativeInline, who } from "../lib/format";
  import type { Account } from "../lib/types";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";

  let { account, variant = "card" }: { account: Account; variant?: "card" | "row" | "tile" } = $props();

  const snap = $derived(app.snap!);
  const label = $derived(accountLabel(account));
  const running = $derived(snap.instances.filter((i) => i.account === account.id).length);
  const isDefault = $derived(snap.bootstrapper.preferences.default_account === account.id);
  const playingAs = $derived(snap.active_account === account.id);

  const status = $derived(
    account.needs_sign_in
      ? { icon: "warning", text: "Sign-in needed", tone: "strong" }
      : running
        ? { icon: "running", text: running === 1 ? "Playing now" : `${running} instances`, tone: "strong" }
        : { icon: "success", text: "Ready", tone: "accent" },
  );

  const activity = $derived.by(() => {
    const entry = snap.bootstrapper.activity.find((a) => a.account_id === account.id);
    const game = entry && snap.bootstrapper.games.find((g) => g.id === entry.game_id);
    if (entry && game) return `Played ${game.name} · ${relativeInline(entry.at)}`;
    return account.last_used ? `Last used ${relativeInline(account.last_used)}` : "Not used yet";
  });

  function pick() {
    if (account.needs_sign_in) app.modal = { kind: "add_account", reauth: account.id };
    else run("set_active_account", { account: account.id });
  }

  function primary(event: MouseEvent) {
    event.stopPropagation();
    if (account.needs_sign_in) app.modal = { kind: "add_account", reauth: account.id };
    else openLaunch(lastGame(), { account: account.id });
  }
</script>

{#snippet badges()}
  <span class="badge {status.tone}"><Icon name={status.icon} size={11} />{status.text}</span>
  {#if playingAs}<span class="badge"><Icon name="play" size={11} />Plays</span>{/if}
  {#if isDefault}<span class="badge"><Icon name="crown" size={11} />Default</span>{/if}
{/snippet}

{#snippet primaryButton()}
  {#if account.needs_sign_in}
    <button class="btn small" onclick={primary}><Icon name="sign-in" size={13} />Sign in</button>
  {:else}
    <button class="btn primary small" onclick={primary}><Icon name="launch" size={13} />Launch</button>
  {/if}
{/snippet}

<div
  class="card glass v-{variant}"
  class:active={playingAs && variant !== "card"}
  role="button"
  tabindex="0"
  onclick={pick}
  onkeydown={(e) => e.key === "Enter" && pick()}
  oncontextmenu={(e) => app.openMenu(e, accountMenu(account))}
>
  {#if variant === "card"}
    <div class="row" style="gap: 10px">
      <Avatar path={snap.images[avatarKey(account)]} name={label} size={40} />
      <div class="col grow" style="gap: 1px">
        <span class="item-title line">{label}</span>
        <span class="secondary line">@{who(account.username)}</span>
      </div>
      <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, accountMenu(account))}><Icon name="more" /></button>
    </div>
    <div class="row" style="gap: 4px; flex-wrap: wrap">{@render badges()}</div>
    <hr class="divider" />
    <div class="row">
      <span class="secondary line grow">{activity}</span>
      {@render primaryButton()}
    </div>
  {:else if variant === "row"}
    <Avatar path={snap.images[avatarKey(account)]} name={label} size={30} />
    <div class="col who" style="gap: 1px">
      <span class="item-title line">{label}</span>
      <span class="secondary line">@{who(account.username)}</span>
    </div>
    <div class="row badges">{@render badges()}</div>
    <span class="secondary line grow">{activity}</span>
    {#if !playingAs}
      <button class="btn small" onclick={(e) => (e.stopPropagation(), pick())}><Icon name="play" size={12} />Play as</button>
    {/if}
    {@render primaryButton()}
    <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, accountMenu(account))}><Icon name="more" /></button>
  {:else}
    <Avatar path={snap.images[avatarKey(account)]} name={label} size={30} />
    <div class="col grow" style="gap: 1px">
      <span class="item-title line">{label}</span>
      <span class="tile-status line">
        <Icon name={account.needs_sign_in ? "warning" : running ? "running" : playingAs ? "check" : "success"} size={10} />
        {account.needs_sign_in ? "Sign-in needed" : running ? "Playing" : playingAs ? "Play uses this" : "Ready"}
      </span>
    </div>
    <button class="icon-btn" aria-label="Launch" onclick={primary}><Icon name="launch" /></button>
  {/if}
</div>

<style>
  .card {
    min-width: 0;
  }
  .v-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .v-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .v-row {
    padding: 7px 10px;
  }
  .v-row:hover {
    transform: none;
  }
  .who {
    width: 170px;
    flex: none;
  }
  .badges {
    gap: 4px;
    width: 230px;
    flex: none;
    overflow: hidden;
  }
  .v-tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
  }
  .tile-status {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: rgb(var(--faint));
  }
  .active .tile-status {
    color: rgb(var(--accent));
  }
</style>
