<!-- A row of picture tiles to choose from (shift lock cursors, Roblox
     icons): Roblox's own, the presets, and "Your own…" for a file. -->
<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    items,
    value,
    custom = null,
    disabled = false,
    dark = false,
    onpick,
    oncustom,
  }: {
    items: { id: string; name: string; picture: string | null }[];
    value: string | null;
    /** The file in use for "custom", shown on its tile. */
    custom?: string | null;
    disabled?: boolean;
    /** Pictures sit on a dark, game-like backdrop (for light crosshairs). */
    dark?: boolean;
    onpick: (id: string | null) => void;
    oncustom: () => void;
  } = $props();
</script>

<div class="choices" class:disabled>
  <button class="tile" class:on={value === null} {disabled} onclick={() => onpick(null)}>
    <span class="pic" class:dark><Icon name="game" size={18} /></span>
    <span class="name">Roblox's own</span>
  </button>
  {#each items as item, i (item.id)}
    <button class="tile" class:on={value === item.id} {disabled} style="--d: {i * 18}ms" onclick={() => onpick(item.id)} title={item.name}>
      <span class="pic" class:dark>
        {#if item.picture}<img src={item.picture} alt="" />{:else}<span class="pulse wait"></span>{/if}
      </span>
      <span class="name">{item.name}</span>
    </button>
  {/each}
  <button class="tile" class:on={value === "custom"} {disabled} onclick={oncustom} title={custom ?? "Pick a picture"}>
    <span class="pic" class:dark><Icon name="folder" size={18} /></span>
    <span class="name">{value === "custom" && custom ? (custom.split(/[\\/]/).pop() ?? "Your own") : "Your own…"}</span>
  </button>
</div>

<style>
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 8px;
    padding: 4px 0 10px;
    transition: opacity var(--med);
  }
  .choices.disabled {
    opacity: 0.45;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 10px 6px 8px;
    border: 1px solid rgb(var(--surface) / 0.07);
    border-radius: var(--r-md);
    background: rgb(var(--surface) / 0.03);
    color: rgb(var(--muted));
    font: inherit;
    cursor: pointer;
    animation: pop 260ms var(--ease) var(--d, 0ms) both;
    transition:
      background var(--fast),
      border-color var(--fast),
      transform 200ms var(--ease);
  }
  .tile:hover:not(:disabled) {
    background: rgb(var(--surface) / 0.07);
    transform: translateY(-2px);
  }
  .tile.on {
    border-color: rgb(var(--accent) / 0.7);
    background: rgb(var(--accent) / 0.1);
    color: rgb(var(--text));
  }
  .pic {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 10px;
    background: rgb(var(--surface) / 0.06);
  }
  .pic.dark {
    background: radial-gradient(circle at 30% 30%, #5b6b7d, #23303d);
  }
  .pic img {
    width: 40px;
    height: 40px;
    object-fit: contain;
  }
  .wait {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: rgb(var(--surface) / 0.12);
  }
  .name {
    max-width: 100%;
    font-size: 11px;
    font-weight: 600;
    text-align: center;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
