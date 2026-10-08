<!-- Confirm, rename, collection and install-version dialogs. -->
<script lang="ts">
  import { app, type Modal } from "../lib/state.svelte";
  import { call, run } from "../lib/api";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";

  let { modal }: { modal: Extract<Modal, { kind: "confirm" | "rename" | "collection" | "install_version" }> } = $props();

  const snap = $derived(app.snap!);

  // Rename
  const account = $derived(modal.kind === "rename" ? snap.bootstrapper.accounts.find((a) => a.id === modal.account) : null);
  let alias = $state("");
  // Collection
  const game = $derived(modal.kind === "collection" ? snap.bootstrapper.games.find((g) => g.id === modal.game) : null);
  let collection = $state("");
  // Install
  let hash = $state("");
  let installError = $state<string | null>(null);
  let buildQuery = $state("");
  const installed = $derived(new Set(snap.bootstrapper.versions.filter((v) => v.valid).map((v) => v.hash)));
  const builds = $derived(
    snap.builds.filter((b) => {
      const q = buildQuery.trim().toLowerCase();
      return !q || b.hash.toLowerCase().includes(q) || b.version.toLowerCase().includes(q) || b.date.toLowerCase().includes(q);
    }),
  );
  const KIND: Record<string, string> = { Current: "Current", Previous: "Previous", Upcoming: "Upcoming", History: "Older" };
  $effect(() => {
    if (modal.kind === "install_version" && !snap.builds.length && !snap.builds_loading) run("load_builds");
  });

  $effect.pre(() => {
    if (modal.kind === "rename") alias = account?.alias ?? account?.display_name ?? "";
    if (modal.kind === "collection") collection = game?.collection ?? "";
  });

  const collections = $derived(
    [...new Set(snap.bootstrapper.games.filter((g) => g.in_library && g.collection).map((g) => g.collection!))].sort((a, b) =>
      a.localeCompare(b),
    ),
  );

  async function confirm() {
    if (modal.kind !== "confirm") return;
    const action = modal.action;
    app.modal = null;
    await action();
  }

  function saveCollection(name: string | null) {
    if (modal.kind !== "collection") return;
    run("set_collection", { game: modal.game, name });
    app.modal = null;
  }

  async function install() {
    try {
      await call("install_hash", { hash });
      app.modal = null;
    } catch (e) {
      installError = String(e);
    }
  }
</script>

