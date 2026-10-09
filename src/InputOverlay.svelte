<!-- The input overlay: the keys and mouse buttons you press, drawn over
     the game like streamers' key overlays, with clicks and keys per second.
     (The frame rate and other numbers are the game stats overlay's job.) It
     never takes the mouse, except while it's being moved (Settings →
     Overlay → Move it). It animates in when it shows up and out before its
     window hides. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app, connect } from "./lib/state.svelte";
  import { keyName, layoutRows } from "./lib/inputLayouts";
  import type { InputOverlay, InputState } from "./lib/types";

  const o = $derived<InputOverlay | undefined>(app.snap?.bootstrapper.preferences.input_overlay);
  const rows = $derived(o ? layoutRows(o) : []);
  const slanted = $derived(o?.style !== "flat");

  let held = $state<Set<string>>(new Set());
  let editing = $state(false);
  // Shown (animated in) or leaving (animating out).
  let visible = $state(false);
  // Wheel notches flash briefly.
  let wheel = $state<"up" | "down" | null>(null);
  let wheelTimer: ReturnType<typeof setTimeout> | undefined;
  let last: InputState | null = null;
  // Presses in the last second, for the rates.
  let clicks: number[] = [];
  let keys: number[] = [];
  let cps = $state(0);
  let kps = $state(0);

  function apply(state: InputState) {
    held = new Set(state.keys);
    const now = performance.now();
    if (last) {
      for (let i = 0; i < state.clicks - last.clicks && i < 50; i++) clicks.push(now);
      for (let i = 0; i < state.key_presses - last.key_presses && i < 50; i++) keys.push(now);
      const turned = state.wheel_up > last.wheel_up ? "up" : state.wheel_down > last.wheel_down ? "down" : null;
      if (turned) {
        wheel = turned;
        clearTimeout(wheelTimer);
        wheelTimer = setTimeout(() => (wheel = null), 160);
      }
    }
    last = state;
    rate();
  }

  function rate() {
    const cutoff = performance.now() - 1000;
    clicks = clicks.filter((t) => t > cutoff);
    keys = keys.filter((t) => t > cutoff);
    cps = clicks.length;
    kps = keys.length;
  }

  // Tell Rust how big the overlay is, so its window fits.
  let root = $state<HTMLElement>();
  $effect(() => {
    if (!root) return;
    const observer = new ResizeObserver(() => {
      const r = root!.getBoundingClientRect();
      invoke("input_overlay_resize", { width: Math.ceil(r.width) + 24, height: Math.ceil(r.height) + 24 }).catch(() => {});
    });
    observer.observe(root);
    return () => observer.disconnect();
  });

  onMount(() => {
    connect();
    const offState = listen<InputState>("input-state", (e) => apply(e.payload));
    const offEdit = listen<boolean>("input-edit", (e) => (editing = e.payload));
    const offVisible = listen<boolean>("input-visible", (e) => (visible = e.payload));
    invoke<InputState>("input_state").then(apply).catch(() => {});
    // The window was made because it's wanted: come in once painted.
    requestAnimationFrame(() => requestAnimationFrame(() => (visible = true)));
    // Rates fall back to zero when nothing's pressed.
    const decay = setInterval(rate, 250);
    return () => {
      offState.then((f) => f());
      offEdit.then((f) => f());
      offVisible.then((f) => f());
      clearInterval(decay);
    };
  });

  function drag(e: MouseEvent) {
    if (editing && e.button === 0 && !(e.target as HTMLElement).closest("button")) getCurrentWindow().startDragging();
  }

  // A running count for the entrance, so keys come in one after another.
  const order = $derived.by(() => {
    const out: number[][] = [];
    let n = 0;
    for (const r of rows) out.push(r.map(() => n++));
    return { keys: out, total: n };
  });
</script>

