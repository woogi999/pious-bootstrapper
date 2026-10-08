<!-- Games: the library with search, filters and sorting. "Recents" also lists
     games only played, which show Add instead of Play. -->
<script lang="ts">
  import { app, viewOf } from "../lib/state.svelte";
  import type { Game } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import GameCard from "../components/GameCard.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";

  const snap = $derived(app.snap!);
  const games = $derived(snap.bootstrapper.games);
  const libraryGames = $derived(games.filter((g) => g.in_library));
  const favorites = $derived(libraryGames.filter((g) => g.favorite).length);
  const recents = $derived(games.filter((g) => g.last_played).length);
  const notAdded = $derived(games.filter((g) => !g.in_library && g.last_played).length);
  const collections = $derived([...new Set(libraryGames.filter((g) => g.collection).map((g) => g.collection!))].sort((a, b) => a.localeCompare(b)));

  const sorts = [
    { value: "recent" as const, label: "Recently played" },
    { value: "name" as const, label: "Name" },
    { value: "added" as const, label: "Recently added" },
    { value: "played" as const, label: "Most played" },
  ];

  const filtered = $derived.by(() => {
    const q = app.gamesQuery.trim().toLowerCase();
    const filter = app.gamesFilter;
    let list = games.filter((g) => {
      if (filter === "recent") return !!g.last_played;
      if (!g.in_library) return false;
      if (filter === "favorites") return g.favorite;
      if (filter.startsWith("collection:")) return g.collection === filter.slice("collection:".length);
      return true;
    });
    if (q) {
      list = list.filter(
        (g) => g.name.toLowerCase().includes(q) || (g.developer ?? "").toLowerCase().includes(q) || String(g.place_id).includes(q),
      );
    }
    const by: Record<string, (a: Game, b: Game) => number> = {
      recent: (a, b) => (b.last_played ?? "").localeCompare(a.last_played ?? "") || b.added_at.localeCompare(a.added_at),
      name: (a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
      added: (a, b) => b.added_at.localeCompare(a.added_at),
      played: (a, b) => b.play_count - a.play_count,
    };
    list.sort(filter === "recent" ? by.recent : by[app.gamesSort]);
    return list;
  });
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Games</h1>
      <span class="meta">
        {libraryGames.length === 0
          ? "Your saved Roblox experiences"
          : libraryGames.length === 1
            ? "1 game in your library"
            : `${libraryGames.length} games in your library · ${favorites} favorites`}
      </span>
    </div>
    <button class="btn primary" onclick={() => (app.modal = { kind: "add_game" })}><Icon name="add" />Add Game</button>
  </div>

  {#if !games.length}
    <EmptyState icon="library" title="Your library is empty" body="Save a Roblox game to start building your library.">
      <button class="btn primary" onclick={() => (app.modal = { kind: "add_game" })}><Icon name="add" />Add Game</button>
    </EmptyState>
  {:else}
    <div class="col" style="gap: 10px">
      <div class="row" style="gap: 12px">
        <label class="search-box glass-base" style="width: 360px">
          <Icon name="search" />
          <input placeholder="Search your library" bind:value={app.gamesQuery} />
        </label>
        <span class="spacer"></span>
        {#if app.gamesFilter !== "recent"}
          <Icon name="sort" size={14} />
          <Select options={sorts} value={app.gamesSort} onchange={(v) => (app.gamesSort = v)} width="170px" />
        {/if}
        <ViewToggle page="games" />
      </div>
      <div class="row" style="gap: 6px; flex-wrap: wrap">
        <button class="chip" class:on={app.gamesFilter === "all"} onclick={() => (app.gamesFilter = "all")}>All</button>
        <button class="chip" class:on={app.gamesFilter === "favorites"} onclick={() => (app.gamesFilter = "favorites")}>Favorites</button>
        <button class="chip" class:on={app.gamesFilter === "recent"} onclick={() => (app.gamesFilter = "recent")}>
          <Icon name="history" />Recents{recents ? ` · ${recents}` : ""}
        </button>
        {#each collections as name (name)}
          <button class="chip" class:on={app.gamesFilter === `collection:${name}`} onclick={() => (app.gamesFilter = `collection:${name}`)}>
            <Icon name="folder" />{name}
          </button>
        {/each}
      </div>
    </div>

    {#if app.gamesFilter === "recent" && notAdded}
      <span class="secondary">Games you played without adding them show Add instead of Play.</span>
    {/if}

    {#if !filtered.length}
      {#if app.gamesFilter === "recent"}
        <EmptyState icon="history" title="Nothing played yet" body="Games you play show up here, even ones you haven't added to your library." />
      {:else}
        <EmptyState icon="search" title="No matching games" body="Try a different search or filter." />
      {/if}
    {:else}
      {#key app.gamesFilter + app.gamesSort + viewOf("games")}
        {#if viewOf("games") === "List"}
          <div class="col list-rows">
            {#each filtered as game (game.id)}<GameCard {game} row />{/each}
          </div>
        {:else}
          <div class="grid">
            {#each filtered as game (game.id)}<GameCard {game} />{/each}
          </div>
        {/if}
      {/key}
    {/if}
  {/if}
</div>
