<!-- Grid or list for one page; remembered per page. -->
<script lang="ts">
  import { viewOf } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import Icon from "./Icon.svelte";

  let { page }: { page: string } = $props();

  const mode = $derived(viewOf(page));
</script>

<div class="toggle" role="radiogroup" aria-label="View">
  <button class="chip" class:on={mode === "Grid"} aria-label="Cards" title="Cards" onclick={() => setPreferences({ views: { [page]: "Grid" } })}>
    <Icon name="grid" />
  </button>
  <button class="chip" class:on={mode === "List"} aria-label="List" title="List" onclick={() => setPreferences({ views: { [page]: "List" } })}>
    <Icon name="list" />
  </button>
</div>

<style>
  .toggle {
    display: inline-flex;
    gap: 4px;
  }
</style>