{#if o}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- The window is sized to this frame: the overlay and, while it's being
       moved, the bar under it (its own row, so it's never cut off). -->
  <div bind:this={root} class="frame" class:editing onmousedown={drag}>
  <div
    class="overlay anim-{o.animation}"
    class:in={visible}
    class:slanted
    class:editing
    class:bold={o.bold}
    class:upper={o.uppercase}
    style="
      --size: {o.key_size * o.scale}px;
      --gap: {o.gap * o.scale}px;
      --radius: {o.radius * o.scale}px;
      --border: {Math.max(0, o.border * Math.min(1.5, Math.max(0.6, o.scale)))}px;
      --lean: {slanted ? -o.slant : 0}deg;
      --bg: {o.background};
      --key: {o.key_color};
      --pressed: {o.pressed_color};
      --text: {o.text_color};
      --pressed-text: {o.pressed_text_color};
      --edge: {o.border_color};
      --font: {o.font}, Manrope, system-ui, sans-serif;
      --font-size: {o.font_size * o.scale}px;
      --release: {o.release_ms}ms;
      --opacity: {o.opacity};
    "
  >
    {#if rows.length}
      <div class="keys">
        {#each rows as keyRow, r (r)}
          <div class="row">
            {#each keyRow as key, i (`${r}-${i}`)}
              {#if key.code}
                <span
                  class="key"
                  class:down={held.has(key.code)}
                  style="width: calc(var(--size) * {key.width} + var(--gap) * {key.width - 1}); --d: {order.keys[r][i] * 14}ms"
                >
                  <span class="label">{keyName(key.code)}</span>
                </span>
              {:else}
                <span class="key blank" style="width: var(--size)"></span>
              {/if}
            {/each}
          </div>
        {/each}
      </div>
    {/if}

    {#if o.show_mouse || o.show_rates}
      <div class="side" style="--d: {order.total * 14}ms">
        {#if o.show_mouse}
          <svg class="mouse" viewBox="-10 -2 120 164" aria-hidden="true">
            <!-- Left and right buttons, cut around the wheel. -->
            <path class="part" class:down={held.has("MouseLeft")} d="M6 64 V28 Q6 4 30 4 H42 Q47 4 47 9 V20 Q38 22 38 31 V50 Q38 59 47 61 V64 Z" />
            <path class="part" class:down={held.has("MouseRight")} d="M94 64 V28 Q94 4 70 4 H58 Q53 4 53 9 V20 Q62 22 62 31 V50 Q62 59 53 61 V64 Z" />
            <rect
              class="part wheel"
              class:down={held.has("MouseMiddle") || (o.show_scroll && wheel !== null)}
              class:up={o.show_scroll && wheel === "up"}
              class:dn={o.show_scroll && wheel === "down"}
              x="43"
              y="24"
              width="14"
              height="32"
              rx="7"
            />
            <path class="part body" d="M6 70 H94 V108 Q94 158 50 158 Q6 158 6 108 Z" />
            <!-- Side buttons. -->
            <rect class="part" class:down={held.has("MouseForward")} x="-6" y="76" width="9" height="20" rx="4" />
            <rect class="part" class:down={held.has("MouseBack")} x="-6" y="100" width="9" height="20" rx="4" />
          </svg>
        {/if}
        {#if o.show_rates}
          <div class="rates">
            <span class="key rate"><span class="label">{cps}<small>CPS</small></span></span>
            <span class="key rate"><span class="label">{kps}<small>KPS</small></span></span>
          </div>
        {/if}
      </div>
    {/if}

  </div>
  {#if editing}
    <div class="edit-bar">
      <span>Drag to move</span>
      <button onclick={() => invoke("input_overlay_edit", { on: false })}>Done</button>
    </div>
  {/if}
  </div>
{/if}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .frame {
    position: absolute;
    top: 12px;
    left: 12px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .frame.editing {
    cursor: move;
  }
  .overlay {
    display: flex;
    align-items: flex-end;
    gap: calc(var(--gap) * 2.5);
    padding: calc(var(--gap) * 1.5) calc(var(--gap) * 1.5 + var(--size) * 0.12);
    border-radius: calc(var(--radius) + var(--gap));
    background: var(--bg);
    font-family: var(--font);
    font-size: var(--font-size);
    color: var(--text);
    user-select: none;
    opacity: 0;
    transform: translateY(10px) scale(0.97);
    filter: blur(4px);
    transition:
      opacity 260ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 300ms cubic-bezier(0.4, 0, 0.2, 1),
      filter 260ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  .overlay.in {
    opacity: var(--opacity);
    transform: none;
    filter: none;
    transition:
      opacity 320ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 420ms cubic-bezier(0.22, 1, 0.36, 1),
      filter 320ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .overlay.editing {
    outline: 2px dashed rgb(255 255 255 / 0.7);
    outline-offset: 2px;
  }
  .bold {
    font-weight: 800;
  }
  .upper {
    text-transform: uppercase;
  }
  .keys {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
  }
  .row {
    display: flex;
    gap: var(--gap);
  }

  /* A key. Its transform is built from parts, so the lean, the entrance and
     the press all apply together. */
  .key {
    --lift: 0px;
    --press: 1;
    display: grid;
    place-items: center;
    flex: none;
    height: var(--size);
    min-width: var(--size);
    border-radius: var(--radius);
    background: var(--key);
    border: var(--border) solid var(--edge);
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    transform: skewX(var(--lean)) translateY(var(--lift)) scale(var(--press));
    transition:
      background var(--release) ease-out,
      color var(--release) ease-out,
      transform var(--release) ease-out,
      box-shadow var(--release) ease-out,
      opacity var(--release) ease-out;
  }
  .label {
    line-height: 1;
    letter-spacing: 0.01em;
  }
  /* Slanted keys lean their letters the same way, like the keys do. */
  .slanted .label {
    display: inline-block;
    transform: skewX(calc(var(--lean) * 0.6));
  }
  .key.blank {
    visibility: hidden;
  }
  .key.down {
    background: var(--pressed);
    color: var(--pressed-text);
    transition: none;
  }
  .anim-pop .key.down {
    --press: 0.9;
  }
  .anim-glow .key.down {
    box-shadow:
      0 0 calc(var(--size) * 0.35) var(--pressed),
      0 0 calc(var(--size) * 0.1) var(--pressed);
  }
  .anim-fade .key:not(.down):not(.rate) {
    opacity: 0.55;
  }

  /* Coming in and going out: keys drop into place one after another, and
     leave together. */
  .overlay:not(.in) .key:not(.blank) {
    --lift: 8px;
    --press: 0.88;
    opacity: 0;
    transition-duration: 200ms;
  }
  .overlay.in .key:not(.down) {
    transition:
      background var(--release) ease-out,
      color var(--release) ease-out,
      box-shadow var(--release) ease-out,
      opacity 360ms cubic-bezier(0.22, 1, 0.36, 1) var(--d, 0ms),
      transform 420ms cubic-bezier(0.34, 1.4, 0.64, 1) var(--d, 0ms);
  }

  .side {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--gap) * 1.5);
    transition:
      opacity 360ms cubic-bezier(0.22, 1, 0.36, 1) var(--d, 0ms),
      transform 420ms cubic-bezier(0.22, 1, 0.36, 1) var(--d, 0ms);
  }
  .overlay:not(.in) .side {
    opacity: 0;
    transform: translateX(-8px);
    transition-delay: 0ms;
  }

  .mouse {
    width: calc(var(--size) * 2.3);
    height: calc(var(--size) * 3.1);
    overflow: visible;
  }
  .part {
    fill: var(--key);
    stroke: var(--edge);
    stroke-width: calc(var(--border) * 1.15);
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
    transition:
      fill var(--release) ease-out,
      transform 120ms ease-out;
  }
  .part.down {
    fill: var(--pressed);
    transition: none;
  }
  .wheel {
    transform-box: fill-box;
    transform-origin: center;
  }
  .wheel.up {
    transform: translateY(-3px);
  }
  .wheel.dn {
    transform: translateY(3px);
  }

  .rates {
    display: flex;
    gap: var(--gap);
  }
  .rate {
    min-width: calc(var(--size) * 1.25);
    height: calc(var(--size) * 0.72);
    padding: 0 calc(var(--size) * 0.18);
    font-variant-numeric: tabular-nums;
  }
  .rate small {
    font-size: 0.62em;
    opacity: 0.75;
    margin-left: 0.3em;
  }
  .edit-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 12px;
    border-radius: 99px;
    background: rgb(0 0 0 / 0.65);
    white-space: nowrap;
    font: 600 12px Manrope, system-ui, sans-serif;
    text-transform: none;
    color: white;
    text-shadow: 0 1px 3px black;
  }
  .edit-bar button {
    padding: 3px 12px;
    border: none;
    border-radius: 99px;
    background: white;
    color: black;
    font: inherit;
    cursor: pointer;
  }
</style>
