<script lang="ts">
  import type { Snippet } from "svelte";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    subtitle,
    width = 480,
    children,
    footer,
  }: { title: string; subtitle?: string; width?: number; children: Snippet; footer?: Snippet } = $props();
</script>

<div class="frame panel" style="width: {width}px">
  <header>
    <div class="col grow">
      <h2>{title}</h2>
      {#if subtitle}<span class="meta">{subtitle}</span>{/if}
    </div>
    <button class="icon-btn" aria-label="Close" onclick={() => (app.modal = null)}><Icon name="close" /></button>
  </header>
  <div class="body">{@render children()}</div>
  {#if footer}
    <hr class="divider" />
    <footer>{@render footer()}</footer>
  {/if}
</div>

<style>
  .frame {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 80px);
    padding: 20px;
    border-radius: var(--r-xl);
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    min-height: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
</style>
