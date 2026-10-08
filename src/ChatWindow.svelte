<!-- The chat window, like Steam's: friends and recent chats on the left,
     open chats as tabs on the right. Messages go through Roblox's own chat
     as the chosen account; chats seen before show at once from the cache
     while fresh messages load. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { app, connect } from "./lib/state.svelte";
  import { handleLaunch, run } from "./lib/api";
  import { accountLabel, relative, who } from "./lib/format";
  import { applyAppearance } from "./lib/theme";
  import { emojiShortcodes } from "./lib/emoji";
  import type { ChatMessage, Conversation, Friend, LaunchOutcome, Uuid } from "./lib/types";
  import Icon from "./components/Icon.svelte";
  import Select from "./components/Select.svelte";
  import Toasts from "./components/Toasts.svelte";

  const win = getCurrentWindow();
  const snap = $derived(app.snap);
  let account = $state<Uuid | null>(null);
  const me = $derived(snap?.bootstrapper.accounts.find((a) => a.id === account));
  const friends = $derived(snap?.friends.account === account ? (snap?.friends.list ?? []) : []);

  $effect(() => {
    if (snap) {
      const p = snap.bootstrapper.preferences;
      applyAppearance(p.appearance, p.reduce_motion, p.search_blur);
    }
  });
  // The same background picture and interface size as the app.
  const picture = $derived(snap?.bootstrapper.preferences.appearance.background_image ? convertFileSrc(snap.bootstrapper.preferences.appearance.background_image) : null);
  let zoom = 1;
  $effect(() => {
    const scale = snap?.bootstrapper.preferences.ui_scale ?? 1;
    if (scale !== zoom) {
      zoom = scale;
      getCurrentWebview().setZoom(scale).catch(() => {});
    }
  });

  // ── Tabs ─────────────────────────────────────────────────────────────
  type Tab = { user: number; conversation: string | null; messages: ChatMessage[]; loading: boolean; error: string | null; draft: string };
  let tabs = $state<Tab[]>([]);
  let active = $state<number | null>(null);
  const current = $derived(tabs.find((t) => t.user === active));
  const friendOf = (user: number) => friends.find((f) => f.id === user);

  let conversations = $state<Conversation[]>([]);
  let query = $state("");
  let scroller = $state<HTMLElement>();
  let composer = $state<HTMLTextAreaElement>();
  let sending = $state(false);
  let focused = $state(true);
  let maximized = $state(false);

  const listed = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const order: Record<string, number> = { in_game: 0, in_studio: 1, online: 2, offline: 3 };
    return friends
      .filter((f) => !q || f.display_name.toLowerCase().includes(q) || f.username.toLowerCase().includes(q))
      .slice()
      .sort((a, b) => order[a.status] - order[b.status] || a.display_name.localeCompare(b.display_name));
  });
  const recent = $derived(
    conversations
      .filter((c) => c.kind.includes("one") && c.participants.length)
      .map((c) => ({ c, friend: friendOf(c.participants.find((p) => p !== me?.user_id) ?? 0) }))
      .filter((r) => r.friend && (!query.trim() || r.friend.display_name.toLowerCase().includes(query.trim().toLowerCase())))
      .slice(0, 8),
  );

  async function openTab(user: number) {
    if (!account) return;
    if (!tabs.some((t) => t.user === user)) {
      tabs.push({ user, conversation: null, messages: [], loading: true, error: null, draft: "" });
    }
    active = user;
    const tab = tabs.find((t) => t.user === user)!;
    await tick();
    composer?.focus();
    if (tab.conversation) return;
    try {
      tab.conversation = await invoke<string>("open_chat", { account, user });
      tab.messages = await invoke<ChatMessage[]>("cached_messages", { account, conversation: tab.conversation });
      await scrollDown(true);
      await load(tab, true);
    } catch (e) {
      tab.error = String(e);
    } finally {
      tab.loading = false;
    }
  }

  function closeTab(user: number) {
    const i = tabs.findIndex((t) => t.user === user);
    if (i < 0) return;
    tabs.splice(i, 1);
    if (active === user) active = tabs[Math.max(0, i - 1)]?.user ?? null;
  }

  async function scrollDown(force: boolean) {
    const atBottom = !scroller || scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 60;
    if (!force && !atBottom) return;
    await tick();
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: force ? "instant" : "smooth" });
  }

  async function load(tab: Tab, scroll: boolean) {
    if (!account || !tab.conversation) return;
    try {
      const next = await invoke<ChatMessage[]>("chat_messages", { account, conversation: tab.conversation });
      if (JSON.stringify(next) !== JSON.stringify(tab.messages)) {
        tab.messages = next;
        if (tab.user === active) await scrollDown(scroll);
      }
      tab.error = null;
    } catch (e) {
      tab.error = String(e);
    }
  }

  async function send() {
    const tab = current;
    if (!tab || !account || !tab.conversation || sending) return;
    const text = tab.draft.trim();
    if (!text) return;
    sending = true;
    // Shown right away; replaced by Roblox's copy when it comes back.
    const pending: ChatMessage = { id: `pending-${Date.now()}`, sender: me?.user_id ?? 0, content: text, sent: new Date().toISOString() };
    tab.messages = [...tab.messages, pending];
    tab.draft = "";
    await scrollDown(true);
    try {
      await invoke("send_chat", { account, conversation: tab.conversation, text });
      await load(tab, true);
    } catch (e) {
      tab.messages = tab.messages.filter((m) => m.id !== pending.id);
      tab.draft = text;
      tab.error = String(e);
    } finally {
      sending = false;
    }
  }

  async function loadConversations() {
    if (!account) return;
    try {
      conversations = await invoke<Conversation[]>("conversations", { account });
    } catch {
      // The list is a convenience; friends still work.
    }
  }

  async function pickAccount(id: Uuid) {
    if (id === account) return;
    account = id;
    tabs = [];
    active = null;
    conversations = await invoke<Conversation[]>("cached_conversations", { account: id }).catch(() => []);
    run("refresh_friends", { account: id });
    loadConversations();
  }

  async function join(f: Friend) {
    if (!f.place_id) return;
    const once = (force: boolean) => run<LaunchOutcome>("join_player", { place: f.place_id, job: f.job, account, name: f.location, force });
    handleLaunch(await once(false), () => once(true));
  }

  function status(f: Friend | undefined): string {
    if (!f) return "";
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

  const stamp = (m: ChatMessage) =>
    m.id.startsWith("pending-") ? "Sending…" : m.sent ? new Date(m.sent).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }) : "";

  // Messages grouped by sender, with a time divider after a long gap.
  const groups = $derived.by(() => {
    const out: { mine: boolean; divider: string | null; messages: ChatMessage[] }[] = [];
    let lastTime = 0;
    for (const m of current?.messages ?? []) {
      const time = m.sent ? Date.parse(m.sent) : lastTime;
      const gap = time - lastTime > 15 * 60_000;
      const mine = m.sender === me?.user_id;
      const last = out[out.length - 1];
      if (!last || last.mine !== mine || gap) {
        out.push({ mine, divider: gap && m.sent ? new Date(m.sent).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : null, messages: [m] });
      } else {
        last.messages.push(m);
      }
      lastTime = time;
    }
    return out;
  });

  onMount(() => {
    connect();
    win.isMaximized().then((m) => (maximized = m));
    const resized = win.onResized(async () => (maximized = await win.isMaximized()));
    const focus = win.onFocusChanged(({ payload }) => (focused = payload));
    const opened = listen<{ account: Uuid | null; user: number | null }>("chat-open", async ({ payload }) => {
      if (payload.account) await pickAccount(payload.account);
      if (payload.user) openTab(payload.user);
    });
    // Where the window was opened for: #chat:<account>:<user>
    const [, acc, user] = decodeURIComponent(location.hash.slice(1)).split(":");
    (async () => {
      const first = await invoke<{ active_account: Uuid | null; friends: { account: Uuid | null } }>("get_snapshot");
      await pickAccount((acc || first.friends.account || first.active_account) as Uuid);
      if (user) openTab(Number(user));
    })();
    // New messages: quickly while looking at the window.
    let tickCount = 0;
    const poll = setInterval(() => {
      tickCount++;
      if (current && (focused || tickCount % 4 === 0)) load(current, false);
      if (tickCount % 6 === 0) loadConversations();
    }, 2500);
    return () => {
      clearInterval(poll);
      resized.then((f) => f());
      focus.then((f) => f());
      opened.then((f) => f());
    };
  });
