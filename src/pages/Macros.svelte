<!-- Macros (recorded or built step by step, each with its own hotkey) and
     the auto-clicker. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app, MACROS_PLUGIN, macrosOn, sortOf, viewOf } from "../lib/state.svelte";
  import { run, setPreferences } from "../lib/api";
  import { keyLabel } from "../lib/keys";
  import { plural } from "../lib/format";
  import type { Autoclicker, InputTarget, Macro, Step } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import HotkeyInput from "../components/HotkeyInput.svelte";
  import AccountTargets from "../components/AccountTargets.svelte";
  import Icon from "../components/Icon.svelte";
  import KeyPicker from "../components/KeyPicker.svelte";
  import PointPicker from "../components/PointPicker.svelte";
  import Select from "../components/Select.svelte";
  import SettingRow from "../components/SettingRow.svelte";
  import Slider from "../components/Slider.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import StepList from "../components/StepList.svelte";
  import Switch from "../components/Switch.svelte";
  import Tip from "../components/Tip.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const prefs = $derived(snap.bootstrapper.preferences);
  const status = $derived(snap.automation);
  const view = $derived(viewOf("macros"));
  let tab = $state<"macros" | "clicker">("macros");

  // ── Macros ───────────────────────────────────────────────────────────
  const sort = $derived(sortOf("macros", "name"));
  const macros = $derived(
    prefs.macros.slice().sort((a, b) => (sort === "steps" ? b.steps.length - a.steps.length : sort === "added" ? 0 : a.name.localeCompare(b.name))),
  );
  let editing = $state<string | null>(null);
  // The macro being edited, saved a moment after each change.
  let draft = $state<Macro | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    // Opened (or the macro was replaced, e.g. by recording).
    const found = prefs.macros.find((m) => m.id === editing);
    if (!found) {
      draft = null;
      return;
    }
    if (!draft || draft.id !== found.id) draft = structuredClone($state.snapshot(found)) as Macro;
  });

  function save() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 350);
  }
  function flush() {
    clearTimeout(saveTimer);
    if (!draft) return;
    const copy = $state.snapshot(draft) as Macro;
    setPreferences({ macros: prefs.macros.map((m) => (m.id === copy.id ? copy : m)) });
  }
  function change(patch: Partial<Macro>) {
    if (!draft) return;
    Object.assign(draft, patch);
    save();
  }

  function create() {
    const macro: Macro = {
      id: crypto.randomUUID(),
      name: `Macro ${prefs.macros.length + 1}`,
      hotkey: "",
      repeat: "Once",
      times: 3,
      speed: 1,
      game_only: false,
      target: "Roblox",
      accounts: [],
      steps: [],
    };
    setPreferences({ macros: [...prefs.macros, macro] });
    editing = macro.id;
  }
  function duplicate(m: Macro) {
    const copy = { ...structuredClone($state.snapshot(m)), id: crypto.randomUUID(), name: `${m.name} copy`, hotkey: "" } as Macro;
    setPreferences({ macros: [...prefs.macros, copy] });
  }
  function remove(m: Macro) {
    app.confirm(`Delete ${m.name}?`, "Its steps and hotkey are gone for good.", "Delete", () => {
      if (editing === m.id) editing = null;
      setPreferences({ macros: prefs.macros.filter((x) => x.id !== m.id) });
    });
  }
  function toggle(m: Macro) {
    flush();
    run("run_macro", { id: m.id });
  }
  function menu(event: MouseEvent, m: Macro) {
    app.openMenu(event, [
      { icon: "play", label: status.running.includes(m.id) ? "Stop" : "Run", action: () => toggle(m) },
      { icon: "edit", label: "Edit", action: () => (editing = m.id) },
      { icon: "copy", label: "Duplicate", action: () => duplicate(m) },
      { icon: "link", label: "Use for the logo", action: () => setPreferences({ logo_action: `macro:${m.id}` }) },
      { icon: "download", label: "Save as AutoHotkey", action: () => (flush(), run("export_ahk", { id: m.id })) },
      "separator",
      { icon: "remove", label: "Delete", danger: true, action: () => remove(m) },
    ]);
  }
  const count = (steps: Step[]): number => steps.reduce((n, s) => n + 1 + (s.kind === "Loop" ? count(s.steps) : 0), 0);
  const repeatText = (m: Macro) =>
    m.repeat === "Once" ? "Runs once" : m.repeat === "Times" ? `Runs ${m.times} times` : m.repeat === "WhileHeld" ? "Runs while the key is held" : "Runs until stopped";

  // ── Auto-clicker ─────────────────────────────────────────────────────
  const clicker = $derived(prefs.autoclicker);
  const setClicker = (patch: Partial<Autoclicker>) => setPreferences({ autoclicker: patch });
  const TARGETS: { value: InputTarget; label: string }[] = [
    { value: "Roblox", label: "The Roblox window you used last" },
    { value: "AllRoblox", label: "Every Roblox window" },
    { value: "Accounts", label: "Windows of accounts I pick" },
    { value: "System", label: "Whatever's in front" },
  ];
  const targetText = (t: InputTarget) =>
    t === "System"
      ? "Like a real keyboard and mouse: your pointer moves, and it goes to whatever window is in front."
      : t === "Accounts"
        ? "Only the Roblox windows of the accounts you pick below, in the background, so each instance can run its own macro."
        : t === "AllRoblox"
          ? "Every Roblox window at once, in the background. Your own mouse and keyboard stay free."
          : "Straight to Roblox, even when it's behind other windows. Your own mouse and keyboard stay free, so you can keep using your PC. (Turning the camera with the mouse needs \"Whatever's in front\".)";

  async function importAhk() {
    try {
      const out = await invoke<{ id: string; skipped: number } | null>("import_ahk", { text: null });
      if (!out) return;
      editing = out.id;
      app.toast(
        out.skipped ? "caution" : "positive",
        out.skipped
          ? `Imported. ${plural(out.skipped, "line")} Pious can't run stayed in as notes.`
          : "Imported your AutoHotkey script as a macro.",
      );
    } catch (e) {
      app.toast("negative", String(e));
    }
  }

  function enablePlugin() {
    setPreferences({ plugins: [...new Set([...prefs.plugins, MACROS_PLUGIN])] });
  }

  const MODE_HELP: Record<Autoclicker["mode"], string> = {
    Toggle: "Press the hotkey to start clicking, and again to stop.",
    Hold: "Clicks only while you hold the hotkey down.",
    MouseHeld: "Press the hotkey to turn it on. Then it clicks rapidly whenever you hold your mouse button down, and stops when you let go.",
    OnClick: "Press the hotkey to turn it on. Then every click you make is followed by extra clicks, like a burst.",
  };
  const modeText = $derived(
    clicker.mode === "Hold"
      ? `Hold ${keyLabel(clicker.hotkey)} to click`
      : clicker.mode === "MouseHeld"
        ? `Press ${keyLabel(clicker.hotkey)}, then hold the mouse to click`
        : clicker.mode === "OnClick"
          ? `Press ${keyLabel(clicker.hotkey)}, then each click gets ${clicker.burst ?? 2} more`
          : `Press ${keyLabel(clicker.hotkey)} to click`,
  );

  const cps = $derived(Math.round((1000 / Math.max(1, clicker.interval_ms + clicker.random_ms / 2)) * 10) / 10);
