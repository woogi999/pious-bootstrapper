<!-- The in-game overlay: parts of Pious as separate panels around the edges
     of the screen, leaving the middle free. Friends (top left), private
     servers and in-game options (left), the bar, what's running, recording
     and quick play (right), and what's playing on the PC (bottom). Panels
     can be dragged by their header, resized from their corner,
     pinned so they can't move, or hidden. Ctrl K searches like in the app.
     Opening blurs the game behind it (from Rust) and animates everything
     in; closing animates out before the window hides.

     It also asks how long a manual clip should be ("clip" mode). -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { app, connect } from "./lib/state.svelte";
  import { closeInstance, handleLaunch, joinServer, launchHooks, play, run, serverHop, setPreferences } from "./lib/api";
  import { copyServerLink, serverLink } from "./lib/menus";
  import { accountLabel, artworkKey, avatarKey, relative, runtime, who } from "./lib/format";
  import { keyLabel, matches } from "./lib/keys";
  import { handleShortcut } from "./lib/shortcuts";
  import { applyAppearance, followUiAssets } from "./lib/theme";
  import type { Friend, LaunchOutcome, NowPlaying, SearchHit, Widget } from "./lib/types";
  import Artwork from "./components/Artwork.svelte";
  import Avatar from "./components/Avatar.svelte";
  import ChatPanel from "./components/ChatPanel.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import Icon from "./components/Icon.svelte";
  import ModalHost from "./components/ModalHost.svelte";
  import SearchPalette from "./components/SearchPalette.svelte";
  import Switch from "./components/Switch.svelte";
  import Toasts from "./components/Toasts.svelte";
  import logo from "../assets/brand/logo.svg?raw";

  const snap = $derived(app.snap);
  let now = $state(Date.now());

  /** Drawn at all (the window is showing). */
  let shown = $state(false);
  /** Animated in. */
  let open = $state(false);
  let mode = $state<"full" | "clip">("full");
  let backdrop = $state<string | null>(null);
  let blur = $state(0);
  let dim = $state(0.35);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  // The manual clip prompt.
  let clipMax = $state<number | null>(null);
  let clipSeconds = $state("30");
  let clipInput = $state<HTMLInputElement>();

  $effect(() => {
    if (snap) {
      const p = snap.bootstrapper.preferences;
      applyAppearance(p.appearance, p.reduce_motion, p.search_blur);
    }
  });

  async function prompt(max: number) {
    clipMax = max;
    clipSeconds = String(Math.min(max, snap?.bootstrapper.preferences.recorder.clip_seconds ?? 30));
    await tick();
    clipInput?.focus();
    clipInput?.select();
  }

  onMount(() => {
    connect();
    // Plugins' and the theme's styles and icons here too.
    followUiAssets();
    // Launches close the overlay once they've started, not before: a "Join
    // anyway?" question has to stay up until it's answered.
    launchHooks.started = () => {
      app.modal = null;
      hide();
    };
    const timer = setInterval(() => (now = Date.now()), 1000);
    type Show = { backdrop: string | null; blur: number; dim: number; mode: "full" | "clip"; clip_max: number | null };
    const offShow = listen<Show>("overlay-show", (event) => onShow(event.payload));
    // Opened before this page had loaded (the window was just made): the
    // "overlay-show" that opened it came too early, so ask for it.
    offShow.then(() => invoke<Show | null>("overlay_pending").then((p) => p && onShow(p)).catch(() => {}));
    async function onShow(p: Show) {
        clearTimeout(hideTimer);
        blur = p.blur;
        dim = p.dim;
        mode = p.mode ?? "full";
        const next = p.backdrop ? convertFileSrc(p.backdrop) : null;
        // Let the picture decode before animating, so nothing pops in late.
        if (next) {
          await new Promise((done) => {
            const img = new Image();
            img.onload = img.onerror = () => done(null);
            img.src = next;
            setTimeout(() => done(null), 250);
          });
        }
        backdrop = next;
        open = false;
        shown = true;
        await tick();
        requestAnimationFrame(() => requestAnimationFrame(() => (open = true)));
        if (mode === "clip" && p.clip_max) prompt(p.clip_max);
    }
    const offPrompt = listen<number>("clip-prompt", (event) => prompt(event.payload));
    const offHide = listen("overlay-hide", () => {
      open = false;
      clearTimeout(hideTimer);
      hideTimer = setTimeout(() => {
        shown = false;
        app.searchOpen = false;
        app.modal = null;
        chatWith = null;
        invoke("finish_hide_overlay");
      }, 260);
    });
    return () => {
      clearInterval(timer);
      offShow.then((f) => f());
      offHide.then((f) => f());
      offPrompt.then((f) => f());
    };
  });

  function hide() {
    if (clipMax !== null) {
      clipMax = null;
      invoke("submit_clip", { seconds: null });
    }
    invoke("hide_overlay");
  }

  function saveClip(seconds?: number) {
    const value = Math.round(seconds ?? Number(clipSeconds));
    if (!value || value < 1) return;
    clipMax = null;
    invoke("submit_clip", { seconds: Math.min(value, snap?.bootstrapper.preferences.recorder.buffer_seconds ?? value) });
    if (mode === "clip") invoke("hide_overlay");
  }

  // ── Panels ─────────────────────────────────────────────────────────────
  const DEFAULTS: Record<string, Widget> = {
    friends: { x: 0.015, y: 0.03, pinned: false, hidden: false },
    servers: { x: 0.015, y: 0.46, pinned: false, hidden: false },
    ingame: { x: 0.015, y: 0.76, pinned: false, hidden: false },
    // x < 0: against the right edge (until moved).
    bar: { x: -1, y: 0.03, pinned: false, hidden: false },
    running: { x: -1, y: 0.14, pinned: false, hidden: false },
    record: { x: -1, y: 0.42, pinned: false, hidden: false },
    quick: { x: -1, y: 0.6, pinned: false, hidden: false },
    media: { x: 0.36, y: 0.84, pinned: false, hidden: false },
  };
  const ORDER = ["bar", "friends", "running", "servers", "record", "quick", "ingame", "media"];
  const TITLES: Record<string, string> = {
    friends: "Friends",
    servers: "Private servers",
    ingame: "In game",
    running: "Running now",
    record: "Recording",
    quick: "Jump back in",
    media: "Now playing",
  };

  // While a panel is moved or resized, and after, until the saved layout
  // catches up. Without this the panel jumps back to where it was for a
  // moment when it's let go.
  let moving = $state<string | null>(null);
  let local = $state<Record<string, Partial<Widget>>>({});

  function saved(name: string): Widget {
    return { ...DEFAULTS[name], ...(snap?.bootstrapper.preferences.overlay.widgets[name] ?? {}) };
  }
  function widget(name: string): Widget {
    return { ...saved(name), ...(local[name] ?? {}) };
  }

  // Forget a local change once the saved layout has it.
  $effect(() => {
    for (const [name, patch] of Object.entries(local)) {
      if (moving === name) continue;
      const s = saved(name);
      // Saved positions come back rounded (stored as 32-bit floats).
      const same = (a: unknown, b: unknown) => (typeof a === "number" && typeof b === "number" ? Math.abs(a - b) < 1e-4 : (a ?? null) === (b ?? null));
      if (Object.entries(patch).every(([k, v]) => same(s[k as keyof Widget], v))) delete local[name];
    }
  });

  function save(name: string, patch: Partial<Widget>) {
    local[name] = { ...(local[name] ?? {}), ...patch };
    setPreferences({ overlay: { widgets: { [name]: { ...widget(name), ...patch } } } });
  }

  /** Follows the pointer (once per frame) until it's let go. */
  function track(event: PointerEvent, name: string, step: (dx: number, dy: number) => Partial<Widget>) {
    event.preventDefault();
    event.stopPropagation();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startY = event.clientY;
    let last: PointerEvent | null = null;
    let frame = 0;
    moving = name;
    const apply = () => {
      frame = 0;
      if (last) local[name] = { ...(local[name] ?? {}), ...step(last.clientX - startX, last.clientY - startY) };
    };
    const move = (e: PointerEvent) => {
      last = e;
      if (!frame) frame = requestAnimationFrame(apply);
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      handle.removeEventListener("pointercancel", up);
      if (frame) cancelAnimationFrame(frame);
      apply();
      moving = null;
      if (last && local[name]) save(name, local[name]);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
    // A lost or interrupted drag ends where it is.
    handle.addEventListener("pointercancel", up);
  }

  function startDrag(name: string, event: PointerEvent) {
    if (widget(name).pinned || (event.target as HTMLElement).closest("button, input, a, .grip")) return;
    const panel = (event.currentTarget as HTMLElement).closest<HTMLElement>(".widget")!;
    const rect = panel.getBoundingClientRect();
    track(event, name, (dx, dy) => {
      // Keep the panel on screen.
      const x = Math.min(Math.max(rect.left + dx, 0), window.innerWidth - rect.width);
      const y = Math.min(Math.max(rect.top + dy, 0), window.innerHeight - 40);
      return { x: x / window.innerWidth, y: y / window.innerHeight };
    });
  }

  function startResize(name: string, event: PointerEvent) {
    const panel = (event.currentTarget as HTMLElement).closest<HTMLElement>(".widget")!;
    const rect = panel.getBoundingClientRect();
    // A panel against the right edge stays where it is while it grows.
    const x = rect.left / window.innerWidth;
    track(event, name, (dx, dy) => ({
      x,
      width: Math.round(Math.min(Math.max(rect.width + dx, 220), window.innerWidth - rect.left)),
      height: Math.round(Math.min(Math.max(rect.height + dy, 120), window.innerHeight - rect.top)),
    }));
  }

  function position(name: string): string {
    const w = widget(name);
    const size = `${w.width ? `width: ${w.width}px;` : ""}${w.height ? `height: ${w.height}px;` : ""}`;
    if (w.x < 0) return `right: 1.5%; top: ${w.y * 100}%; ${size}`;
    return `left: ${w.x * 100}%; top: ${w.y * 100}%; ${size}`;
  }

  function resetLayout() {
    local = {};
    setPreferences({ overlay: { widgets: DEFAULTS } });
  }

  // ── Content ────────────────────────────────────────────────────────────
  const recent = $derived(
    (snap?.bootstrapper.games ?? [])
      .filter((g) => g.last_played)
      .sort((a, b) => b.last_played!.localeCompare(a.last_played!))
      .slice(0, 4),
  );
  const favorites = $derived((snap?.bootstrapper.games ?? []).filter((g) => g.favorite && g.in_library).slice(0, 4));
  let quickTab = $state<"recent" | "favorites">("recent");
  const hotkey = $derived(keyLabel(snap?.bootstrapper.preferences.overlay.hotkey ?? ""));
  const clock = $derived(new Date(now).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }));
  const friendsOnline = $derived((snap?.friends.list ?? []).filter((f) => f.status !== "offline").slice(0, 12));
  const recorder = $derived(snap?.recorder);
  const rec = $derived(snap?.bootstrapper.preferences.recorder);
  let chatWith = $state<Friend | null>(null);

  // ── Quick switch ───────────────────────────────────────────────────────
  // The game being played (the newest with an account): another account
  // can take its place in the same server.
  const playing = $derived(
    (snap?.instances ?? []).filter((i) => i.account && i.status !== "launching").sort((a, b) => b.started.localeCompare(a.started))[0] ?? null,
  );
  let switching = $state(false);
  async function quickSwitch() {
    if (!snap?.active_account) return;
    switching = true;
    const account = snap.active_account;
    const once = () => run<LaunchOutcome>("quick_switch", { account });
    handleLaunch(await once(), once);
    switching = false;
  }

  // ── Now playing ────────────────────────────────────────────────────────
  let media = $state<NowPlaying | null>(null);
  // Where it is, counted on between checks while it plays.
  let mediaAt = 0;
  const mediaPosition = $derived(media?.position == null ? null : media.position + (media.playing ? Math.max(0, now - mediaAt) / 1000 : 0));
  const wantsMedia = $derived(shown && mode === "full" && !!snap?.bootstrapper.preferences.overlay.media && !widget("media").hidden);
  async function checkMedia() {
    const next = await invoke<NowPlaying | null>("media_now_playing").catch(() => null);
    media = next;
    mediaAt = Date.now();
  }
  $effect(() => {
    if (!wantsMedia) return;
    checkMedia();
    const timer = setInterval(checkMedia, 1500);
    return () => clearInterval(timer);
  });
  async function mediaPress(action: "toggle" | "next" | "previous") {
    if (media && action === "toggle") media.playing = !media.playing;
    try {
      await invoke("media_control", { action });
    } catch (e) {
      app.toast("caution", String(e));
    }
    setTimeout(checkMedia, 250);
  }
  const clockOf = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

  // Friends load on first open.
  $effect(() => {
    if (shown && snap && !snap.friends.account && !snap.friends.loading && snap.bootstrapper.accounts.length) {
      invoke("refresh_friends", { account: null });
    }
  });

  async function launch(game: string) {
    await play(game);
  }

  async function joinFriend(f: Friend) {
    if (!f.place_id) return;
    const account = snap?.friends.account ?? snap?.active_account ?? null;
    const once = (force: boolean) => run<LaunchOutcome>("join_player", { place: f.place_id, job: f.job, account, name: f.location, force });
    handleLaunch(await once(false), () => once(true));
  }

  function friendStatus(f: Friend): string {
    if (f.status === "in_game") return f.location ? `Playing ${f.location}` : "In a game";
    if (f.status === "in_studio") return "In Studio";
    return "Online";
  }

  // Search results do what makes sense over a game.
  function pick(hit: SearchHit) {
    switch (hit.kind) {
      case "game":
        launch(hit.id);
        break;
      case "server":
        joinServer(hit.id);
        break;
      case "account":
        run("set_active_account", { account: hit.id });
        break;
      default:
        hide();
        invoke("show_main");
    }
  }

  function key(event: KeyboardEvent) {
    if (!open) return;
    if (clipMax !== null) {
      if (event.key === "Escape") {
        event.preventDefault();
        clipMax = null;
        invoke("submit_clip", { seconds: null });
        if (mode === "clip") invoke("hide_overlay");
      }
      return;
    }
    // The overlay's own hotkey closes it (when Windows hands it to us).
    if (snap && matches(event, snap.bootstrapper.preferences.overlay.hotkey) && !app.searchOpen) {
      event.preventDefault();
      hide();
      return;
    }
    if (handleShortcut(event, true)) return;
    if (event.key === "Escape") {
      if (chatWith) chatWith = null;
      else hide();
    }
  }
