<!-- One version's notes from the changelog, with its **bold** and `code`. -->
<script lang="ts">
  import { spans, type Release } from "../lib/changelog";

  let { release }: { release: Release } = $props();
</script>

{#snippet line(text: string)}
  {#each spans(text) as span, k (k)}
    {#if span.kind === "bold"}<strong>{span.text}</strong>{:else if span.kind === "code"}<code>{span.text}</code>{:else}{span.text}{/if}
  {/each}
{/snippet}

<div class="notes stagger">
  {#each release.blocks as block, i (i)}
    {#if block.kind === "heading"}
      <h4>{block.text}</h4>
    {:else if block.kind === "list"}
      <ul>
        {#each block.items as item, j (j)}<li>{@render line(item)}</li>{/each}
      </ul>
    {:else}
      <p>{@render line(block.text)}</p>
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
  strong {
    font-weight: 600;
    color: rgb(var(--text));
  }
  code {
    padding: 1px 5px;
    border-radius: 5px;
    background: rgb(var(--surface) / 0.08);
    font: 12px ui-monospace, Consolas, monospace;
    color: rgb(var(--text));
  }
  li::marker {
    color: rgb(var(--faint));
  }
</style>
