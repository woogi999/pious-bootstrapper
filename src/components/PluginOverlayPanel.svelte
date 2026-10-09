<!-- A plugin's panel in the in-game overlay ("overlay" in its plugin.json):
     its page, walled off like plugin pages in the app, talking to Pious only
     through the bridge (lib/pluginbridge.ts), with the same permissions. It
     takes the height of its content (the page says how tall it is), up to
     part of the screen, unless the panel was resized. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { bridge, pageSrc } from "../lib/pluginbridge";
  import type { Plugin } from "../lib/types";

  let { plugin, sized = false }: { plugin: Plugin; sized?: boolean } = $props();

  const src = $derived(plugin.overlay ? pageSrc(plugin.overlay) : null);
  let frame = $state<HTMLIFrameElement>();
  let height = $state(120);

  const link = bridge(() => app.snap?.plugins.find((p) => p.id === plugin.id && p.enabled), "page", () => frame);
  $effect(() => () => link.disconnect());
  $effect(() => {
    app.snap;
    link.snapshotChanged();
  });

  onMount(() => {
    const onsize = (event: MessageEvent) => {
      if (!frame || event.source !== frame.contentWindow) return;
      const m = event.data;
      if (m && m.pious === "size" && typeof m.height === "number") height = m.height;
    };
    window.addEventListener("message", onsize);
    return () => window.removeEventListener("message", onsize);
  });
</script>

{#if src}
  {#key src}
    <iframe
      bind:this={frame}
      {src}
      title={plugin.name}
      sandbox="allow-scripts allow-forms"
      class="frame"
      class:sized
      style={sized ? "" : `height: min(${Math.max(48, height)}px, 60vh)`}
    ></iframe>
  {/key}
{/if}

<style>
  .frame {
    display: block;
    width: 100%;
    border: none;
    border-radius: var(--r-md);
    background: transparent;
    color-scheme: dark;
    transition: height 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .frame.sized {
    flex: 1;
    min-height: 0;
  }
</style>