</script>

<svelte:window onkeydown={key} />

{#snippet head(name: string)}
  {@const w = widget(name)}
  <div class="head" class:grab={!w.pinned} role="toolbar" tabindex="-1" onpointerdown={(e) => startDrag(name, e)}>
    <span class="title">{TITLES[name]}</span>
    <span class="spacer"></span>
    <button class="icon-btn mini" title={w.pinned ? "Unpin (allow moving)" : "Pin in place"} aria-label="Pin" onclick={() => save(name, { pinned: !w.pinned })}>
      <Icon name={w.pinned ? "lock" : "lock-open"} size={12} />
    </button>
    <button class="icon-btn mini" title="Hide" aria-label="Hide" onclick={() => save(name, { hidden: true })}><Icon name="close" size={12} /></button>
  </div>
{/snippet}

{#if shown}
  <div class="stage" class:open>
    <div class="backdrop">
      {#if backdrop && blur > 0}
        <img class="still" src={backdrop} alt="" style="filter: blur({Math.round(blur * 36)}px) saturate(1.15)" />
      {/if}
      <div class="dim" style="background: rgb(0 0 0 / {dim})"></div>
    </div>
    <button class="catcher" aria-label="Back to the game" onclick={hide}></button>

    {#if snap && mode === "full"}
      {#each ORDER as name, i (name)}
        {@const w = widget(name)}
        {#if !w.hidden && (name !== "media" || snap.bootstrapper.preferences.overlay.media)}
          <div class="widget" class:dragging={moving === name} class:sized={!!w.height} style="{position(name)} --i: {i}">
            {#if name === "bar"}
              <div class="glass-elevated bar" role="toolbar" tabindex="-1" class:grab={!w.pinned} onpointerdown={(e) => startDrag(name, e)}>
                <span class="logo">{@html logo}</span>
                <div class="col grow" style="gap: 0">
                  <span class="clock">{clock}</span>
                  <span class="secondary line">{hotkey} or Esc to close</span>
                </div>
                <button class="icon-btn" title="Search (Ctrl K)" aria-label="Search" onclick={() => (app.searchOpen = true)}><Icon name="search" /></button>
                <button class="icon-btn" title="Open Pious" aria-label="Open Pious" onclick={() => (hide(), invoke("show_main"))}><Icon name="external" /></button>
                <button class="icon-btn" title="Reset layout" aria-label="Reset layout" onclick={resetLayout}><Icon name="refresh" /></button>
                <button class="icon-btn" title={w.pinned ? "Unpin" : "Pin"} aria-label="Pin" onclick={() => save(name, { pinned: !w.pinned })}>
                  <Icon name={w.pinned ? "lock" : "lock-open"} />
                </button>
                <button class="icon-btn" aria-label="Close overlay" onclick={hide}><Icon name="close" /></button>
                {#if Object.keys(TITLES).some((other) => widget(other).hidden)}
                  <div class="hidden-list">
                    {#each Object.keys(TITLES) as other (other)}
                      {#if widget(other).hidden}
                        <button class="chip" title="Show {TITLES[other]}" onclick={() => save(other, { hidden: false })}><Icon name="add" />{TITLES[other]}</button>
                      {/if}
                    {/each}
                  </div>
                {/if}
              </div>
            {:else if name === "friends"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                {#if chatWith && (snap.friends.account ?? snap.active_account)}
                  <div class="chat-box">
                    <ChatPanel account={(snap.friends.account ?? snap.active_account)!} friend={chatWith} compact onclose={() => (chatWith = null)} />
                  </div>
                {:else if snap.friends.error}
                  <span class="meta">{snap.friends.error}</span>
                {:else if !friendsOnline.length}
                  <span class="meta">{snap.friends.loading ? "Loading friends…" : "No friends online right now."}</span>
                {:else}
                  <div class="scroll-list">
                    {#each friendsOnline as f (f.id)}
                      <div class="friend">
                        <span class="face">{#if f.avatar}<img src={f.avatar} alt="" />{:else}{f.display_name.charAt(0)}{/if}<span class="status {f.status}"></span></span>
                        <div class="col grow" style="gap: 0">
                          <span class="item-title line">{who(f.display_name)}</span>
                          <span class="secondary line">{friendStatus(f)}</span>
                        </div>
                        {#if f.status === "in_game" && f.place_id}
                          <button class="btn small primary" onclick={() => joinFriend(f)}>Join</button>
                        {/if}
                        <button class="icon-btn" title="Message" aria-label="Message" onclick={() => (chatWith = f)}><Icon name="chat" /></button>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {:else if name === "servers"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                {#if !snap.bootstrapper.servers.length}
                  <span class="meta">Saved private servers show up here.</span>
                {:else}
                  <div class="scroll-list">
                    {#each [...snap.bootstrapper.servers].sort((a, b) => Number(b.favorite) - Number(a.favorite) || (b.last_joined ?? "").localeCompare(a.last_joined ?? "")).slice(0, 8) as server (server.id)}
                      {@const game = snap.bootstrapper.games.find((g) => g.id === server.game_id)}
                      <div class="friend">
                        <Artwork path={game ? snap.images[artworkKey(game)] : null} name={game?.name ?? server.name} radius="var(--r-sm)" class="mini-thumb" />
                        <div class="col grow" style="gap: 0">
                          <span class="item-title line">{server.name}</span>
                          <span class="secondary line">{game?.name ?? ""}</span>
                        </div>
                        <button class="btn small" onclick={() => joinServer(server.id)}><Icon name="arrow-right" size={12} />Join</button>
                      </div>
                    {/each}
                  </div>
                {/if}
                <button class="btn small tertiary" style="align-self: flex-start" onclick={() => (app.modal = { kind: "join_link" })}><Icon name="link" />Join a link…</button>
              </div>
            {:else if name === "running"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                {#if !snap.instances.length}
                  <span class="meta">Nothing's running.</span>
                {:else}
                  <div class="scroll-list">
                    {#each snap.instances as instance (instance.id)}
                      {@const game = snap.bootstrapper.games.find((g) => g.id === instance.game)}
                      {@const account = snap.bootstrapper.accounts.find((a) => a.id === instance.account)}
                      {@const hopping = snap.hopping.includes(`instance:${instance.id}`)}
                      <div class="running">
                        <Artwork path={game ? snap.images[artworkKey(game)] : null} name={game?.name ?? "?"} radius="var(--r-sm)" class="thumb" />
                        <div class="col grow" style="gap: 0">
                          <span class="item-title line">{game?.name ?? "Roblox"}</span>
                          <span class="secondary line">{account ? accountLabel(account) : "Browser account"} · {runtime(instance.started, now)}{instance.location ? ` · ${instance.location}` : ""}</span>
                        </div>
                      </div>
                      <div class="row actions">
                        <button class="btn small" onclick={() => (hide(), run("focus_instance", { instance: instance.id }))}><Icon name="focus" />Focus</button>
                        <button class="btn small" onclick={() => (app.modal = { kind: "servers", game: instance.game })}><Icon name="server" />Servers</button>
                        <button class="btn small" disabled={!serverLink(instance)} title="Copy a link that joins this exact server" onclick={() => copyServerLink(instance)}><Icon name="link" />Link</button>
                        <button class="btn small" disabled={instance.status !== "running" || hopping || !instance.account} onclick={() => serverHop({ instance: instance.id })}>
                          <Icon name="hop" />{hopping ? "Hopping…" : "Hop"}
                        </button>
                        <button class="btn small danger" aria-label="Close" onclick={() => closeInstance(instance.id)}><Icon name="power" /></button>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {:else if name === "record" && recorder && rec}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                {#if !recorder.installed}
                  <span class="meta">Set up recording in Pious → Settings → Recording.</span>
                {:else}
                  <div class="row" style="gap: 8px">
                    {#if recorder.recording_since !== null}
                      <span class="rec-dot"></span><span class="item-title grow">Recording · {Math.floor(recorder.recording_since / 60)}:{String(Math.floor(recorder.recording_since % 60)).padStart(2, "0")}</span>
                      <button class="btn small danger" onclick={() => invoke("toggle_recording")}><Icon name="power" />Stop</button>
                    {:else}
                      <span class="meta grow">{recorder.capturing ? `Clip buffer · ${Math.round(recorder.buffered)}s` : "Not recording"}</span>
                      <button class="btn small primary" onclick={() => invoke("toggle_recording")}><Icon name="record" />Record <span class="kbd">{keyLabel(rec.record_hotkey)}</span></button>
                    {/if}
                  </div>
                  {#if rec.clips}
                    <div class="row" style="gap: 6px; flex-wrap: wrap">
                      <span class="secondary">Save the last</span>
                      {#each [15, 30, 60, 120].filter((s) => s <= rec.buffer_seconds) as s (s)}
                        <button class="chip" disabled={!recorder.capturing} onclick={() => invoke("save_clip", { seconds: s })}>{s}s</button>
                      {/each}
                    </div>
                  {:else}
                    <span class="secondary">Turn on clips in Settings to save what just happened.</span>
                  {/if}
                {/if}
              </div>
            {:else if name === "quick"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                <div class="row" style="gap: 6px">
                  <button class="chip" class:on={quickTab === "recent"} onclick={() => (quickTab = "recent")}>Recent</button>
                  <button class="chip" class:on={quickTab === "favorites"} onclick={() => (quickTab = "favorites")}>Favorites</button>
                </div>
                <div class="games">
                  {#each quickTab === "recent" ? recent : favorites as game (game.id)}
                    <button class="card glass game" onclick={() => launch(game.id)}>
                      <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="0" class="art" />
                      <div class="col" style="gap: 0; padding: 6px 8px">
                        <span class="item-title line">{game.name}</span>
                        <span class="secondary line">{game.last_played ? relative(game.last_played) : "Not played yet"}</span>
                      </div>
                      <span class="play"><Icon name="play" size={15} /></span>
                    </button>
                  {:else}
                    <span class="meta">{quickTab === "recent" ? "Games you play show up here." : "Favorite games show up here."}</span>
                  {/each}
                </div>
              </div>
            {:else if name === "media"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                {#if !media}
                  <span class="meta">Nothing's playing. Start a song on Spotify or a video in your browser and it shows up here.</span>
                {:else}
                  <div class="media">
                    <span class="cover" class:spin={media.playing && !media.artwork}>
                      {#if media.artwork}<img src={media.artwork} alt="" />{:else}<Icon name="media" size={20} />{/if}
                    </span>
                    <div class="col grow" style="gap: 1px; min-width: 0">
                      <span class="item-title line" title={media.title}>{media.title || "Untitled"}</span>
                      <span class="secondary line">{media.artist || media.app}</span>
                      {#if media.artist}<span class="secondary line app">{media.app}</span>{/if}
                    </div>
                  </div>
                  {#if media.duration}
                    <div class="progress"><span style="width: {Math.min(100, ((mediaPosition ?? 0) / media.duration) * 100)}%"></span></div>
                    <div class="row times secondary">
                      <span>{clockOf(mediaPosition ?? 0)}</span><span class="spacer"></span><span>{clockOf(media.duration)}</span>
                    </div>
                  {/if}
                  <div class="row media-buttons">
                    <button class="icon-btn" aria-label="Previous" title="Previous" disabled={!media.can_previous} onclick={() => mediaPress("previous")}>
                      <Icon name="previous" size={16} />
                    </button>
                    <button class="btn primary media-play" aria-label={media.playing ? "Pause" : "Play"} disabled={!media.can_pause} onclick={() => mediaPress("toggle")}>
                      <Icon name={media.playing ? "pause" : "play"} size={16} />
                    </button>
                    <button class="icon-btn" aria-label="Next" title="Next" disabled={!media.can_next} onclick={() => mediaPress("next")}>
                      <Icon name="next" size={16} />
                    </button>
                  </div>
                {/if}
              </div>
            {:else if name === "ingame"}
              <div class="glass-elevated panel-box">
                {@render head(name)}
                <span class="secondary">Play as</span>
                <div class="row" style="gap: 6px; flex-wrap: wrap">
                  {#each snap.bootstrapper.accounts as account (account.id)}
                    <button class="chip" class:on={account.id === snap.active_account} onclick={() => run("set_active_account", { account: account.id })}>
                      <Avatar path={snap.images[avatarKey(account)]} name={accountLabel(account)} size={16} />{accountLabel(account)}
                    </button>
                  {/each}
                </div>
                {#if playing && snap.bootstrapper.accounts.length > 1}
                  {@const same = playing.account === snap.active_account}
                  {@const next = snap.bootstrapper.accounts.find((a) => a.id === snap.active_account)}
                  <div class="row quick">
                    <button class="btn small" disabled={same || !next || switching} onclick={quickSwitch}><Icon name="hop" size={14} />Quick switch</button>
                    <span class="secondary line">{same || !next ? "Pick another account above to swap it into this server" : `${accountLabel(next)} joins this server`}</span>
                  </div>
                {/if}
                <div class="toggle"><span class="grow">Anti-AFK</span><Switch on={snap.bootstrapper.preferences.anti_afk.enabled} onchange={(on) => setPreferences({ anti_afk: { enabled: on } })} /></div>
                <div class="toggle"><span class="grow">Rejoin when disconnected</span><Switch on={snap.bootstrapper.preferences.auto_rejoin} onchange={(on) => setPreferences({ auto_rejoin: on })} /></div>
                <div class="toggle"><span class="grow">Discord activity</span><Switch on={snap.bootstrapper.preferences.discord_presence} onchange={(on) => setPreferences({ discord_presence: on })} /></div>
              </div>
            {/if}
            {#if name !== "bar" && !w.pinned}
              <span
                class="grip"
                role="separator"
                aria-label="Resize"
                title="Drag to resize. Double-click for the normal size."
                onpointerdown={(e) => startResize(name, e)}
                ondblclick={() => save(name, { width: null, height: null })}
              ></span>
            {/if}
          </div>
        {/if}
      {/each}
    {/if}

    {#if clipMax !== null}
      <div class="clip-place">
        <form
          class="glass-elevated clip-prompt"
          onsubmit={(e) => {
            e.preventDefault();
            saveClip();
          }}
        >
          <div class="row" style="gap: 10px">
            <Icon name="clip" size={16} />
            <span class="item-title grow">Save a clip</span>
            <span class="secondary">up to {clipMax}s back</span>
          </div>
          <div class="row" style="gap: 8px">
            <span class="meta">The last</span>
            <input bind:this={clipInput} class="input seconds" type="number" min="1" max={clipMax} bind:value={clipSeconds} />
            <span class="meta grow">seconds before you pressed the key</span>
          </div>
          <div class="row" style="gap: 6px">
            {#each [10, 30, 60, 120, 300].filter((s) => s <= clipMax!) as s (s)}
              <button type="button" class="chip" onclick={() => saveClip(s)}>{s}s</button>
            {/each}
            <span class="spacer"></span>
            <button type="submit" class="btn primary small"><Icon name="check" />Save</button>
          </div>
        </form>
      </div>
    {/if}

    {#if snap}
      <ModalHost />
      {#if app.searchOpen}<SearchPalette onpick={pick} />{/if}
    {/if}
    <ContextMenu />
  </div>
{/if}
<Toasts />

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .stage {
    position: fixed;
    inset: 0;
  }
  .backdrop,
  .catcher {
    position: absolute;
    inset: 0;
    border: none;
    background: none;
  }
  .backdrop {
    opacity: 0;
    transition: opacity 220ms cubic-bezier(0.2, 0.7, 0.2, 1);
    pointer-events: none;
    overflow: hidden;
  }
  .open .backdrop {
    opacity: 1;
  }
  .still {
    position: absolute;
    inset: -60px;
    width: calc(100% + 120px);
    height: calc(100% + 120px);
    object-fit: cover;
    transform: scale(1.04);
    transition: transform 400ms cubic-bezier(0.2, 0.7, 0.2, 1);
  }
  .open .still {
    transform: scale(1);
  }
  .dim {
    position: absolute;
    inset: 0;
  }

  /* Panels rise in one after another, and settle back out. */
  .widget {
    position: absolute;
    width: min(400px, 28vw);
    opacity: 0;
    transform: translateY(14px) scale(0.98);
    transition:
      opacity 220ms ease,
      transform 300ms cubic-bezier(0.2, 0.8, 0.2, 1);
    transition-delay: calc(var(--i) * 30ms);
  }
  .open .widget {
    opacity: 1;
    transform: none;
  }
  .widget.dragging {
    transition: none;
    z-index: 5;
    will-change: left, top, width, height;
  }
  .widget.dragging > :global(.glass-elevated) {
    box-shadow: 0 24px 60px -20px rgb(0 0 0 / 0.75);
  }
  /* Resized panels fill their size, and their lists take the room. */
  .widget.sized > :global(.panel-box) {
    height: 100%;
    min-height: 0;
  }
  .widget.sized :global(.scroll-list),
  .widget.sized :global(.chat-box) {
    flex: 1;
    min-height: 0;
    max-height: none;
  }
  /* On the corner itself, mostly outside the panel, so it never covers
     what's inside (a switch or button in the bottom-right corner). */
  .quick {
    gap: 8px;
    margin-top: 2px;
  }
  .grip {
    position: absolute;
    right: -7px;
    bottom: -7px;
    width: 14px;
    height: 14px;
    z-index: 3;
    cursor: nwse-resize;
    opacity: 0;
    background:
      linear-gradient(
        135deg,
        transparent 52%,
        rgb(var(--surface) / 0.5) 52%,
        rgb(var(--surface) / 0.5) 60%,
        transparent 60%,
        transparent 72%,
        rgb(var(--surface) / 0.5) 72%,
        rgb(var(--surface) / 0.5) 80%,
        transparent 80%
      );
    transition: opacity var(--fast);
  }
  .widget:hover .grip,
  .widget.dragging .grip {
    opacity: 1;
  }
  .stage:not(.open) .widget {
    transition-delay: 0ms;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: rgb(var(--panel) / 0.96);
    flex-wrap: wrap;
  }
  .hidden-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    width: 100%;
  }
  .grab {
    cursor: grab;
    touch-action: none;
  }
  .grab:active {
    cursor: grabbing;
  }
  .logo {
    display: grid;
    width: 26px;
    height: 26px;
    color: rgb(var(--text));
  }
  .logo :global(svg) {
    width: 100%;
    height: 100%;
  }
  .clock {
    font-weight: 700;
    font-size: 18px;
    font-variant-numeric: tabular-nums;
  }
  .panel-box {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 12px;
    background: rgb(var(--panel) / 0.96);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 4px;
    user-select: none;
  }
  .title {
    font-weight: 600;
    font-size: 13px;
  }
  .mini {
    width: 22px;
    height: 22px;
  }
  .scroll-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 26vh;
    overflow-y: auto;
    margin-right: -6px;
    padding-right: 6px;
  }
  .friend {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .face {
    position: relative;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
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
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid rgb(var(--panel));
    background: #3e9cf5;
  }
  .status.in_game {
    background: #30a46c;
  }
  .status.in_studio {
    background: #f5a524;
  }
  .chat-box {
    height: 340px;
    margin: 0 -12px -12px;
  }
  .friend :global(.mini-thumb) {
    width: 48px;
    height: 27px;
    flex: none;
  }
  .running {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .running :global(.thumb) {
    width: 64px;
    height: 36px;
    flex: none;
  }
  .actions {
    gap: 6px;
    padding-left: 74px;
    margin-bottom: 4px;
  }
  .rec-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #e5484d;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .games {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .game {
    display: flex;
    flex-direction: column;
    padding: 0;
    text-align: left;
    color: inherit;
  }
  .game :global(.art) {
    aspect-ratio: 16 / 9;
    width: 100%;
  }
  .play {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
    opacity: 0;
    transform: scale(0.85);
    transition: opacity var(--fast), transform var(--fast);
    z-index: 3;
  }
  .game:hover .play {
    opacity: 1;
    transform: none;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
  }
  .media {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .cover {
    display: grid;
    place-items: center;
    width: 58px;
    height: 58px;
    flex: none;
    overflow: hidden;
    border-radius: 10px;
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--muted));
    box-shadow: 0 8px 20px -10px rgb(0 0 0 / 0.7);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fade 300ms var(--ease);
  }
  .cover.spin :global(.icon) {
    animation: spin 3s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .app {
    font-size: 11px;
    opacity: 0.8;
  }
  .progress {
    height: 4px;
    border-radius: 99px;
    overflow: hidden;
    background: rgb(var(--surface) / 0.1);
  }
  .progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: rgb(var(--accent));
    transition: width 1s linear;
  }
  .times {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    margin-top: -4px;
  }
  .media-buttons {
    justify-content: center;
    gap: 14px;
  }
  .media-buttons .media-play {
    width: 40px;
    height: 40px;
    padding: 0;
    border-radius: 50%;
    justify-content: center;
    transition: transform 200ms var(--ease);
  }
  .media-buttons .media-play:not(:disabled):hover {
    transform: scale(1.07);
  }
  .media-buttons .media-play:active {
    transform: scale(0.94);
  }
  .clip-place {
    position: absolute;
    inset: 9vh 0 auto 0;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .clip-prompt {
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 420px;
    padding: 14px 16px;
    background: rgb(var(--panel) / 0.97);
    animation: rise 240ms var(--ease);
  }
  .seconds {
    width: 84px;
    height: 30px;
    text-align: center;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
