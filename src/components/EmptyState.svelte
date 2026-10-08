<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { icon, title, body, children }: { icon: string; title: string; body: string; children?: Snippet } = $props();
</script>

<div class="glass-base empty">
  <span class="tile-icon" style="width: 52px; height: 52px; border-radius: 50%"><Icon name={icon} size={22} /></span>
  <div class="col" style="align-items: center; gap: 4px">
    <span style="font-weight: 600; font-size: 15px">{title}</span>
    <span class="meta" style="text-align: center">{body}</span>
  </div>
  {#if children}{@render children()}{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 28px;
    animation: fade 300ms var(--ease);
  }
</style>
