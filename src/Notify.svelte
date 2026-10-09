<!-- Pious's pop-ups in the corner of the screen: a friend messaged you, a
     friend request, a friend started playing, Roblox notifications. They
     stack newest at the bottom, leave by themselves (not while the pointer
     is on them), and open the right thing when clicked. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { app, connect } from "./lib/state.svelte";
  import { run } from "./lib/api";
  import { who } from "./lib/format";
  import { applyAppearance, followUiAssets } from "./lib/theme";
  import { overrides } from "./lib/overrides.svelte";
  import Icon from "./components/Icon.svelte";

  interface Notice {
    id: string;
    kind: "message" | "friend_request" | "friend_join" | "roblox";
    title: string;
    body: string;
    avatar: string | null;
    action: { chat?: { account: string; user: number | null }; url?: string; join?: Record<string, unknown>; main?: boolean };
    /** The game it's about (game template). */
    game?: { name: string; icon: string | null; banner: string | null; playing: number | null; creator: string | null };
  }
  type Shown = Notice & { leaving: boolean; timer?: ReturnType<typeof setTimeout> };

  const snap = $derived(app.snap);
  $effect(() => {
    if (snap) {
      const p = snap.bootstrapper.preferences;
      applyAppearance(p.appearance, p.reduce_motion, p.search_blur);
    }
  });

  let list = $state<Shown[]>([]);
  let seconds = $state(6);
  /** "rich": each kind its own template; "compact": one small card. */
  let style = $state<"rich" | "compact">("rich");
  const playing = (n: number | null) => (n == null ? "" : n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}K playing` : `${n} playing`);
  let stack = $state<HTMLElement>();

  const ICONS: Record<Notice["kind"], string> = { message: "chat", friend_request: "add-account", friend_join: "play", roblox: "bell" };
  const LABELS: Record<Notice["kind"], string> = { message: "Message", friend_request: "Friend request", friend_join: "Friend playing", roblox: "Roblox" };

  // The window fits the cards.
  $effect(() => {
    if (!stack) return;
    const observer = new ResizeObserver(() => fit());
    observer.observe(stack);
    return () => observer.disconnect();
  });
  function fit() {
    const height = list.length && stack ? Math.ceil(stack.getBoundingClientRect().height) + 4 : 0;
    invoke("notify_resize", { height }).catch(() => {});
  }

  function chime() {
    // A plugin's or theme's sound, if one replaces it.
    const custom = overrides.sounds["notification"];
    if (custom) {
      const audio = new Audio(custom);
      audio.volume = 0.6;
      audio.play().catch(() => {});
      return;
    }
    try {
      const ctx = new AudioContext();
      const gain = ctx.createGain();
      gain.gain.setValueAtTime(0.0001, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.08, ctx.currentTime + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.5);
      gain.connect(ctx.destination);
      for (const [freq, at] of [
        [880, 0],
        [1318.5, 0.09],
      ]) {
        const osc = ctx.createOscillator();
        osc.type = "sine";
        osc.frequency.value = freq;
        osc.connect(gain);
        osc.start(ctx.currentTime + at);
        osc.stop(ctx.currentTime + at + 0.4);
      }
      setTimeout(() => ctx.close(), 800);
    } catch {
      // No sound device: nothing to play.
    }
  }

  function schedule(n: Shown) {
    clearTimeout(n.timer);
    n.timer = setTimeout(() => dismiss(n.id), seconds * 1000);
  }

  function dismiss(id: string) {
    const n = list.find((x) => x.id === id);
    if (!n || n.leaving) return;
    clearTimeout(n.timer);
    n.leaving = true;
    setTimeout(async () => {
      list = list.filter((x) => x.id !== id);
      await tick();
      fit();
    }, 260);
  }

  async function open(n: Shown) {
    const a = n.action;
    if (a.chat) invoke("open_chat_window", { account: a.chat.account, user: a.chat.user }).catch(() => {});
    else if (a.url) run("open_url", { url: a.url });
    // The main window joins: it can ask "Join anyway?" or to sign in again.
    // The join waits in storage both windows share, in case the main window
    // is being made (it takes it when it starts).
    else if (a.join) {
      try {
        localStorage.setItem("pious-pending-join", JSON.stringify(a.join));
      } catch {}
      invoke("show_main")
        .then(() => emitTo("main", "notify-join"))
        .catch(() => {});
    }
    else invoke("show_main").catch(() => {});
    dismiss(n.id);
  }

  onMount(() => {
    connect();
    const assets = followUiAssets();
    async function show(batch: { notices: Notice[]; seconds: number; sound: boolean; style?: "rich" | "compact" }) {
      seconds = batch.seconds;
      style = batch.style ?? "rich";
      const fresh = batch.notices.filter((n) => !list.some((x) => x.id === n.id));
      if (!fresh.length) return;
      // At most four at once; the oldest make room.
      for (const old of list.slice(0, Math.max(0, list.length + fresh.length - 4))) dismiss(old.id);
      list = [...list, ...fresh.map((n) => ({ ...n, leaving: false }))];
      for (const n of list) if (!n.timer) schedule(n);
      if (batch.sound) chime();
      await tick();
      fit();
    }
    // Pop-ups wait in Pious until this window takes them, so the ones that
    // made it open aren't lost while it loads.
    async function take() {
      const batches = await invoke<{ notices: Notice[]; seconds: number; sound: boolean }[]>("notify_take").catch(() => []);
      for (const batch of batches) await show(batch);
    }
    const off = listen("notify", take);
    take();
    return () => {
      off.then((f) => f());
      assets();
    };
  });
</script>

<div class="stack" bind:this={stack}>
  {#each list as n (n.id)}
    <div
      class="card kind-{n.kind}"
      class:leaving={n.leaving}
      class:rich={style === "rich"}
      role="button"
      tabindex="-1"
      onclick={() => open(n)}
      onkeydown={(e) => e.key === "Enter" && open(n)}
      onmouseenter={() => clearTimeout(n.timer)}
      onmouseleave={() => schedule(n)}
    >
      {#if style === "rich" && n.kind === "friend_join" && n.game}
        <!-- A game: its banner, icon and name, and who's playing. -->
        <div class="game">
          {#if n.game.banner}<img class="banner" src={n.game.banner} alt="" />{/if}
          <div class="shade"></div>
          <div class="game-info">
            {#if n.game.icon}<img class="game-icon" src={n.game.icon} alt="" />{/if}
            <span class="col" style="gap: 0; min-width: 0">
              <span class="game-name line">{n.game.name}</span>
              <span class="game-meta line">{[n.game.creator, playing(n.game.playing)].filter(Boolean).join(" · ")}</span>
            </span>
          </div>
        </div>
        <div class="game-foot">
          <span class="mini-face">{#if n.avatar}<img src={n.avatar} alt="" />{:else}<Icon name="account" size={12} />{/if}</span>
          <span class="grow line"><b>{who(n.title)}</b> started playing</span>
          <span class="join">Join</span>
        </div>
      {:else if style === "rich" && n.kind === "message"}
        <!-- A message, like a chat app: who, when, and a bubble. -->
        <span class="face">
          {#if n.avatar}<img src={n.avatar} alt="" />{:else}<Icon name="chat" size={18} />{/if}
          <span class="online"></span>
        </span>
        <span class="col grow" style="gap: 3px; min-width: 0">
          <span class="top"><span class="sender line">{who(n.title)}</span><span class="when">now</span></span>
          <span class="bubble">{n.body}</span>
          <span class="hint">Click to reply</span>
        </span>
      {:else}
      <span class="face">
        {#if n.avatar}<img src={n.avatar} alt="" />{:else}<Icon name={ICONS[n.kind]} size={18} />{/if}
        <span class="kind"><Icon name={ICONS[n.kind]} size={9} /></span>
      </span>
      <span class="col grow" style="gap: 1px; min-width: 0">
        <span class="top">
          <span class="label">{LABELS[n.kind]}</span>
          <span class="dot">·</span>
          <span class="label">Pious</span>
        </span>
        <span class="title line">{who(n.title)}</span>
        <span class="body">{n.body}</span>
      </span>
      {/if}
      <button
        class="close"
        aria-label="Dismiss"
        onclick={(e) => {
          e.stopPropagation();
          dismiss(n.id);
        }}><Icon name="close" size={11} /></button
      >
      <span class="timer" style="animation-duration: {seconds}s"></span>
    </div>
  {/each}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .stack {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 2px;
  }
  .card {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    overflow: hidden;
    border-radius: 14px;
    background: rgb(var(--panel) / 0.97);
    border: 1px solid rgb(var(--surface) / 0.1);
    box-shadow:
      inset 0 1px 0 rgb(var(--rim) / 0.08),
      0 10px 30px -12px rgb(0 0 0 / 0.65);
    color: rgb(var(--text));
    cursor: pointer;
    animation: enter 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
    transition:
      transform 260ms cubic-bezier(0.4, 0, 0.2, 1),
      opacity 220ms ease,
      background var(--fast);
  }
  .card:hover {
    background: rgb(var(--panel));
  }
  .card.leaving {
    opacity: 0;
    transform: translateX(40px) scale(0.98);
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateX(60px) scale(0.96);
    }
  }
  .face {
    position: relative;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex: none;
    border-radius: 50%;
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--muted));
  }
  .face img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .kind {
    position: absolute;
    right: -3px;
    bottom: -3px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
    border: 2px solid rgb(var(--panel));
  }
  .kind-friend_join .kind {
    background: #30a46c;
    color: white;
  }
  .kind-friend_request .kind {
    background: #3e9cf5;
    color: white;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: rgb(var(--faint));
  }
  .title {
    font-weight: 700;
    font-size: 13.5px;
  }
  .body {
    font-size: 12.5px;
    line-height: 1.35;
    color: rgb(var(--muted));
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .close {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    margin: -4px -6px 0 0;
    border: none;
    border-radius: 6px;
    background: none;
    color: rgb(var(--faint));
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--fast),
      background var(--fast);
  }
  .card:hover .close {
    opacity: 1;
  }
  .close:hover {
    background: rgb(var(--surface) / 0.1);
    color: rgb(var(--text));
  }
  /* Rich: a game's banner. */
  .card.rich.kind-friend_join:has(.game) {
    flex-direction: column;
    gap: 0;
    padding: 0;
  }
  .game {
    position: relative;
    height: 112px;
    width: 100%;
    overflow: hidden;
    background: rgb(var(--surface) / 0.06);
  }
  .banner {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scale(1.02);
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(transparent 25%, rgb(0 0 0 / 0.78));
  }
  .game-info {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: white;
  }
  .game-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    border: 2px solid rgb(255 255 255 / 0.85);
    flex: none;
  }
  .game-name {
    font-weight: 800;
    font-size: 14.5px;
    text-shadow: 0 1px 6px rgb(0 0 0 / 0.6);
  }
  .game-meta {
    font-size: 11.5px;
    opacity: 0.85;
  }
  .game-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 9px 12px 11px;
    font-size: 12.5px;
    color: rgb(var(--muted));
  }
  .game-foot b {
    color: rgb(var(--text));
  }
  .mini-face {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    background: rgb(var(--surface) / 0.1);
  }
  .mini-face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .join {
    padding: 3px 10px;
    border-radius: 999px;
    background: #30a46c;
    color: white;
    font-weight: 700;
    font-size: 11.5px;
  }
  .card.rich:has(.game) .close {
    position: absolute;
    top: 8px;
    right: 8px;
    margin: 0;
    background: rgb(0 0 0 / 0.45);
    color: white;
    z-index: 1;
  }
  /* Rich: a message. */
  .online {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #30a46c;
    border: 2px solid rgb(var(--panel));
  }
  .sender {
    font-weight: 700;
    font-size: 13.5px;
    color: rgb(var(--text));
    text-transform: none;
    letter-spacing: 0;
  }
  .when {
    margin-left: auto;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
  }
  .bubble {
    align-self: flex-start;
    max-width: 100%;
    padding: 7px 11px;
    border-radius: 4px 14px 14px 14px;
    background: rgb(var(--surface) / 0.09);
    color: rgb(var(--text));
    font-size: 12.5px;
    line-height: 1.35;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .hint {
    font-size: 10.5px;
    color: rgb(var(--faint));
  }
  .timer {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    width: 100%;
    background: rgb(var(--accent) / 0.5);
    transform-origin: left;
    animation: run linear forwards;
  }
  .card:hover .timer {
    animation-play-state: paused;
  }
  @keyframes run {
    to {
      transform: scaleX(0);
    }
  }
</style>
