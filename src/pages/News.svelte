<!-- Roblox news: new Roblox builds, Roblox's announcements and release
     notes, and Hyperion (Byfron) posts from the Developer Forum. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { run, setPreferences } from "../lib/api";
  import type { NewsItem } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import NewsCard from "../components/NewsCard.svelte";
  import Switch from "../components/Switch.svelte";

  const snap = $derived(app.snap!);
  const news = $derived(snap.news);

  type Filter = "all" | NewsItem["kind"];
  let filter = $state<Filter>("all");
  const FILTERS: [Filter, string][] = [
    ["all", "Everything"],
    ["updates", "Roblox updates"],
    ["roblox", "Announcements"],
    ["hyperion", "Hyperion · Byfron"],
  ];
  const shown = $derived(news.items.filter((n) => filter === "all" || n.kind === filter));
  const count = (kind: Filter) => (kind === "all" ? news.items.length : news.items.filter((n) => n.kind === kind).length);
</script>

<div class="page news-page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">News</h1>
      <span class="meta">New Roblox versions, Roblox's announcements, and what's happening with Hyperion (Byfron), its anti-cheat</span>
    </div>
    <button class="btn" disabled={news.loading} onclick={() => run("refresh_news", { force: true })}>
      <Icon name="refresh" />{news.loading ? "Loading…" : "Refresh"}
    </button>
  </div>

  <div class="row filters">
    {#each FILTERS as [id, label] (id)}
      <button class="chip" class:on={filter === id} onclick={() => (filter = id)}>{label} · {count(id)}</button>
    {/each}
    <span class="spacer"></span>
    <label class="row home-toggle meta">
      Show on Home
      <Switch on={snap.bootstrapper.preferences.news_on_home} onchange={(on) => setPreferences({ news_on_home: on })} />
    </label>
  </div>

  {#if news.errors.length}
    <div class="notice caution"><Icon name="warning" />Couldn't reach {news.errors.map((e) => e.split(":")[0]).join(", ")}. The rest is below.</div>
  {/if}

  {#if !shown.length}
    {#if news.loading}
      <div class="list">
        {#each [0, 1, 2, 3, 4] as i (i)}<div class="glass skeleton pulse"></div>{/each}
      </div>
    {:else}
      <EmptyState icon="news" title="No news here yet" body="Pious couldn't load any news. Check your connection, then refresh.">
        <button class="btn primary" onclick={() => run("refresh_news", { force: true })}><Icon name="refresh" />Refresh</button>
      </EmptyState>
    {/if}
  {:else}
    {#key filter}
      <div class="list">
        {#each shown as item, i (item.id)}<NewsCard {item} index={i} />{/each}
      </div>
    {/key}
  {/if}
  <span class="secondary">From weao.xyz and the Roblox Developer Forum. Hyperion posts include reports from players, not only from Roblox.</span>
</div>

<style>
  .news-page {
    max-width: 920px;
    margin: 0 auto;
    width: 100%;
    gap: 14px;
  }
  .filters {
    gap: 6px;
    flex-wrap: wrap;
  }
  .home-toggle {
    gap: 8px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .skeleton {
    height: 78px;
  }
</style>
