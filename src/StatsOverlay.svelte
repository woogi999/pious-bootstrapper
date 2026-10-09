<!-- The stats overlay: frame rate, the server's ping and players, where the
     server is, how long you've played and what Roblox uses on this PC, drawn
     over the game (see service/stats.rs for where each number comes from).
     It never takes the mouse, except while it's being moved. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, connect } from "./lib/state.svelte";
  import type { StatItem, StatsOverlay, StatsReading } from "./lib/types";

  const o = $derived<StatsOverlay | undefined>(app.snap?.bootstrapper.preferences.stats_overlay);
  let reading = $state<StatsReading | null>(null);
  let visible = $state(false);
  let editing = $state(false);
  let now = $state(new Date());

  const LABELS: Record<StatItem, string> = {
    fps: "FPS",
    ping: "Ping",
    players: "Players",
    server_fps: "Server FPS",
    location: "Server",
    session: "Playing",
    cpu: "CPU",
    memory: "RAM",
    clock: "Time",
  };

  function duration(seconds: number) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    return h ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
  }

  /** What one item shows, and whether its value is good, so-so or bad. */
  function value(item: StatItem, r: StatsReading | null): { text: string; tone?: "good" | "warn" | "bad" } {
    const none = { text: "—" };
    if (item === "clock") return { text: now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) };
    if (!r) return none;
    switch (item) {
      case "fps":
        return r.fps === null ? none : { text: String(r.fps), tone: r.fps >= 55 ? "good" : r.fps >= 30 ? "warn" : "bad" };
      case "ping":
        if (r.ping === null) return { text: r.private ? "Private" : "—" };
        return { text: `${r.ping} ms`, tone: r.ping <= 80 ? "good" : r.ping <= 160 ? "warn" : "bad" };
      case "players":
        return r.players ? { text: `${r.players[0]}/${r.players[1]}` } : r.private ? { text: "Private" } : none;
      case "server_fps":
        return r.server_fps === null ? none : { text: r.server_fps.toFixed(0), tone: r.server_fps >= 55 ? "good" : r.server_fps >= 40 ? "warn" : "bad" };
      case "location":
        return r.location ? { text: r.location.split(",")[0] } : none;
      case "session":
        return r.session_seconds === null ? none : { text: duration(r.session_seconds) };
      case "cpu":
        return r.cpu === null ? none : { text: `${r.cpu.toFixed(0)}%` };
      case "memory":
        return r.memory === null ? none : { text: r.memory >= 1 << 30 ? `${(r.memory / 2 ** 30).toFixed(1)} GB` : `${Math.round(r.memory / 2 ** 20)} MB` };
    }
  }

  const TITLES: Partial<Record<StatItem, string>> = {
    ping: "The server's average ping, as Roblox's server list reports it",
    server_fps: "How fast the server is running (Roblox's server list)",
    fps: "Frames the game showed in the last second",
  };

  // Fit the window to what's shown.
  let root = $state<HTMLElement>();
  $effect(() => {
    if (!root) return;
    const observer = new ResizeObserver(() => {
      const r = root!.getBoundingClientRect();
      invoke("stats_overlay_resize", { width: Math.ceil(r.width) + 16, height: Math.ceil(r.height) + 16 }).catch(() => {});
    });
    observer.observe(root);
    return () => observer.disconnect();
  });

  onMount(() => {
    connect();
    const offs = [
      listen<StatsReading>("stats", (e) => (reading = e.payload)),
      listen<boolean>("stats-visible", (e) => (visible = e.payload)),
      listen<boolean>("stats-edit", (e) => (editing = e.payload)),
    ];
    requestAnimationFrame(() => requestAnimationFrame(() => (visible = true)));
    const tick = setInterval(() => (now = new Date()), 10_000);
    return () => {
      for (const off of offs) off.then((f) => f());
      clearInterval(tick);
    };
  });

  function drag(e: MouseEvent) {
    if (editing && e.button === 0 && !(e.target as HTMLElement).closest("button")) getCurrentWindow().startDragging();
  }
</script>

{#if o}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={root}
    class="stats {o.layout}"
    class:in={visible}
    class:editing
    onmousedown={drag}
    style="
      --scale: {o.scale};
      --bg: {o.background};
      --text: {o.text_color};
      --dim: {o.label_color};
      --font: {o.font}, Manrope, system-ui, sans-serif;
      --opacity: {o.opacity};
    "
  >
    {#each o.items as item, i (item)}
      {@const v = value(item, reading)}
      <span class="stat" style="--d: {i * 40}ms" title={TITLES[item] ?? ""}>
        {#if o.labels}<span class="name">{LABELS[item]}</span>{/if}
        <span class="value {v.tone ?? ''}">{v.text}</span>
      </span>
    {/each}
    {#if editing}
      <button class="done" onclick={() => invoke("stats_overlay_edit", { on: false })}>Done</button>
    {/if}
  </div>
{/if}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .stats {
    position: absolute;
    top: 8px;
    left: 8px;
    display: flex;
    align-items: center;
    gap: calc(14px * var(--scale));
    padding: calc(6px * var(--scale)) calc(12px * var(--scale));
    border-radius: calc(10px * var(--scale));
    background: var(--bg);
    color: var(--text);
    font-family: var(--font);
    font-size: calc(13px * var(--scale));
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    user-select: none;
    opacity: 0;
    transform: translateY(-6px);
    transition:
      opacity 260ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 320ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .stats.in {
    opacity: var(--opacity);
    transform: none;
  }
  .stats.column {
    flex-direction: column;
    align-items: stretch;
    gap: calc(3px * var(--scale));
  }
  .stats.editing {
    outline: 2px dashed rgb(255 255 255 / 0.7);
    outline-offset: 2px;
    cursor: move;
  }
  .stat {
    display: inline-flex;
    align-items: baseline;
    gap: calc(5px * var(--scale));
    transition: opacity 300ms ease var(--d);
  }
  .column .stat {
    justify-content: space-between;
    gap: calc(16px * var(--scale));
  }
  .stats:not(.in) .stat {
    opacity: 0;
  }
  .name {
    color: var(--dim);
    font-size: 0.78em;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .value {
    font-weight: 700;
  }
  .value.good {
    color: #4ade80;
  }
  .value.warn {
    color: #facc15;
  }
  .value.bad {
    color: #f87171;
  }
  .done {
    padding: 2px 10px;
    border: none;
    border-radius: 99px;
    background: white;
    color: black;
    font: 600 12px Manrope, system-ui, sans-serif;
    cursor: pointer;
  }
</style>
