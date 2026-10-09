<script lang="ts">
  import { icon } from "../lib/icons";
  import { overrides } from "../lib/overrides.svelte";

  let { name, size = 15, class: className = "" }: { name: string; size?: number; class?: string } = $props();
  // A plugin's or theme's picture for this icon: shown as a mask, so it takes
  // the text color like Pious's own icons (and an SVG file can't run code).
  const custom = $derived(overrides.icons[name]);
</script>

{#if custom}
  <span class="icon custom {className}" style="--size: {size}px; --mask: url('{custom}')"></span>
{:else}
  <span class="icon {className}" style="--size: {size}px">{@html icon(name)}</span>
{/if}

<style>
  .icon {
    display: inline-grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    flex: none;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }
  .custom {
    background: currentColor;
    mask: var(--mask) center / contain no-repeat;
    -webkit-mask: var(--mask) center / contain no-repeat;
  }
</style>
