<!-- A first-visit tip on a page. Shown once, until dismissed. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { slide } from "svelte/transition";
  import { app } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import Icon from "./Icon.svelte";

  let { id, children }: { id: string; children: Snippet } = $props();

  const prefs = $derived(app.snap?.bootstrapper.preferences);
  const visible = $derived(!!prefs && prefs.onboarded && !prefs.seen_tips.includes(id));

  function dismiss() {
    if (!prefs) return;
    setPreferences({ seen_tips: [...prefs.seen_tips, id] });
  }
</script>

{#if visible}
  <div class="tip glass-base" transition:slide={{ duration: 260 }}>
    <span class="bulb"><Icon name="tip" size={15} /></span>
    <span class="text grow">{@render children()}</span>
    <button class="btn small tertiary" onclick={dismiss}>Got it</button>
  </div>
{/if}

<style>
  .tip {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    border-color: rgb(var(--accent) / 0.22);
    animation: rise 320ms var(--ease) both;
  }
  .bulb {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 50%;
    background: rgb(var(--accent) / 0.12);
    color: rgb(var(--accent));
    animation: glow 2.4s ease-in-out infinite;
  }
  @keyframes glow {
    50% {
      box-shadow: 0 0 0 5px rgb(var(--accent) / 0.08);
    }
  }
  .text {
    font-size: 12.5px;
    color: rgb(var(--muted));
    line-height: 1.45;
  }
</style>
