<!-- Tweaks → Mods → Mod maker: recolor Roblox's own interface (top bar,
     menus, chat, cursors, the logo…) with a color or a gradient, with a
     preview made from the Roblox version you have. Pious makes the mod from
     each version's original files when tweaks apply (core/modmaker.rs). -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { call, setPreferences } from "../lib/api";
  import type { UiMod } from "../lib/types";
  import Icon from "./Icon.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Slider from "./Slider.svelte";
  import Switch from "./Switch.svelte";

  let { off }: { off: boolean } = $props();

  const ui = $derived(app.snap!.bootstrapper.preferences.tweaks.ui_mod);
  const disabled = $derived(off || !ui.enabled);

  function set(patch: Partial<UiMod>) {
    setPreferences({ tweaks: { ui_mod: { ...ui, ...patch } } });
  }

  let parts = $state<[string, string][]>([]);
  $effect(() => {
    call<[string, string][]>("mod_maker_parts").then((list) => (parts = list));
  });

  function togglePart(id: string) {
    const on = ui.parts.includes(id);
    set({ parts: on ? ui.parts.filter((p) => p !== id) : [...ui.parts, id] });
  }

  const PALETTES: { name: string; colors: string[]; angle?: number }[] = [
    { name: "Pious", colors: ["#7C8CFF"] },
    { name: "Mint", colors: ["#3DDC97"] },
    { name: "Sakura", colors: ["#FF8FC7"] },
    { name: "Gold", colors: ["#FFC857"] },
    { name: "Crimson", colors: ["#E5484D"] },
    { name: "Sunset", colors: ["#FF8A3D", "#C850C0"], angle: 45 },
    { name: "Ocean", colors: ["#00C6FF", "#0072FF"], angle: 90 },
    { name: "Aurora", colors: ["#43E97B", "#38F9D7"], angle: 0 },
  ];

  // The preview follows the settings, a moment after they stop changing.
  let preview = $state<[string, string][]>([]);
  let previewError = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const wanted = $state.snapshot(ui);
    clearTimeout(timer);
    if (!wanted.enabled || !wanted.parts.length) {
      preview = [];
      return;
    }
    timer = setTimeout(() => {
      call<[string, string][]>("mod_maker_preview", { ui: wanted })
        .then((list) => {
          preview = list;
          previewError = null;
        })
        .catch((e) => (previewError = String(e)));
    }, 250);
  });
</script>

<SettingRow
  icon="sparkles"
  title="Mod maker"
  description="Recolor Roblox's own interface: the top bar, menus, chat, cursors, the logo and more, in one color or a gradient. Made from your Roblox version's own pictures, so it follows Roblox's updates; applies the next time a game starts. A cursor or shift lock you picked above, plugins and your mods folder win over it."
  {off}
>
  <Switch on={ui.enabled} disabled={off} onchange={(enabled) => set({ enabled })} />
</SettingRow>

<div class="maker" class:disabled>
  <div class="row wrap">
    <span class="secondary what">Recolor</span>
    {#each parts as [id, name] (id)}
      <button class="chip" class:on={ui.parts.includes(id)} {disabled} onclick={() => togglePart(id)}>{name}</button>
    {/each}
  </div>

  <div class="row wrap">
    <span class="secondary what">Colors</span>
    {#each PALETTES as p (p.name)}
      <button
        class="swatch"
        {disabled}
        title={p.name}
        aria-label={p.name}
        style="background: {p.colors.length > 1 ? `linear-gradient(${(p.angle ?? 0) + 90}deg, ${p.colors.join(', ')})` : p.colors[0]}"
        onclick={() => set({ colors: [...p.colors], angle: p.angle ?? ui.angle })}
      ></button>
    {/each}
    <span class="spacer"></span>
    <label class="row pick" title="Color">
      <input type="color" {disabled} value={ui.colors[0] ?? "#7C8CFF"} onchange={(e) => set({ colors: [e.currentTarget.value.toUpperCase(), ...ui.colors.slice(1, 2)] })} />
    </label>
    {#if ui.colors.length > 1}
      <Icon name="arrow-right" size={12} />
      <label class="row pick" title="Gradient's second color">
        <input type="color" {disabled} value={ui.colors[1]} onchange={(e) => set({ colors: [ui.colors[0], e.currentTarget.value.toUpperCase()] })} />
      </label>
      <button class="btn small tertiary" {disabled} onclick={() => set({ colors: [ui.colors[0]] })}>One color</button>
    {:else}
      <button class="btn small tertiary" {disabled} onclick={() => set({ colors: [ui.colors[0] ?? "#7C8CFF", "#FFFFFF"] })}>Gradient</button>
    {/if}
  </div>

  {#if ui.colors.length > 1}
    <div class="row">
      <span class="secondary what">Angle</span>
      <Slider value={ui.angle} min={0} max={360} step={5} scale={1} unit="°" {disabled} onchange={(angle) => set({ angle })} />
    </div>
  {/if}

  <div class="row">
    <span class="secondary what">Shading</span>
    <Switch on={ui.keep_shading} {disabled} onchange={(keep_shading) => set({ keep_shading })} />
    <span class="secondary">{ui.keep_shading ? "Keeps each picture's light and shade" : "One flat color"}</span>
  </div>

  {#if ui.enabled && ui.parts.length}
    {#if previewError}
      <span class="secondary">{previewError}</span>
    {:else if preview.length}
      <div class="preview">
        {#each preview as [part, picture], i (i)}
          <span class="tile" title={part}><img src={picture} alt={part} /></span>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .maker {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 2px 0 12px;
    transition: opacity var(--med);
  }
  .maker.disabled {
    opacity: 0.45;
  }
  .wrap {
    flex-wrap: wrap;
    row-gap: 6px;
  }
  .what {
    width: 64px;
    flex: none;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 1px solid rgb(var(--surface) / 0.18);
    cursor: pointer;
    transition: transform var(--fast) var(--ease);
  }
  .swatch:hover:not(:disabled) {
    transform: scale(1.12);
  }
  .pick input {
    width: 30px;
    height: 24px;
    padding: 0;
    border: 1px solid rgb(var(--surface) / 0.12);
    border-radius: var(--r-sm);
    background: none;
    cursor: pointer;
  }
  .preview {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px;
    border-radius: var(--r-md);
    /* A game-like backdrop, so light icons show. */
    background: radial-gradient(circle at 30% 30%, #4a5867, #1d262f);
  }
  .tile {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-sm);
    background: rgb(0 0 0 / 0.25);
    animation: pop 220ms var(--ease) both;
  }
  .tile img {
    max-width: 34px;
    max-height: 34px;
    object-fit: contain;
  }
</style>
