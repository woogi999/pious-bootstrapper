<!-- Internet settings for Roblox: its DNS servers, a connection check and
     clearing Windows' DNS cache. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../lib/state.svelte";
  import { run } from "../lib/api";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";

  interface Provider {
    id: string;
    name: string;
    servers: string[];
  }
  interface Check {
    lookup_ms: number | null;
    addresses: string[];
    response_ms: number | null;
    error: string | null;
    servers: string[];
  }

  const prefs = $derived(app.snap!.bootstrapper.preferences);
  let providers = $state<Provider[]>([]);
  invoke<Provider[]>("dns_providers").then((p) => (providers = p));

  // What's picked here, applied with the button (Windows asks first).
  let choice = $state("");
  let custom = $state("");
  $effect(() => {
    choice = prefs.dns || "auto";
    custom = prefs.dns_custom.join(", ");
  });
  const customList = $derived(custom.split(/[\s,;]+/).filter(Boolean));
  const changed = $derived(choice !== (prefs.dns || "auto") || (choice === "custom" && customList.join(",") !== prefs.dns_custom.join(",")));
  let applying = $state(false);
  async function apply() {
    applying = true;
    await run("set_dns", { choice, custom: choice === "custom" ? customList : [] });
    applying = false;
    check();
  }

  let checking = $state(false);
  let result = $state<Check | null>(null);
  async function check() {
    checking = true;
    result = await invoke<Check>("check_connection").catch((e) => ({ lookup_ms: null, addresses: [], response_ms: null, error: String(e), servers: [] }));
    checking = false;
  }
  const options = $derived([
    { value: "auto", label: "Your network's DNS" },
    ...providers.map((p) => ({ value: p.id, label: `${p.name} (${p.servers[0]})` })),
    { value: "custom", label: "Custom…" },
  ]);
  const speed = (ms: number | null) => (ms == null ? "—" : `${ms} ms`);
</script>

<section class="group">
  <span class="label">Internet</span>
  <div class="glass list">
    <SettingRow
      icon="globe"
      title="DNS for Roblox"
      description="Which servers look up Roblox's addresses. A faster or more reliable DNS can fix games that won't load or connect. Only Roblox's addresses use it; the rest of your PC keeps its own. Windows asks for administrator rights to change it."
    >
      <Select {options} value={choice} onchange={(v) => (choice = v)} width="230px" />
    </SettingRow>
    {#if choice === "custom"}
      <div class="custom">
        <input class="input" placeholder="DNS servers, like 1.1.1.1, 9.9.9.9" bind:value={custom} spellcheck="false" />
      </div>
    {/if}
    {#if changed}
      <div class="apply">
        <span class="secondary grow">Not applied yet.</span>
        <button class="btn tertiary" onclick={() => ((choice = prefs.dns || "auto"), (custom = prefs.dns_custom.join(", ")))}>Undo</button>
        <button class="btn primary" disabled={applying || (choice === "custom" && !customList.length)} onclick={apply}>
          <Icon name="shield" />{applying ? "Waiting for Windows…" : "Apply"}
        </button>
      </div>
    {/if}
    <hr class="divider" />
    <SettingRow icon="signal" title="Check the connection" description="Times looking up Roblox's address and Roblox's reply.">
      <button class="btn" disabled={checking} onclick={check}><span class:spin={checking}><Icon name="refresh" /></span>{checking ? "Checking…" : "Check"}</button>
    </SettingRow>
    {#if result}
      <div class="result">
        {#if result.error}
          <div class="notice caution"><Icon name="warning" />{result.error}</div>
        {/if}
        <div class="stats">
          <div class="glass-base stat"><span class="stat-value">{speed(result.lookup_ms)}</span><span class="secondary">Address lookup</span></div>
          <div class="glass-base stat"><span class="stat-value">{speed(result.response_ms)}</span><span class="secondary">Roblox's reply</span></div>
          <div class="glass-base stat">
            <span class="stat-value small">{result.servers.length ? result.servers.join(", ") : "Your network's"}</span><span class="secondary">DNS for Roblox</span>
          </div>
        </div>
      </div>
    {/if}
    <hr class="divider" />
    <SettingRow icon="remove" title="Clear the DNS cache" description="Makes Windows look addresses up again. Helps after changing DNS or when a game's servers moved.">
      <button class="btn" onclick={() => run("flush_dns")}><Icon name="refresh" />Clear</button>
    </SettingRow>
  </div>
</section>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list {
    padding: 2px 14px;
  }
  .custom,
  .apply,
  .result {
    display: flex;
    gap: 8px;
    padding: 0 0 12px;
    align-items: center;
    animation: rise 220ms var(--ease) both;
  }
  .result {
    flex-direction: column;
    align-items: stretch;
  }
  .custom .input {
    flex: 1;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-radius: var(--r-md);
  }
  .stat-value {
    font-weight: 800;
    font-size: 17px;
    font-variant-numeric: tabular-nums;
  }
  .stat-value.small {
    font-size: 13px;
    padding: 3px 0;
  }
  .spin {
    display: inline-flex;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
