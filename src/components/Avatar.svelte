<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

  let { path, name, size = 28 }: { path: string | null | undefined; name: string; size?: number } = $props();
  const initial = $derived((name || "?").charAt(0).toUpperCase());
</script>

<span class="avatar" style="width: {size}px; height: {size}px; font-size: {size * 0.42}px">
  {#if path}
    <img src={convertFileSrc(path)} alt="" decoding="async" />
  {:else}
    {initial}
  {/if}
</span>

<style>
  .avatar {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    background: rgb(var(--surface) / 0.08);
    font-weight: 600;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
