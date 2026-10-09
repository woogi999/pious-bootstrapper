<!-- A plugin's page, walled off in a frame of its own. It can only talk to
     Pious through messages (see lib/pluginbridge.ts), and only for what its
     manifest asked for. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { bridge, pageSrc } from "../lib/pluginbridge";
  import EmptyState from "../components/EmptyState.svelte";

  let { id }: { id: string } = $props();

  const snap = $derived(app.snap!);
  const plugin = $derived(snap.plugins.find((p) => p.id === id && p.enabled));
  const src = $derived(plugin?.page ? pageSrc(plugin.page) : null);
  let frame = $state<HTMLIFrameElement>();

  const link = bridge(() => plugin, "page", () => frame);
  $effect(() => () => link.disconnect());

  // Live updates for plugins that asked for them.
  $effect(() => {
    app.snap;
    link.snapshotChanged();
  });
</script>

{#if !plugin}
  <EmptyState icon="plugins" title="This plugin is off" body="Turn it on in Settings → Plugins." />
{:else if !src}
  <EmptyState icon="plugins" title="{plugin.name} has no page" body="It works in the background." />
{:else}
  {#key src}
    <iframe bind:this={frame} {src} title={plugin.name} sandbox="allow-scripts allow-forms allow-popups allow-downloads" class="frame"></iframe>
  {/key}
{/if}

<style>
  .frame {
    width: 100%;
    height: calc(100vh - 130px);
    border: none;
    border-radius: var(--r-lg);
    background: transparent;
    animation: rise 300ms var(--ease) both;
  }
</style>
