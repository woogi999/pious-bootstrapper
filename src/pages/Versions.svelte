<script lang="ts">
  import { app, sortOf, viewOf } from "../lib/state.svelte";
  import { run } from "../lib/api";
  import type { VersionChoice } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import InstallProgress from "../components/InstallProgress.svelte";
  import Select from "../components/Select.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import Tip from "../components/Tip.svelte";
  import VersionCard from "../components/VersionCard.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const installed = $derived(snap.usable_versions.length);
  const sort = $derived(sortOf("versions", "newest"));
  const versions = $derived(
    [...snap.bootstrapper.versions].sort((a, b) => {
      const usable = Number(snap.usable_versions.includes(b.hash)) - Number(snap.usable_versions.includes(a.hash));
      if (sort === "name") return usable || snap.version_titles[a.hash].localeCompare(snap.version_titles[b.hash], undefined, { numeric: true });
      if (sort === "source") return usable || a.source.localeCompare(b.source) || b.installed_at.localeCompare(a.installed_at);
      if (sort === "oldest") return usable || a.installed_at.localeCompare(b.installed_at);
      return usable || b.installed_at.localeCompare(a.installed_at);
    }),
  );
  // weao.xyz's builds; the older ones are in Install version.
  const known = $derived(snap.builds.filter((b) => b.kind !== "History"));
  const older = $derived(snap.builds.length - known.length);
  const pending = $derived(
    Object.entries(snap.installs).filter(([hash]) => !snap.bootstrapper.versions.some((v) => v.hash === hash) && snap.latest?.hash !== hash),
  );
  const choices = $derived([
    { value: "Latest" as VersionChoice, label: snap.latest_installed ? `Latest installed · ${snap.version_titles[snap.latest_installed]}` : "Latest installed" },
    ...snap.bootstrapper.versions
      .filter((v) => snap.usable_versions.includes(v.hash))
      .map((v) => ({ value: { Specific: v.hash } as VersionChoice, label: `${snap.version_titles[v.hash]} (${v.hash.replace("version-", "").slice(0, 8)})` })),
  ]);

  function effects(b: (typeof snap.bootstrappers)[number]): string[] {
    const list: string[] = [];
    if (b.opens_links) list.push("Opens Roblox when you press Play on roblox.com");
    if (b.discord_presence) list.push("Shows what you're playing on Discord");
    if (b.multi_instance) list.push("Allows several Roblox windows at once");
    if (b.fast_flags) list.push(`Applies ${b.fast_flags} FastFlag${b.fast_flags === 1 ? "" : "s"} to the Roblox client`);
    if (b.mods) list.push(`Replaces ${b.mods} Roblox file${b.mods === 1 ? "" : "s"} with mods`);
    if (b.activity_tracking) list.push("Reads the Roblox log to track your servers");
    return list;
  }

  function notes(b: (typeof snap.bootstrappers)[number]): string[] {
    const handle = snap.bootstrapper.preferences.handle_roblox_links;
    const list: string[] = [];
    if (b.opens_links) {
      list.push(
        handle
          ? `Pious opens Roblox links for now; turning that off in Settings hands them back to ${b.name}.`
          : `Pious leaves Roblox links to ${b.name}. Games Pious starts still use the version you chose.`,
      );
    } else if (handle) {
      list.push(`Pious opens Roblox links instead of ${b.name} (see Settings).`);
    }
    // Only while it actually starts games (opens links, or Pious launches through it).
    const active = (b.opens_links && !handle) || snap.bootstrapper.preferences.launch_via === b.name;
    if (b.discord_presence && active) list.push(`Games ${b.name} starts keep its Discord presence; Pious shows it for games it starts itself.`);
    if (b.fast_flags || b.mods) {
      list.push(`Pious never edits ${b.name}'s FastFlags or mods. They're applied to its own builds; launch one of those to use them.`);
    }
    return list;
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Roblox Versions</h1>
      <span class="meta">
        {installed === 0 ? "The Roblox builds available to Pious" : installed === 1 ? "1 version ready to launch" : `${installed} versions ready to launch`}
      </span>
    </div>
    <button class="btn" disabled={snap.scanning} onclick={() => run("scan_versions")}><Icon name="refresh" />{snap.scanning ? "Checking…" : "Check again"}</button>
    <button class="btn" onclick={() => (app.modal = { kind: "install_version" })}><Icon name="download" />Install version</button>
  </div>

  <Tip id="versions">Pick which Roblox build each game launches with. Install version lists every build weao.xyz and Roblox know about.</Tip>

  <div class="glass panel-row">
    <span class="tile-icon" style="width: 32px; height: 32px"><Icon name="pin" size={15} /></span>
    <div class="col grow" style="gap: 1px">
      <span class="item-title">Default version</span>
      <span class="meta">Games without their own version use this one.</span>
    </div>
    <Select
      options={choices}
      value={snap.bootstrapper.preferences.default_version}
      onchange={(choice) => run("set_default_version", { choice })}
      placeholder="Choose a version"
      width="300px"
    />
  </div>

  <section class="section">
    <h2 class="section-title">Available from Roblox</h2>
    <div class="glass-elevated panel-row">
      {#if snap.latest}
        {@const latest = snap.latest}
        <span class="tile-icon" style="width: 36px; height: 36px"><Icon name="cloud" size={18} /></span>
        <div class="col grow" style="gap: 1px">
          <span class="label">Current Roblox release</span>
          <span class="item-title">Version {latest.version}</span>
          <span class="secondary selectable">{latest.hash}</span>
        </div>
        {#if snap.installs[latest.hash]}
          <div style="width: 340px"><InstallProgress install={snap.installs[latest.hash]} /></div>
        {:else if snap.usable_versions.includes(latest.hash)}
          <span class="badge strong"><Icon name="success" size={11} />Installed</span>
        {:else}
          <button class="btn primary" onclick={() => run("install_version", { hash: latest.hash, version: latest.version })}>
            <Icon name="download" />Download
          </button>
        {/if}
      {:else if snap.latest_error}
        <Icon name="warning" size={18} />
        <div class="col grow"><span class="item-title">Couldn't check for the current Roblox release</span><span class="secondary">{snap.latest_error}</span></div>
        <button class="btn" onclick={() => run("scan_versions")}><Icon name="refresh" />Retry</button>
      {:else}
        <Icon name="clock" size={18} /><span class="meta">Checking Roblox for the current release…</span>
      {/if}
    </div>
  </section>

  <section class="section">
    <div class="section-header">
      <div class="col grow" style="gap: 1px">
        <h2 class="section-title">Previous & upcoming builds</h2>
        <span class="meta">Listed by weao.xyz, downloaded straight from Roblox{older ? ` · ${older} older builds in Install version` : ""}</span>
      </div>
      <button class="btn tertiary" disabled={snap.builds_loading} onclick={() => run("load_builds")}>
        <Icon name="refresh" />{snap.builds_loading ? "Checking…" : "Refresh"}
      </button>
    </div>
    <div class="glass builds stagger">
      {#if !known.length && snap.builds_error}
        <div class="notice caution"><Icon name="warning" />Couldn't reach weao.xyz: {snap.builds_error}</div>
      {:else if !known.length}
        <span class="meta" style="padding: 8px">Checking weao.xyz for builds…</span>
      {:else}
        {#each known as build, i (`${build.kind}-${build.hash}-${i}`)}
          <div class="row build">
            <span class="kind"><span class="badge {build.kind === 'Current' ? 'strong' : build.kind === 'Upcoming' ? 'accent' : ''}"><Icon name="cube" size={11} />{build.kind}</span></span>
            <div class="col grow" style="gap: 1px">
              <span class="item-title">{build.version ? `Version ${build.version}` : build.hash}</span>
              <span class="secondary line selectable">{build.hash} · {build.date}</span>
            </div>
            {#if snap.installs[build.hash]}
              <div style="width: 260px"><InstallProgress install={snap.installs[build.hash]} /></div>
            {:else if snap.usable_versions.includes(build.hash)}
              <span class="badge strong"><Icon name="success" size={11} />Installed</span>
            {:else}
              <button class="btn small" onclick={() => run("install_version", { hash: build.hash, version: build.version || null })}>
                <Icon name="download" size={12} />Install
              </button>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </section>

  <section class="section">
    <div class="section-header">
      <h2 class="section-title grow">Installed</h2>
      {#if snap.roblox_uninstallable}
        <button
          class="btn tertiary"
          title="Runs Roblox's own uninstaller"
          onclick={() =>
            app.confirm(
              "Uninstall Roblox?",
              "Roblox's own uninstaller opens and removes the Roblox player from this PC. Versions Pious downloaded stay until you remove them here.",
              "Uninstall",
              () => run("uninstall_app", { name: "Roblox" }),
            )}
        >
          <Icon name="remove" />Uninstall Roblox
        </button>
      {/if}
      <SortSelect
        page="versions"
        value={sort}
        options={[
          { value: "newest", label: "Newest" },
          { value: "oldest", label: "Oldest" },
          { value: "name", label: "Version" },
          { value: "source", label: "Where from" },
        ]}
      />
      <ViewToggle page="versions" />
    </div>
    {#if !versions.length && !pending.length}
      <EmptyState icon="versions" title="No Roblox versions detected" body="Pious couldn't find an available Roblox version.">
        <button class="btn primary" onclick={() => run("scan_versions")}><Icon name="refresh" />Check Again</button>
      </EmptyState>
    {:else}
      <div class={viewOf("versions") === "List" ? "list-rows" : "grid"} style="--card: 290px">
        {#each pending as [hash, install] (hash)}
          <div class="glass pending">
            <div class="row" style="gap: 10px">
              <span class="tile-icon" style="width: 36px; height: 36px"><Icon name="download" size={18} /></span>
              <div class="col grow"><span class="item-title">{install.version ? `Version ${install.version}` : "New build"}</span><span class="secondary line">{hash}</span></div>
            </div>
            <InstallProgress {install} />
          </div>
        {/each}
        {#each versions as record (record.hash)}<VersionCard {record} row={viewOf("versions") === "List"} />{/each}
      </div>
    {/if}
  </section>

  <section class="section">
    <div class="section-header">
      <div class="col grow" style="gap: 1px">
        <h2 class="section-title">Bootstrappers</h2>
        <span class="meta">Bloxstrap, Fishstrap and similar launchers found on this PC</span>
      </div>
      <button class="btn tertiary" onclick={() => run("refresh_bootstrappers")}><Icon name="refresh" />Check again</button>
    </div>
    {#if !snap.bootstrappers.length}
      <div class="glass-base panel-row meta">None found. Pious launches Roblox on its own, so nothing else is changing your client.</div>
    {:else}
      {#each snap.bootstrappers as b (b.name)}
        <div class="glass boot">
          <div class="row" style="gap: 10px">
            <span class="tile-icon" style="width: 30px; height: 30px"><Icon name="lightning" size={15} /></span>
            <div class="col grow" style="gap: 1px"><span class="item-title">{b.name}</span><span class="secondary line">{b.folder}</span></div>
            <button class="btn tertiary small" onclick={() => run("open_path", { path: b.folder })}><Icon name="folder" />Open</button>
            {#if b.uninstallable}
              <button
                class="btn small danger"
                onclick={() =>
                  app.confirm(
                    `Uninstall ${b.name}?`,
                    `${b.name}'s own uninstaller opens. If it opens Roblox links now, set "Roblox links open with" to Pious or Roblox in Settings afterwards.`,
                    "Uninstall",
                    () => run("uninstall_app", { name: b.name }),
                  )}
              >
                <Icon name="remove" />Uninstall
              </button>
            {/if}
          </div>
          <div class="col" style="gap: 4px">
            {#each effects(b) as effect, i (i)}<span class="meta effect">{effect}</span>{:else}<span class="secondary">Installed, but not changing anything right now.</span>{/each}
          </div>
          {#each notes(b) as note, i (i)}<div class="notice"><Icon name="shield" />{note}</div>{/each}
        </div>
      {/each}
    {/if}
  </section>
</div>

<style>
  .panel-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
  }
  .builds {
    padding: 4px 12px;
  }
  .build {
    gap: 12px;
    padding: 8px 0;
  }
  .build + .build {
    border-top: 1px solid rgb(var(--surface) / 0.05);
  }
  .kind {
    width: 100px;
    flex: none;
  }
  .pending,
  .boot {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .effect::before {
    content: "•";
    margin-right: 8px;
    color: rgb(var(--faint));
  }
</style>
