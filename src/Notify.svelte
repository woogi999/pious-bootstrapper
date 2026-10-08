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
  import { applyAppearance } from "./lib/theme";
  import Icon from "./components/Icon.svelte";

  interface Notice {
    id: string;
    kind: "message" | "friend_request" | "friend_join" | "roblox";
    title: string;
    body: string;
    avatar: string | null;
    action: { chat?: { account: string; user: number | null }; url?: string; join?: Record<string, unknown>; main?: boolean };
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
    async function show(batch: { notices: Notice[]; seconds: number; sound: boolean }) {
      seconds = batch.seconds;
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
    };
  });
</script>

<div class="stack" bind:this={stack}>
  {#each list as n (n.id)}
    <div
      class="card kind-{n.kind}"
      class:leaving={n.leaving}
      role="button"
      tabindex="-1"
      onclick={() => open(n)}
      onkeydown={(e) => e.key === "Enter" && open(n)}
      onmouseenter={() => clearTimeout(n.timer)}
      onmouseleave={() => schedule(n)}
    >
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
