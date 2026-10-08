<!-- Game artwork that covers its box with rounded corners, fading in once
     loaded. Without a picture, a soft gradient with a game icon. -->
<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Icon from "./Icon.svelte";
  import { seed } from "../lib/format";

  let {
    path,
    name = "",
    radius = "var(--r-lg)",
    class: className = "",
  }: { path: string | null | undefined; name?: string; radius?: string; class?: string } = $props();

  let loaded = $state(false);
  const shade = $derived([20, 16, 24, 13, 19, 22][seed(name) % 6]);
  const src = $derived(path ? convertFileSrc(path) : null);
</script>

<div class="art {className}" style="border-radius: {radius}; --shade: {shade}%">
  {#if src}
    <img {src} alt="" loading="lazy" decoding="async" class:loaded onload={() => (loaded = true)} />
  {:else}
    <Icon name="game" size={22} />
  {/if}
</div>

<style>
  .art {
    position: relative;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: linear-gradient(
      135deg,
      color-mix(in srgb, rgb(var(--surface)) var(--shade), rgb(var(--bg))),
      color-mix(in srgb, rgb(var(--surface)) 6%, rgb(var(--bg)))
    );
    color: rgb(var(--surface) / 0.2);
    /* Keeps the rounded clip on the image's own layer. */
    isolation: isolate;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 380ms var(--ease);
  }
  img.loaded {
    opacity: 1;
  }
</style>
