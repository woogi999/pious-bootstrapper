<!-- The Pious mark. At rest it's the flat logo; each time `spins` goes up
     it turns once as a real 3D icosahedron (shaded, with the holographic
     sheen of the opening logo) and lands back on the flat mark. -->
<script lang="ts">
  import logo from "../../assets/brand/logo.svg?raw";
  import { drawMark, textColor } from "../lib/icosahedron";

  let { spins = 0, size = 28, reduce = false }: { spins?: number; size?: number; reduce?: boolean } = $props();

  const DURATION = 950; // ms
  let canvas = $state<HTMLCanvasElement>();
  let spinning = $state(false);
  let raf = 0;

  const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

  function spin() {
    cancelAnimationFrame(raf);
    spinning = true;
    // The canvas appears on the next frame.
    requestAnimationFrame(() => {
      const ctx = canvas?.getContext("2d");
      if (!ctx || !canvas) {
        spinning = false;
        return;
      }
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(size * dpr);
      canvas.height = Math.round(size * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const text = textColor();
      const start = performance.now();
      const frame = (now: number) => {
        const t = Math.min(1, (now - start) / DURATION);
        const e = easeInOut(t);
        // Shaded and holographic while it turns, flat again as it lands.
        const solid = Math.sin(Math.PI * Math.min(1, t * 1.15));
        drawMark(
          ctx,
          size,
          {
            // One full turn around y (the mark comes back to its pose),
            // tipping toward the viewer and back on the way.
            yaw: e * Math.PI * 2,
            pitch: Math.sin(Math.PI * e) * 0.55,
            roll: Math.sin(Math.PI * e) * -0.25,
            flat: 1 - solid,
            scale: 1 + 0.12 * Math.sin(Math.PI * e),
            shine: e * Math.PI * 2 + 0.8,
          },
          text,
        );
        if (t < 1) raf = requestAnimationFrame(frame);
        else spinning = false;
      };
      raf = requestAnimationFrame(frame);
    });
  }

  let seen = 0;
  $effect(() => {
    if (spins === seen) return;
    seen = spins;
    if (!reduce) spin();
  });
  $effect(() => () => cancelAnimationFrame(raf));
</script>

<span class="mark" style="width: {size}px; height: {size}px">
  {#if spinning}
    <canvas bind:this={canvas} style="width: {size}px; height: {size}px"></canvas>
  {:else}
    {@html logo}
  {/if}
</span>

<style>
  .mark {
    display: grid;
    place-items: center;
  }
  .mark :global(svg),
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
