<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { app, connect } from "./lib/state.svelte";
  import { handleLaunch, run, setPreferences } from "./lib/api";
  import type { LaunchOutcome } from "./lib/types";
  import { handleShortcut } from "./lib/shortcuts";
  import { applyAppearance } from "./lib/theme";
  import ContextMenu from "./components/ContextMenu.svelte";
  import Icon from "./components/Icon.svelte";
  import ModalHost from "./components/ModalHost.svelte";
  import SearchPalette from "./components/SearchPalette.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Titlebar from "./components/Titlebar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Accounts from "./pages/Accounts.svelte";
  import Friends from "./pages/Friends.svelte";
  import GameDetail from "./pages/GameDetail.svelte";
  import Games from "./pages/Games.svelte";
  import Home from "./pages/Home.svelte";
  import Macros from "./pages/Macros.svelte";
  import News from "./pages/News.svelte";
  import PluginPage from "./pages/PluginPage.svelte";
  import Running from "./pages/Running.svelte";
  import Keybinds from "./pages/Keybinds.svelte";
  import Servers from "./pages/Servers.svelte";
  import Settings from "./pages/Settings.svelte";
  import Versions from "./pages/Versions.svelte";
  import TweaksPage from "./pages/Tweaks.svelte";

  const snap = $derived(app.snap);
  const look = $derived(snap?.bootstrapper.preferences.appearance);
  const background = $derived(look?.background_image ? convertFileSrc(look.background_image) : null);

  $effect(() => {
    if (snap) {
      const p = snap.bootstrapper.preferences;
      applyAppearance(p.appearance, p.reduce_motion, p.search_blur);
    }
  });

  // Interface size.
  let zoom = 1;
  $effect(() => {
    const scale = snap?.bootstrapper.preferences.ui_scale ?? 1;
    if (scale !== zoom) {
      zoom = scale;
      getCurrentWebview().setZoom(scale).catch(() => {});
    }
  });

  // The adjustable see-through blur: fresh captures of what's behind the
  // window, blurred here.
  let backdrop = $state<string | null>(null);
  const adjustable = $derived(!!look?.see_through && look?.blur === "Adjustable");

  // The window fades in once the startup logo (its own window) is done.
  let shown = $state(false);

  // First time: the welcome. After an update: what's new.
  let greeted = false;
  $effect(() => {
    if (!snap?.loaded || greeted || !shown) return;
    greeted = true;
    const p = snap.bootstrapper.preferences;
    if (!p.onboarded) app.modal = { kind: "welcome" };
    else if (p.seen_version !== snap.current_version) {
      if (p.seen_version) app.modal = { kind: "whats_new" };
      else setPreferences({ seen_version: snap.current_version });
    }
  });

  onMount(() => {
    connect();
    const frames = listen<string>("backdrop-frame", (event) => (backdrop = event.payload));
    const appear = () => {
      if (shown) return;
      shown = true;
      app.pageKey++;
    };
    const showing = listen("app-shown", appear);
    // A "friend started playing" pop-up was clicked: its join waits in
    // storage (taken once, whether this window was open or just made).
    const takeJoin = async () => {
      let join: { place: number; job: string | null; account: string | null; name: string | null } | null = null;
      try {
        join = JSON.parse(localStorage.getItem("pious-pending-join") ?? "null");
        localStorage.removeItem("pious-pending-join");
      } catch {}
      if (!join) return;
      const once = (force: boolean) => run<LaunchOutcome>("join_player", { ...join, force });
      handleLaunch(await once(false), () => once(true));
    };
    const joins = listen("notify-join", takeJoin);
    takeJoin();
    const focused = getCurrentWindow().onFocusChanged(({ payload }) => payload && appear());
    // Opened without the logo (e.g. shown from the tray later).
    getCurrentWindow()
      .isVisible()
      .then((visible) => visible && appear());
    const win = getCurrentWindow();
    const sync = async () => {
      app.maximized = await win.isMaximized();
      app.fullscreen = await win.isFullscreen();
    };
    sync();
    const unlisten = win.onResized(sync);
    return () => {
      unlisten.then((f) => f());
      frames.then((f) => f());
      showing.then((f) => f());
      joins.then((f) => f());
      focused.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={(e) => handleShortcut(e)} />

<div class="window" class:maximized={app.maximized || app.fullscreen} class:shown>
  {#if adjustable && backdrop}
    <img class="live" src={backdrop} alt="" style="filter: blur({Math.round((look?.blur_strength ?? 0.5) * 12)}px)" />
  {/if}
  {#if background}
    <img class="picture" src={background} alt="" />
  {/if}
  <div class="backdrop" class:with-picture={!!background}></div>
  <Titlebar />
  <div class="shell">
    <Sidebar />
    <main>
      <div class="topbar" class:full={app.fullscreen}>
        <button
          class="icon-btn"
          aria-label="Toggle sidebar"
          title="Toggle sidebar"
          onclick={() => setPreferences({ sidebar_collapsed: !snap?.bootstrapper.preferences.sidebar_collapsed })}
        >
          <span class="flip" class:flipped={snap?.bootstrapper.preferences.sidebar_collapsed}><Icon name="sidebar-collapse" /></span>
        </button>
        {#if app.page.name === "game"}
          <button class="btn tertiary" onclick={() => app.back()}><Icon name="back" />Back</button>
        {/if}
        <span class="spacer"></span>
        {#if snap?.bootstrapper.preferences.streamer_mode}
          <button class="chip" title="Names are hidden. Click to show them." onclick={() => setPreferences({ streamer_mode: false })}><Icon name="streamer" />Streamer mode</button>
        {/if}
        {#if snap && (snap.automation.running.length || snap.automation.clicking || snap.automation.recording)}
          <button class="chip rec" onclick={() => app.navigate({ name: "macros" })}>
            <span class="rec-dot"></span>{snap.automation.recording ? "Recording a macro" : snap.automation.clicking ? "Auto-clicking" : "Macro running"}
          </button>
        {/if}
        {#if snap?.recorder.recording_since != null}
          <button class="chip rec" onclick={() => app.navigate({ name: "settings" })}><span class="rec-dot"></span>Recording</button>
        {/if}
        {#if snap && (snap.update.kind === "available" || snap.update.kind === "ready")}
          <button class="chip" onclick={() => ((app.settingsTab = "about"), app.navigate({ name: "settings" }))}>
            <Icon name="update" />{snap.update.kind === "ready" ? "Restart to update" : "Update available"}
          </button>
        {/if}
        {#if snap?.instances.length}
          <button class="chip" onclick={() => app.navigate({ name: "running" })}>
            <span class="dot" class:pulse={snap.instances.some((i) => i.status === "launching")}></span>{snap.instances.length} running
          </button>
        {/if}
      </div>
      <div class="scroll" id="page-scroll">
        <div class="content">
          {#if !snap?.loaded}
            <div class="loading meta">Loading your library…</div>
          {:else}
            {#key app.pageKey}
              <div class="enter">
                {#if app.page.name === "home"}<Home />
                {:else if app.page.name === "news"}<News />
                {:else if app.page.name === "games"}<Games />
                {:else if app.page.name === "game"}<GameDetail id={app.page.id} />
                {:else if app.page.name === "servers"}<Servers />
                {:else if app.page.name === "friends"}<Friends />
                {:else if app.page.name === "accounts"}<Accounts />
                {:else if app.page.name === "versions"}<Versions />
                {:else if app.page.name === "tweaks"}<TweaksPage />
                {:else if app.page.name === "running"}<Running />
                {:else if app.page.name === "keybinds"}<Keybinds />
                {:else if app.page.name === "macros"}<Macros />
                {:else if app.page.name === "plugin"}<PluginPage id={app.page.id} />
                {:else}<Settings />{/if}
              </div>
            {/key}
          {/if}
        </div>
      </div>
    </main>
  </div>

  {#if snap}
    <ModalHost />
    {#if app.searchOpen}<SearchPalette />{/if}
  {/if}
  <ContextMenu />
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
  }
  .window:not(.shown) {
    opacity: 0;
    transform: scale(0.985);
  }
  .window {
    transition:
      opacity 420ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 520ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .window.maximized {
    border-radius: 0;
    border: none;
  }
  .live {
    position: absolute;
    inset: -24px;
    width: calc(100% + 48px);
    height: calc(100% + 48px);
    object-fit: cover;
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
  .shell {
    position: relative;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 46px;
    flex: none;
    padding: 0 24px;
  }
  /* Room for the exit-fullscreen button in the corner. */
  .topbar.full {
    padding-right: 60px;
  }
  .flip {
    display: inline-flex;
    transition: transform 260ms var(--ease);
  }
  .flip.flipped {
    transform: rotate(180deg);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(var(--text));
  }
  .rec-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e5484d;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
  }
  .content {
    max-width: 1500px;
    margin: 0 auto;
    padding: 4px 24px 40px;
  }
  .enter {
    animation: rise 280ms var(--ease);
  }
  .loading {
    display: grid;
    place-items: center;
    height: 60vh;
  }
</style>
