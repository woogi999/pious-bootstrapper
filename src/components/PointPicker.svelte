<!-- Picks a point on the screen: counts down while the pointer is moved
     there, then reads where it is (and the color under it). -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "./Icon.svelte";

  let { onpick, label = "Pick" }: { onpick: (x: number, y: number, color: string | null) => void; label?: string } = $props();

  let count = $state(0);

  function start() {
    if (count) return;
    count = 3;
    const timer = setInterval(async () => {
      count--;
      if (count <= 0) {
        clearInterval(timer);
        const info = await invoke<{ x: number; y: number; color: string | null }>("cursor_info");
        onpick(info.x, info.y, info.color);
      }
    }, 1000);
  }
</script>

<button class="btn small" class:counting={count > 0} onclick={start} title="Move the pointer to the spot within 3 seconds">
  {#if count > 0}{#key count}<span class="n">{count}</span>{/key}Move the pointer there{:else}<Icon name="target" />{label}{/if}
</button>

<style>
  .counting {
    border-color: rgb(var(--accent) / 0.5);
  }
  .n {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
    font-size: 11px;
    font-weight: 800;
    animation: pop 260ms var(--ease);
  }
</style>
