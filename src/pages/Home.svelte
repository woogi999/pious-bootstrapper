<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { handleLaunch, openLaunch, play, run, serverHop, setPreferences } from "../lib/api";
  import { gameMenu } from "../lib/menus";
  import { accountName, artworkKey, greeting, plural, relative, serverLabel, shortVersion, who } from "../lib/format";
  import type { Friend, Game, LaunchOutcome } from "../lib/types";
  import AccountCard from "../components/AccountCard.svelte";
  import Artwork from "../components/Artwork.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import GameCard from "../components/GameCard.svelte";
  import Icon from "../components/Icon.svelte";
  import InstanceCard from "../components/InstanceCard.svelte";
  import NewsCard from "../components/NewsCard.svelte";
  import RecommendationCard from "../components/RecommendationCard.svelte";
  import ServerCard from "../components/ServerCard.svelte";
  import Tip from "../components/Tip.svelte";

  const snap = $derived(app.snap!);
  const library = $derived(snap.bootstrapper);

  // The greeting uses the main (default) account's display name.
  const main = $derived(
    library.accounts.find((a) => a.id === library.preferences.default_account) ?? library.accounts[0],
  );
  let now = $state(new Date());
  const tick = setInterval(() => (now = new Date()), 1000);
  onDestroy(() => clearInterval(tick));
  const hour = $derived(now.getHours());
  const daytime = $derived(hour >= 6 && hour < 18);
  const time = $derived(now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }));
  const date = $derived(now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" }));
  const libraryGames = $derived(library.games.filter((g) => g.in_library));

  const summary = $derived(
    [
      plural(libraryGames.length, "game"),
      plural(library.accounts.length, "account"),
      library.servers.length ? plural(library.servers.length, "private server") : null,
      snap.instances.length ? `${snap.instances.length} running` : null,
    ]
      .filter(Boolean)
      .join(" · "),
  );

  const recent = $derived(
    library.games
      .filter((g) => g.last_played)
      .sort((a, b) => b.last_played!.localeCompare(a.last_played!))
      .slice(0, 4),
  );
  const featured = $derived<Game | undefined>(recent[0] ?? libraryGames.find((g) => g.favorite) ?? libraryGames[0]);
  const featuredConfig = $derived(featured ? snap.effective[featured.id] : null);

  const shelfGames = $derived(
    [...libraryGames]
      .sort(
        (a, b) =>
          Number(b.favorite) - Number(a.favorite) ||
          (b.last_played ?? "").localeCompare(a.last_played ?? "") ||
          b.added_at.localeCompare(a.added_at),
      )
      .slice(0, 10),
  );
  // What friends are playing, by game (most friends first).
  const friendGames = $derived.by(() => {
    const games = new Map<number, { place: number; name: string; friends: Friend[] }>();
    for (const f of snap.friends.list) {
      if (f.status !== "in_game" || !f.place_id) continue;
      const entry = games.get(f.place_id) ?? { place: f.place_id, name: f.location ?? "A Roblox game", friends: [] };
      entry.friends.push(f);
      games.set(f.place_id, entry);
    }
    return [...games.values()].sort((a, b) => b.friends.length - a.friends.length || a.name.localeCompare(b.name));
  });
  const names = (friends: Friend[]) => {
    const shown = friends.slice(0, 2).map((f) => who(f.display_name));
    return friends.length > 2 ? `${shown.join(", ")} and ${friends.length - 2} more` : shown.join(" and ");
  };
  async function joinFriend(f: Friend) {
    const once = (force: boolean) =>
      run<LaunchOutcome>("join_player", { place: f.place_id, job: f.job, account: snap.friends.account, name: f.location, force });
    handleLaunch(await once(false), () => once(true));
  }

  // Recommendations: a row, or everything.
  let allRecs = $state(false);

  const servers = $derived(
    [...library.servers]
      .sort((a, b) => Number(b.favorite) - Number(a.favorite) || (b.last_joined ?? "").localeCompare(a.last_joined ?? ""))
      .slice(0, 4),
  );
</script>

<div class="page">
  <div class="page-header hello">
    <div class="col grow" style="gap: 4px">
      <h1 class="greeting">{greeting()}{main ? `, ${main.alias ?? who(main.display_name)}` : ""}</h1>
      <span class="meta">{summary}</span>
    </div>
    <div class="clock glass-base">
      <span class="sky" class:night={!daytime}><Icon name={daytime ? "sun" : "moon"} size={18} /></span>
      <span class="col" style="gap: 0">
        <span class="time">{time}</span>
        <span class="secondary">{date}</span>
      </span>
    </div>
    <button class="btn" onclick={() => (app.modal = { kind: "join_player" })}><Icon name="users" />Join a player</button>
    <button class="btn" onclick={() => (app.modal = { kind: "add_game" })}><Icon name="add" />Add game</button>
  </div>

  {#if !library.games.length && !library.accounts.length}
    <div class="glass-elevated welcome">
      <h2 class="display" style="font-size: 24px">Welcome to Pious</h2>
      <p class="meta">
        Your personal library for Roblox. Save your games, accounts, private servers and Roblox versions — then launch any combination in one
        click.
      </p>
    </div>
    <div class="steps">
      {#each [{ n: "1", icon: "add-account", title: "Add an account", body: "Sign in on Roblox's own page. Pious never sees your password.", label: "Add Account", action: () => (app.modal = { kind: "add_account", reauth: null }) }, { n: "2", icon: "game", title: "Save a game", body: "Paste a game link or place ID to add it to your library.", label: "Add Game", action: () => (app.modal = { kind: "add_game" }) }, { n: "3", icon: "versions", title: "Check your Roblox versions", body: snap.usable_versions.length ? "Roblox is installed and ready. Manage versions any time." : "Install a Roblox version so Pious can launch it directly.", label: "Open Versions", action: () => app.navigate({ name: "versions" }) }] as step (step.n)}
        <div class="glass step">
          <div class="row"><span class="tile-icon" style="width: 38px; height: 38px"><Icon name={step.icon} size={18} /></span><span class="spacer"></span><span class="num">{step.n}</span></div>
          <div class="col"><span class="item-title">{step.title}</span><span class="meta">{step.body}</span></div>
          <span class="spacer"></span>
          <button class="btn" class:primary={step.n === "1"} onclick={step.action}>{step.label}</button>
        </div>
      {/each}
    </div>
  {:else}
    {#if featured && featuredConfig}
      <section class="section">
        <div class="section-header"><h2 class="section-title">{recent.length ? "Jump back in" : "Ready to play"}</h2></div>
        <div class="glass-elevated featured" role="region" oncontextmenu={(e) => app.openMenu(e, gameMenu(featured))}>
          <Artwork path={snap.images[artworkKey(featured)]} name={featured.name} radius="0" class="art" />
          <div class="shade"></div>
          <div class="content">
            <span class="kicker">{featured.last_played ? `LAST PLAYED ${relative(featured.last_played).toUpperCase()}` : "READY TO PLAY"}</span>
            <h2 class="name line">{featured.name}</h2>
            {#if featured.developer}<span class="meta" style="color: rgb(255 255 255 / 0.75)">by {featured.developer}</span>{/if}
            <div class="row chips">
              <span class="art-chip"><Icon name="account" size={12} />{accountName(snap, featuredConfig.account)}</span>
              <span class="art-chip"><Icon name="versions" size={12} />{shortVersion(snap, featuredConfig.version)}</span>
              <span class="art-chip"><Icon name={featuredConfig.server === "Public" ? "globe" : "server"} size={12} />{serverLabel(snap, featuredConfig.server)}</span>
            </div>
            <div class="row" style="gap: 8px">
              <button class="btn primary hero" onclick={() => play(featured.id)}><Icon name="play" />Play</button>
              <button class="btn" onclick={() => openLaunch(featured.id)}><Icon name="launch" />Play with…</button>
              <button class="btn" onclick={() => serverHop({ game: featured.id })}><Icon name="hop" />Random server</button>
              <button class="btn" onclick={() => app.navigate({ name: "game", id: featured.id })}>Details</button>
            </div>
          </div>
        </div>
        {#if recent.length > 1}
          <div class="recents">
            {#each recent.slice(1) as game (game.id)}
              <div
                class="card glass recent"
                role="button"
                tabindex="0"
                onclick={() => app.navigate({ name: "game", id: game.id })}
                onkeydown={(e) => e.key === "Enter" && app.navigate({ name: "game", id: game.id })}
                oncontextmenu={(e) => app.openMenu(e, gameMenu(game))}
              >
                <Artwork path={snap.images[artworkKey(game)]} name={game.name} radius="var(--r-sm)" class="thumb" />
                <div class="col grow" style="gap: 1px">
                  <span class="item-title line">{game.name}</span>
                  <span class="secondary line">{relative(game.last_played!)}</span>
                </div>
                <button class="icon-btn" aria-label="Play" onclick={(e) => (e.stopPropagation(), play(game.id))}><Icon name="play" /></button>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    <Tip id="home">Click the Pious logo (top left) to come back here from anywhere. Settings → General changes what it does.</Tip>

    {#if library.preferences.news_on_home && snap.news.items.length}
      <section class="section">
        <div class="section-header">
          <div class="col grow" style="gap: 1px">
            <h2 class="section-title">News</h2>
            <span class="meta">Roblox updates and Hyperion (Byfron) news</span>
          </div>
          <button class="link" title="Hide news on Home (it stays on the News page)" onclick={() => setPreferences({ news_on_home: false })}>Hide</button>
          <button class="link" onclick={() => app.navigate({ name: "news" })}>All news<Icon name="arrow-right" /></button>
        </div>
        <div class="news-grid">
          {#each snap.news.items.slice(0, 4) as item, i (item.id)}<NewsCard {item} compact index={i} />{/each}
        </div>
      </section>
    {/if}

    {#if friendGames.length}
      <section class="section">
        <div class="section-header">
          <div class="col grow" style="gap: 1px">
            <h2 class="section-title">Games your friends are playing</h2>
            <span class="meta">{plural(snap.friends.list.filter((f) => f.status === "in_game").length, "friend")} in game right now</span>
          </div>
          <button class="link" onclick={() => app.navigate({ name: "friends" })}>Friends<Icon name="arrow-right" /></button>
        </div>
        <div class="grid" style="--card: 230px">
          {#each friendGames.slice(0, 8) as g (g.place)}
            <div class="card glass fgame">
              <div class="art-wrap">
                <Artwork path={snap.images[`game-${g.place}`]} name={g.name} radius="0" class="art" />
                <div class="faces">
                  {#each g.friends.slice(0, 4) as f (f.id)}
                    <span class="face" title={who(f.display_name)}>{#if f.avatar}<img src={f.avatar} alt="" />{:else}{f.display_name.charAt(0)}{/if}</span>
                  {/each}
                  {#if g.friends.length > 4}<span class="face more">+{g.friends.length - 4}</span>{/if}
                </div>
              </div>
              <div class="body">
                <span class="item-title line">{g.name}</span>
                <span class="secondary line">{names(g.friends)}</span>
                <div class="row">
                  <button class="btn primary small" onclick={() => joinFriend(g.friends.find((f) => f.job) ?? g.friends[0])}>
                    <Icon name="users" size={13} />Join {g.friends.length === 1 ? who(g.friends[0].display_name) : "them"}
                  </button>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if snap.recs_loading || snap.recommendations.length || snap.recs_error}
      <section class="section">
        <div class="section-header">
          <div class="col grow" style="gap: 1px">
            <h2 class="section-title">Recommended for you</h2>
            <span class="meta">{library.accounts.length > 1 ? `Based on what your ${library.accounts.length} accounts play` : "Based on what you play"}</span>
          </div>
          <button class="btn tertiary" disabled={snap.recs_loading} onclick={() => run("refresh_recommendations")}>
            <Icon name="refresh" />{snap.recs_loading ? "Finding games…" : "Refresh"}
          </button>
        </div>
        {#if !snap.recommendations.length && snap.recs_error}
          <div class="notice caution"><Icon name="warning" />Couldn't load recommendations: {snap.recs_error}</div>
        {:else if !snap.recommendations.length}
          <div class="grid">
            {#each [0, 1, 2, 3, 4] as i (i)}<div class="glass skeleton pulse"></div>{/each}
          </div>
        {:else}
          <div class="grid">
            {#each snap.recommendations.slice(0, allRecs ? 60 : 10) as rec, i (`${rec.universe_id}-${i}`)}<RecommendationCard {rec} />{/each}
          </div>
          {#if snap.recommendations.length > 10}
            <button class="btn more" onclick={() => (allRecs = !allRecs)}>
              <span class="flip" class:open={allRecs}><Icon name="caret-down" /></span>
              {allRecs ? "Show fewer" : `See ${snap.recommendations.length - 10} more`}
            </button>
          {/if}
        {/if}
      </section>
    {/if}

    {#if snap.instances.length}
      <section class="section">
        <div class="section-header">
          <div class="col grow" style="gap: 1px">
            <h2 class="section-title">Running now</h2>
            <span class="meta">{plural(snap.instances.length, "instance")} active</span>
          </div>
          <button class="link" onclick={() => app.navigate({ name: "running" })}>Manage<Icon name="arrow-right" /></button>
        </div>
        <div class="grid" style="--card: 240px">
          {#each snap.instances as instance (instance.id)}<InstanceCard {instance} variant="tile" />{/each}
        </div>
      </section>
    {/if}

    <section class="section">
      <div class="section-header">
        <h2 class="section-title grow">Your library</h2>
        {#if libraryGames.length}<button class="link" onclick={() => app.navigate({ name: "games" })}>View all<Icon name="arrow-right" /></button>{/if}
      </div>
      {#if libraryGames.length}
        <div class="grid">
          {#each shelfGames as game (game.id)}<GameCard {game} />{/each}
        </div>
      {:else}
        <EmptyState icon="library" title="Your library is empty" body="Save a Roblox game to start building your library.">
          <button class="btn primary" onclick={() => (app.modal = { kind: "add_game" })}><Icon name="add" />Add Game</button>
        </EmptyState>
      {/if}
    </section>

    {#if servers.length}
      <section class="section">
        <div class="section-header">
          <h2 class="section-title grow">Private servers</h2>
          <button class="link" onclick={() => app.navigate({ name: "servers" })}>View all<Icon name="arrow-right" /></button>
        </div>
        <div class="grid" style="--card: 250px">
          {#each servers as server (server.id)}<ServerCard {server} />{/each}
        </div>
      </section>
    {/if}

    <section class="section">
      <div class="section-header">
        <div class="col grow" style="gap: 1px">
          <h2 class="section-title">Your accounts</h2>
          <span class="meta">Click an account to play as it</span>
        </div>
        <button class="link" onclick={() => app.navigate({ name: "accounts" })}>Manage<Icon name="arrow-right" /></button>
      </div>
      <div class="grid" style="--card: 210px">
        {#each library.accounts as account (account.id)}<AccountCard {account} variant="tile" />{/each}
        <button class="card glass-base add-tile" onclick={() => (app.modal = { kind: "add_account", reauth: null })}>
          <Icon name="add" size={15} /><span class="meta">Add account</span>
        </button>
      </div>
    </section>
  {/if}
</div>

<style>
  .news-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 8px;
  }
  .more {
    align-self: center;
    animation: rise 260ms var(--ease) both;
  }
  .flip {
    display: inline-flex;
    transition: transform 260ms var(--ease);
  }
  .flip.open {
    transform: rotate(180deg);
  }
  .fgame {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
  }
  .fgame .art-wrap {
    position: relative;
    aspect-ratio: 16 / 9;
  }
  .fgame .art-wrap :global(.art) {
    width: 100%;
    height: 100%;
  }
  .fgame .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px 12px;
  }
  .faces {
    position: absolute;
    left: 10px;
    bottom: 8px;
    display: flex;
  }
  .face {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-right: -8px;
    border-radius: 50%;
    border: 2px solid rgb(var(--panel));
    background: rgb(var(--surface) / 0.2);
    overflow: hidden;
    font-size: 11px;
    font-weight: 700;
    animation: pop 300ms var(--ease) both;
  }
  .face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .face.more {
    background: rgb(var(--bg) / 0.85);
  }
  .hello {
    align-items: center;
    gap: 10px;
  }
  .greeting {
    margin: 0;
    font-weight: 800;
    font-size: 34px;
    line-height: 1.1;
    letter-spacing: -0.01em;
  }
  .clock {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px 8px 10px;
    border-radius: var(--r-lg);
    margin-right: 4px;
  }
  .sky {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: #f7c948;
    background: rgb(247 201 72 / 0.14);
  }
  .sky.night {
    color: #b8c4ff;
    background: rgb(184 196 255 / 0.14);
  }
  .time {
    font-weight: 700;
    font-size: 17px;
    font-variant-numeric: tabular-nums;
  }
  .welcome {
    padding: 22px;
  }
  .welcome p {
    margin: 8px 0 0;
    max-width: 640px;
  }
  .steps {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .step {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    min-height: 180px;
    align-items: flex-start;
  }
  .step .row {
    width: 100%;
  }
  .num {
    font-weight: 700;
    font-size: 26px;
    color: rgb(var(--surface) / 0.08);
  }
  .featured {
    position: relative;
    height: 220px;
    overflow: hidden;
  }
  .featured :global(.art),
  .shade {
    position: absolute;
    inset: 0;
  }
  .shade {
    background:
      linear-gradient(90deg, rgb(0 0 0 / 0.8), rgb(0 0 0 / 0.45) 38%, transparent 70%),
      linear-gradient(transparent 30%, rgb(0 0 0 / 0.85));
  }
  .content {
    position: absolute;
    inset: auto 0 0 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 20px;
    max-width: 680px;
    color: white;
  }
  .kicker {
    font-weight: 600;
    font-size: 10.5px;
    letter-spacing: 0.04em;
    color: rgb(255 255 255 / 0.7);
  }
  .name {
    margin: 0;
    font-weight: 800;
    font-size: 24px;
    line-height: 1.15;
  }
  .chips {
    gap: 6px;
    margin: 2px 0 4px;
  }
  .content .btn:not(.primary) {
    background: rgb(255 255 255 / 0.1);
    border-color: rgb(255 255 255 / 0.14);
    color: white;
  }
  .recents {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }
  .recent {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px;
  }
  .recent :global(.thumb) {
    width: 96px;
    height: 54px;
    flex: none;
  }
  .skeleton {
    height: 236px;
  }
  .add-tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    min-height: 46px;
    color: rgb(var(--muted));
  }
</style>
