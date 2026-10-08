<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { lastGame, openLaunch, run } from "../lib/api";
  import { versionMenu } from "../lib/menus";
  import { relativeInline } from "../lib/format";
  import type { VersionRecord } from "../lib/types";
  import Icon from "./Icon.svelte";
  import InstallProgress from "./InstallProgress.svelte";

  let { record, row = false }: { record: VersionRecord; row?: boolean } = $props();

  const snap = $derived(app.snap!);
  const install = $derived(snap.installs[record.hash]);
  const usable = $derived(snap.usable_versions.includes(record.hash));
  const inUse = $derived(snap.instances.some((i) => i.version === record.hash));
  const isLatest = $derived(snap.latest?.hash === record.hash);
  const prefDefault = $derived(snap.bootstrapper.preferences.default_version);
  const isDefault = $derived(
    (typeof prefDefault === "object" && prefDefault.Specific === record.hash) ||
      (prefDefault === "Latest" && snap.latest_installed === record.hash),
  );
  const managedBy = $derived(snap.bootstrappers.find((b) => b.version === record.hash)?.name);
  const state = $derived(
    install
      ? { icon: install.updating ? "refresh" : "download", text: install.updating ? "Updating" : "Downloading", tone: "accent" }
      : !record.valid && !usable
        ? { icon: "warning", text: "Unavailable", tone: "strong" }
        : inUse
          ? { icon: "running", text: "In use", tone: "strong" }
          : { icon: "success", text: "Installed", tone: "strong" },
  );
  const source = $derived(
    managedBy ? `${managedBy}'s current build` : record.source === "Pious" ? "Pious" : record.source === "Roblox" ? "Roblox installer" : "Bootstrapper",
  );
</script>

<div class="card glass version" class:row-mode={row} role="listitem" oncontextmenu={(e) => app.openMenu(e, versionMenu(record.hash))}>
  <div class="row" style="gap: 10px">
    <span class="tile-icon" style="width: 36px; height: 36px"><Icon name="cube" size={18} /></span>
    <div class="col grow" style="gap: 1px">
      <span class="item-title line">{snap.version_titles[record.hash]}</span>
      <span class="secondary line selectable">{record.hash}</span>
    </div>
    <button
      class="icon-btn"
      aria-label="Remove version"
      title={inUse ? "In use" : "Remove this version"}
      disabled={inUse || !!install}
      onclick={(e) => {
        const item = versionMenu(record.hash).find((m) => m !== "separator" && m.label === "Remove version");
        if (item && item !== "separator") item.action();
        e.stopPropagation();
      }}
    >
      <Icon name="remove" />
    </button>
    <button class="icon-btn" aria-label="More" onclick={(e) => app.openMenu(e, versionMenu(record.hash))}><Icon name="more" /></button>
  </div>
  <div class="row" style="gap: 4px; flex-wrap: wrap">
    <span class="badge {state.tone}"><Icon name={state.icon} size={11} />{state.text}</span>
    {#if isLatest}<span class="badge"><Icon name="lightning" size={11} />Latest</span>{/if}
    {#if isDefault}<span class="badge"><Icon name="pin" size={11} />Default</span>{/if}
    {#if snap.tweaked_versions.includes(record.hash)}<span class="badge accent"><Icon name="sliders" size={11} />Tweaked</span>{/if}
  </div>
  <span class="secondary line">{source} · installed {relativeInline(record.installed_at)}</span>
  <hr class="divider" />
  {#if install}
    <InstallProgress {install} />
  {:else}
    <div class="row" style="justify-content: flex-end; gap: 6px">
      <button class="btn small" disabled={isDefault || !usable} onclick={() => run("set_default_version", { choice: { Specific: record.hash } })}>
        <Icon name="pin" size={12} />Make default
      </button>
      <button class="btn primary small" disabled={!usable} onclick={() => openLaunch(lastGame(), { version: record.hash })}>
        <Icon name="play" size={12} />Launch
      </button>
    </div>
  {/if}
</div>

<style>
  .version.row-mode {
    display: grid;
    grid-template-columns: minmax(240px, 1.3fr) minmax(150px, auto) minmax(160px, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 8px 12px;
  }
  .version.row-mode > hr {
    display: none;
  }
  .version {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    cursor: default;
  }
  .version:hover {
    transform: none;
  }
</style>
