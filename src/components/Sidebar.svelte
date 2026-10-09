<!-- Navigation, the search shortcut and the play-as account switcher. The
     switcher picks the account Play uses; it never changes the default. -->
<script lang="ts">
  import { app, type Page } from "../lib/state.svelte";
  import { run } from "../lib/api";
  import { keysFor, runAction } from "../lib/shortcuts";
  import { keyLabel } from "../lib/keys";
  import { accountLabel, avatarKey } from "../lib/format";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";

  const snap = $derived(app.snap);
  const collapsed = $derived(snap?.bootstrapper.preferences.sidebar_collapsed ?? false);
  const account = $derived(snap?.bootstrapper.accounts.find((a) => a.id === snap.active_account));
  const running = $derived(snap?.instances.length ?? 0);
  const installing = $derived(Object.keys(snap?.installs ?? {}).length > 0);
  const onlineFriends = $derived(snap?.friends.list.filter((f) => f.status === "in_game").length ?? 0);

  const selected = (name: Page["name"], id?: string) =>
    (app.page.name === name && (!id || (app.page.name === "plugin" && app.page.id === id))) || (name === "games" && app.page.name === "game");
  const plugins = $derived((snap?.plugins ?? []).filter((p) => p.enabled && p.page));

  // The logo spins when clicked, then does what Settings says.
  let spins = $state(0);
  function logoClick() {
    spins++;
    runAction(snap?.bootstrapper.preferences.logo_action || "home");
  }

  // The highlight slides to the selected item.
  let aside = $state<HTMLElement>();
  let pill = $state<{ top: number; left: number; width: number; height: number } | null>(null);
  let instant = $state(true);
  function place() {
    const on = aside?.querySelector<HTMLElement>(".nav.on");
    if (!aside || !on) {
      pill = null;
      return;
    }
    const outer = aside.getBoundingClientRect();
    const rect = on.getBoundingClientRect();
    pill = { top: rect.top - outer.top, left: rect.left - outer.left, width: rect.width, height: rect.height };
  }
  $effect(() => {
    app.page.name;
    app.page.name === "plugin" && app.page.id;
    plugins.length;
    collapsed;
    // After the DOM settles (and again once the sidebar finishes resizing).
    requestAnimationFrame(place);
    const later = setTimeout(place, 280);
    return () => clearTimeout(later);
  });
  $effect(() => {
    if (!aside) return;
    const observer = new ResizeObserver(() => place());
    observer.observe(aside);
    // The first placement doesn't animate in from nowhere.
    const ready = setTimeout(() => (instant = false), 400);
    return () => {
      observer.disconnect();
      clearTimeout(ready);
    };
  });

  function switcher(event: MouseEvent) {
    if (!snap) return;
    const items = snap.bootstrapper.accounts.map((a) => ({
      icon: a.id === snap.active_account ? "check" : "account",
      label: accountLabel(a),
      action: () => run("set_active_account", { account: a.id }),
    }));
    app.openMenu(event, [
      ...items,
      "separator",
      { icon: "add-account", label: "Add account", action: () => (app.modal = { kind: "add_account", reauth: null }) },
      { icon: "accounts", label: "Manage accounts", action: () => app.navigate({ name: "accounts" }) },
    ]);
  }
</script>

