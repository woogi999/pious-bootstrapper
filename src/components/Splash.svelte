<!-- The opening logo, floating on the desktop in a window of its own: a
     white, holographic icosahedron spins in, "Pious" appears beside it, and
     the shape settles into the flat logo before the app fades in. Drawn on
     a canvas from the logo's own geometry, so it lands exactly on the flat
     mark. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { Snapshot } from "../lib/types";
  import { drawMark, textColor } from "../lib/icosahedron";

  // Ready once the library has loaded (the app has something to show).
  let ready = $state(false);
  let reduce = $state(false);
  /** How long the logo takes to fade out (matches the CSS below). */
  const FADE_MS = 560;
  // The app's window opens only once the logo is completely gone, so the
  // two never show at the same time.
  const ondone = () => setTimeout(() => invoke("splash_done").catch(() => {}), reduce ? 0 : FADE_MS);
  async function poll() {
    try {
      const snap = await invoke<Snapshot>("get_snapshot");
      reduce = snap.bootstrapper.preferences.reduce_motion;
      ready = snap.loaded;
    } catch {
      // Not up yet.
    }
    if (!ready) setTimeout(poll, 150);
  }
  poll();

  let canvas = $state<HTMLCanvasElement>();
  let phase = $state<"spin" | "word" | "flat" | "out">("spin");
  let finished = false;

  const SIZE = 150; // CSS pixels
  /** Seconds: spin until SETTLE; the word from WORD; flat from FLAT. */
  const SETTLE = 2.1;
  const WORD = 0.75;
  const FLAT = 1.5;
  const FLAT_END = 2.3;
  const HOLD = 2.7;

  const easeOut = (t: number) => 1 - Math.pow(1 - Math.min(1, Math.max(0, t)), 3);
  const easeInOut = (t: number) => {
    t = Math.min(1, Math.max(0, t));
    return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
  };

  onMount(() => {
    if (reduce) {
      phase = "flat";
    }
    const ctx = canvas?.getContext("2d");
    if (!ctx || !canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = SIZE * dpr;
    canvas.height = SIZE * dpr;
    ctx.scale(dpr, dpr);
    const text = textColor();

    let start = performance.now();
    let raf = 0;
    // The window starts hidden and shows once the first frame is drawn, so
    // an empty window never flashes before the logo.
    let visible = false;
    const reveal = () => {
      if (visible) return;
      visible = true;
      start = performance.now();
      getCurrentWindow().show().catch(() => {});
    };

    const draw = (time: number) => {
      const t = reduce ? 99 : (time - start) / 1000;
      const settle = easeOut(t / SETTLE);
      // Two turns around y and one around x, slowing into the logo pose.
      // Turning into the flat mark: the shading and reflection fade into
      // one solid color while the gaps between faces open up, with a small
      // breath as it lands.
      const flat = easeInOut((t - FLAT) / (FLAT_END - FLAT));
      drawMark(
        ctx,
        SIZE,
        {
          yaw: (1 - settle) * Math.PI * 4,
          pitch: (1 - settle) * Math.PI * 2,
          roll: (1 - settle) * 0.6,
          flat,
          scale: 1 + 0.035 * Math.sin(Math.PI * flat),
          shine: t * 1.6 + 0.8,
        },
        text,
      );
      if (!visible) {
        reveal();
        raf = requestAnimationFrame(draw);
        return;
      }

      if (t >= WORD && phase === "spin") phase = "word";
      if (t >= FLAT && phase === "word") phase = "flat";
      if (t < HOLD || !ready) {
        raf = requestAnimationFrame(draw);
        return;
      }
      if (!finished) {
        finished = true;
        phase = "out";
        ondone();
      }
    };
    raf = requestAnimationFrame(draw);

    // Ready might arrive later; keep drawing until then (cheap).
    const restart = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(draw);
    };
    const watch = setInterval(() => {
      if (ready && !finished) restart();
    }, 250);
    // Never block the app for long, whatever happens.
    const failsafe = setTimeout(() => {
      if (!finished) {
        finished = true;
        phase = "out";
        ondone();
      }
    }, 9000);
    return () => {
      cancelAnimationFrame(raf);
      clearInterval(watch);
      clearTimeout(failsafe);
      start = 0;
    };
  });
</script>

<div class="splash phase-{phase}" class:reduce aria-hidden="true">
  <div class="mark">
    <canvas bind:this={canvas} style="width: {SIZE}px; height: {SIZE}px"></canvas>
    <span class="word">Pious</span>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .splash {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    transition:
      opacity 520ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 620ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  .splash.phase-out {
    opacity: 0;
    transform: scale(1.04);
  }
  /* The main layout: the mark, then the name to its right. */
  .mark {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 22px;
  }
  canvas {
    display: block;
    filter: drop-shadow(0 16px 30px rgb(0 0 0 / 0.55));
  }
  .word {
    font-weight: 800;
    font-size: 66px;
    line-height: 1;
    letter-spacing: 0.3em;
    margin-right: -0.3em;
    color: rgb(var(--text));
    text-shadow: 0 10px 30px rgb(0 0 0 / 0.55);
    opacity: 0;
    transform: translateX(-22px);
    filter: blur(8px);
    transition:
      opacity 700ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 800ms cubic-bezier(0.22, 1, 0.36, 1),
      letter-spacing 900ms cubic-bezier(0.22, 1, 0.36, 1),
      filter 700ms;
  }
  .phase-word .word,
  .phase-flat .word,
  .phase-out .word {
    opacity: 1;
    transform: none;
    filter: none;
    letter-spacing: 0.02em;
    margin-right: -0.02em;
  }
  .reduce .word {
    transition: none;
  }
</style>
