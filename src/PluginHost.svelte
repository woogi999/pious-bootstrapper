<!-- The hidden window plugins' engines run in: one walled-off frame per
     enabled plugin with a "main" script, each talking to Pious only through
     the bridge (lib/pluginbridge.ts), like plugin pages do. Pious makes this
     window while some plugin has an engine, and closes it when none has. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { app, connect } from "./lib/state.svelte";
  import { bridge, pageSrc, type BridgedPlugin } from "./lib/pluginbridge";

  type Engine = BridgedPlugin & { page: string };
  let engines = $state<Engine[]>([]);
  const frames: Record<string, HTMLIFrameElement | undefined> = $state({});
  const links = new Map<string, ReturnType<typeof bridge>>();

  async function load() {
    const next = await invoke<Engine[]>("plugin_engines").catch(() => []);
    // Engines whose plugin went away (or changed) stop; new ones start.
    for (const [id, link] of links) {
      const still = next.find((e) => e.id === id);
      const before = engines.find((e) => e.id === id);
      if (!still || JSON.stringify(still) !== JSON.stringify(before)) {
        link.disconnect();
        links.delete(id);
      }
    }
    engines = next;
    for (const e of next) {
      if (!links.has(e.id)) {
        links.set(e.id, bridge(() => engines.find((x) => x.id === e.id), "engine", () => frames[e.id]));
      }
    }
  }

  $effect(() => {
    app.snap;
    for (const link of links.values()) link.snapshotChanged();
  });

  onMount(() => {
    connect();
    load();
    const off = listen("plugin-engines-changed", load);
    return () => {
      off.then((f) => f());
      for (const link of links.values()) link.disconnect();
    };
  });
</script>

{#each engines as e (e.id + e.page)}
  <iframe bind:this={frames[e.id]} src={pageSrc(e.page)} title={e.name} sandbox="allow-scripts"></iframe>
{/each}

<style>
  iframe {
    width: 1px;
    height: 1px;
    border: none;
  }
</style>
