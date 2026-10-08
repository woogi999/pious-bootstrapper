<script lang="ts">
  import { app, sortOf, viewOf } from "../lib/state.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import Tip from "../components/Tip.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import ServerCard from "../components/ServerCard.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const all = $derived(snap.bootstrapper.servers);
  const sort = $derived(sortOf("servers", "recent"));
  const gameName = (id: string) => snap.bootstrapper.games.find((g) => g.id === id)?.name ?? "";
  const filtered = $derived.by(() => {
    const q = app.serversQuery.trim().toLowerCase();
    return all
      .filter((s) => !app.serversFavorites || s.favorite)
      .filter((s) => {
        if (!q) return true;
        const game = snap.bootstrapper.games.find((g) => g.id === s.game_id)?.name ?? "";
        return [s.name, game, s.notes].some((t) => t.toLowerCase().includes(q));
      })
      .sort((a, b) => {
        const fav = Number(b.favorite) - Number(a.favorite);
        if (sort === "name") return fav || a.name.localeCompare(b.name);
        if (sort === "game") return fav || gameName(a.game_id).localeCompare(gameName(b.game_id)) || a.name.localeCompare(b.name);
        if (sort === "added") return fav || b.added_at.localeCompare(a.added_at);
        return fav || (b.last_joined ?? "").localeCompare(a.last_joined ?? "");
      });
  });

  function add() {
    app.modal = { kind: "server", editing: null, game: null };
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Private Servers</h1>
      <span class="meta">{all.length === 0 ? "Your saved private servers" : all.length === 1 ? "1 saved server" : `${all.length} saved servers`}</span>
    </div>
    <button class="btn" onclick={() => (app.modal = { kind: "join_link" })} title="Join a link without saving it (Ctrl L)"><Icon name="link" />Join a Link</button>
    <button class="btn primary" onclick={add}><Icon name="add" />Add Private Server</button>
  </div>

  <Tip id="servers">Paste any private server or share link with Join a Link (Ctrl L), like Direct Connect. Save it to join again in one click.</Tip>
  {#if !all.length}
    <EmptyState icon="server" title="No private servers yet" body="Save a private server link to join it in one click.">
      <button class="btn primary" onclick={add}><Icon name="add" />Add Private Server</button>
    </EmptyState>
  {:else}
    <div class="row" style="gap: 10px">
      <label class="search-box glass-base" style="width: 360px">
        <Icon name="search" />
        <input placeholder="Search servers" bind:value={app.serversQuery} />
      </label>
      <button class="chip" class:on={!app.serversFavorites} onclick={() => (app.serversFavorites = false)}>All</button>
      <button class="chip" class:on={app.serversFavorites} onclick={() => (app.serversFavorites = true)}><Icon name="star" />Favorites</button>
      <span class="spacer"></span>
      <SortSelect
        page="servers"
        value={sort}
        options={[
          { value: "recent", label: "Recently joined" },
          { value: "name", label: "Name" },
          { value: "game", label: "Game" },
          { value: "added", label: "Recently added" },
        ]}
      />
      <ViewToggle page="servers" />
    </div>
    {#if !filtered.length}
      <EmptyState icon="search" title="No matching servers" body="Try a different search or filter." />
    {:else}
      {#if viewOf("servers") === "List"}
        <div class="col list-rows">
          {#each filtered as server (server.id)}<ServerCard {server} row />{/each}
        </div>
      {:else}
        <div class="grid" style="--card: 250px">
          {#each filtered as server (server.id)}<ServerCard {server} />{/each}
        </div>
      {/if}
    {/if}
  {/if}
</div>
