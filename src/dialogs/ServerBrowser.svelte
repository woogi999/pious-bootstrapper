<!-- Every public server of a game with room, sortable, each with Join. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { call, handleLaunch, run } from "../lib/api";
  import type { LaunchOutcome, PublicServer, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";

  let { game }: { game: Uuid } = $props();

  type Sort = "players" | "free" | "fill" | "ping" | "fps";
  const snap = $derived(app.snap!);
  const name = $derived(snap.bootstrapper.games.find((g) => g.id === game)?.name ?? "this game");
  const current = $derived(new Set(snap.instances.filter((i) => i.game === game).map((i) => i.job)));

  let servers = $state<PublicServer[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let sort = $state<Sort>("players");
  let descending = $state(true);
  let hideAlmostFull = $state(false);

  async function load(refresh = false) {
    loading = true;
    error = null;
    try {
      servers = await call<PublicServer[]>("list_servers", { game, refresh });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(() => load());

  const key: Record<Sort, (s: PublicServer) => number> = {
    players: (s) => s.playing,
    free: (s) => s.max_players - s.playing,
    fill: (s) => (s.max_players ? s.playing / s.max_players : 0),
    ping: (s) => s.ping ?? 9999,
    fps: (s) => s.fps ?? 0,
  };
  const sorted = $derived(
    servers
      .filter((s) => !hideAlmostFull || s.max_players - s.playing >= 2)
      .sort((a, b) => (descending ? key[sort](b) - key[sort](a) : key[sort](a) - key[sort](b))),
  );

  function pick(by: Sort) {
    if (sort === by) descending = !descending;
    else {
      sort = by;
      // Lower is better for ping; higher for the rest.
      descending = by !== "ping";
    }
  }

  async function join(job: string) {
    app.modal = null;
    const once = (force: boolean) => run<LaunchOutcome>("join_job", { game, job, force });
    handleLaunch(await once(false), () => once(true));
  }
</script>

{#snippet header(by: Sort, label: string)}
  <button class="th" class:on={sort === by} onclick={() => pick(by)}>
    {label}{#if sort === by}<span class="arrow" class:up={!descending}><Icon name="caret-down" size={11} /></span>{/if}
  </button>
{/snippet}

<ModalFrame title="Servers" subtitle={`Public servers of ${name} with room to join`} width={620}>
  <div class="row" style="gap: 8px">
    <Select
      options={[
        { value: "players" as Sort, label: "Most players" },
        { value: "free" as Sort, label: "Most free slots" },
        { value: "fill" as Sort, label: "Fullest" },
        { value: "ping" as Sort, label: "Lowest ping" },
        { value: "fps" as Sort, label: "Highest server FPS" },
      ]}
      value={sort}
      onchange={(v) => ((sort = v), (descending = v !== "ping"))}
      width="200px"
    />
    <button class="chip" class:on={hideAlmostFull} onclick={() => (hideAlmostFull = !hideAlmostFull)}>At least 2 free slots</button>
    <span class="spacer"></span>
    <span class="secondary">{sorted.length} servers</span>
    <button class="icon-btn" aria-label="Refresh" disabled={loading} onclick={() => load(true)}><Icon name="refresh" /></button>
  </div>

  {#if error}
    <div class="notice negative"><Icon name="warning" />{error}</div>
  {:else if loading && !servers.length}
    <div class="meta pulse" style="padding: 20px; text-align: center">Asking Roblox for servers…</div>
  {:else}
    <div class="table">
      <div class="tr head">
        {@render header("players", "Players")}
        {@render header("free", "Free")}
        {@render header("fill", "Full")}
        {@render header("ping", "Ping")}
        {@render header("fps", "FPS")}
        <span></span>
      </div>
      {#each sorted as server (server.job)}
        {@const fill = server.max_players ? server.playing / server.max_players : 0}
        <div class="tr" class:mine={current.has(server.job)}>
          <span class="num">{server.playing}/{server.max_players}</span>
          <span class="num">{server.max_players - server.playing}</span>
          <span class="bar"><span style="width: {fill * 100}%"></span></span>
          <span class="num">{server.ping != null ? `${server.ping} ms` : "—"}</span>
          <span class="num">{server.fps != null ? Math.round(server.fps) : "—"}</span>
          {#if current.has(server.job)}
            <span class="badge strong">You're here</span>
          {:else}
            <button class="btn small primary" onclick={() => join(server.job)}><Icon name="arrow-right" size={12} />Join</button>
          {/if}
        </div>
      {:else}
        <div class="meta" style="padding: 16px">No public servers have room right now.</div>
      {/each}
    </div>
  {/if}
</ModalFrame>

<style>
  .table {
    display: flex;
    flex-direction: column;
    max-height: 52vh;
    overflow-y: auto;
    border-radius: var(--r-md);
    border: 1px solid rgb(var(--surface) / 0.06);
  }
  .tr {
    display: grid;
    grid-template-columns: 90px 60px minmax(0, 1fr) 80px 60px 96px;
    align-items: center;
    gap: 10px;
    padding: 5px 10px;
  }
  .tr:nth-child(even):not(.head) {
    background: rgb(var(--surface) / 0.025);
  }
  .tr.mine {
    background: rgb(var(--accent) / 0.08);
  }
  .tr.head {
    position: sticky;
    top: 0;
    z-index: 3;
    background: rgb(var(--panel));
  }
  .th {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    border: none;
    background: none;
    padding: 4px 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgb(var(--faint));
    cursor: pointer;
    text-align: left;
  }
  .th.on {
    color: rgb(var(--text));
  }
  .arrow.up {
    transform: rotate(180deg);
  }
  .num {
    font-variant-numeric: tabular-nums;
    font-size: 12.5px;
  }
  .bar {
    height: 5px;
    border-radius: 99px;
    background: rgb(var(--surface) / 0.08);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: rgb(var(--accent));
    border-radius: inherit;
  }
</style>
