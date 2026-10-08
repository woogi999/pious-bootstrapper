<!-- One news item: a Roblox build, a Roblox announcement or a Hyperion
     (Byfron) post. Opens where it came from. -->
<script lang="ts">
  import { run } from "../lib/api";
  import { relative } from "../lib/format";
  import type { NewsItem } from "../lib/types";
  import Icon from "./Icon.svelte";

  let { item, compact = false, index = 0 }: { item: NewsItem; compact?: boolean; index?: number } = $props();

  const KINDS: Record<NewsItem["kind"], [string, string]> = {
    updates: ["Roblox update", "update"],
    roblox: ["Roblox", "news"],
    hyperion: ["Hyperion · Byfron", "shield"],
  };
  const [label, icon] = $derived(KINDS[item.kind] ?? ["News", "news"]);
</script>

<button class="news glass-base shine kind-{item.kind}" class:compact style="--delay: {Math.min(index, 8) * 35}ms" onclick={() => run("open_url", { url: item.url })} title={item.url}>
  <span class="badge"><Icon name={icon} size={14} /></span>
  <span class="col grow" style="gap: 3px; min-width: 0">
    <span class="row top">
      <span class="tag">{label}</span>
      <span class="secondary line">{item.source}{item.date ? ` · ${relative(item.date)}` : ""}</span>
    </span>
    <span class="item-title title">{item.title}</span>
    {#if item.summary && !compact}<span class="meta summary">{item.summary}</span>{/if}
  </span>
  <span class="go"><Icon name="external" size={13} /></span>
</button>

<style>
  .news {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border-radius: var(--r-md);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    animation: news-in 380ms var(--ease) var(--delay, 0ms) both;
    transition:
      background var(--fast),
      border-color var(--fast),
      box-shadow 260ms var(--ease),
      transform 260ms var(--ease);
  }
  @keyframes news-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
      filter: blur(3px);
    }
    to {
      opacity: 1;
      transform: none;
      filter: none;
    }
  }
  .news:hover {
    background: rgb(var(--surface) / 0.07);
    transform: translateY(-2px);
    box-shadow: 0 10px 28px -14px rgb(0 0 0 / 0.6);
  }
  .news:active {
    transform: translateY(0) scale(0.99);
    transition-duration: 90ms;
  }
  .compact {
    padding: 10px 12px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 9px;
    background: rgb(var(--surface) / 0.07);
    color: rgb(var(--muted));
    transition: transform 320ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .news:hover .badge {
    transform: scale(1.1) rotate(-6deg);
  }
  .kind-hyperion .badge {
    color: #8fb8ff;
    background: rgb(143 184 255 / 0.12);
  }
  .kind-updates .badge {
    color: #7ee2a8;
    background: rgb(126 226 168 / 0.12);
  }
  .top {
    gap: 8px;
    min-width: 0;
  }
  .tag {
    flex: none;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgb(var(--faint));
  }
  .title {
    line-height: 1.3;
  }
  .summary {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .go {
    flex: none;
    color: rgb(var(--faint));
    opacity: 0;
    transform: translate(-4px, 4px);
    transition:
      opacity var(--fast),
      transform 260ms var(--ease);
  }
  .news:hover .go {
    opacity: 1;
    transform: none;
  }
  :global(.reduce-motion) .news {
    animation: none;
  }
</style>
