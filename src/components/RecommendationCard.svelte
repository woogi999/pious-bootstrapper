<!-- A recommended game. Clicking opens it and Play launches it; neither adds
     it to the library. Only Add does. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { run } from "../lib/api";
  import { openRecommendation, playRecommendation, recommendationMenu } from "../lib/menus";
  import { artworkKey, compact, recommendationReason } from "../lib/format";
  import type { Recommendation } from "../lib/types";
  import Artwork from "./Artwork.svelte";
  import Icon from "./Icon.svelte";

  let { rec }: { rec: Recommendation } = $props();
  const snap = $derived(app.snap!);
  const stats = $derived(
    [`${compact(rec.players)} playing`, rec.rating != null ? `${Math.round(rec.rating * 100)}% liked` : null]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<div
  class="card glass rec"
  role="button"
  tabindex="0"
  onclick={() => openRecommendation(rec.universe_id)}
  onkeydown={(e) => e.key === "Enter" && openRecommendation(rec.universe_id)}
  oncontextmenu={(e) => app.openMenu(e, recommendationMenu(rec))}
>
  <div class="art-wrap">
    <Artwork path={snap.images[artworkKey(rec)]} name={rec.name} radius="0" class="art" />
    <div class="over"><span class="art-chip"><Icon name="sparkles" size={11} />For you</span></div>
  </div>
  <div class="body">
    <div class="col" style="gap: 1px">
      <span class="item-title line">{rec.name}</span>
      <span class="secondary line">{rec.creator || "Roblox experience"}</span>
    </div>
    <div class="row info"><Icon name="users" size={11} /><span class="secondary line">{stats}</span></div>
    <div class="row info"><Icon name="sparkles" size={11} /><span class="secondary line">{recommendationReason(rec)}</span></div>
    <div class="row">
      <button class="btn primary small" onclick={(e) => (e.stopPropagation(), playRecommendation(rec.universe_id))}>
        <Icon name="play" size={13} />Play
      </button>
      <span class="spacer"></span>
      <button
        class="icon-btn"
        title="Add to library"
        aria-label="Add to library"
        onclick={(e) => (e.stopPropagation(), run("recommendation_game", { universe: rec.universe_id, add: true }))}
      >
        <Icon name="add" />
      </button>
      <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, recommendationMenu(rec))}><Icon name="more" /></button>
    </div>
  </div>
</div>

<style>
  .rec {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .art-wrap {
    position: relative;
    aspect-ratio: 16 / 9;
  }
  .art-wrap :global(.art) {
    position: absolute;
    inset: 0;
  }
  .over {
    position: absolute;
    inset: 8px 8px auto 8px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }
  .info {
    gap: 5px;
    color: rgb(var(--faint));
    height: 18px;
  }
</style>
