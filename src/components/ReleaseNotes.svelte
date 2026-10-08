<!-- One version's notes from the changelog. -->
<script lang="ts">
  import type { Release } from "../lib/changelog";

  let { release }: { release: Release } = $props();
</script>

<div class="notes stagger">
  {#each release.blocks as block, i (i)}
    {#if block.kind === "heading"}
      <h4>{block.text}</h4>
    {:else if block.kind === "list"}
      <ul>
        {#each block.items as item, j (j)}<li>{item}</li>{/each}
      </ul>
    {:else}
      <p>{block.text}</p>
    {/if}
  {/each}
</div>

<style>
  .notes {
    display: flex;
    flex-direction: column;
    gap: 6px;
    user-select: text;
  }
  h4 {
    margin: 8px 0 0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgb(var(--faint));
  }
  ul {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li,
  p {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: rgb(var(--muted));
  }
  li::marker {
    color: rgb(var(--faint));
  }
</style>