</script>

{#snippet face(f: Friend | undefined, size: number)}
  <span class="face" style="width: {size}px; height: {size}px">
    {#if f?.avatar}<img src={f.avatar} alt="" />{:else}<span>{(f?.display_name ?? "?").charAt(0)}</span>{/if}
    {#if f}<span class="dot {f.status}"></span>{/if}
  </span>
{/snippet}

<div class="window" class:maximized>
  {#if picture}<img class="picture" src={picture} alt="" />{/if}
  <div class="backdrop" class:with-picture={!!picture}></div>
  <header class="bar" data-tauri-drag-region>
    <span class="title" data-tauri-drag-region><Icon name="chat" size={14} />Chat</span>
    {#if snap && snap.bootstrapper.accounts.length > 1}
      <Select
        options={snap.bootstrapper.accounts.map((a) => ({ value: a.id as Uuid | null, label: `As ${accountLabel(a)}` }))}
        value={account}
        onchange={(id) => id && pickAccount(id)}
        width="190px"
      />
    {/if}
    <span class="spacer" data-tauri-drag-region></span>
    <div class="controls">
      <button aria-label="Minimize" onclick={() => win.minimize()}><Icon name="minimize" size={15} /></button>
      <button aria-label="Maximize" onclick={() => win.toggleMaximize()}><Icon name={maximized ? "restore" : "maximize"} size={14} /></button>
      <button class="close" aria-label="Close" onclick={() => win.close()}><Icon name="window-close" size={16} /></button>
    </div>
  </header>

  <div class="body">
    <aside class="side">
      <div class="search-box glass-base">
        <Icon name="search" />
        <input placeholder="Find a friend" bind:value={query} />
      </div>
      <div class="people">
        {#if recent.length}
          <span class="label">Recent</span>
          <div class="stagger col tight">
            {#each recent as r (r.c.id)}
              <button class="person" class:on={active === r.friend!.id} onclick={() => openTab(r.friend!.id)}>
                {@render face(r.friend, 32)}
                <span class="col grow" style="gap: 0">
                  <span class="name line">{who(r.friend!.display_name)}</span>
                  <span class="secondary line" class:unread={r.c.unread}>{r.c.preview ?? status(r.friend)}</span>
                </span>
                {#if r.c.unread}<span class="badge-dot"></span>{/if}
              </button>
            {/each}
          </div>
        {/if}
        <span class="label">Friends · {listed.filter((f) => f.status !== "offline").length} online</span>
        {#if snap?.friends.loading && !friends.length}
          {#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton pulse"></div>{/each}
        {:else if !friends.length}
          <span class="meta pad">{snap?.friends.error ?? "No friends to show."}</span>
        {:else}
          <div class="stagger col tight">
            {#each listed as f (f.id)}
              <button class="person" class:on={active === f.id} class:offline={f.status === "offline"} onclick={() => openTab(f.id)}>
                {@render face(f, 32)}
                <span class="col grow" style="gap: 0">
                  <span class="name line">{who(f.display_name)}</span>
                  <span class="secondary line" class:playing={f.status === "in_game"}>{status(f)}</span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </aside>

    <section class="main">
      {#if tabs.length}
        <div class="tabs">
          {#each tabs as tab (tab.user)}
            {@const f = friendOf(tab.user)}
            <div class="tab" class:on={active === tab.user}>
              <button class="tab-open" onclick={() => openTab(tab.user)}>
                {@render face(f, 20)}<span class="line">{who(f?.display_name ?? "Friend")}</span>
              </button>
              <button class="tab-close" aria-label="Close chat" onclick={() => closeTab(tab.user)}><Icon name="close" size={12} /></button>
            </div>
          {/each}
        </div>
      {/if}

      {#if current}
        {@const f = friendOf(current.user)}
        {#key current.user}
          <div class="thread">
            <div class="thread-head">
              {@render face(f, 40)}
              <div class="col grow" style="gap: 1px">
                <span class="item-title line">{who(f?.display_name ?? "Friend")} <span class="secondary">@{who(f?.username ?? "")}</span></span>
                <span class="meta line" class:playing={f?.status === "in_game"}>{status(f)}</span>
              </div>
              {#if f?.status === "in_game" && f.place_id}
                <button class="btn small primary" onclick={() => join(f)}><Icon name="play" />Join</button>
              {/if}
              <button class="icon-btn" title="Profile" aria-label="Profile" onclick={() => run("open_url", { url: `https://www.roblox.com/users/${current.user}/profile` })}>
                <Icon name="external" />
              </button>
            </div>
            <div class="messages" bind:this={scroller}>
              {#if current.loading && !current.messages.length}
                <span class="meta center">Opening the chat…</span>
              {:else if current.error && !current.messages.length}
                <span class="meta center">{current.error}</span>
              {:else if !current.messages.length}
                <div class="center col" style="align-items: center; gap: 8px">
                  {@render face(f, 56)}
                  <span class="meta">No messages yet. Say hi to {who(f?.display_name ?? "them")}!</span>
                </div>
              {:else}
                {#each groups as group, gi (gi + ":" + group.messages[0].id)}
                  {#if group.divider}<div class="divider-time">{group.divider}</div>{/if}
                  <div class="group" class:mine={group.mine}>
                    {#if !group.mine}{@render face(f, 28)}{/if}
                    <div class="col bubbles" style="gap: 2px">
                      {#each group.messages as m (m.id)}
                        <div class="bubble" class:pending={m.id.startsWith("pending-")} title={m.sent ? new Date(m.sent).toLocaleString() : ""}>{m.content}</div>
                      {/each}
                      <span class="stamp">{stamp(group.messages[group.messages.length - 1])}</span>
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
            {#if current.error && current.messages.length}<span class="secondary pad error">{current.error}</span>{/if}
            <form
              class="compose glass-base"
              onsubmit={(e) => {
                e.preventDefault();
                send();
              }}
            >
              <textarea
                bind:this={composer}
                rows="1"
                placeholder="Message {who(f?.display_name ?? '')}"
                bind:value={current.draft}
                use:emojiShortcodes={snap?.bootstrapper.preferences.emoji_shortcodes.in_app ?? true}
                maxlength="500"
                disabled={!current.conversation}
                onkeydown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    send();
                  }
                }}
              ></textarea>
              <button class="btn primary send" class:ready={!!current.draft.trim()} type="submit" disabled={!current.draft.trim() || sending || !current.conversation} aria-label="Send">
                <Icon name="send" />
              </button>
            </form>
            <span class="secondary hint">Sent through Roblox chat as {me ? accountLabel(me) : "your account"}. Roblox filters messages as usual.</span>
          </div>
        {/key}
      {:else}
        <div class="empty">
          <span class="empty-icon"><Icon name="chat" size={30} /></span>
          <span class="section-title">Pick a friend to chat with</span>
          <span class="meta">Your chats open here as tabs.</span>
        </div>
      {/if}
    </section>
  </div>
  <Toasts />
</div>

<style>
  .window {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 8px;
    border: 1px solid rgb(var(--surface) / 0.08);
    animation: open 360ms var(--ease) both;
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: scale(0.985);
    }
  }
  .window.maximized {
    border-radius: 0;
    border: none;
  }
  .picture {
    position: absolute;
    inset: calc(var(--image-blur) * -2);
    width: calc(100% + var(--image-blur) * 4);
    height: calc(100% + var(--image-blur) * 4);
    object-fit: cover;
    filter: blur(var(--image-blur));
  }
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgb(var(--bg) / var(--window-alpha));
  }
  .backdrop.with-picture {
    background: rgb(var(--bg) / calc(var(--image-dim) * var(--window-alpha)));
  }
  .bar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    flex: none;
    padding-left: 14px;
    background: rgb(var(--bg) / 0.35);
    border-bottom: 1px solid rgb(var(--surface) / 0.05);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 700;
    font-size: 12.5px;
    color: rgb(var(--muted));
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .controls button {
    display: grid;
    place-items: center;
    width: 44px;
    height: 100%;
    border: none;
    background: none;
    color: rgb(var(--muted));
    cursor: pointer;
    transition: background var(--fast), color var(--fast);
  }
  .controls button:hover {
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--text));
  }
  .controls .close:hover {
    background: #c42b1c;
    color: white;
  }
  .body {
    position: relative;
    flex: 1;
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 10px;
    min-height: 0;
    border-right: 1px solid rgb(var(--surface) / calc(0.05 * var(--glass)));
    background: rgb(var(--surface) / calc(0.015 * var(--glass)));
  }
  .people {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .people .label {
    padding: 8px 6px 2px;
  }
  .tight {
    gap: 2px;
  }
  .pad {
    padding: 6px;
  }
  .person {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: var(--r-md);
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--fast), opacity var(--fast);
  }
  .person:hover {
    background: rgb(var(--surface) / 0.05);
  }
  .person.on {
    background: rgb(var(--surface) / calc(0.09 * var(--glass)));
  }
  .person.offline {
    opacity: 0.55;
  }
  .person.offline:hover,
  .person.offline.on {
    opacity: 1;
  }
  .name {
    font-weight: 600;
    font-size: 13px;
  }
  .unread {
    color: rgb(var(--text));
    font-weight: 600;
  }
  .badge-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgb(var(--accent));
    flex: none;
    animation: pop 240ms var(--ease);
  }
  .playing {
    color: #4cc38a;
  }
  .face {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(var(--surface) / 0.08);
    font-weight: 700;
    font-size: 12px;
  }
  .face img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .dot {
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 32%;
    height: 32%;
    min-width: 8px;
    min-height: 8px;
    border-radius: 50%;
    border: 2px solid rgb(var(--panel));
  }
  .dot.online {
    background: #3e9cf5;
  }
  .dot.in_game {
    background: #30a46c;
  }
  .dot.in_studio {
    background: #f5a524;
  }
  .dot.offline {
    display: none;
  }
  .skeleton {
    height: 44px;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / 0.05);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 8px 10px 0;
    overflow-x: auto;
    flex: none;
    border-bottom: 1px solid rgb(var(--surface) / calc(0.05 * var(--glass)));
  }
  .tab {
    display: flex;
    align-items: center;
    max-width: 190px;
    border-radius: var(--r-md) var(--r-md) 0 0;
    background: rgb(var(--surface) / 0.03);
    color: rgb(var(--muted));
    animation: rise 220ms var(--ease) both;
    transition: background var(--fast), color var(--fast);
  }
  .tab.on {
    background: rgb(var(--surface) / calc(0.09 * var(--glass)));
    color: rgb(var(--text));
  }
  .tab-open {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    padding: 7px 4px 7px 10px;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
  }
  .tab-close {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin-right: 4px;
    border: none;
    border-radius: 6px;
    background: none;
    color: inherit;
    cursor: pointer;
    opacity: 0.6;
  }
  .tab-close:hover {
    opacity: 1;
    background: rgb(var(--surface) / 0.08);
  }
  .thread {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    animation: fade 220ms var(--ease);
  }
  .thread-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid rgb(var(--surface) / calc(0.05 * var(--glass)));
    background: linear-gradient(rgb(var(--surface) / calc(0.03 * var(--glass))), transparent);
  }
  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    mask-image: linear-gradient(transparent, black 18px);
  }
  .center {
    margin: auto;
    text-align: center;
  }
  .divider-time {
    align-self: center;
    padding: 2px 10px;
    border-radius: 99px;
    background: rgb(var(--surface) / 0.05);
    color: rgb(var(--faint));
    font-size: 11px;
    font-weight: 600;
    animation: fade 300ms var(--ease);
  }
  .group {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    max-width: 78%;
  }
  .group.mine {
    align-self: flex-end;
    flex-direction: row-reverse;
  }
  .bubbles {
    align-items: flex-start;
  }
  .mine .bubbles {
    align-items: flex-end;
  }
  .bubble {
    max-width: 100%;
    padding: 8px 12px;
    border-radius: 16px;
    background: rgb(var(--surface) / calc(0.08 * var(--glass)));
    font-size: 13px;
    line-height: 1.4;
    user-select: text;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    animation: bubble-in 260ms var(--ease) both;
    transform-origin: bottom left;
  }
  .mine .bubble {
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
    transform-origin: bottom right;
  }
  .bubble.pending {
    opacity: 0.6;
  }
  /* Bubbles in a run hug each other, like a chat app's. */
  .group:not(.mine) .bubble:not(:first-child) {
    border-top-left-radius: 6px;
  }
  .group:not(.mine) .bubble:not(:nth-last-child(2)) {
    border-bottom-left-radius: 6px;
  }
  .mine .bubble:not(:first-child) {
    border-top-right-radius: 6px;
  }
  .mine .bubble:not(:nth-last-child(2)) {
    border-bottom-right-radius: 6px;
  }
  .stamp {
    padding: 2px 6px 0;
    font-size: 10.5px;
    color: rgb(var(--faint));
    opacity: 0;
    transform: translateY(-3px);
    transition:
      opacity var(--fast),
      transform 200ms var(--ease);
  }
  .group:hover .stamp,
  .group:last-child .stamp {
    opacity: 1;
    transform: none;
  }
  @keyframes bubble-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.96);
    }
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin: 8px 16px 6px;
    padding: 5px 5px 5px 14px;
    border-radius: 20px;
    transition:
      border-color var(--fast),
      box-shadow 220ms var(--ease);
  }
  .compose:focus-within {
    box-shadow: 0 0 0 3px rgb(var(--accent) / 0.12);
  }
  .compose textarea {
    flex: 1;
    resize: none;
    min-height: 34px;
    max-height: 120px;
    padding: 8px 0 6px;
    border: none;
    outline: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 13px;
    field-sizing: content;
  }
  .send {
    height: 34px;
    width: 34px;
    padding: 0;
    border-radius: 50%;
    justify-content: center;
    transition: transform 200ms var(--ease), opacity var(--fast);
  }
  .send:not(:disabled):hover {
    transform: scale(1.06);
  }
  .send.ready {
    animation: pop 220ms var(--ease);
  }
  .hint {
    padding: 0 16px 10px;
    font-size: 11px;
  }
  .error {
    color: #ff8b8b;
  }
  .empty {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
    animation: rise 320ms var(--ease) both;
  }
  .empty-icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgb(var(--surface) / 0.06);
    color: rgb(var(--muted));
    margin-bottom: 6px;
  }
</style>
