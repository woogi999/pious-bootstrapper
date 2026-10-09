<!-- Settings → Overlay: the game stats drawn over the game (FPS, ping,
     players, where the server is…), what's shown and how it looks. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { run, setPreferences } from "../lib/api";
  import type { StatItem, StatsOverlay } from "../lib/types";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Slider from "./Slider.svelte";
  import Switch from "./Switch.svelte";

  const o = $derived(app.snap!.bootstrapper.preferences.stats_overlay);
  const off = $derived(!o.enabled);

  // Dragging sliders sends at most one change per frame.
  let pending: Partial<StatsOverlay> = {};
  let frame = 0;
  function set(patch: Partial<StatsOverlay>) {
    pending = { ...pending, ...patch };
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      const out = pending;
      pending = {};
      setPreferences({ stats_overlay: out });
    });
  }

  const ITEMS: [StatItem, string, string][] = [
    ["fps", "FPS", "Frames the game shows each second."],
    ["ping", "Ping", "The server's average ping, as Roblox's server list reports it (not for private servers)."],
    ["players", "Players", "Players in your server and how many fit."],
    ["server_fps", "Server FPS", "How fast the server itself is running."],
    ["location", "Server location", "The city your server is in."],
    ["session", "Time played", "How long this game has been open."],
    ["cpu", "Roblox CPU", "How much of your processor Roblox is using."],
    ["memory", "Roblox memory", "How much memory Roblox is using."],
    ["clock", "Clock", "The time."],
  ];

  function toggle(item: StatItem, on: boolean) {
    // Kept in the list's order.
    const next = ITEMS.map(([id]) => id).filter((id) => (id === item ? on : o.items.includes(id)));
    set({ items: next });
  }

  const hexOf = (color: string) => (color.length >= 7 ? color.slice(0, 7) : "#000000");
  const alphaOf = (color: string) => (color.length === 9 ? parseInt(color.slice(7, 9), 16) / 255 : 1);
  const withAlpha = (hex: string, alpha: number) =>
    `${hex.toUpperCase()}${Math.round(Math.min(1, Math.max(0, alpha)) * 255)
      .toString(16)
      .padStart(2, "0")
      .toUpperCase()}`;
</script>

<section class="glass list">
  <SettingRow icon="chart" title="Show game stats" description="FPS, ping, players, where the server is and more, drawn over the game.">
    <Switch on={o.enabled} onchange={(enabled) => set({ enabled })} />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="target" title="Where it is" description="Move it anywhere over the game. Its spot is kept as a share of the screen." {off}>
    <button class="btn" disabled={off} onclick={() => run("stats_overlay_edit", { on: true })}><Icon name="target" />Move it</button>
    <button class="btn tertiary" disabled={off} onclick={() => set({ x: 0.01, y: 0.01 })}>Reset</button>
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="game" title="Only while playing" description="Show it only while a Roblox window is in front." {off}>
    <Switch on={o.only_in_game} disabled={off} onchange={(only_in_game) => set({ only_in_game })} />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="record" title="Show in recordings" description="Let Pious's recordings and clips (and OBS) see it." {off}>
    <Switch on={o.show_in_recordings} disabled={off} onchange={(show_in_recordings) => set({ show_in_recordings })} />
  </SettingRow>
</section>

<span class="label">What it shows</span>
<section class="glass list" class:off-all={off}>
  {#each ITEMS as [id, title, description], i (id)}
    {#if i > 0}<hr class="divider" />{/if}
    <SettingRow icon="chart" {title} {description} {off}>
      <Switch on={o.items.includes(id)} disabled={off} onchange={(on) => toggle(id, on)} />
    </SettingRow>
  {/each}
</section>

<span class="label">Look</span>
<section class="glass list" class:off-all={off}>
  <SettingRow icon="grid" title="Layout" description="Side by side, or one under another." {off}>
    <Select
      disabled={off}
      options={[
        { value: "row" as const, label: "In a row" },
        { value: "column" as const, label: "In a column" },
      ]}
      value={o.layout}
      onchange={(layout) => set({ layout })}
      width="180px"
    />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="edit" title="Labels" description="Small names (FPS, Ping…) next to each number." {off}>
    <Switch on={o.labels} disabled={off} onchange={(labels) => set({ labels })} />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="focus" title="Size" description="How big it is." {off}>
    <Slider value={o.scale} min={0.6} max={2} step={0.05} disabled={off} onchange={(scale) => set({ scale })} />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="contrast" title="Opacity" description="How see-through all of it is." {off}>
    <Slider value={o.opacity} min={0.2} max={1} step={0.05} disabled={off} onchange={(opacity) => set({ opacity })} />
  </SettingRow>
  <hr class="divider" />
  <SettingRow icon="sparkles" title="Colors" description="Behind it (with its own see-through), the numbers, and the labels." {off}>
    <input type="color" disabled={off} title="Behind it" value={hexOf(o.background)} onchange={(e) => set({ background: withAlpha(e.currentTarget.value, alphaOf(o.background)) })} />
    <Slider value={alphaOf(o.background)} min={0} max={1} step={0.05} width="90px" disabled={off} onchange={(a) => set({ background: withAlpha(hexOf(o.background), a) })} />
    <input type="color" disabled={off} title="Numbers" value={hexOf(o.text_color)} onchange={(e) => set({ text_color: e.currentTarget.value.toUpperCase() })} />
    <input type="color" disabled={off} title="Labels" value={hexOf(o.label_color)} onchange={(e) => set({ label_color: withAlpha(e.currentTarget.value, alphaOf(o.label_color)) })} />
  </SettingRow>
</section>

<style>
  .list {
    padding: 2px 16px;
  }
  .off-all {
    opacity: 0.55;
  }
  input[type="color"] {
    width: 30px;
    height: 26px;
    padding: 0;
    border: 1px solid rgb(var(--surface) / 0.12);
    border-radius: var(--r-sm);
    background: none;
    cursor: pointer;
  }
</style>
