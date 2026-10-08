<!-- Picks which accounts' Roblox windows a macro or the auto-clicker sends
     to. Accounts with a window open right now are marked. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { accountLabel, avatarKey } from "../lib/format";
  import type { Uuid } from "../lib/types";
  import Avatar from "./Avatar.svelte";

  let { value, onchange }: { value: Uuid[]; onchange: (accounts: Uuid[]) => void } = $props();

  const snap = $derived(app.snap!);
  const running = $derived(new Set(snap.instances.map((i) => i.account).filter(Boolean)));
  const accounts = $derived(
    [...snap.bootstrapper.accounts].sort((a, b) => Number(running.has(b.id)) - Number(running.has(a.id)) || accountLabel(a).localeCompare(accountLabel(b))),
  );

  function flip(id: Uuid) {
    onchange(value.includes(id) ? value.filter((x) => x !== id) : [...value, id]);
  }
</script>

<div class="targets">
  {#each accounts as a (a.id)}
    <button class="chip" class:on={value.includes(a.id)} onclick={() => flip(a.id)} title={running.has(a.id) ? "Has a Roblox window open" : "No Roblox window open right now"}>
      <Avatar path={snap.images[avatarKey(a)]} name={accountLabel(a)} size={16} />
      {accountLabel(a)}
      {#if running.has(a.id)}<span class="live"></span>{/if}
    </button>
  {:else}
    <span class="meta">Add an account first.</span>
  {/each}
</div>

<style>
  .targets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .live {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #30a46c;
    box-shadow: 0 0 0 3px rgb(48 164 108 / 0.2);
  }
</style>
