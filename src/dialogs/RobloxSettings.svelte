<!-- Roblox's local settings for one account (or the PC's own). Roblox only
     keeps these per PC; with "own settings" on, Pious keeps a copy for the
     account and loads it whenever that account plays. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { call, run } from "../lib/api";
  import { accountLabel } from "../lib/format";
  import type { SettingKind, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import Slider from "../components/Slider.svelte";
  import Switch from "../components/Switch.svelte";

  let { account }: { account: Uuid | null } = $props();

  const snap = $derived(app.snap!);
  const who = $derived(account ? snap.bootstrapper.accounts.find((a) => a.id === account) : null);
  const own = $derived(who?.custom_settings ?? true);

  let values = $state<Record<string, string>>({});
  let kinds = $state<Record<string, SettingKind>>({});
  let loaded = $state(false);

  async function load() {
    const list = await call<[string, SettingKind, string][]>("get_roblox_settings", { account: own ? account : null });
    values = Object.fromEntries(list.map(([name, , value]) => [name, value]));
    kinds = Object.fromEntries(list.map(([name, kind]) => [name, kind]));
    loaded = true;
  }
  onMount(load);

  const labels: Record<string, { title: string; hint: string }> = {
    MasterVolume: { title: "Volume", hint: "Roblox's master volume" },
    MouseSensitivity: { title: "Camera sensitivity", hint: "How fast the camera turns with the mouse" },
    SavedQualityLevel: { title: "Graphics quality", hint: "0 = automatic, 1–10 = manual" },
    FramerateCap: { title: "Frame rate cap", hint: "Roblox's own in-game limit" },
    Fullscreen: { title: "Fullscreen", hint: "Start in fullscreen" },
    CameraYInverted: { title: "Invert camera", hint: "Flip the camera's up and down" },
    PerformanceStatsVisible: { title: "Performance stats", hint: "Show the stats overlay" },
    ReducedMotion: { title: "Reduced motion", hint: "Fewer camera and UI effects" },
    ChatVisible: { title: "Chat open", hint: "Start with chat visible" },
    VoiceChatVolume: { title: "Voice chat volume", hint: "How loud other players' voices are" },
    PlayerListVisible: { title: "Player list", hint: "Show the player list" },
  };

  const fpsChoices = [-1, 30, 60, 120, 144, 165, 240].map((n) => ({ value: String(n), label: n === -1 ? "Unlimited" : `${n} FPS` }));

  /** Copies what Roblox is using right now into this profile. */
  async function importCurrent() {
    await run("import_roblox_settings", { account: own ? account : null });
    await load();
  }

  async function save() {
    await run("set_roblox_settings", { account: own ? account : null, values: Object.entries(values) });
  }
</script>

<ModalFrame
  title="Roblox settings"
  subtitle={who ? `For ${accountLabel(who)}` : "This PC's own settings, used by accounts without their own"}
  width={540}
>
  {#if who}
    <div class="row toggle">
      <div class="col grow" style="gap: 1px">
        <span class="item-title">Keep its own settings</span>
        <span class="meta">Roblox saves settings per PC, not per account. With this on, Pious keeps {accountLabel(who)}'s settings and loads them whenever it plays.</span>
      </div>
      <Switch on={who.custom_settings} onchange={async (on) => (await run("set_custom_settings", { account, on }), load())} />
    </div>
    <hr class="divider" />
  {/if}

  {#if !loaded}
    <div class="meta pulse">Reading Roblox's settings…</div>
  {:else if !Object.keys(values).length}
    <div class="notice"><Icon name="info" />Roblox hasn't saved any settings on this PC yet. Play once, then come back.</div>
  {:else}
    <div class="fields" class:off={!own}>
      {#each Object.entries(labels) as [name, label] (name)}
        {#if values[name] !== undefined}
          <div class="field-row">
            <div class="col grow" style="gap: 0">
              <span class="item-title">{label.title}</span>
              <span class="secondary">{label.hint}</span>
            </div>
            {#if kinds[name] === "Bool"}
              <Switch on={values[name] === "true"} onchange={(on) => (values[name] = on ? "true" : "false")} />
            {:else if name === "FramerateCap"}
              <Select options={fpsChoices} value={values[name]} onchange={(v) => (values[name] = v)} width="150px" />
            {:else if name === "SavedQualityLevel"}
              <Select
                options={Array.from({ length: 11 }, (_, i) => ({ value: String(i), label: i === 0 ? "Automatic" : `Level ${i}` }))}
                value={values[name]}
                onchange={(v) => (values[name] = v)}
                width="150px"
              />
            {:else}
              {@const max = name === "MouseSensitivity" ? 4 : 1}
              <Slider
                value={Number(values[name])}
                min={0}
                {max}
                width="150px"
                scale={max === 1 ? 100 : 1}
                unit={max === 1 ? "%" : ""}
                digits={max === 1 ? 0 : 2}
                onchange={(v) => (values[name] = String(Math.round(v * 1000) / 1000))}
              />
            {/if}
          </div>
        {/if}
      {/each}
    </div>
    {#if who && !own}
      <span class="secondary">These are the PC's shared settings. Turn on "Keep its own settings" to change them only for {accountLabel(who)}.</span>
    {/if}
  {/if}

  {#snippet footer()}
    <button
      class="btn tertiary"
      style="margin-right: auto"
      disabled={!!who && !own}
      title="Copy the settings Roblox is using on this PC right now"
      onclick={importCurrent}
    >
      <Icon name="download" />Import current Roblox settings
    </button>
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Close</button>
    <button class="btn primary" disabled={!loaded || !Object.keys(values).length || (!!who && !own)} onclick={save}><Icon name="check" />Save</button>
  {/snippet}
</ModalFrame>

<style>
  .toggle {
    gap: 12px;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 2px;
    transition: opacity var(--med);
  }
  .fields.off {
    opacity: 0.5;
    pointer-events: none;
  }
  .field-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 0;
  }
</style>