{#if modal.kind === "confirm"}
  <ModalFrame title={modal.title} width={440}>
    <p class="meta body">{modal.body}</p>
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn danger" autofocus onclick={confirm}>{modal.confirm}</button>
    {/snippet}
  </ModalFrame>
{:else if modal.kind === "rename"}
  <ModalFrame title="Rename account" subtitle="A local nickname only you see. Your Roblox name doesn't change." width={440}>
    <div class="field">
      <span class="label">Nickname</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        autofocus
        placeholder="e.g. Main, Alt, Trading"
        bind:value={alias}
        onkeydown={(e) => e.key === "Enter" && (run("rename_account", { account: modal.account, alias }), (app.modal = null))}
      />
    </div>
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
      <button class="btn primary" onclick={() => (run("rename_account", { account: modal.account, alias }), (app.modal = null))}>
        <Icon name="check" />Save
      </button>
    {/snippet}
  </ModalFrame>
{:else if modal.kind === "collection"}
  <ModalFrame title="Add to collection" subtitle="Group games however you like." width={480}>
    <div class="field">
      <span class="label">Collection</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        autofocus
        placeholder="e.g. Grinding, With friends"
        bind:value={collection}
        onkeydown={(e) => e.key === "Enter" && saveCollection(collection)}
      />
    </div>
    {#if collections.length}
      <div class="field">
        <span class="label">Existing</span>
        <div class="row" style="flex-wrap: wrap; gap: 6px">
          {#each collections as name (name)}
            <button class="chip" class:on={name === collection} onclick={() => saveCollection(name)}><Icon name="folder" />{name}</button>
          {/each}
        </div>
      </div>
    {/if}
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => saveCollection(null)}>Remove from collection</button>
      <span class="spacer"></span>
      <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
      <button class="btn primary" onclick={() => saveCollection(collection)}><Icon name="check" />Save</button>
    {/snippet}
  </ModalFrame>
{:else}
  <ModalFrame title="Install a Roblox version" subtitle="Pick a build from weao.xyz and Roblox's deploy history, or paste its hash." width={580}>
    <div class="field">
      <span class="label">Builds</span>
      <div class="search-box glass-base">
        <Icon name="search" />
        <input placeholder="Search by version, hash or date" bind:value={buildQuery} />
        {#if snap.builds_loading}<span class="secondary">Loading…</span>{/if}
      </div>
      <div class="builds">
        {#if snap.builds_error && !snap.builds.length}
          <div class="notice caution"><Icon name="warning" />{snap.builds_error}</div>
        {:else if !snap.builds.length}
          {#each [0, 1, 2, 3] as i (i)}<div class="build-skeleton pulse"></div>{/each}
        {:else}
          <div class="list-rows">
            {#each builds.slice(0, 120) as b (b.hash)}
              <button class="build" class:on={hash === b.hash} onclick={() => ((hash = b.hash), (installError = null))}>
                <span class="kind {b.kind.toLowerCase()}">{KIND[b.kind] ?? b.kind}</span>
                <span class="col grow" style="gap: 0">
                  <span class="item-title line">{b.version || b.hash}</span>
                  <span class="secondary line">{b.hash}{b.date ? ` · ${b.date}` : ""}</span>
                </span>
                {#if installed.has(b.hash)}<span class="badge">Installed</span>{/if}
                {#if hash === b.hash}<Icon name="check" />{/if}
              </button>
            {/each}
          </div>
          {#if !builds.length}<span class="meta">No builds match.</span>{/if}
        {/if}
      </div>
    </div>
    <div class="field">
      <span class="label">Build hash</span>
      <input
        class="input"
        placeholder="version-1a2b3c4d5e6f7a8b"
        bind:value={hash}
        oninput={() => (installError = null)}
        onkeydown={(e) => e.key === "Enter" && install()}
      />
    </div>
    <span class="secondary">Pious downloads the build directly from Roblox and keeps it alongside your other versions.</span>
    {#if snap.latest}
      <div class="row">
        <span class="meta grow">Current release: {snap.latest.version}</span>
        <button class="link" onclick={() => (hash = snap.latest!.hash)}>Use current<Icon name="arrow-right" /></button>
      </div>
    {/if}
    {#if installError}<div class="notice negative"><Icon name="warning" />{installError}</div>{/if}
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
      <button class="btn primary" disabled={!hash.trim()} onclick={install}><Icon name="download" />Install</button>
    {/snippet}
  </ModalFrame>
{/if}

<style>
  .body {
    margin: 0;
    line-height: 1.5;
  }
  .builds {
    max-height: 280px;
    overflow-y: auto;
    margin: 0 -4px;
    padding: 0 4px;
  }
  .build {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / 0.03);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--fast), border-color var(--fast);
  }
  .build:hover {
    background: rgb(var(--surface) / 0.07);
  }
  .build.on {
    border-color: rgb(var(--accent) / 0.5);
    background: rgb(var(--accent) / 0.08);
  }
  .kind {
    flex: none;
    width: 74px;
    padding: 2px 0;
    border-radius: 99px;
    text-align: center;
    font-size: 10.5px;
    font-weight: 700;
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--muted));
  }
  .kind.current {
    background: rgb(48 164 108 / 0.18);
    color: #4cc38a;
  }
  .kind.upcoming {
    background: rgb(62 156 245 / 0.18);
    color: #70b8ff;
  }
  .kind.previous {
    background: rgb(245 165 36 / 0.16);
    color: #f5b94a;
  }
  .build-skeleton {
    height: 46px;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / 0.05);
    margin-bottom: 6px;
  }
</style>