</script>

<div class="page">
  {#if !macrosOn()}
  <EmptyState
    icon="macros"
    title="Macros are off"
    body="Macros and the auto-clicker are a plugin that comes with Pious. Turn it on to record and run macros and use the auto-clicker. Their keys and clicks can go to Roblox in the background, so you can keep using your PC."
  >
    <div class="row">
      <button class="btn" onclick={() => ((app.settingsTab = "plugins"), app.navigate({ name: "settings" }))}>Plugins</button>
      <button class="btn primary" onclick={enablePlugin}><Icon name="power" />Turn on Macros</button>
    </div>
  </EmptyState>
  {:else}
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Macros</h1>
      <span class="meta">Repeat what you do with one key, or let Pious click for you</span>
    </div>
    <button class="btn" class:recording={status.recording} onclick={() => run("toggle_macro_recording")}>
      {#if status.recording}<span class="rec-dot"></span>Stop recording{:else}<Icon name="record" />Record{/if}
      <span class="kbd">{keyLabel(prefs.macro_settings.record_hotkey)}</span>
    </button>
    <button class="btn" title="Turn an AutoHotkey script (.ahk) into a macro" onclick={importAhk}><Icon name="download" />Import AutoHotkey</button>
    {#if status.running.length || status.clicking}
      <button class="btn danger" onclick={() => run("stop_automation")}><Icon name="stop" />Stop all</button>
    {/if}
  </div>

  <div class="tabs row">
    <button class="chip" class:on={tab === "macros"} onclick={() => (tab = "macros")}><Icon name="macros" />Macros · {prefs.macros.length}</button>
    <button class="chip" class:on={tab === "clicker"} onclick={() => (tab = "clicker")}>
      <Icon name="autoclick" />Auto-clicker{#if status.clicking}<span class="live"></span>{/if}
    </button>
  </div>

  {#key tab}
    <div class="enter-soft col" style="gap: 18px">
      {#if tab === "macros"}
        <Tip id="macros">
          Press Record ({keyLabel(prefs.macro_settings.record_hotkey)}) and do something: your keys, clicks and timing become a macro. Or build one
          step by step. Give it a hotkey to run it anywhere, even in game. {keyLabel(prefs.macro_settings.stop_hotkey)} stops everything.
        </Tip>
        <div class="layout" class:editing={!!draft}>
          <div class="col" style="gap: 10px; min-width: 0">
            <div class="row" style="gap: 8px">
              <button class="btn primary" onclick={create}><Icon name="add" />New macro</button>
              <span class="spacer"></span>
              <SortSelect
                page="macros"
                value={sort}
                options={[
                  { value: "name", label: "Name" },
                  { value: "steps", label: "Most steps" },
                  { value: "added", label: "Added" },
                ]}
              />
              <ViewToggle page="macros" />
            </div>
            {#if !prefs.macros.length}
              <EmptyState icon="macros" title="No macros yet" body="Record one with {keyLabel(prefs.macro_settings.record_hotkey)}, or build one step by step.">
                <div class="row">
                  <button class="btn" onclick={() => run("toggle_macro_recording")}><Icon name="record" />Record</button>
                  <button class="btn primary" onclick={create}><Icon name="add" />New macro</button>
                </div>
              </EmptyState>
            {:else}
              <div class={view === "List" || draft ? "list-rows" : "grid"} style="--card: 240px">
                {#each macros as m (m.id)}
                  {@const running = status.running.includes(m.id)}
                  <div
                    class="macro glass-base"
                    class:card={view !== "List" && !draft}
                    class:on={editing === m.id}
                    class:running
                    role="button"
                    tabindex="0"
                    onclick={() => (editing = editing === m.id ? null : m.id)}
                    onkeydown={(e) => e.key === "Enter" && (editing = m.id)}
                    oncontextmenu={(e) => menu(e, m)}
                  >
                    <span class="badge-icon"><Icon name="macros" size={16} /></span>
                    <div class="col grow" style="gap: 1px">
                      <span class="item-title line">{m.name}</span>
                      <span class="secondary line">{plural(count(m.steps), "step")} · {repeatText(m)}</span>
                    </div>
                    {#if m.hotkey}<span class="kbd">{keyLabel(m.hotkey)}</span>{/if}
                    <button
                      class="btn small"
                      class:primary={!running}
                      class:danger={running}
                      onclick={(e) => {
                        e.stopPropagation();
                        toggle(m);
                      }}
                    >
                      <Icon name={running ? "stop" : "play"} />{running ? "Stop" : "Run"}
                    </button>
                    <button
                      class="icon-btn"
                      aria-label="More"
                      onclick={(e) => {
                        e.stopPropagation();
                        menu(e, m);
                      }}><Icon name="more" /></button
                    >
                  </div>
                {/each}
              </div>
            {/if}

            <section class="panel col settings">
              <span class="section-title">Recording</span>
              <SettingRow icon="record" title="Record hotkey" description="Starts and stops recording a new macro, anywhere.">
                <HotkeyInput value={prefs.macro_settings.record_hotkey} onchange={(record_hotkey) => setPreferences({ macro_settings: { record_hotkey } })} />
              </SettingRow>
              <SettingRow icon="stop" title="Stop hotkey" description="Stops every macro and the auto-clicker.">
                <HotkeyInput value={prefs.macro_settings.stop_hotkey} onchange={(stop_hotkey) => setPreferences({ macro_settings: { stop_hotkey } })} />
              </SettingRow>
              <SettingRow icon="clock" title="Keep the timing" description="Wait between actions as long as you did.">
                <Switch on={prefs.macro_settings.record_timing} onchange={(record_timing) => setPreferences({ macro_settings: { record_timing } })} />
              </SettingRow>
              <SettingRow icon="target" title="Record pointer movement" description="Every move, not just where clicks happen. Makes bigger macros.">
                <Switch on={prefs.macro_settings.record_moves} onchange={(record_moves) => setPreferences({ macro_settings: { record_moves } })} />
              </SettingRow>
            </section>
          </div>

          {#if draft}
            {#key draft.id}
              <aside class="editor panel">
                <div class="row">
                  <input class="input title" value={draft.name} onchange={(e) => change({ name: e.currentTarget.value.trim() || "Macro" })} aria-label="Name" />
                  <button class="icon-btn" aria-label="Close" onclick={() => (flush(), (editing = null))}><Icon name="close" /></button>
                </div>
                <div class="options">
                  <label class="field">
                    <span class="label">Hotkey</span>
                    <HotkeyInput value={draft.hotkey} clearable onchange={(hotkey) => change({ hotkey })} width="100%" />
                  </label>
                  <label class="field">
                    <span class="label">Repeat</span>
                    <Select
                      options={[
                        { value: "Once" as const, label: "Once" },
                        { value: "Times" as const, label: "A number of times" },
                        { value: "UntilStopped" as const, label: "Until stopped" },
                        { value: "WhileHeld" as const, label: "While the hotkey is held" },
                      ]}
                      value={draft.repeat}
                      onchange={(repeat) => change({ repeat })}
                    />
                  </label>
                  {#if draft.repeat === "Times"}
                    <label class="field">
                      <span class="label">Times</span>
                      <input class="input" type="number" min="1" value={draft.times} onchange={(e) => change({ times: Math.max(1, Number(e.currentTarget.value) || 1) })} />
                    </label>
                  {/if}
                  <label class="field">
                    <span class="label">Speed</span>
                    <Slider value={draft.speed} min={0.25} max={4} step={0.05} scale={1} unit="×" digits={2} width="100%" onchange={(speed) => change({ speed })} />
                  </label>
                </div>
                <div class="row">
                  <span class="meta grow">Hotkey only while playing Roblox</span>
                  <Switch on={draft.game_only} onchange={(game_only) => change({ game_only })} />
                </div>
                <label class="field">
                  <span class="label">Send keys and clicks to</span>
                  <Select options={TARGETS} value={draft.target ?? "Roblox"} onchange={(target) => change({ target })} />
                  <span class="secondary">{targetText(draft.target ?? "Roblox")}</span>
                </label>
                {#if draft.target === "Accounts"}
                  <AccountTargets value={draft.accounts ?? []} onchange={(accounts) => change({ accounts })} />
                {/if}
                <button class="btn small tertiary" style="align-self: flex-start" onclick={() => draft && (flush(), run("export_ahk", { id: draft.id }))}>
                  <Icon name="download" />Save as AutoHotkey
                </button>
                <hr class="divider" />
                <div class="row">
                  <span class="section-title grow">Steps</span>
                  <button class="btn small" class:primary={!status.running.includes(draft.id)} onclick={() => draft && toggle(draft)}>
                    <Icon name={status.running.includes(draft.id) ? "stop" : "play"} />{status.running.includes(draft.id) ? "Stop" : "Try it"}
                  </button>
                </div>
                <div class="step-scroll">
                  <StepList steps={draft.steps} onchange={(steps) => change({ steps })} />
                </div>
              </aside>
            {/key}
          {/if}
        </div>
      {:else}
        <Tip id="autoclicker">Turn it on, then press {keyLabel(clicker.hotkey)} in any app or game to start and stop clicking.</Tip>
        <div class="clicker">
          <section class="panel col">
            <div class="row hero">
              <div class="col grow" style="gap: 2px">
                <span class="section-title">Auto-clicker</span>
                <span class="meta">{clicker.enabled ? modeText : "Off: its hotkey does nothing"}</span>
              </div>
              <Switch on={clicker.enabled} onchange={(enabled) => setClicker({ enabled })} />
            </div>
            <button class="go" class:on={status.clicking} onclick={() => run("toggle_autoclicker")}>
              <span class="ring"></span>
              <Icon name={status.clicking ? "stop" : "autoclick"} size={20} />
              {status.clicking ? "Stop clicking" : "Start now"}
            </button>
            <span class="secondary center">About {cps} clicks a second</span>
          </section>

          <section class="panel col">
            <span class="section-title">What it does</span>
            <SettingRow icon="keyboard" title="Hotkey" description="Starts and stops it.">
              <HotkeyInput value={clicker.hotkey} onchange={(hotkey) => setClicker({ hotkey })} />
            </SettingRow>
            <SettingRow icon="motion" title="Mode" description={MODE_HELP[clicker.mode]}>
              <Select
                options={[
                  { value: "Toggle" as const, label: "Press to start and stop" },
                  { value: "Hold" as const, label: "Click while the hotkey is held" },
                  { value: "MouseHeld" as const, label: "Click while I hold the mouse" },
                  { value: "OnClick" as const, label: "Add clicks to each of mine" },
                ]}
                value={clicker.mode}
                onchange={(mode) => setClicker({ mode })}
                width="250px"
              />
            </SettingRow>
            {#if clicker.mode === "MouseHeld" || clicker.mode === "OnClick"}
              <SettingRow icon="autoclick" title="Your mouse button" description={clicker.mode === "MouseHeld" ? "The button you hold down." : "The button whose clicks get extra ones."}>
                <Select
                  options={[
                    { value: "Left" as const, label: "Left button" },
                    { value: "Right" as const, label: "Right button" },
                    { value: "Middle" as const, label: "Middle button" },
                    { value: "Back" as const, label: "Back side button" },
                    { value: "Forward" as const, label: "Forward side button" },
                  ]}
                  value={clicker.trigger ?? "Left"}
                  onchange={(trigger) => setClicker({ trigger })}
                  width="190px"
                />
              </SettingRow>
              {#if clicker.mode === "OnClick"}
                <SettingRow icon="copy" title="Extra clicks" description="How many more clicks each of yours gets.">
                  <label class="unit">
                    <input class="input num" type="number" min="1" max="50" value={clicker.burst ?? 2} onchange={(e) => setClicker({ burst: Math.min(50, Math.max(1, Number(e.currentTarget.value) || 1)) })} />
                    more
                  </label>
                </SettingRow>
              {/if}
            {/if}
            <SettingRow icon="autoclick" title="Click with" description="A mouse button, or press a key over and over.">
              <div class="row">
                <Select
                  options={[
                    { value: "Mouse" as const, label: "Mouse" },
                    { value: "Key" as const, label: "A key" },
                  ]}
                  value={clicker.input}
                  onchange={(input) => setClicker({ input })}
                  width="110px"
                />
                {#if clicker.input === "Mouse"}
                  <Select
                    options={[
                      { value: "Left" as const, label: "Left button" },
                      { value: "Right" as const, label: "Right button" },
                      { value: "Middle" as const, label: "Middle button" },
                    ]}
                    value={clicker.button}
                    onchange={(button) => setClicker({ button })}
                    width="150px"
                  />
                {:else}
                  <KeyPicker value={clicker.key} onchange={(key) => setClicker({ key })} />
                {/if}
              </div>
            </SettingRow>
            <SettingRow icon="copy" title="Double click" description="Two clicks each time.">
              <Switch on={clicker.double} onchange={(double) => setClicker({ double })} />
            </SettingRow>
          </section>

          <section class="panel col">
            <span class="section-title">Timing</span>
            <SettingRow icon="clock" title="Every" description="Time between clicks.">
              <label class="unit"><input class="input num" type="number" min="1" value={clicker.interval_ms} onchange={(e) => setClicker({ interval_ms: Math.max(1, Number(e.currentTarget.value) || 1) })} /> ms</label>
            </SettingRow>
            <SettingRow icon="sparkles" title="Random extra" description="Up to this much longer each time, so it looks less robotic.">
              <label class="unit"><input class="input num" type="number" min="0" value={clicker.random_ms} onchange={(e) => setClicker({ random_ms: Math.max(0, Number(e.currentTarget.value) || 0) })} /> ms</label>
            </SettingRow>
            <SettingRow icon="lock" title="Hold each click" description="How long the button stays down.">
              <label class="unit"><input class="input num" type="number" min="0" value={clicker.hold_ms} onchange={(e) => setClicker({ hold_ms: Math.max(0, Number(e.currentTarget.value) || 0) })} /> ms</label>
            </SettingRow>
            <SettingRow icon="stop" title="Stop after" description="Or keep going until you stop it.">
              <div class="row">
                <Select
                  options={[
                    { value: "Unlimited" as const, label: "Never" },
                    { value: "Clicks" as const, label: "A number of clicks" },
                    { value: "Seconds" as const, label: "A number of seconds" },
                  ]}
                  value={clicker.limit}
                  onchange={(limit) => setClicker({ limit })}
                  width="190px"
                />
                {#if clicker.limit !== "Unlimited"}
                  <input class="input num" type="number" min="1" value={clicker.limit_value} onchange={(e) => setClicker({ limit_value: Math.max(1, Number(e.currentTarget.value) || 1) })} />
                {/if}
              </div>
            </SettingRow>
          </section>

          <section class="panel col">
            <span class="section-title">Where</span>
            <SettingRow icon="game" title="Send clicks to" description={targetText(clicker.target ?? "Roblox")}>
              <Select options={TARGETS} value={clicker.target ?? "Roblox"} onchange={(target) => setClicker({ target })} width="240px" />
            </SettingRow>
            {#if clicker.target === "Accounts"}
              <AccountTargets value={clicker.accounts ?? []} onchange={(accounts) => setClicker({ accounts })} />
            {/if}
            <SettingRow icon="target" title="Click at" description={clicker.fixed ? `Always at ${clicker.fixed[0]}, ${clicker.fixed[1]}.` : "Wherever the pointer is."}>
              <div class="row">
                {#if clicker.fixed}<button class="btn small tertiary" onclick={() => setClicker({ fixed: null })}>Use the pointer</button>{/if}
                <PointPicker label={clicker.fixed ? "Pick again" : "Pick a spot"} onpick={(x, y) => setClicker({ fixed: [x, y] })} />
              </div>
            </SettingRow>
            <SettingRow icon="game" title="Only while playing" description="The hotkey works only while a Roblox window is in front.">
              <Switch on={clicker.game_only} onchange={(game_only) => setClicker({ game_only })} />
            </SettingRow>
          </section>
        </div>
      {/if}
    </div>
  {/key}
{/if}
</div>

<style>
  .tabs {
    gap: 6px;
  }
  .live {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #30a46c;
    animation: pulse 1.2s ease-in-out infinite;
  }
  .rec-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #e5484d;
    animation: pulse 1.2s ease-in-out infinite;
  }
  .btn.recording {
    border-color: rgb(229 72 77 / 0.5);
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .layout.editing {
    grid-template-columns: minmax(260px, 0.8fr) minmax(380px, 1.2fr);
  }
  .macro {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    cursor: pointer;
    transition: border-color var(--fast), background var(--fast), transform var(--fast);
  }
  .macro:hover {
    background: rgb(var(--surface) / 0.06);
  }
  .macro.on {
    border-color: rgb(var(--accent) / 0.45);
  }
  .macro.running {
    border-color: rgb(48 164 108 / 0.55);
  }
  .macro.card {
    flex-wrap: wrap;
    padding: 14px;
  }
  .badge-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
    border-radius: 9px;
    background: rgb(var(--surface) / 0.07);
    color: rgb(var(--muted));
  }
  .running .badge-icon {
    color: #30a46c;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .editor {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    max-height: calc(100vh - 150px);
    animation: slide-in 280ms var(--ease) both;
  }
  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateX(14px);
    }
  }
  .title {
    flex: 1;
    font-weight: 700;
    font-size: 15px;
  }
  .options {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 10px;
  }
  .step-scroll {
    overflow-y: auto;
    min-height: 120px;
    margin: 0 -6px;
    padding: 0 6px 4px;
  }
  .settings {
    gap: 0;
    padding: 14px 16px 6px;
  }
  .clicker {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 14px;
    align-items: start;
  }
  .clicker .panel {
    gap: 0;
    padding: 14px 16px 6px;
    animation: rise 300ms var(--ease) both;
  }
  .clicker .panel:nth-child(2) {
    animation-delay: 40ms;
  }
  .clicker .panel:nth-child(3) {
    animation-delay: 80ms;
  }
  .clicker .panel:nth-child(4) {
    animation-delay: 120ms;
  }
  .hero {
    padding-bottom: 12px;
  }
  .go {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 64px;
    border-radius: var(--r-lg);
    border: 1px solid rgb(var(--surface) / 0.12);
    background: rgb(var(--surface) / 0.06);
    color: rgb(var(--text));
    font: inherit;
    font-weight: 700;
    font-size: 15px;
    cursor: pointer;
    overflow: hidden;
    transition: background var(--fast), transform 160ms var(--ease), border-color var(--fast);
  }
  .go:hover {
    background: rgb(var(--surface) / 0.1);
  }
  .go:active {
    transform: scale(0.98);
  }
  .go.on {
    border-color: rgb(48 164 108 / 0.6);
    background: rgb(48 164 108 / 0.14);
  }
  .ring {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .go.on .ring::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 50%;
    width: 20px;
    height: 20px;
    margin: -10px;
    border-radius: 50%;
    border: 2px solid rgb(48 164 108 / 0.7);
    animation: ripple 1.1s ease-out infinite;
  }
  @keyframes ripple {
    from {
      transform: scale(0.6);
      opacity: 1;
    }
    to {
      transform: scale(9);
      opacity: 0;
    }
  }
  .center {
    text-align: center;
    padding: 8px 0 6px;
  }
  .unit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: rgb(var(--muted));
  }
  .num {
    width: 90px;
  }
</style>
