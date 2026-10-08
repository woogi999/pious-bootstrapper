<script lang="ts">
  import { icon } from "../lib/icons";

  let { name, size = 15, class: className = "" }: { name: string; size?: number; class?: string } = $props();
</script>

<span class="icon {className}" style="--size: {size}px">{@html icon(name)}</span>

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
</style>
