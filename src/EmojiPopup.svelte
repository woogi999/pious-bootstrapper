<!-- The emoji list shown over Roblox while a :shortcode is typed in its
     chat. It never takes focus or the mouse: the keyboard hook in Rust
     handles the arrows, Enter and Tab. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  let query = $state("");
  let matches = $state<[string, string][]>([]);
  let selected = $state(0);

  onMount(() => {
    const off = listen<{ query: string; matches: [string, string][]; selected: number }>("emoji-list", (e) => {
      query = e.payload.query;
      matches = e.payload.matches;
      selected = e.payload.selected;
    });
    return () => {
      off.then((f) => f());
    };
  });
</script>

{#if matches.length}
  <div class="list">
    <div class="head">Emoji matching :{query}<span class="keys">↑↓ · Enter</span></div>
    {#each matches as [name, emoji], i (name)}
      <div class="item" class:on={i === selected}>
        <span class="glyph">{emoji}</span><span class="name">:{name}:</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .list {
    margin: 4px;
    padding: 4px;
    width: 280px;
    border-radius: 12px;
    background: rgb(16 16 18 / 0.94);
    border: 1px solid rgb(255 255 255 / 0.12);
    box-shadow: 0 12px 30px rgb(0 0 0 / 0.5);
    color: #ededef;
    font-size: 13px;
    animation: rise 140ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .head {
    display: flex;
    padding: 4px 8px 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgb(255 255 255 / 0.5);
  }
  .keys {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-radius: 8px;
  }
  .item.on {
    background: rgb(255 255 255 / 0.12);
  }
  .glyph {
    width: 24px;
    font-size: 19px;
    text-align: center;
  }
  .name {
    opacity: 0.85;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
