<!-- Settings → Overlay: the keys, mouse, CPS, KPS and FPS drawn over the
     game, with when they show and everything about how they look. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { run, setPreferences } from "../lib/api";
  import { keyName, PRESETS } from "../lib/inputLayouts";
  import type { InputOverlay } from "../lib/types";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import Slider from "./Slider.svelte";
  import Switch from "./Switch.svelte";

  const o = $derived(app.snap!.bootstrapper.preferences.input_overlay);
  const off = $derived(!o.enabled);

  // Dragging sliders sends at most one change per frame.
  let pending: Partial<InputOverlay> = {};
  let frame = 0;
  function set(patch: Partial<InputOverlay>) {
    pending = { ...pending, ...patch };
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      const out = pending;
      pending = {};
      setPreferences({ input_overlay: out });
    });
  }

  // ── Colors with transparency (#RRGGBBAA) ─────────────────────────────
  const hexOf = (color: string) => (color.length >= 7 ? color.slice(0, 7) : "#000000");
  const alphaOf = (color: string) => (color.length === 9 ? parseInt(color.slice(7, 9), 16) / 255 : 1);
  const withAlpha = (hex: string, alpha: number) =>
    `${hex.toUpperCase()}${Math.round(Math.min(1, Math.max(0, alpha)) * 255)
      .toString(16)
      .padStart(2, "0")
      .toUpperCase()}`;

  type ColorKey = "key_color" | "pressed_color" | "text_color" | "pressed_text_color" | "border_color" | "background";
  const COLORS: [ColorKey, string][] = [
    ["key_color", "Keys"],
    ["pressed_color", "Pressed keys"],
    ["text_color", "Letters"],
    ["pressed_text_color", "Pressed letters"],
    ["border_color", "Borders"],
    ["background", "Behind the keys"],
  ];

  const THEMES: { name: string; colors: Partial<InputOverlay> }[] = [
    {
      name: "Outline",
      colors: { key_color: "#0000008C", pressed_color: "#FFFFFFF2", text_color: "#FFFFFFFF", pressed_text_color: "#000000FF", border_color: "#FFFFFFE6", background: "#00000000", border: 2 },
    },
    { name: "Pious", colors: { key_color: "#0A0A0BB3", pressed_color: "#F2F2F3F2", text_color: "#EDEDEF", pressed_text_color: "#0A0A0B", border_color: "#FFFFFF26", background: "#00000000" } },
    { name: "Glass", colors: { key_color: "#FFFFFF1F", pressed_color: "#FFFFFFB3", text_color: "#FFFFFFFF", pressed_text_color: "#111111FF", border_color: "#FFFFFF40", background: "#00000000" } },
    { name: "Neon", colors: { key_color: "#0B0B1AE6", pressed_color: "#3DF5FFFF", text_color: "#A6F9FFFF", pressed_text_color: "#001014FF", border_color: "#3DF5FF66", background: "#00000000" } },
    { name: "Sakura", colors: { key_color: "#2A1620D9", pressed_color: "#FF8FC7FF", text_color: "#FFD6EBFF", pressed_text_color: "#2A0B1AFF", border_color: "#FF8FC766", background: "#00000000" } },
    { name: "Classic", colors: { key_color: "#1E1E1EFF", pressed_color: "#E5484DFF", text_color: "#FFFFFFFF", pressed_text_color: "#FFFFFFFF", border_color: "#000000FF", background: "#00000080" } },
  ];

  // ── The custom layout, as text ─────────────────────────────────────────
  const TOKENS: Record<string, string> = {
    space: "Space",
    shift: "ShiftLeft",
    rshift: "ShiftRight",
    ctrl: "ControlLeft",
    alt: "AltLeft",
    tab: "Tab",
    caps: "CapsLock",
    esc: "Escape",
    enter: "Enter",
    backspace: "Backspace",
    lmb: "MouseLeft",
    rmb: "MouseRight",
    mmb: "MouseMiddle",
    m4: "MouseBack",
    m5: "MouseForward",
    up: "ArrowUp",
    down: "ArrowDown",
    left: "ArrowLeft",
    right: "ArrowRight",
  };
  function toCode(token: string): string {
    const t = token.trim();
    // A gap.
    if (t === "_") return "";
    if (TOKENS[t.toLowerCase()]) return TOKENS[t.toLowerCase()];
    if (/^[a-z]$/i.test(t)) return `Key${t.toUpperCase()}`;
    if (/^\d$/.test(t)) return `Digit${t}`;
    if (/^f\d{1,2}$/i.test(t)) return t.toUpperCase();
    return t;
  }
  const customText = $derived(o.rows.map((r) => r.map((code) => (code ? keyName(code) : "_")).join(" ")).join("\n"));
  function setCustom(text: string) {
    const rows = text
      .split("\n")
      .map((line) => line.split(/\s+/).filter(Boolean).map(toCode))
      .filter((r) => r.length);
    setPreferences({ input_overlay: { rows, layout: "custom" } });
  }

  const FONTS = ["Manrope", "Segoe UI", "Bahnschrift", "Consolas", "Arial Black", "Impact", "Comic Sans MS", "Trebuchet MS", "Verdana"];
