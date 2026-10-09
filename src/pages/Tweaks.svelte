<!-- Bootstrapper features, built in, in tabs like Settings: how games
     start, Discord and online presence, performance (FastFlags Roblox allows
     and its own settings file, like Fishstrap), mods, a FastFlag editor
     with saved profiles, what happens in game, internet and Windows
     compatibility. Pious puts them on the builds it starts and undoes them
     exactly when they're turned off. -->
<script lang="ts">
  import { app, type TweaksTab } from "../lib/state.svelte";
  import InGameSettings from "../components/InGameSettings.svelte";
  import { call, run, setPreferences } from "../lib/api";
  import { parseFlags } from "../lib/flags";
  import InternetSettings from "../components/InternetSettings.svelte";
  import LaunchSettings from "../components/LaunchSettings.svelte";
  import ModMaker from "../components/ModMaker.svelte";
  import { copy } from "../lib/menus";
  import type { FlagProfile, GraphicsApi, Tweaks } from "../lib/types";
  import FontPreview from "../components/FontPreview.svelte";
  import Icon from "../components/Icon.svelte";
  import PictureChoices from "../components/PictureChoices.svelte";
  import Select from "../components/Select.svelte";
  import SettingRow from "../components/SettingRow.svelte";
  import Switch from "../components/Switch.svelte";

  const snap = $derived(app.snap!);
  const prefs = $derived(snap.bootstrapper.preferences);
  const tweaks = $derived(prefs.tweaks);
  const off = $derived(!tweaks.enabled);

  const TABS: [TweaksTab, string][] = [
    ["launching", "Launching"],
    ["presence", "Presence"],
    ["performance", "Performance"],
    ["mods", "Mods"],
    ["flags", "FastFlags"],
    ["ingame", "In game"],
    ["internet", "Internet"],
    ["system", "System"],
  ];

  function set(patch: Partial<Tweaks>) {
    setPreferences({ tweaks: patch });
  }

  // ── Shift lock cursor and Roblox icon ──────────────────────────────────
  type Choice = { id: string; name: string; picture: string | null };
  let crosshairs = $state<Choice[]>([]);
  let icons = $state<Choice[]>([]);
  let cursors = $state<Choice[]>([]);
  $effect(() => {
    call<[Tweaks["cursor"], string, string | null][]>("cursor_previews").then(
      (list) => (cursors = list.filter(([id]) => id !== "Default").map(([id, name, picture]) => ({ id, name, picture }))),
    );
  });
  $effect(() => {
    const color = tweaks.shiftlock_color;
    call<[string, string, string][]>("shiftlock_previews", { color }).then(
      (list) => (crosshairs = list.map(([id, name, picture]) => ({ id, name, picture }))),
    );
  });
  $effect(() => {
    icons = [];
    call<[string, string, string | null][]>("player_icon_previews").then((list) => (icons = list.map(([id, name, picture]) => ({ id, name, picture }))));
  });
  let skies = $state<Choice[]>([]);
  $effect(() => {
    call<[string, string, string][]>("skybox_previews").then((list) => (skies = list.map(([id, name, picture]) => ({ id, name, picture }))));
  });
  async function pickSky() {
    try {
      const folder = await call<string | null>("pick_skybox_folder");
      if (folder) set({ skybox: "custom", skybox_folder: folder });
    } catch (e) {
      app.toast("negative", e instanceof Error ? e.message : String(e));
    }
  }

  // Graphics cards Windows can pick for Roblox ("Specific GPU 1, 2…").
  let adapters = $state<string[]>([]);
  $effect(() => {
    call<string[]>("graphics_adapters").then((list) => (adapters = list));
  });
  const gpuChoices = $derived([
    { value: null as string | null, label: "Windows' setting" },
    { value: "default" as string | null, label: "Let Windows decide" },
    { value: "power_saving" as string | null, label: "Power saving" },
    { value: "high_performance" as string | null, label: "High performance" },
    ...adapters.map((name, i) => ({ value: `adapter:${i}` as string | null, label: `Specific GPU ${i + 1} · ${name}` })),
  ]);

  async function pickPicture(title: string, field: "shiftlock" | "player_icon") {
    const path = await call<string | null>("pick_picture", { title });
    if (path) set(field === "shiftlock" ? { shiftlock: "custom", shiftlock_file: path } : { player_icon: "custom", player_icon_file: path });
  }

  // ── Choices ────────────────────────────────────────────────────────────
  const fpsChoices = [
    { value: null as number | null, label: "Roblox's own" },
    ...[60, 120, 144, 165, 240, 360].map((n) => ({ value: n as number | null, label: `${n} FPS` })),
    { value: 9999 as number | null, label: "Unlimited" },
    { value: -1 as number | null, label: "Custom (type it)" },
  ];
  const graphicsChoices: { value: GraphicsApi; label: string }[] = [
    { value: "Automatic", label: "Automatic" },
    { value: "Direct3D11", label: "Direct3D 11" },
    { value: "Vulkan", label: "Vulkan" },
    { value: "OpenGL", label: "OpenGL" },
  ];
  // -1 is "off (×0)", kept in its own field: msaa 0 means Roblox decides.
  const msaaChoices = [0, -1, 1, 2, 4, 8].map((n) => ({ value: n, label: n === 0 ? "Automatic" : n === -1 ? "Off (×0)" : `${n}×` }));
  const renderQualityChoices = [
    { value: null as number | null, label: "Follow the slider" },
    ...Array.from({ length: 21 }, (_, i) => i + 1).map((n) => ({
      value: n as number | null,
      label: n === 1 ? "1 · lowest" : n === 21 ? "21 · highest" : String(n),
    })),
  ];
  const textureChoices = [
    { value: null as number | null, label: "Automatic" },
    ...["Lowest", "Low", "Medium", "Highest"].map((label, i) => ({ value: i as number | null, label })),
  ];
  const qualityChoices = [
    { value: null as number | null, label: "Roblox's own" },
    ...[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((n) => ({ value: n as number | null, label: `Level ${n}` })),
  ];
  const meshChoices = [
    { value: null as number | null, label: "Automatic" },
    ...["Lowest", "Low", "Medium", "High", "Highest"].map((label, i) => ({ value: i as number | null, label })),
  ];
  const emojiChoices: { value: Tweaks["emoji"]; label: string }[] = [
    { value: "Default", label: "Roblox's (Twemoji)" },
    { value: "Apple", label: "iOS (Apple) · experimental" },
    { value: "Windows11Fluent", label: "Windows 11 Fluent" },
    { value: "Windows11", label: "Windows 11 (22H2)" },
    { value: "Windows11Original", label: "Windows 11 (original)" },
    { value: "Windows10", label: "Windows 10" },
    { value: "Windows10Anniversary", label: "Windows 10 (2016)" },
    { value: "Windows8", label: "Windows 8" },
    { value: "TwemojiSvg", label: "Twitter (sharp SVG)" },
    { value: "EmojiOne", label: "EmojiOne" },
    { value: "Catmoji", label: "Catmoji" },
    { value: "Mona12", label: "Pixel (Mona 12)" },
    { value: "Docomo", label: "Classic phone (DoCoMo)" },
    { value: "NotoMono", label: "Noto, black and white" },
    { value: "OpenMojiMono", label: "OpenMoji, black and white" },
  ];
  const fontChoices = $derived([
    { value: null as string | null, label: "Roblox's own fonts" },
    ...snap.font_presets.map((p) => ({ value: p.id as string | null, label: `${p.group} · ${p.label}` })),
    ...(tweaks.font ? [{ value: "file" as string | null, label: `Your file · ${tweaks.font.split(/[\\/]/).pop()}` }] : []),
  ]);
  const fontValue = $derived(tweaks.font_preset ?? (tweaks.font ? "file" : null));
  function pickFont(value: string | null) {
    if (value === "file") set({ font_preset: null });
    else set({ font_preset: value, ...(value === null ? { font: null } : {}) });
  }
  const FPS_PRESETS = [60, 120, 144, 165, 240, 360];
  const cleanerChoices = [
    { value: null as number | null, label: "Off" },
    ...[1, 3, 7, 14, 30].map((d) => ({ value: d as number | null, label: d === 1 ? "Older than a day" : `Older than ${d} days` })),
  ];

  // ── FastFlag profiles ──────────────────────────────────────────────────
  const profiles = $derived(tweaks.flag_profiles);
  let selected = $state<string | null>(app.snap!.bootstrapper.preferences.tweaks.active_profile ?? app.snap!.bootstrapper.preferences.tweaks.flag_profiles[0]?.id ?? null);
  const profile = $derived(profiles.find((p) => p.id === selected) ?? profiles[0] ?? null);
  let filter = $state("");
  let newKey = $state("");
  let newValue = $state("");
  let importing = $state(false);
  let importText = $state("");
  let importError = $state<string | null>(null);
  /** Where imported flags go: the profile shown, or a new one. */
  let importInto = $state<"profile" | "new">("profile");
  let saved = $state<[string, Record<string, unknown>][] | null>(null);
  let renaming = $state(false);
  let rename = $state("");

  const rows = $derived(
    profile
      ? Object.entries(profile.flags)
          .filter(([key]) => key.toLowerCase().includes(filter.trim().toLowerCase()))
          .sort(([a], [b]) => a.localeCompare(b))
      : [],
  );

  function saveProfiles(next: FlagProfile[], active = tweaks.active_profile) {
    set({ flag_profiles: next, active_profile: active });
  }

  function update(id: string, change: (p: FlagProfile) => FlagProfile) {
    saveProfiles(profiles.map((p) => (p.id === id ? change(p) : p)));
  }

  function create(name: string, flags: Record<string, string> = {}) {
    const id = crypto.randomUUID();
    saveProfiles([...profiles, { id, name, flags }], tweaks.active_profile ?? id);
    selected = id;
  }

  function remove(id: string) {
    const next = profiles.filter((p) => p.id !== id);
    saveProfiles(next, tweaks.active_profile === id ? null : tweaks.active_profile);
    selected = next[0]?.id ?? null;
  }

  function setFlag(key: string, value: string) {
    if (!profile || !key.trim()) return;
    update(profile.id, (p) => ({ ...p, flags: { ...p.flags, [key.trim()]: value } }));
  }

  function deleteFlag(key: string) {
    if (!profile) return;
    update(profile.id, (p) => {
      const flags = { ...p.flags };
      delete flags[key];
      return { ...p, flags };
    });
  }

  function addFlag() {
    if (!newKey.trim()) return;
    if (!profile) create("My flags", { [newKey.trim()]: newValue });
    else setFlag(newKey, newValue);
    newKey = "";
    newValue = "";
  }

  /** Adds flags to the profile shown, or makes a new profile of them. */
  function addFlags(flags: Record<string, string>, name: string) {
    const count = Object.keys(flags).length;
    if (!count) {
      importError = "There were no flags in that.";
      return;
    }
    if (profile && importInto === "profile") update(profile.id, (p) => ({ ...p, flags: { ...p.flags, ...flags } }));
    else create(name, flags);
    importing = false;
    importText = "";
    importError = null;
    app.toast("positive", `Imported ${count} flag${count === 1 ? "" : "s"}`);
  }

  function doImport() {
    try {
      addFlags(parseFlags(importText), "Imported flags");
    } catch (e) {
      importError = (e as Error).message;
    }
  }

  async function importFile() {
    try {
      const text = await call<string | null>("read_json_file", { title: "Import FastFlags" });
      if (text != null) addFlags(parseFlags(text), "Imported flags");
    } catch (e) {
      importError = e instanceof Error ? e.message : String(e);
    }
  }

  async function openImport() {
    importing = !importing;
    importError = null;
    if (importing && saved === null) saved = (await run<[string, Record<string, unknown>][]>("bootstrapper_flags")) ?? [];
  }

  function fromBootstrapper(name: string, flags: Record<string, unknown>) {
    addFlags(Object.fromEntries(Object.entries(flags).map(([k, v]) => [k, typeof v === "string" ? v : JSON.stringify(v)])), `From ${name}`);
  }

  function exportFile() {
    if (!profile) return;
    run("save_json_file", { name: `${profile.name}.json`, text: JSON.stringify(profile.flags, null, 2) });
  }

  const PLACEHOLDER = '{\n  "FIntDebugForceMSAASamples": "4",\n  "DFIntTextureQualityOverride": "3"\n}';

  const preview = $derived(snap.fast_flags && Object.keys(snap.fast_flags).length ? JSON.stringify(snap.fast_flags, null, 2) : null);
</script>

<div class="page tweaks">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Tweaks</h1>
      <span class="meta">Everything bootstrappers do, built in: FPS unlock, graphics, FastFlags, mods, fonts and more</span>
    </div>
    <button
      class="btn tertiary"
      title="Every tweak back to Roblox's normal value, and everything they changed undone"
      onclick={() =>
        app.confirm(
          "Reset tweaks to default?",
          "Every tweak goes back to Roblox's normal value and everything tweaks changed is undone. Your FastFlag profiles are kept, but none is in use.",
          "Reset",
          () => run("reset_tweaks"),
        )}><Icon name="refresh" />Reset to default</button
    >
    <Switch on={tweaks.enabled} onchange={(on) => set({ enabled: on })} />
  </div>
  <div class="tabs" role="tablist">
    {#each TABS as [id, label] (id)}
      <button class="tab" class:on={app.tweaksTab === id} role="tab" aria-selected={app.tweaksTab === id} onclick={() => (app.tweaksTab = id)}>{label}</button>
    {/each}
  </div>

  {#key app.tweaksTab}
  <div class="tab-body">
  {#if app.tweaksTab === "launching"}
  <LaunchSettings part="launch" />
  {:else if app.tweaksTab === "presence"}
  <LaunchSettings part="presence" />
  {:else if app.tweaksTab === "ingame"}
  <InGameSettings />
  {:else if app.tweaksTab === "internet"}
  <InternetSettings />
  {:else}
  {#if prefs.launch_via}
    <div class="notice caution">
      <Icon name="info" />Games start through {prefs.launch_via} right now (Settings), so its own mods and FastFlags apply instead of these.
    </div>
  {/if}
  <div class="notice">
    <Icon name="shield" />Pious applies these to the Roblox build it starts and backs up every file it replaces. Turning tweaks off puts
    the originals back, including anything a bootstrapper put there.
  </div>
  <!-- Off: every tweak below is greyed out and can't be changed. -->
  <fieldset class="tweak-fields" disabled={off} aria-disabled={off}>

  {#if app.tweaksTab === "performance"}
  <section class="group">
    <span class="label">Performance</span>
    <div class="glass list">
      <SettingRow
        icon="lightning"
        title="Frame rate limit"
        description="Unlock Roblox's frame rate to match your monitor. Set in Roblox's own settings file when a game starts, like Fishstrap does (Roblox no longer accepts the old FastFlag)."
        {off}
      >
        <Select
          disabled={off}
          options={fpsChoices}
          value={tweaks.fps_limit === null || tweaks.fps_limit === 9999 || FPS_PRESETS.includes(tweaks.fps_limit) ? tweaks.fps_limit : -1}
          onchange={(v) => v !== -1 && set({ fps_limit: v })}
          width="230px"
        />
        <input
          class="input fps"
          type="number"
          min="1"
          max="9999"
          placeholder="Any"
          disabled={off}
          title="Type any frame rate"
          value={tweaks.fps_limit ?? ""}
          onchange={(e) => {
            const n = Math.round(Number(e.currentTarget.value));
            set({ fps_limit: e.currentTarget.value === "" || !n ? null : Math.min(9999, Math.max(1, n)) });
          }}
        />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="cube" title="Graphics API" description="What Roblox prefers to draw with. Try another if the game stutters or crashes." {off}>
        <Select disabled={off} options={graphicsChoices} value={tweaks.graphics} onchange={(v) => set({ graphics: v })} width="190px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="sparkles" title="Anti-aliasing" description="Smooths jagged edges; higher costs more frames. Off (×0) turns it off completely for the most frames." {off}>
        <Select
          disabled={off}
          options={msaaChoices}
          value={tweaks.msaa_off ? -1 : tweaks.msaa}
          onchange={(v) => set(v === -1 ? { msaa_off: true, msaa: 0 } : { msaa_off: false, msaa: v })}
          width="190px"
        />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="picture" title="Texture quality" description="Force a texture quality instead of Roblox choosing one." {off}>
        <Select disabled={off} options={textureChoices} value={tweaks.texture_quality} onchange={(v) => set({ texture_quality: v })} width="190px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="sun" title="Graphics quality" description="Roblox's own graphics slider, set before each game starts (levels 1–10)." {off}>
        <Select disabled={off} options={qualityChoices} value={tweaks.graphics_quality} onchange={(v) => set({ graphics_quality: v })} width="190px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow
        icon="sun"
        title="Render quality (21 levels)"
        description="Roblox's old hidden quality setting: 21 steps instead of the slider's 10, so it goes lower than the slider's lowest and higher than its highest. Overrides the slider while set."
        {off}
      >
        <Select disabled={off} options={renderQualityChoices} value={tweaks.render_quality} onchange={(v) => set({ render_quality: v })} width="190px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow
        icon="sun"
        title="Pause voxelizer"
        description="Stops Roblox from recomputing light and shadows as things move. Big frame gain, flatter and sometimes stale lighting."
        {off}
      >
        <Switch on={tweaks.pause_voxelizer} disabled={off} onchange={(on) => set({ pause_voxelizer: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="cpu-chip" title="Roblox priority" description="Give Roblox the CPU ahead of other programs. High can make the rest of your PC sluggish while you play." {off}>
        <Select
          disabled={off}
          options={[
            { value: null, label: "Normal" },
            { value: "above_normal", label: "Above normal" },
            { value: "high", label: "High" },
          ]}
          value={tweaks.priority}
          onchange={(v) => set({ priority: v })}
          width="190px"
        />
      </SettingRow>
      <hr class="divider" />
      <SettingRow
        icon="cpu-chip"
        title="Graphics card"
        description="Which GPU Windows runs Roblox on, like Settings → System → Display → Graphics. Takes effect the next time Roblox starts."
        {off}
      >
        <Select disabled={off} options={gpuChoices} value={tweaks.gpu} onchange={(v) => set({ gpu: v })} width="230px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="cloud" title="Gray sky" description="A plain gray sky instead of the game's." {off}>
        <Switch on={tweaks.gray_sky} disabled={off} onchange={(on) => set({ gray_sky: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="sparkles" title="Still grass" description="Grass that doesn't sway in the wind." {off}>
        <Switch on={tweaks.still_grass} disabled={off} onchange={(on) => set({ still_grass: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="sparkles" title="No grass" description="Don't draw grass at all. A real frame gain on games with lots of terrain." {off}>
        <Switch on={tweaks.no_grass} disabled={off} onchange={(on) => set({ no_grass: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="grid" title="Mesh detail" description="How far away objects keep their full detail." {off}>
        <Select disabled={off} options={meshChoices} value={tweaks.mesh_detail} onchange={(v) => set({ mesh_detail: v })} width="190px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="contrast" title="Ignore display scaling" description="Render at your screen's real resolution on high-DPI displays." {off}>
        <Switch on={tweaks.disable_dpi_scaling} disabled={off} onchange={(on) => set({ disable_dpi_scaling: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="focus" title="Alt + Enter fullscreen" description="Switch to exclusive fullscreen with Alt + Enter." {off}>
        <Switch on={tweaks.alt_enter_fullscreen} disabled={off} onchange={(on) => set({ alt_enter_fullscreen: on })} />
      </SettingRow>
    </div>
  </section>
  {/if}

  {#if app.tweaksTab === "system"}
  <section class="group">
    <span class="label">Compatibility</span>
    <div class="glass list">
      <SettingRow
        icon="window"
        title="Disable fullscreen optimizations"
        description="Windows' own setting for Roblox. Can cut input lag and fix stutter in fullscreen on some PCs."
        {off}
      >
        <Switch on={tweaks.disable_fullscreen_optimizations} disabled={off} onchange={(on) => set({ disable_fullscreen_optimizations: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="contrast" title="High DPI scaling" description="Who sizes Roblox on high-DPI screens, like in Roblox's Properties → Compatibility." {off}>
        <Select
          disabled={off}
          options={[
            { value: null, label: "Windows decides" },
            { value: "application", label: "Roblox" },
            { value: "system", label: "Windows" },
            { value: "system_enhanced", label: "Windows (enhanced)" },
          ]}
          value={tweaks.dpi_override}
          onchange={(v) => set({ dpi_override: v })}
          width="190px"
        />
      </SettingRow>
    </div>
  </section>
  {/if}

  {#if app.tweaksTab === "mods"}
  <section class="group">
    <span class="label">Mods</span>
    <div class="glass list">
      <SettingRow
        icon="arrow-right"
        title="Mouse cursor"
        description="Roblox's pointer: a classic one, or a set from Voidstrap or Froststrap. Applies to every Roblox version right away (and to new ones as they install)."
        {off}
      />
      <PictureChoices
        items={cursors}
        value={tweaks.cursor === "Default" ? null : tweaks.cursor}
        disabled={off}
        dark
        onpick={(id) => set({ cursor: (id ?? "Default") as Tweaks["cursor"] })}
      />
      <hr class="divider" />
      <SettingRow
        icon="target"
        title="Shift lock cursor"
        description="The crosshair while Roblox's own shift lock is on: one in the style of another game, or any picture (a 64×64 PNG works best). Applies the next time a game starts. Games with their own shift lock (many battlegrounds games) draw their own crosshair and keep it."
        {off}
      >
        {#if tweaks.shiftlock && tweaks.shiftlock !== "custom"}
          <label class="row color-pick" title="Its color">
            <input
              type="color"
              value={tweaks.shiftlock_color ?? "#ffffff"}
              disabled={off}
              onchange={(e) => set({ shiftlock_color: e.currentTarget.value })}
            />
            {#if tweaks.shiftlock_color}<button class="link" onclick={() => set({ shiftlock_color: null })}>Its own color</button>{/if}
          </label>
        {/if}
      </SettingRow>
      <PictureChoices
        items={crosshairs}
        value={tweaks.shiftlock}
        custom={tweaks.shiftlock_file}
        disabled={off}
        dark
        onpick={(id) => set({ shiftlock: id })}
        oncustom={() => pickPicture("Pick a shift lock cursor", "shiftlock")}
      />
      <hr class="divider" />
      <SettingRow
        icon="game"
        title="Roblox icon"
        description="The icon on Roblox's window and taskbar button: Pious's, a bootstrapper's, Roblox's through the years, or any picture. Changes as soon as a game window opens."
        {off}
      />
      <PictureChoices
        items={icons}
        value={tweaks.player_icon}
        custom={tweaks.player_icon_file}
        disabled={off}
        onpick={(id) => set({ player_icon: id })}
        oncustom={() => pickPicture("Pick a Roblox icon", "player_icon")}
      />
      <hr class="divider" />
      <SettingRow
        icon="cloud"
        title="Sky"
        description="The sky in games that don't set their own. Your own: a folder with six pictures named back, front, left, right, up and down (or Roblox's bk, ft, lf, rt, up, dn; .tex files work too). Applies the next time a game starts."
        {off}
      />
      <PictureChoices items={skies} value={tweaks.skybox} custom={tweaks.skybox_folder} disabled={off} onpick={(id) => set({ skybox: id })} oncustom={pickSky} />
      <hr class="divider" />
      <ModMaker {off} />
      <hr class="divider" />
      <SettingRow
        icon="sparkles"
        title="Emoji style"
        description={tweaks.emoji === "Apple"
          ? "Apple's emoji, turned into a font Roblox can draw, on this PC (the first launch takes a moment). Experimental: if emoji disappear in game, pick another style."
          : "Change how emoji look in chat and text."}
        {off}
      >
        <Select disabled={off} options={emojiChoices} value={tweaks.emoji} onchange={(v) => set({ emoji: v })} width="230px" />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="motion" title="Old character sounds" description="The classic walk, jump and get-up sounds." {off}>
        <Switch on={tweaks.old_character_sounds} disabled={off} onchange={(on) => set({ old_character_sounds: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow icon="picture" title="Old avatar editor background" description="The classic backdrop behind your avatar in the editor." {off}>
        <Switch on={tweaks.old_avatar_background} disabled={off} onchange={(on) => set({ old_avatar_background: on })} />
      </SettingRow>
      <hr class="divider" />
      <SettingRow
        icon="edit"
        title="Font"
        description="Replace Roblox's interface font: one of Roblox's own, a game font like Minecraft or Pokémon, or any .ttf or .otf file. Emoji stay as they are."
        {off}
      >
        <Select disabled={off} options={fontChoices} value={fontValue} onchange={pickFont} width="280px" />
        <button class="btn" disabled={off} title="Use a font file" onclick={() => run("pick_font")}><Icon name="folder" />File…</button>
      </SettingRow>
      <div class:dim={off}><FontPreview preset={tweaks.font_preset} file={tweaks.font_preset ? null : tweaks.font} /></div>
      <hr class="divider" />
      <SettingRow icon="window" title="Roblox window title" description="What Roblox's window is called (leave empty for Roblox)." {off}>
        <input
          class="input"
          style="width: 220px"
          placeholder="Roblox"
          disabled={off}
          value={tweaks.window_title}
          onchange={(e) => set({ window_title: e.currentTarget.value })}
        />
      </SettingRow>
      <hr class="divider" />
      <SettingRow
        icon="folder"
        title="Mods folder"
        description="Copy everything in Pious's Modifications folder over Roblox, in the same folder layout (e.g. content/sounds/ouch.ogg for the old death sound, content/textures for icons). It wins over the mods above."
        {off}
      >
        <button class="btn" onclick={() => run("open_mods_folder")}><Icon name="external" />Open</button>
        <Switch on={tweaks.use_mods_folder} disabled={off} onchange={(on) => set({ use_mods_folder: on })} />
      </SettingRow>
    </div>
  </section>
  {/if}

  {#if app.tweaksTab === "flags"}
  {#if snap.denied_flags.length}
    <div class="notice caution denied">
      <Icon name="warning" />
      <div class="col" style="gap: 4px">
        <span>
          Roblox refused {snap.denied_flags.length === 1 ? "this flag" : `these ${snap.denied_flags.length} flags`} at the last launch, because they're
          not on its allowlist. Pious still writes them, but Roblox ignores them:
        </span>
        <span class="denied-list">{snap.denied_flags.join(", ")}</span>
      </div>
    </div>
  {/if}
  <section class="group">
    <span class="label">FastFlag editor</span>
    <div class="glass editor" class:off>
      <div class="row profiles">
        <Select
          disabled={off}
          options={profiles.map((p) => ({ value: p.id as string | null, label: `${p.name}${p.id === tweaks.active_profile ? " (in use)" : ""}` }))}
          value={profile?.id ?? null}
          onchange={(id) => (selected = id)}
          placeholder="No profiles yet"
          width="240px"
        />
        {#if profile}
          {#if profile.id === tweaks.active_profile}
            <button class="btn small" onclick={() => saveProfiles(profiles, null)}><Icon name="close" />Stop using</button>
          {:else}
            <button class="btn primary small" onclick={() => saveProfiles(profiles, profile.id)}><Icon name="check" />Use this profile</button>
          {/if}
        {/if}
        <span class="spacer"></span>
        <button class="icon-btn" title="New profile" aria-label="New profile" onclick={() => create(`Profile ${profiles.length + 1}`)}><Icon name="add" /></button>
        {#if profile}
          <button class="icon-btn" title="Rename" aria-label="Rename" onclick={() => ((renaming = true), (rename = profile.name))}><Icon name="edit" /></button>
          <button class="icon-btn" title="Duplicate" aria-label="Duplicate" onclick={() => create(`${profile.name} copy`, { ...profile.flags })}><Icon name="copy" /></button>
          <button class="icon-btn" title="Copy as JSON" aria-label="Copy as JSON" onclick={() => copy(JSON.stringify(profile.flags, null, 2))}><Icon name="copy" /></button>
          <button class="icon-btn" title="Save as a JSON file" aria-label="Save as a JSON file" onclick={exportFile}><Icon name="arrow-down-on-square" /></button>
        {/if}
        <button class="btn small" class:primary={importing} onclick={openImport}><Icon name="download" />Import</button>
        {#if profile}
          <button
            class="icon-btn"
            title="Delete profile"
            aria-label="Delete profile"
            onclick={() => app.confirm("Delete profile?", `${profile.name} and its ${Object.keys(profile.flags).length} flags will be deleted.`, "Delete", () => remove(profile.id))}
          ><Icon name="remove" /></button>
        {/if}
      </div>

      {#if renaming && profile}
        <div class="row">
          <!-- svelte-ignore a11y_autofocus -->
          <input class="input grow" autofocus bind:value={rename} onkeydown={(e) => e.key === "Enter" && (update(profile.id, (p) => ({ ...p, name: rename.trim() || p.name })), (renaming = false))} />
          <button class="btn primary small" onclick={() => (update(profile.id, (p) => ({ ...p, name: rename.trim() || p.name })), (renaming = false))}>Save</button>
        </div>
      {/if}

      {#if importing}
        <div class="import-panel">
          <div class="row" style="gap: 6px; flex-wrap: wrap">
            <span class="meta grow">Paste FastFlags JSON (or Name=Value lines), choose a file, or take them from a bootstrapper.</span>
            {#if profile}
              <button class="chip" class:on={importInto === "profile"} onclick={() => (importInto = "profile")}>Into {profile.name}</button>
            {/if}
            <button class="chip" class:on={importInto === "new" || !profile} onclick={() => (importInto = "new")}>As a new profile</button>
          </div>
          <textarea class="input code import" spellcheck="false" placeholder={PLACEHOLDER} bind:value={importText}></textarea>
          {#if importError}<div class="notice negative"><Icon name="warning" />{importError}</div>{/if}
          <div class="row" style="gap: 6px; flex-wrap: wrap">
            <button class="btn small" onclick={importFile}><Icon name="folder" />From a file…</button>
            {#each saved ?? [] as [name, flags] (name)}
              <button class="btn small" title="The FastFlags {name} has saved" onclick={() => fromBootstrapper(name, flags)}>
                <Icon name="download" />From {name} · {Object.keys(flags).length}
              </button>
            {/each}
            <span class="spacer"></span>
            <button class="btn tertiary small" onclick={() => (importing = false)}>Cancel</button>
            <button class="btn primary small" disabled={!importText.trim()} onclick={doImport}><Icon name="download" />Import what's pasted</button>
          </div>
        </div>
      {/if}

      <label class="search-box glass-base">
        <Icon name="search" />
        <input placeholder="Filter flags" bind:value={filter} />
        <span class="secondary">{profile ? Object.keys(profile.flags).length : 0} flags</span>
      </label>

      <div class="table">
        <div class="tr head"><span>Flag</span><span>Value</span><span></span></div>
        {#each rows as [key, value] (key)}
          <div class="tr">
            <span class="code line selectable" title={key}>{key}</span>
            <input class="input code cell" value={value} onchange={(e) => setFlag(key, e.currentTarget.value)} />
            <button class="icon-btn" aria-label="Delete flag" onclick={() => deleteFlag(key)}><Icon name="close" size={13} /></button>
          </div>
        {:else}
          <div class="meta empty">{profile ? "No flags match." : "Add a flag below to start your first profile."}</div>
        {/each}
        <div class="tr add">
          <input class="input code cell" placeholder="FlagName" bind:value={newKey} onkeydown={(e) => e.key === "Enter" && addFlag()} />
          <input class="input code cell" placeholder="Value" bind:value={newValue} onkeydown={(e) => e.key === "Enter" && addFlag()} />
          <button class="icon-btn" aria-label="Add flag" disabled={!newKey.trim()} onclick={addFlag}><Icon name="add" /></button>
        </div>
      </div>
      <span class="secondary">
        Every flag here is written to Roblox as is, whatever it is. The profile in use is applied after the presets above, so its flags
        win.
      </span>
      {#if preview}
        <details>
          <summary class="meta">What Roblox gets ({Object.keys(snap.fast_flags ?? {}).length} flags)</summary>
          <pre class="code preview selectable">{preview}</pre>
        </details>
      {/if}
    </div>
  </section>
  {/if}

  {#if app.tweaksTab === "system"}
  <section class="group">
    <span class="label">Maintenance</span>
    <div class="glass list">
      <SettingRow icon="remove" title="Clean Roblox's leftovers" description="Delete old Roblox logs and cache files once a day to save space." {off}>
        <Select disabled={off} options={cleanerChoices} value={tweaks.cleaner_days} onchange={(v) => set({ cleaner_days: v })} width="190px" />
        <button class="btn" onclick={() => run("clean_roblox")}><Icon name="refresh" />Clean now</button>
      </SettingRow>
    </div>
  </section>
  {/if}
  </fieldset>
  {/if}
  </div>
  {/key}
</div>

<style>
  .tweak-fields {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
    transition: opacity var(--med);
  }
  .tweak-fields:disabled {
    opacity: 0.45;
    filter: grayscale(0.6);
    pointer-events: none;
    user-select: none;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / calc(0.035 * var(--glass)));
    border: 1px solid rgb(var(--surface) / calc(0.06 * var(--glass)));
    position: sticky;
    top: 0;
    z-index: 3;
    backdrop-filter: blur(18px);
    overflow-x: auto;
  }
  .tab {
    flex: 1;
    height: 30px;
    padding: 0 12px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: rgb(var(--muted));
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--fast), color var(--fast);
  }
  .tab:hover {
    color: rgb(var(--text));
  }
  .tab.on {
    background: rgb(var(--surface) / calc(0.1 * var(--glass)));
    color: rgb(var(--text));
  }
  .tab-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .tab-body > :global(*) {
    animation: rise 280ms var(--ease) both;
  }
  .denied-list {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 11.5px;
    word-break: break-all;
  }
  .dim {
    opacity: 0.4;
    padding-bottom: 10px;
  }
  div:has(> :global(.preview)) {
    padding-bottom: 10px;
  }
  .fps {
    width: 84px;
  }
  .tweaks {
    max-width: 880px;
    margin: 0 auto;
    width: 100%;
    gap: 16px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list {
    padding: 2px 14px;
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    transition: opacity var(--med);
  }
  .editor.off {
    opacity: 0.55;
  }
  .profiles {
    gap: 6px;
    flex-wrap: wrap;
  }
  .code {
    font-family: "Cascadia Code", Consolas, monospace;
    font-size: 12px;
  }
  .import-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border-radius: var(--r-md);
    background: rgb(var(--surface) / 0.03);
    border: 1px solid rgb(var(--surface) / 0.06);
    animation: rise 220ms var(--ease) both;
  }
  .import {
    height: 110px;
    padding: 10px 12px;
    resize: vertical;
  }
  .table {
    display: flex;
    flex-direction: column;
    max-height: 360px;
    overflow-y: auto;
    border-radius: var(--r-md);
    border: 1px solid rgb(var(--surface) / 0.06);
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) 30px;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
  }
  .tr:nth-child(even):not(.head) {
    background: rgb(var(--surface) / 0.025);
  }
  .tr.head {
    position: sticky;
    top: 0;
    z-index: 3;
    padding: 6px 8px;
    background: rgb(var(--panel));
    font-size: 11px;
    font-weight: 600;
    color: rgb(var(--faint));
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .tr.add {
    border-top: 1px solid rgb(var(--surface) / 0.06);
    padding: 6px 8px;
  }
  .cell {
    height: 28px;
    padding: 0 8px;
  }
  .empty {
    padding: 14px 10px;
  }
  .preview {
    margin: 8px 0 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: rgb(0 0 0 / 0.25);
    color: rgb(var(--muted));
    max-height: 240px;
    overflow: auto;
  }
  summary {
    cursor: pointer;
  }
</style>
