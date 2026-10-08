<!-- Friends of one of your accounts (or all of them together): who's online, what they're playing,
     joining them, and chatting (through Roblox's own chat). -->
<script lang="ts">
  import { app, sortOf, viewOf } from "../lib/state.svelte";
  import { handleLaunch, run, setPreferences } from "../lib/api";
  import { accountLabel, relative, who } from "../lib/format";
  import type { Friend, LaunchOutcome, Uuid } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import Tip from "../components/Tip.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const friends = $derived(snap.friends);
  const account = $derived(friends.account ?? snap.active_account);
  const view = $derived(viewOf("friends"));

  let query = $state("");
  let filter = $state<"all" | "in_game" | "online">("all");
  const sort = $derived(sortOf("friends", "status"));
  const ORDER: Record<string, number> = { in_game: 0, in_studio: 1, online: 2, offline: 3 };

  const shown = $derived(
    friends.list
      .filter((f) => {
        if (filter === "in_game" && f.status !== "in_game") return false;
        if (filter === "online" && f.status === "offline") return false;
        const q = query.trim().toLowerCase();
        return !q || f.display_name.toLowerCase().includes(q) || f.username.toLowerCase().includes(q) || (f.location ?? "").toLowerCase().includes(q);
      })
      .slice()
      .sort((a, b) => {
        if (sort === "name") return a.display_name.localeCompare(b.display_name);
        if (sort === "game") return (a.location ?? "~").localeCompare(b.location ?? "~") || a.display_name.localeCompare(b.display_name);
        if (sort === "recent") return Date.parse(b.last_online ?? "0") - Date.parse(a.last_online ?? "0") || ORDER[a.status] - ORDER[b.status];
        return ORDER[a.status] - ORDER[b.status] || a.display_name.localeCompare(b.display_name);
      }),
  );
  const counts = $derived({
    in_game: friends.list.filter((f) => f.status === "in_game").length,
    online: friends.list.filter((f) => f.status !== "offline").length,
  });

  // "all" lists every account's friends together.
  async function pickAccount(id: Uuid | "all") {
    if (id === "all") {
      setPreferences({ friends_all: true });
      return;
    }
    if (snap.bootstrapper.preferences.friends_all) await setPreferences({ friends_all: false });
    run("refresh_friends", { account: id });
  }
  /** The account that knows them: the one picked, else (all accounts) the first that's friends with them. */
  const through = (f: Friend) => (friends.all && f.via?.length ? (f.via.includes(account!) ? account : f.via[0]) : account);
  const viaText = (f: Friend) =>
    friends.all && f.via?.length
      ? "Friends with " + f.via.map((id) => snap.bootstrapper.accounts.find((a) => a.id === id)).filter((a) => a).map((a) => accountLabel(a!)).join(", ")
      : "";

  function statusText(f: Friend): string {
    switch (f.status) {
      case "in_game":
        return f.location ? `Playing ${f.location}` : "In a game";
      case "in_studio":
        return "In Roblox Studio";
      case "online":
        return "Online";
      default:
        return f.last_online ? `Last online ${relative(f.last_online)}` : "Offline";
    }
  }

  async function join(f: Friend) {
    if (!f.place_id) return;
    const once = (force: boolean) =>
      run<LaunchOutcome>("join_player", { place: f.place_id, job: f.job, account: through(f), name: f.location, force });
    handleLaunch(await once(false), () => once(true));
  }

  // Chats open in their own window, like Steam's.
  function openChat(f: Friend) {
    run("open_chat_window", { account: through(f), user: f.id });
  }

  const me = $derived(snap.bootstrapper.accounts.find((a) => a.id === account));
</script>

{#snippet avatar(f: Friend, size: number)}
  <span class="face" style="width: {size}px; height: {size}px">
    {#if f.avatar}<img src={f.avatar} alt="" loading="lazy" decoding="async" />{:else}<span>{f.display_name.charAt(0)}</span>{/if}
    <span class="status {f.status}"></span>
  </span>
{/snippet}

{#snippet actions(f: Friend)}
  {#if f.status === "in_game" && f.place_id}
    <button class="btn small primary" onclick={() => join(f)} title={f.job ? "Join their server" : "Join their game"}><Icon name="play" />Join</button>
  {/if}
  <button class="btn small" onclick={() => openChat(f)}><Icon name="chat" />Message</button>
  <button class="icon-btn" title="Profile on roblox.com" aria-label="Profile" onclick={() => run("open_url", { url: `https://www.roblox.com/users/${f.id}/profile` })}>
    <Icon name="external" />
  </button>
{/snippet}

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Friends</h1>
      <span class="meta">
        {#if friends.list.length}
          {counts.in_game} playing · {counts.online} online · {friends.list.length} friends
        {:else}
          Who's online and what they're playing
        {/if}
      </span>
    </div>
    {#if snap.bootstrapper.accounts.length > 1}
      <Select
        options={[
          { value: "all" as Uuid | "all" | null, label: "Friends of all accounts" },
          ...snap.bootstrapper.accounts.map((a) => ({ value: a.id as Uuid | "all" | null, label: `Friends of ${accountLabel(a)}` })),
        ]}
        value={friends.all ? "all" : account}
        onchange={(id) => id && pickAccount(id)}
        width="230px"
      />
    {/if}
    <button class="btn" onclick={() => run("open_chat_window", { account, user: null })}><Icon name="chat" />Chat</button>
    <SortSelect
      page="friends"
      value={sort}
      options={[
        { value: "status", label: "Online first" },
        { value: "name", label: "Name" },
        { value: "game", label: "Game" },
        { value: "recent", label: "Recently online" },
      ]}
    />
    <ViewToggle page="friends" />
    <button class="icon-btn" title="Refresh (Ctrl R)" aria-label="Refresh" onclick={() => run("refresh_friends", { account })}>
      <span class:spin={friends.loading}><Icon name="refresh" /></span>
    </button>
  </div>

  {#if !snap.bootstrapper.accounts.length}
    <EmptyState icon="friends" title="Add an account first" body="Pious shows the friends of your Roblox accounts.">
      <button class="btn primary" onclick={() => (app.modal = { kind: "add_account", reauth: null })}><Icon name="add-account" />Add Account</button>
    </EmptyState>
  {:else}
    <Tip id="friends">Click Message to chat in a window of its own, and Join to hop into a friend's server. Streamer mode (Ctrl Shift S) hides names.</Tip>
    <div class="row" style="gap: 8px">
      <div class="search-box glass-base grow">
        <Icon name="search" />
        <input placeholder="Search friends or games" bind:value={query} />
      </div>
      <button class="chip" class:on={filter === "all"} onclick={() => (filter = "all")}>All</button>
      <button class="chip" class:on={filter === "in_game"} onclick={() => (filter = "in_game")}>Playing · {counts.in_game}</button>
      <button class="chip" class:on={filter === "online"} onclick={() => (filter = "online")}>Online · {counts.online}</button>
    </div>

    {#if friends.error}
      <div class="notice negative"><Icon name="warning" />{friends.error}</div>
    {/if}

    <div class="layout">
      <div class="col" style="gap: 8px; min-width: 0">
        {#if friends.loading && !friends.list.length}
          <div class="col stagger" style="gap: 8px">
            {#each [0, 1, 2, 3, 4] as i (i)}<div class="glass-base skeleton pulse"></div>{/each}
          </div>
        {:else if !shown.length}
          <EmptyState icon="friends" title={friends.list.length ? "No one matches" : "No friends to show"} body={friends.list.length ? "Try another search or filter." : `${me ? accountLabel(me) : "This account"} has no friends on Roblox yet.`} />
        {:else if view === "List"}
          <div class="list-rows">
            {#each shown as f (f.id)}
              <div class="glass-base row-item">
                {@render avatar(f, 38)}
                <div class="col grow" style="gap: 0">
                  <span class="item-title line">{who(f.display_name)} <span class="secondary">@{who(f.username)}</span></span>
                  <span class="meta line" class:playing={f.status === "in_game"}>{statusText(f)}</span>
                  {#if viaText(f)}<span class="secondary line via">{viaText(f)}</span>{/if}
                </div>
                {@render actions(f)}
              </div>
            {/each}
          </div>
        {:else}
          <div class="grid" style="--card: 220px">
            {#each shown as f (f.id)}
              <div class="card glass friend-card">
                <div class="row" style="gap: 10px">
                  {@render avatar(f, 46)}
                  <div class="col grow" style="gap: 0">
                    <span class="item-title line">{who(f.display_name)}</span>
                    <span class="secondary line">@{who(f.username)}</span>
                  </div>
                </div>
                <span class="meta two-lines" class:playing={f.status === "in_game"}>{statusText(f)}</span>
                {#if viaText(f)}<span class="secondary line via">{viaText(f)}</span>{/if}
                <div class="row" style="gap: 6px">{@render actions(f)}</div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    </div>
  {/if}
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .rows {
    gap: 6px;
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: var(--r-md);
  }
  .friend-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    cursor: default;
  }
  .two-lines {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 2.8em;
  }
  .playing {
    color: rgb(var(--text));
  }
  .via {
    font-size: 11.5px;
  }
  .face {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(var(--surface) / 0.08);
    font-weight: 700;
  }
  .face img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .status {
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 30%;
    height: 30%;
    min-width: 9px;
    min-height: 9px;
    border-radius: 50%;
    border: 2px solid rgb(var(--panel));
    background: rgb(var(--faint));
  }
  .status.online {
    background: #3e9cf5;
  }
  .status.in_game {
    background: #30a46c;
  }
  .status.in_studio {
    background: #f5a524;
  }
  .status.offline {
    display: none;
  }
  .skeleton {
    height: 54px;
    border-radius: var(--r-md);
  }
  .spin {
    display: inline-flex;
    animation: spin 900ms linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
