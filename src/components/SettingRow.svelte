<script lang="ts">
  import type { Snippet } from "svelte";

  // `icon` is still accepted; rows don't show icons.
  let {
    icon: _icon,
    title,
    description,
    off = false,
    children,
  }: { icon: string; title: string; description: string; off?: boolean; children?: Snippet } = $props();
</script>

<div class="item" class:off>
  <div class="col grow" style="gap: 1px"><span class="item-title">{title}</span><span class="meta">{description}</span></div>
  {#if children}{@render children()}{/if}
</div>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    transition: opacity var(--med);
  }
  .item.off {
    opacity: 0.4;
  }
</style>