</script>

{#snippet setting(title: string, description: string)}
  <div class="col grow" style="gap: 1px"><span class="item-title">{title}</span><span class="meta">{description}</span></div>
{/snippet}

<section class="glass list">
  <div class="item">
    {@render setting("Input overlay", "Shows the keys and mouse buttons you press over the game, like streamers use, so viewers can see what you do.")}
    <Switch on={o.enabled} onchange={(enabled) => setPreferences({ input_overlay: { enabled } })} />
  </div>
  <hr class="divider" />
  <div class="item" class:off>
    {@render setting("Size", `${Math.round(o.scale * 100)}%. Everything gets bigger or smaller together.`)}
    <div class="row chips">
      {#each [["S", 0.75], ["M", 1], ["L", 1.35], ["XL", 1.75]] as [label, scale] (label)}
        <button class="chip" class:on={Math.abs(o.scale - Number(scale)) < 0.01} disabled={off} onclick={() => setPreferences({ input_overlay: { scale: Number(scale) } })}>{label}</button>
      {/each}
    </div>
    <Slider value={o.scale} min={0.4} max={3} step={0.05} width="160px" disabled={off} onchange={(scale) => set({ scale })} />
  </div>
  <hr class="divider" />
  <div class="item" class:off>
    {@render setting("Where it is", "Move it over the game, anywhere on the screen. Its spot is kept as a share of the screen, so it stays put at any resolution.")}
    <button class="btn" disabled={off} onclick={() => run("input_overlay_edit", { on: true })}><Icon name="target" />Move it</button>
    <button class="btn tertiary" disabled={off} onclick={() => setPreferences({ input_overlay: { x: 0.02, y: 0.72 } })}>Reset</button>
  </div>
  <hr class="divider" />
  <div class="item" class:off>
    {@render setting(
      "When it shows",
      o.only_while_capturing
        ? "Only while Pious is recording or keeping clips, so it's in your videos but out of the way otherwise."
        : o.only_in_game
          ? "While a Roblox window is in front."
          : "All the time, over anything.",
    )}
    <Select
      options={[
        { value: "playing", label: "While playing" },
        { value: "capturing", label: "While recording or clipping" },
        { value: "always", label: "Always" },
      ]}
      value={o.only_while_capturing ? "capturing" : o.only_in_game ? "playing" : "always"}
      onchange={(when) => setPreferences({ input_overlay: { only_in_game: when !== "always", only_while_capturing: when === "capturing" } })}
      width="230px"
    />
  </div>
  <hr class="divider" />
  <div class="item" class:off>
    {@render setting(
      "Show in recordings and clips",
      "On: it's in Pious's recordings and clips, and in OBS or Discord screen sharing. Off: only you see it; every capture leaves it out.",
    )}
    <Switch on={o.show_in_recordings} disabled={off} onchange={(show_in_recordings) => setPreferences({ input_overlay: { show_in_recordings } })} />
  </div>
</section>

<span class="label">Layout</span>
<section class="glass list" class:off-all={off}>
  <div class="item">
    {@render setting("Keys", "Which keys it shows.")}
    <div class="row chips">
      {#each Object.entries(PRESETS) as [id, preset] (id)}
        <button class="chip" class:on={o.layout === id} onclick={() => setPreferences({ input_overlay: { layout: id } })}>{preset.label}</button>
      {/each}
      <button class="chip" class:on={o.layout === "custom"} onclick={() => setPreferences({ input_overlay: { layout: "custom" } })}>Your own</button>
    </div>
  </div>
  {#if o.layout === "custom"}
    <div class="item custom">
      {@render setting(
        "Your keys",
        "One row per line, keys separated by spaces: letters, numbers, F1–F12, Space, Shift, Ctrl, Alt, Tab, Caps, Esc, Enter, arrows (Up, Down, Left, Right), LMB, RMB, MMB, M4, M5. Use _ for a gap.",
      )}
      <textarea class="input code" rows="4" spellcheck="false" value={customText} onchange={(e) => setCustom(e.currentTarget.value)}></textarea>
    </div>
  {/if}
  <hr class="divider" />
  <div class="item">
    {@render setting("Mouse", "A mouse with its buttons, including the side buttons.")}
    <Switch on={o.show_mouse} onchange={(show_mouse) => setPreferences({ input_overlay: { show_mouse } })} />
  </div>
  <hr class="divider" />
  <div class="item" class:off={!o.show_mouse}>
    {@render setting("Scrolling", "The mouse wheel lights up when you scroll.")}
    <Switch on={o.show_scroll} disabled={!o.show_mouse} onchange={(show_scroll) => setPreferences({ input_overlay: { show_scroll } })} />
  </div>
  <hr class="divider" />
  <div class="item">
    {@render setting("Clicks and keys per second", "CPS and KPS counters.")}
    <Switch on={o.show_rates} onchange={(show_rates) => setPreferences({ input_overlay: { show_rates } })} />
  </div>
  <hr class="divider" />
  <div class="item">
    {@render setting(
      "Show FPS",
      "The game's frame rate: the frames you actually see, counted from the screen, so it tops out at your monitor's refresh rate. Pick \"No keys\" above to show only the counters.",
    )}
    <Switch on={o.show_fps} onchange={(show_fps) => setPreferences({ input_overlay: { show_fps } })} />
  </div>
</section>

<span class="label">Look</span>
<section class="glass list" class:off-all={off}>
  <div class="item">
    {@render setting("Theme", "A starting point; change any color below.")}
    <div class="row chips">
      {#each THEMES as theme (theme.name)}
        <button class="chip" onclick={() => setPreferences({ input_overlay: theme.colors })}>
          <span class="swatch" style="background: {theme.colors.pressed_color}"></span>{theme.name}
        </button>
      {/each}
    </div>
  </div>
  <hr class="divider" />
  <div class="item">
    {@render setting("Key shape", "Slanted keys lean like a streamer's overlay; flat ones stand straight.")}
    <div class="row chips">
      <button class="chip" class:on={o.style !== "flat"} onclick={() => setPreferences({ input_overlay: { style: "slanted" } })}>Slanted</button>
      <button class="chip" class:on={o.style === "flat"} onclick={() => setPreferences({ input_overlay: { style: "flat" } })}>Flat</button>
    </div>
  </div>
  <div class="item" class:off={o.style === "flat"}>
    {@render setting("Lean", `${Math.round(o.slant)}°`)}
    <Slider value={o.slant} min={0} max={20} step={1} scale={1} unit="°" disabled={o.style === "flat"} onchange={(v) => set({ slant: Math.round(v) })} />
  </div>
  <hr class="divider" />
  {#each COLORS as [key, label] (key)}
    <div class="item color">
      <span class="item-title grow">{label}</span>
      <input type="color" class="picker" value={hexOf(o[key])} oninput={(e) => set({ [key]: withAlpha(e.currentTarget.value, alphaOf(o[key])) })} />
      <span class="meta">See-through</span>
      <Slider value={1 - alphaOf(o[key])} min={0} max={1} width="150px" onchange={(v) => set({ [key]: withAlpha(hexOf(o[key]), 1 - v) })} />
    </div>
  {/each}
  <hr class="divider" />
  <div class="item">
    {@render setting("Key size", "One key's width and height.")}
    <Slider value={o.key_size} min={20} max={120} step={1} scale={1} unit="px" onchange={(v) => set({ key_size: Math.round(v) })} />
  </div>
  <div class="item">
    {@render setting("Space between keys", "")}
    <Slider value={o.gap} min={0} max={24} step={1} scale={1} unit="px" onchange={(v) => set({ gap: Math.round(v) })} />
  </div>
  <div class="item">
    {@render setting("Roundness", "0 for square keys.")}
    <Slider value={o.radius} min={0} max={60} step={1} scale={1} unit="px" onchange={(v) => set({ radius: Math.round(v) })} />
  </div>
  <div class="item">
    {@render setting("Border", "")}
    <Slider value={o.border} min={0} max={6} step={1} scale={1} unit="px" onchange={(v) => set({ border: Math.round(v) })} />
  </div>
  <div class="item">
    {@render setting("Opacity", "The whole overlay.")}
    <Slider value={o.opacity} min={0.1} max={1} onchange={(opacity) => set({ opacity })} />
  </div>
  <hr class="divider" />
  <div class="item">
    {@render setting("Font", "")}
    <Select
      options={[...FONTS, ...(FONTS.includes(o.font) ? [] : [o.font])].map((f) => ({ value: f, label: f }))}
      value={o.font}
      onchange={(font) => setPreferences({ input_overlay: { font } })}
      width="190px"
    />
    <input class="input" style="width: 160px" placeholder="Any installed font" onchange={(e) => e.currentTarget.value.trim() && setPreferences({ input_overlay: { font: e.currentTarget.value.trim() } })} />
  </div>
  <div class="item">
    {@render setting("Letter size", "")}
    <Slider value={o.font_size} min={8} max={40} step={1} scale={1} unit="px" onchange={(v) => set({ font_size: Math.round(v) })} />
  </div>
  <div class="item">
    {@render setting("Bold", "")}
    <Switch on={o.bold} onchange={(bold) => setPreferences({ input_overlay: { bold } })} />
  </div>
  <div class="item">
    {@render setting("Capital letters", "Shift instead of shift, and so on.")}
    <Switch on={o.uppercase} onchange={(uppercase) => setPreferences({ input_overlay: { uppercase } })} />
  </div>
  <hr class="divider" />
  <div class="item">
    {@render setting("When pressed", "How a key reacts as you press it.")}
    <div class="row chips">
      {#each [["none", "Just lights up"], ["pop", "Presses in"], ["glow", "Glows"], ["fade", "Fades in"]] as [id, label] (id)}
        <button class="chip" class:on={o.animation === id} onclick={() => setPreferences({ input_overlay: { animation: id } })}>{label}</button>
      {/each}
    </div>
  </div>
  <div class="item">
    {@render setting("Let-go fade", "How long a key takes to go back after you let go.")}
    <Slider value={o.release_ms} min={0} max={600} step={10} scale={1} unit="ms" onchange={(v) => set({ release_ms: Math.round(v) })} />
  </div>
</section>

<style>
  .list {
    padding: 2px 16px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    min-height: 50px;
    transition: opacity var(--med);
  }
  .item.off,
  .off-all {
    opacity: 0.45;
    pointer-events: none;
  }
  .chips {
    gap: 6px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .custom {
    align-items: flex-start;
  }
  .custom textarea {
    width: 300px;
    padding: 8px 10px;
    font-family: "Cascadia Code", Consolas, monospace;
    font-size: 12px;
    resize: vertical;
  }
  .color {
    min-height: 42px;
    padding: 6px 0;
  }
  .picker {
    width: 34px;
    height: 26px;
    padding: 0;
    border: 1px solid rgb(var(--surface) / 0.15);
    border-radius: 6px;
    background: none;
    cursor: pointer;
  }
  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 4px;
    border: 1px solid rgb(255 255 255 / 0.2);
  }
</style>
