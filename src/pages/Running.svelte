<script lang="ts">
  import { app, sortOf, viewOf } from "../lib/state.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import { closeAll } from "../lib/api";
  import { plural } from "../lib/format";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import InstanceCard from "../components/InstanceCard.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const sort = $derived(sortOf("running", "started"));
  const gameName = (id: string) => snap.bootstrapper.games.find((g) => g.id === id)?.name ?? "";
  const instances = $derived(
    snap.instances.slice().sort((a, b) => {
      if (sort === "game") return gameName(a.game).localeCompare(gameName(b.game));
      if (sort === "newest") return b.started.localeCompare(a.started);
      return a.started.localeCompare(b.started);
    }),
  );
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Instances</h1>
      <span class="meta">{snap.instances.length ? `${plural(snap.instances.length, "Roblox instance")} running` : "Roblox windows opened by Pious"}</span>
    </div>
    {#if snap.instances.length}
      <SortSelect
        page="running"
        value={sort}
        options={[
          { value: "started", label: "Started first" },
          { value: "newest", label: "Newest" },
          { value: "game", label: "Game" },
        ]}
      />
      <ViewToggle page="running" />
      <button class="btn danger" onclick={closeAll}><Icon name="power" />Close all</button>
    {/if}
  </div>
  {#if !snap.instances.length}
    <EmptyState icon="running" title="Nothing running" body="Games you launch from Pious show up here, with the account and server they use.">
      <button class="btn primary" onclick={() => app.navigate({ name: "games" })}><Icon name="game" />Browse games</button>
    </EmptyState>
  {:else}
    {#if viewOf("running") === "Grid"}
      <div class="grid" style="--card: 260px">
        {#each instances as instance (instance.id)}<InstanceCard {instance} variant="tile" />{/each}
      </div>
    {:else}
      <div class="list-rows">
        {#each instances as instance (instance.id)}<InstanceCard {instance} />{/each}
      </div>
    {/if}
  {/if}
</div>