{#snippet item(name: Page["name"], icon: string, label: string, badge?: string | null, id?: string)}
  <button class="nav" class:on={selected(name, id)} title={collapsed ? label : undefined} onclick={() => app.navigate((id ? { name, id } : { name }) as Page)}>
    <Icon name={icon} size={15} />
    <span class="text line">{label}</span>
    {#if badge}<span class="count">{badge}</span>{/if}
  </button>
{/snippet}

<aside class:collapsed bind:this={aside}>
  {#if pill}
    <span
      class="pill"
      class:instant
      style="transform: translate({pill.left}px, {pill.top}px); width: {pill.width}px; height: {pill.height}px"
      aria-hidden="true"
    ></span>
  {/if}
  <div class="head">
    <button class="logo" onclick={logoClick} title="Pious" aria-label="Pious">
      <LogoMark {spins} reduce={snap?.bootstrapper.preferences.reduce_motion ?? false} />
    </button>
    <button class="search glass-base" onclick={() => (app.searchOpen = true)} title="Search ({keyLabel(keysFor("search"))})">
      <Icon name="search" size={14} />
      <span class="text line grow">Search</span>
      {#if keysFor("search")}<span class="kbd text">{keyLabel(keysFor("search")).replace(/ \+ /g, " ")}</span>{/if}
    </button>
  </div>

  <nav>
    {@render item("home", "home", "Home")}
    {@render item("news", "news", "News")}
    <span class="label group text">Library</span>
    {@render item("games", "game", "Games")}
    {@render item("servers", "server", "Private Servers")}
    {@render item("friends", "friends", "Friends", onlineFriends ? String(onlineFriends) : null)}
    <span class="gap"></span>
    {@render item("accounts", "accounts", "Accounts")}
    {@render item("versions", "versions", "Versions", installing ? "↓" : null)}
    {@render item("tweaks", "sliders", "Tweaks")}
    {@render item("keybinds", "keyboard", "Keybinds")}
    {@render item("running", "running", "Instances", running ? String(running) : null)}
    {#if plugins.length}
      <span class="label group text">Plugins</span>
      {#each plugins as plugin (plugin.id)}
        {@render item("plugin", plugin.icon, plugin.name, null, plugin.id)}
      {/each}
    {/if}
  </nav>

  <span class="spacer"></span>

  {#if account}
    <button class="switcher glass-base" onclick={switcher} title="Play as">
      <Avatar path={snap?.images[avatarKey(account)]} name={accountLabel(account)} size={28} />
      <span class="col grow text" style="gap: 0">
        <span class="line" style="font-weight: 600; font-size: 12.5px">{accountLabel(account)}</span>
        <span class="line secondary" style="font-size: 11px">Plays as this account</span>
      </span>
      <span class="text" style="color: rgb(var(--faint))"><Icon name="caret-down" size={12} /></span>
    </button>
  {:else}
    <button class="switcher glass-base" onclick={() => (app.modal = { kind: "add_account", reauth: null })}>
      <Icon name="add-account" size={16} />
      <span class="text line">Add an account</span>
    </button>
  {/if}
  <hr class="divider" />
  {@render item("help", "book-open", "Help")}
  {@render item("settings", "settings", "Settings")}
</aside>

<style>
  aside {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 212px;
    flex: none;
    padding: 12px 10px;
    background: rgb(var(--surface) / calc(0.018 * var(--glass)));
    border-right: 1px solid rgb(var(--surface) / calc(0.05 * var(--glass)));
    transition: width 260ms var(--ease);
    overflow: hidden;
  }
  aside.collapsed {
    width: 60px;
  }
  .text {
    transition: opacity 160ms var(--ease);
  }
  .collapsed .text {
    opacity: 0;
    pointer-events: none;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
    margin-left: 2px;
    padding: 2px;
    border: none;
    border-radius: var(--r-md);
    background: none;
    color: rgb(var(--text));
    cursor: pointer;
    transition: background var(--fast), transform 160ms var(--ease);
  }
  .logo:hover {
    background: rgb(var(--surface) / 0.06);
  }
  .logo:active {
    transform: scale(0.92);
  }
  .collapsed .search {
    display: none;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 9px;
    border-radius: var(--r-md);
    color: rgb(var(--faint));
    font-size: 12px;
    cursor: pointer;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .group {
    padding: 10px 12px 4px;
    height: 28px;
  }
  .gap {
    height: 6px;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 12px;
    border: none;
    border-radius: var(--r-md);
    background: transparent;
    color: rgb(var(--muted));
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    position: relative;
    transition: background var(--fast), color var(--fast);
  }
  .nav:hover {
    background: rgb(var(--surface) / 0.05);
    color: rgb(var(--text));
  }
  .nav.on {
    color: rgb(var(--text));
    font-weight: 700;
  }
  .nav.on:hover {
    background: transparent;
  }
  /* The sliding highlight behind the selected item, with its accent bar. */
  .pill {
    position: absolute;
    top: 0;
    left: 0;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / calc(0.08 * var(--glass)));
    pointer-events: none;
    transition:
      transform 380ms cubic-bezier(0.22, 1, 0.36, 1),
      width 260ms var(--ease),
      height 260ms var(--ease);
    will-change: transform;
  }
  .pill::before {
    content: "";
    position: absolute;
    left: 1px;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 3px;
    background: rgb(var(--accent));
  }
  .pill.instant {
    transition: none;
  }
  nav,
  .head,
  .switcher,
  .divider,
  :global(aside > .nav) {
    position: relative;
  }
  .count {
    margin-left: auto;
    padding: 0 7px;
    border-radius: 99px;
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
    font-size: 11px;
    font-weight: 700;
  }
  .collapsed .count {
    position: absolute;
    top: 3px;
    right: 3px;
    padding: 0 5px;
    font-size: 9.5px;
  }
  .switcher {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 42px;
    padding: 6px 8px;
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
    color: inherit;
  }
</style>
