<!-- "Play with…" (pick the account, version and server for one launch) and
     "Edit launch configuration" (what Play does for a game). -->
<script lang="ts">
  import { untrack } from "svelte";
  import { app, type Modal } from "../lib/state.svelte";
  import { launch, run } from "../lib/api";
  import { accountLabel, versionLabel, who } from "../lib/format";
  import type { ServerChoice, Uuid, VersionChoice } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";

  let { modal }: { modal: Extract<Modal, { kind: "launch" }> } = $props();

  const snap = $derived(app.snap!);
  // The dialog edits its own copy, starting from what it was opened with.
  const initial = untrack(() => modal);
  let game = $state(initial.game);
  let account = $state(initial.account);
  let version = $state<VersionChoice>(initial.version);
  let server = $state<ServerChoice>(initial.server);
  let remember = $state(false);
  // Where a public server is joined (only for public servers).
  let region = $state(app.snap!.bootstrapper.preferences.region);
  let regions = $state<[string, string][]>([]);
  run<[string, string][]>("regions").then((r) => (regions = r ?? []));
  const edit = initial.mode === "edit";

  const current = $derived(snap.bootstrapper.games.find((g) => g.id === game));
  const games = $derived(snap.bootstrapper.games.map((g) => ({ value: g.id as Uuid | null, label: g.name })));

  const accounts = $derived.by(() => {
    const list: { value: Uuid | null; label: string }[] = [];
    if (edit) {
      const active = snap.bootstrapper.accounts.find((a) => a.id === snap.active_account);
      list.push({ value: null, label: `Whichever account is picked in the sidebar${active ? ` · now ${accountLabel(active)}` : ""}` });
    }
    for (const a of snap.bootstrapper.accounts) {
      const running = snap.instances.some((i) => i.account === a.id);
      list.push({
        value: a.id,
        label: `${accountLabel(a)} (@${who(a.username)})${a.needs_sign_in ? " — sign-in needed" : running ? " — playing" : ""}`,
      });
    }
    return list;
  });

  const versions = $derived([
    { value: "Default" as VersionChoice, label: versionLabel(snap, "Default") },
    { value: "Latest" as VersionChoice, label: versionLabel(snap, "Latest") },
    ...snap.bootstrapper.versions
      .filter((v) => snap.usable_versions.includes(v.hash))
      .map((v) => ({ value: { Specific: v.hash } as VersionChoice, label: `${snap.version_titles[v.hash]} (${v.hash.replace("version-", "").slice(0, 8)})` })),
  ]);

  const servers = $derived([
    { value: "Public" as ServerChoice, label: "Public server" },
    ...snap.bootstrapper.servers
      .filter((s) => s.game_id === game)
      .map((s) => ({ value: { Private: s.id } as ServerChoice, label: `${s.name}${s.favorite ? "  ★" : ""}` })),
  ]);

  const missingVersion = $derived(
    typeof version === "object"
      ? "Specific" in version && !snap.usable_versions.includes(version.Specific)
      : !snap.default_version && !snap.latest_installed,
  );
  const canConfirm = $derived(!!game && (!!account || edit) && !(typeof version === "object" && missingVersion));

  function pickGame(id: Uuid | null) {
    game = id;
    if (typeof server === "object" && !snap.bootstrapper.servers.some((s) => s.id === (server as { Private: Uuid }).Private && s.game_id === id)) {
      server = "Public";
    }
  }

  function confirm() {
    if (!game) return;
    app.modal = null;
    if (edit) {
      run("set_launch_config", { game, account, version, server });
    } else {
      launch({ game, account, version, server, remember, pin_account: remember, region: server === "Public" ? region : null });
    }
  }
</script>

<ModalFrame
  title={edit ? "Launch Configuration" : "Launch Game"}
  subtitle={edit ? "What Play uses for this game." : current?.name}
  width={500}
>
  {#if !edit && initial.pickGame}
    {#if games.length}
      <div class="field"><span class="label">Game</span><Select options={games} value={game} onchange={pickGame} /></div>
    {:else}
      <div class="notice caution"><Icon name="info" />Add a game to your library first.</div>
    {/if}
  {/if}
  <div class="field"><span class="label">Account</span><Select options={accounts} value={account} onchange={(v) => (account = v)} /></div>
  <div class="field"><span class="label">Roblox version</span><Select options={versions} value={version} onchange={(v) => (version = v)} /></div>
  <div class="field"><span class="label">Server</span><Select options={servers} value={server} onchange={(v) => (server = v)} /></div>
    {#if server === "Public" && modal.mode === "launch"}
      <div class="field">
        <span class="label">Region</span>
        <Select
          options={[{ value: "auto", label: "Automatic (Roblox picks)" }, { value: "best_ping", label: "Best ping" }, ...regions.map(([value, label]) => ({ value, label }))]}
          value={region}
          onchange={(v) => (region = v)}
        />
      </div>
    {/if}
  {#if missingVersion}
    <div class="notice">
      <Icon name="info" />{typeof version === "object"
        ? "That version isn't installed. Pick another or install it from Versions."
        : "No managed version is installed — Roblox will launch with your system installation."}
    </div>
  {/if}
  {#if !edit && current}
    <label class="row check">
      <input type="checkbox" bind:checked={remember} />
      <span class="meta">Always use this setup for this game</span>
    </label>
  {/if}
  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    <button class="btn primary" style="padding: 0 22px" disabled={!canConfirm} onclick={confirm}>
      <Icon name={edit ? "check" : "play"} />{edit ? "Save" : "PLAY"}
    </button>
  {/snippet}
</ModalFrame>

<style>
  .check {
    gap: 10px;
    cursor: pointer;
  }
  .check input {
    accent-color: rgb(var(--accent));
    width: 15px;
    height: 15px;
  }
</style>
