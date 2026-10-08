<!-- After an update: what changed in this version. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import { parseChangelog, type Release } from "../lib/changelog";
  import ModalFrame from "../components/ModalFrame.svelte";
  import ReleaseNotes from "../components/ReleaseNotes.svelte";
  import Icon from "../components/Icon.svelte";

  const version = $derived(app.snap!.current_version);
  let release = $state<Release | null>(null);

  onMount(async () => {
    const text = await invoke<string>("changelog");
    release = parseChangelog(text).find((r) => r.version === version) ?? null;
  });
  onDestroy(() => setPreferences({ seen_version: app.snap?.current_version ?? "" }));
</script>

<ModalFrame title="What's new in Pious {version}" subtitle="Pious updated itself. Here's what changed." width={560}>
  <div class="hero"><span class="gift"><Icon name="gift" size={22} /></span></div>
  {#if release}
    <ReleaseNotes {release} />
  {:else}
    <span class="meta">Bug fixes and improvements.</span>
  {/if}
  {#snippet footer()}
    <button class="btn primary" onclick={() => (app.modal = null)}><Icon name="check" />Nice</button>
  {/snippet}
</ModalFrame>

<style>
  .hero {
    display: grid;
    place-items: center;
    padding: 4px 0 2px;
  }
  .gift {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: rgb(var(--accent) / 0.12);
    color: rgb(var(--accent));
    animation: pop 420ms var(--ease) both, glow 2.4s ease-in-out 420ms infinite;
  }
  @keyframes glow {
    50% {
      box-shadow: 0 0 0 8px rgb(var(--accent) / 0.07);
    }
  }
</style>
