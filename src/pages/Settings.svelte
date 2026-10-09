<script lang="ts">
  import { untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { app, macrosOn, type SettingsTab } from "../lib/state.svelte";
  import { call, run, setPreferences } from "../lib/api";
  import { accountLabel, bytes, who } from "../lib/format";
  import { keyLabel } from "../lib/keys";
  import { FIXED, keysFor, SHORTCUTS } from "../lib/shortcuts";
  import { parseChangelog, type Release } from "../lib/changelog";
  import type { StatsSummary } from "../lib/types";
  import ReleaseNotes from "../components/ReleaseNotes.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { NAV, PAGE_TITLES } from "../lib/state.svelte";
  import { fromHsl, parseHex, toHex, toHsl } from "../lib/theme";
  import type { Appearance, Blur, RecorderPrefs, Uuid } from "../lib/types";
  import InputOverlaySettings from "../components/InputOverlaySettings.svelte";
  import StatsOverlaySettings from "../components/StatsOverlaySettings.svelte";
  import HotkeyInput from "../components/HotkeyInput.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import Slider from "../components/Slider.svelte";
  import Switch from "../components/Switch.svelte";
  import logo from "../../assets/brand/logo.svg?raw";

  const snap = $derived(app.snap!);
  const prefs = $derived(snap.bootstrapper.preferences);
  const look = $derived(prefs.appearance);
  const rec = $derived(prefs.recorder);
  const status = $derived(snap.recorder);

  const TABS: [SettingsTab, string][] = [
    ["general", "General"],
    ["appearance", "Appearance"],
    ["overlay", "Overlay"],
    ["recording", "Recording"],
    ["keyboard", "Keyboard"],
    ["plugins", "Plugins"],
    ["about", "About"],
  ];

  // Macros, from the Macros plugin (for the logo choices).
  let macroList = $state<{ id: string; name: string }[]>([]);
  $effect(() => {
    if (app.settingsTab === "general" && macrosOn()) call<{ id: string; name: string }[]>("plugin_request", { feature: "macros", method: "list", args: {} }).then((l) => (macroList = l ?? [])).catch(() => {});
  });
  // ── General ────────────────────────────────────────────────────────────
  const logoChoices = $derived([
    { value: "home", label: "Go home" },
    { value: "sidebar", label: "Collapse the sidebar" },
    { value: "search", label: "Search" },
    { value: "play_last", label: "Play your last game" },
    { value: "chat", label: "Open chat" },
    { value: "streamer", label: "Toggle streamer mode" },
    { value: "none", label: "Just spin" },
    ...NAV.filter((p) => p !== "home").map((p) => ({ value: `page:${p}`, label: `Open ${PAGE_TITLES[p]}` })),
    // Macros come from the Macros plugin, when it's on.
    ...(macrosOn() ? [{ value: "page:macros", label: "Open Macros" }, ...macroList.map((m) => ({ value: `macro:${m.id}`, label: `Run macro: ${m.name}` }))] : []),
  ]);
  const streamerApps = $derived(prefs.streamer_apps.join(", "));

  // ── Keyboard ───────────────────────────────────────────────────────────
  const clash = (id: string) => {
    const keys = keysFor(id);
    return keys ? SHORTCUTS.find((s) => s.id !== id && keysFor(s.id) === keys) : undefined;
  };
  function setShortcut(id: string, keys: string) {
    const def = SHORTCUTS.find((s) => s.id === id)!;
    const next = { ...prefs.shortcuts };
    if (keys === def.keys) delete next[id];
    else next[id] = keys;
    run("set_shortcuts", { shortcuts: next });
  }

  // ── Plugins and MCP ────────────────────────────────────────────────────
  const PERMISSION_TEXT: Record<string, string> = {
    read: "see your games, accounts, friends and running games",
    launch: "start and join games",
    notify: "show notices",
    navigate: "open Pious pages",
    macros: "run your macros",
    links: "open web links",
    input: "press keys, click and read the screen in Roblox and other windows",
    hotkeys: "use keyboard shortcuts",
    run: "open programs, files and links",
    full: "do anything Pious can, including changing every setting",
  };
  function togglePlugin(id: string, on: boolean) {
    const set = new Set(prefs.plugins);
    if (on) set.add(id);
    else set.delete(id);
    setPreferences({ plugins: [...set] });
  }
  let mcpInfo = $state<{ url: string; token: string; exe: string } | null>(null);
  let mcpApps = $state<{ id: string; name: string; connected: boolean; path: string | null }[]>([]);
  let mcpManual = $state(false);
  async function loadMcpApps() {
    mcpApps = await invoke<typeof mcpApps>("mcp_apps").catch(() => []);
  }
  async function connectApp(id: string, on: boolean) {
    try {
      await invoke("mcp_connect", { app: id, on });
    } catch (e) {
      app.toast("negative", String(e));
    }
    loadMcpApps();
  }
  const chatgptUrl = $derived(mcpInfo ? `${mcpInfo.url}?token=${mcpInfo.token}` : "");
  $effect(() => {
    if (app.settingsTab === "plugins") {
      invoke<{ url: string; token: string; exe: string }>("mcp_info").then((i) => (mcpInfo = i));
      loadMcpApps();
    }
  });
  const claudeConfig = $derived(mcpInfo ? JSON.stringify({ mcpServers: { pious: { command: mcpInfo.exe, args: ["--mcp"] } } }, null, 2) : "");
  const httpConfig = $derived(
    mcpInfo ? JSON.stringify({ mcpServers: { pious: { type: "http", url: mcpInfo.url, headers: { Authorization: `Bearer ${mcpInfo.token}` } } } }, null, 2) : "",
  );
  async function copyText(text: string, what: string) {
    await navigator.clipboard.writeText(text);
    app.toast("positive", `${what} copied`);
  }

  // ── About ──────────────────────────────────────────────────────────────
  let releases = $state<Release[]>([]);
  let openRelease = $state(0);
  let stats = $state<StatsSummary | null>(null);
  $effect(() => {
    if (app.settingsTab !== "about") return;
    invoke<string>("changelog").then((t) => (releases = parseChangelog(t)));
    invoke<StatsSummary>("stats_summary").then((s) => (stats = s));
  });
  const hours = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m` : `${Math.floor(s / 60)}m`);

  // ── Colors ─────────────────────────────────────────────────────────────
  type Target = "accent" | "accent_2" | "background" | "surface" | "text";
  const targets: [Target, string][] = [
    ["accent", "Accent"],
    ["accent_2", "Second accent"],
    ["background", "Background"],
    ["surface", "Glass"],
    ["text", "Text"],
  ];
  let target = $state<Target>("accent");
  let hsl = $state<[number, number, number]>([0, 0, 0.95]);
  let hex = $state("");

  // Re-read the picker only when switching which color is edited, so
  // snapshots arriving mid-drag (or a grey's undefined hue) never reset it.
  $effect(() => {
    const key = target;
    untrack(() => {
      const value = app.snap!.bootstrapper.preferences.appearance[key];
      const rgb = parseHex(value);
      if (rgb) hsl = toHsl(rgb);
      hex = value;
    });
  });

  // Follow changes made elsewhere (like restoring the original look), but
  // not while the picker itself is being used.
  let lastEdit = 0;
  $effect(() => {
    const value = app.snap!.bootstrapper.preferences.appearance[target];
    if (Date.now() - lastEdit < 800) return;
    untrack(() => {
      if (value.toUpperCase() === hex.toUpperCase()) return;
      const rgb = parseHex(value);
      if (rgb) hsl = toHsl(rgb);
      hex = value;
    });
  });

  // Changes made while dragging go out at most once per frame.
  let pending: Record<string, unknown> = {};
  let frame = 0;
  function throttled(patch: Record<string, unknown>) {
    for (const [key, value] of Object.entries(patch)) {
      const before = pending[key];
      pending[key] =
        before && typeof before === "object" && value && typeof value === "object" ? { ...before, ...(value as object) } : value;
    }
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      const out = pending;
      pending = {};
      setPreferences(out);
    });
  }
  const appearance = (patch: Partial<Appearance>) => throttled({ appearance: patch });
  const recorder = (patch: Partial<RecorderPrefs>) => throttled({ recorder: patch });

  function setHsl(h: number, s: number, l: number) {
    lastEdit = Date.now();
    hsl = [h, s, l];
    hex = toHex(fromHsl(h, s, l));
    appearance({ [target]: hex });
  }

  function typedHex(value: string) {
    lastEdit = Date.now();
    hex = value;
    const rgb = parseHex(value);
    if (rgb) {
      hsl = toHsl(rgb);
      appearance({ [target]: toHex(rgb) });
    }
  }

  const defaults: Appearance = {
    accent: "#F2F2F3",
    background: "#0A0A0B",
    surface: "#FFFFFF",
    text: "#EDEDEF",
    background_image: null,
    image_dim: 0.6,
    image_blur: 0,
    see_through: true,
    window_opacity: 0.72,
    glass: 1,
    blur: "Frosted",
    blur_strength: 0.5,
    theme: null,
    gradient: { enabled: false, from: "#2B1F4A", to: "#0A0A0B", angle: 135, opacity: 0.6 },
    font: "",
    radius: 1,
    font_scale: 1,
    accent_2: "",
  };

  // ── Themes and fonts ───────────────────────────────────────────────────
  let systemFonts = $state<string[]>([]);
  $effect(() => {
    if (app.settingsTab === "appearance" && !systemFonts.length) call<string[]>("system_fonts").then((f) => (systemFonts = f)).catch(() => {});
  });
  const fontChoices = $derived([
    { value: "", label: "Manrope (Pious's own)" },
    ...systemFonts.map((f) => ({ value: f, label: f })),
    ...(look.font && !systemFonts.includes(look.font) ? [{ value: look.font, label: look.font }] : []),
  ]);
  const gradient = (patch: Partial<Appearance["gradient"]>) => appearance({ gradient: { ...look.gradient, ...patch } });

  // ── Regions and arranging ──────────────────────────────────────────────
  let regions = $state<[string, string][]>([]);
  $effect(() => {
    if (app.settingsTab === "general" && !regions.length) call<[string, string][]>("regions").then((r) => (regions = r)).catch(() => {});
  });
  const regionChoices = $derived([
    { value: "auto", label: "Automatic (Roblox picks)" },
    { value: "best_ping", label: "Best ping" },
    ...regions.map(([value, label]) => ({ value, label })),
  ]);

  // ── Crash reports ──────────────────────────────────────────────────────
  let crashText = $state<{ file: string; text: string } | null>(null);
  async function openCrash(file: string) {
    const text = await run<string>("read_crash", { file });
    if (text != null) crashText = { file, text };
  }
  function reportCrash(text: string) {
    const title = encodeURIComponent("Crash: " + (text.split(/\r?\n/).find((l) => l.startsWith("What:") || l.startsWith("Roblox crashed")) ?? "Pious crashed").slice(0, 120));
    const body = encodeURIComponent("What were you doing when it crashed?\n\n\n<details><summary>Crash report</summary>\n\n```\n" + text.slice(0, 5000) + "\n```\n</details>");
    run("open_external", { url: `https://github.com/woogi999/pious-bootstrapper/issues/new?title=${title}&body=${body}` });
  }
  const isDefault = $derived(JSON.stringify(look) === JSON.stringify(defaults));

  // ── Default account ────────────────────────────────────────────────────
  const accountChoices = $derived([
    { value: null as Uuid | null, label: "Most recently used" },
    ...snap.bootstrapper.accounts.map((a) => ({ value: a.id as Uuid | null, label: `${accountLabel(a)} (@${who(a.username)})` })),
  ]);
  // ── Recording ──────────────────────────────────────────────────────────
  const QUALITY: [string, number][] = [
    ["Smaller files", 28],
    ["Balanced", 23],
    ["High", 20],
    ["Highest", 16],
  ];
  const qualityPreset = $derived(rec.rate_control === "Quality" ? (QUALITY.find(([, q]) => q === rec.quality)?.[0] ?? "Custom") : "Custom");
  let advanced = $state(false);
  let mics = $state<[string, string][]>([]);
  $effect(() => {
    if (app.settingsTab === "recording" && rec.mic && !mics.length) {
      call<[string, string][]>("microphones").then((list) => (mics = list)).catch(() => {});
    }
  });

  const ENCODER_NAMES: Record<string, string> = {
    h264_nvenc: "NVIDIA · H.264",
    hevc_nvenc: "NVIDIA · HEVC",
    av1_nvenc: "NVIDIA · AV1",
    h264_amf: "AMD · H.264",
    hevc_amf: "AMD · HEVC",
    av1_amf: "AMD · AV1",
    h264_qsv: "Intel · H.264",
    hevc_qsv: "Intel · HEVC",
    av1_qsv: "Intel · AV1",
    libx264: "Processor · H.264 (x264)",
    libx265: "Processor · HEVC (x265)",
    libsvtav1: "Processor · AV1 (SVT)",
  };
  const encoderChoices = $derived([
    { value: "auto", label: "Automatic (graphics card first)" },
    ...(status.encoders ?? []).map((e) => ({ value: e, label: ENCODER_NAMES[e] ?? e })),
  ]);

  // ── Updates ────────────────────────────────────────────────────────────
  const update = $derived(snap.update);
  const updateText = $derived.by(() => {
    const v = snap.current_version;
    switch (update.kind) {
      case "up_to_date":
        return `Version ${v} — you're up to date.`;
      case "checking":
        return `Version ${v} — checking GitHub…`;
      case "available":
        return `Version ${update.release.version} is available (you have ${v}).`;
      case "downloading":
        return `Downloading version ${update.release.version}… ${update.total ? Math.round((update.received / update.total) * 100) : 0}%`;
      case "ready":
        return `Version ${update.release.version} is ready. Restart Pious to finish.`;
      case "failed":
        return `Version ${v} — ${update.error}`;
      default:
        return `Version ${v}`;
    }
  });
</script>

{#snippet setting(title: string, description: string)}
  <div class="col grow" style="gap: 1px"><span class="item-title">{title}</span><span class="meta">{description}</span></div>
{/snippet}

<div class="page settings">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Settings</h1>
      <span class="meta">Make Pious look and work the way you like</span>
    </div>
  </div>

  <div class="tabs" role="tablist">
    {#each TABS as [id, label] (id)}
      <button class="tab" class:on={app.settingsTab === id} role="tab" aria-selected={app.settingsTab === id} onclick={() => (app.settingsTab = id)}>{label}</button>
    {/each}
  </div>

  {#key app.settingsTab}
    <div class="panel-body stagger">
      {#if app.settingsTab === "general"}
        <section class="glass list">
          <div class="item">
            {@render setting("Close button", "What the X in the title bar does.")}
            <Select
              options={[
                { value: "Tray" as const, label: "Keep running in the tray" },
                { value: "Quit" as const, label: "Quit Pious" },
              ]}
              value={prefs.close_action}
              onchange={(close_action) => setPreferences({ close_action })}
              width="230px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("When a game starts", "What Pious does once Roblox opens.")}
            <Select
              options={[
                { value: "Stay" as const, label: "Stay open" },
                { value: "Minimize" as const, label: "Minimize" },
                { value: "HideUntilClosed" as const, label: "Hide, come back when Roblox closes" },
              ]}
              value={prefs.on_game_launch}
              onchange={(on_game_launch) => setPreferences({ on_game_launch })}
              width="280px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Start with Windows", "Open Pious when you sign in to Windows.")}
            <Switch on={prefs.start_with_windows} onchange={(on) => setPreferences({ start_with_windows: on })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.start_with_windows}>
            {@render setting("Start in the tray", "When it starts with Windows, wait in the tray instead of opening the window.")}
            <Switch on={prefs.start_hidden} disabled={!prefs.start_with_windows} onchange={(on) => setPreferences({ start_hidden: on })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Keep on top", "Pin Pious above other windows. Also the pin at the left of the title bar (Ctrl T).")}
            <Switch on={prefs.pinned} onchange={(on) => setPreferences({ pinned: on })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Default account", "Picked in the sidebar when Pious starts. Switching accounts in the sidebar doesn't change it.")}
            <Select options={accountChoices} value={prefs.default_account} onchange={(account) => run("set_default_account", { account })} width="260px" />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Roblox settings", "Volume, sensitivity, graphics and more for this PC. Accounts can keep their own (Accounts → … → Roblox settings).")}
            <button class="btn" onclick={() => (app.modal = { kind: "roblox_settings", account: null })}><Icon name="edit" />Edit</button>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting(
              "Keep Roblox up to date",
              snap.roblox_updating ? "Updating Roblox in the background…" : "Install new Roblox versions in the background as they come out, so games start straight away.",
            )}
            <Switch on={prefs.auto_update_roblox} onchange={(on) => setPreferences({ auto_update_roblox: on })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Server region", "Where public servers are joined when you don't pick one. Joining friends, links and private servers isn't affected.")}
            <Select options={regionChoices} value={prefs.region} onchange={(region) => setPreferences({ region })} width="240px" />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Auto arrange", "How Instances → Arrange lays out Roblox windows across your screens.")}
            <Select
              options={[
                { value: "grid", label: "Grid" },
                { value: "columns", label: "Side by side" },
                { value: "rows", label: "Stacked" },
                { value: "cascade", label: "Cascade" },
              ]}
              value={prefs.arrange_layout}
              onchange={(arrange_layout) => setPreferences({ arrange_layout })}
              width="180px"
            />
          </div>
        </section>
        <section class="glass list">
          <div class="item">
            {@render setting("Streamer mode", `Shows only the start of every Roblox name, so you can stream or record safely. ${keyLabel(keysFor("streamer"))} turns it on and off.`)}
            <Switch on={prefs.streamer_mode} onchange={(on) => setPreferences({ streamer_mode: on })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting(
              "Streamer mode by itself",
              "Turns streamer mode on while OBS, Streamlabs or other streaming software is open, and off again when it closes.",
            )}
            <Switch on={prefs.streamer_auto} onchange={(on) => setPreferences({ streamer_auto: on })} />
          </div>
          {#if prefs.streamer_auto}
            <div class="item">
              {@render setting("Streaming software", "The programs that count, by file name, separated by commas.")}
              <input
                class="input"
                style="width: 300px"
                value={streamerApps}
                onchange={(e) =>
                  setPreferences({
                    streamer_apps: e.currentTarget.value
                      .split(",")
                      .map((a) => a.trim())
                      .filter(Boolean),
                  })}
              />
            </div>
          {/if}
          <hr class="divider" />
          <div class="item">
            {@render setting("Open links in", "Profiles, roblox.com pages and other links.")}
            <Select
              options={[
                { value: "Pious" as const, label: "A Pious window" },
                { value: "Browser" as const, label: "Your web browser" },
              ]}
              value={prefs.link_target}
              onchange={(link_target) => setPreferences({ link_target })}
              width="220px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Clicking the logo", "The Pious logo at the top left spins, then does this.")}
            <Select options={logoChoices} value={prefs.logo_action} onchange={(logo_action) => setPreferences({ logo_action })} width="260px" />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("The welcome tour", "Shows how Pious works again, and brings back the tips on each page.")}
            <button class="btn" onclick={() => (setPreferences({ seen_tips: [] }), (app.modal = { kind: "welcome" }))}><Icon name="tip" />Show again</button>
          </div>
        </section>
        <span class="label">Notifications</span>
        <section class="glass list">
          <div class="item">
            {@render setting("Pop-ups from Roblox", "Pious's own notifications in the corner of your screen, for the account you play as. Click one to open it.")}
            <button class="btn small" disabled={!prefs.notifications.enabled} onclick={() => run("notify_test", { kind: null })}>Try one</button>
            <Switch on={prefs.notifications.enabled} onchange={(enabled) => setPreferences({ notifications: { enabled } })} />
          </div>
          {#each [["messages", "Messages", "A friend messaged you."], ["friend_requests", "Friend requests", "Someone wants to be friends."], ["friend_joins", "Friends playing", "A friend started a game. You can join them from the pop-up."], ["roblox", "Roblox notifications", "New ones in Roblox's bell."], ["in_game", "Over games", "Also while a Roblox window is in front."], ["sound", "Sound", "A short chime with each one."]] as const as [key, title, description] (key)}
            <hr class="divider" />
            <div class="item" class:off={!prefs.notifications.enabled}>
              {@render setting(title, description)}
              <Switch on={prefs.notifications[key]} disabled={!prefs.notifications.enabled} onchange={(on) => setPreferences({ notifications: { [key]: on } })} />
            </div>
          {/each}
          <hr class="divider" />
          <div class="item" class:off={!prefs.notifications.enabled}>
            {@render setting("How long they stay", "Not while the pointer is on one.")}
            <Select
              options={[4, 6, 8, 12, 20].map((n) => ({ value: n, label: `${n} seconds` }))}
              value={prefs.notifications.seconds}
              onchange={(seconds) => setPreferences({ notifications: { seconds } })}
              width="150px"
            />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.notifications.enabled}>
            {@render setting("Look", "Rich: a game's banner when a friend starts playing, a chat bubble for messages. Compact: one small card for everything.")}
            <Select
              options={[
                { value: "rich" as const, label: "Rich" },
                { value: "compact" as const, label: "Compact" },
              ]}
              value={prefs.notifications.style}
              onchange={(style) => setPreferences({ notifications: { style } })}
              width="150px"
            />
          </div>
        </section>
        <span class="label">Taskbar</span>
        <section class="glass list">
          {#each [["flash", "Flash for attention", "Flash Pious's taskbar button when something arrives while you're elsewhere."], ["badge", "Badge", "A count of pop-ups you haven't seen on the taskbar button, cleared when you open Pious."], ["progress", "Download progress", "Show Roblox downloads on the taskbar button."]] as const as [key, title, description], i (key)}
            {#if i}<hr class="divider" />{/if}
            <div class="item">
              {@render setting(title, description)}
              <Switch on={prefs.taskbar[key]} onchange={(on) => setPreferences({ taskbar: { [key]: on } })} />
            </div>
          {/each}
          <span class="meta" style="padding: 0 0 10px">Windows doesn't show badges with small taskbar buttons, and some taskbar replacements show none of these.</span>
        </section>
      {:else if app.settingsTab === "appearance"}
        <span class="label">Themes</span>
        <section class="glass list">
          <div class="item">
            {@render setting(
              "Themes",
              look.theme ? `Using ${snap.themes.find((t) => t.id === look.theme)?.name ?? look.theme}. Fine-tune it below.` : "A theme sets the whole look at once. Add more by dropping a theme folder in the themes folder.",
            )}
            <button class="btn" onclick={() => run("open_themes_folder")}><Icon name="folder" />Themes folder</button>
            <button class="btn tertiary" onclick={() => app.navigate({ name: "help", doc: "THEMES" })}><Icon name="book-open" />Make one</button>
          </div>
          <div class="themes">
            {#each snap.themes as t (t.id)}
              <button
                class="theme-card glass-base"
                class:on={look.theme === t.id}
                class:broken={!!t.problem}
                title={t.problem ?? t.description}
                onclick={() => (t.problem ? app.toast("caution", `${t.name} can't be used: ${t.problem}`) : run("apply_theme", { id: t.id }))}
              >
                <span class="theme-swatch" style="background: linear-gradient(135deg, {t.swatch[4] ?? t.swatch[1]}, {t.swatch[5] ?? t.swatch[1]})">
                  <span style="background: {t.swatch[0]}"></span><span style="background: {t.swatch[3]}"></span>
                </span>
                <span class="col" style="gap: 0; min-width: 0">
                  <span class="line" style="font-weight: 600">{t.name}</span>
                  <span class="line meta">{t.problem ? "Can't be used" : t.plugin ? `From a plugin` : t.author || "Theme"}</span>
                </span>
              </button>
            {:else}
              <span class="meta">No themes yet. Put a theme's folder in the themes folder.</span>
            {/each}
          </div>
        </section>
        <span class="label">Look</span>
        <section class="glass list">
          <div class="colors">
            {@render setting("Colors", "Pick any color for the accent, background, glass and text. Changes apply instantly.")}
            <div class="row" style="gap: 6px">
              {#each targets as [key, name] (key)}
                <button class="chip" class:on={target === key} onclick={() => (target = key)}>
                  <span class="swatch" style="background: {look[key] || look.accent}"></span>{name}
                </button>
              {/each}
            </div>
            <div class="row" style="gap: 22px; align-items: center">
              <div class="col preview">
                <span class="big-swatch" style="background: {hex}"></span>
                <input class="input" value={hex} oninput={(e) => typedHex(e.currentTarget.value)} placeholder="#RRGGBB" />
              </div>
              <div class="col grow" style="gap: 12px">
                <label class="track">
                  <span class="meta">Hue</span>
                  <input type="range" min="0" max="360" step="1" value={hsl[0]} class="rail hue" oninput={(e) => setHsl(Number(e.currentTarget.value), hsl[1], hsl[2])} />
                </label>
                <label class="track">
                  <span class="meta">Saturation</span>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.005"
                    value={hsl[1]}
                    class="rail"
                    style="background: linear-gradient(90deg, {toHex(fromHsl(hsl[0], 0, Math.min(0.85, Math.max(0.15, hsl[2]))))}, {toHex(fromHsl(hsl[0], 1, Math.min(0.85, Math.max(0.15, hsl[2]))))})"
                    oninput={(e) => setHsl(hsl[0], Number(e.currentTarget.value), hsl[2])}
                  />
                </label>
                <label class="track">
                  <span class="meta">Lightness</span>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.005"
                    value={hsl[2]}
                    class="rail"
                    style="background: linear-gradient(90deg, #000, {toHex(fromHsl(hsl[0], hsl[1], 0.5))}, #fff)"
                    oninput={(e) => setHsl(hsl[0], hsl[1], Number(e.currentTarget.value))}
                  />
                </label>
              </div>
            </div>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Gradient", "A gradient across the background, in two colors of your choice.")}
            {#if look.gradient.enabled}
              <label class="row color-pick" title="From"><input type="color" value={look.gradient.from} oninput={(e) => gradient({ from: e.currentTarget.value.toUpperCase() })} /></label>
              <label class="row color-pick" title="To"><input type="color" value={look.gradient.to} oninput={(e) => gradient({ to: e.currentTarget.value.toUpperCase() })} /></label>
            {/if}
            <Switch on={look.gradient.enabled} onchange={(enabled) => gradient({ enabled })} />
          </div>
          {#if look.gradient.enabled}
            <div class="item">
              {@render setting("Gradient angle", "Which way it runs.")}
              <Slider value={look.gradient.angle} min={0} max={360} step={5} onchange={(angle) => gradient({ angle })} />
            </div>
            <div class="item">
              {@render setting("Gradient strength", "How strongly it shows over the background.")}
              <Slider value={look.gradient.opacity} min={0} max={1} onchange={(opacity) => gradient({ opacity })} />
            </div>
          {/if}
          <hr class="divider" />
          <div class="item">
            {@render setting("Font", "Pious's own, or any font installed on your PC.")}
            <Select options={fontChoices} value={look.font} onchange={(font) => setPreferences({ appearance: { font } })} width="260px" />
          </div>
          <div class="font-sample" style="font-family: {look.font ? `'${look.font.replace(/'/g, '')}', ` : ''}Manrope, sans-serif">
            The quick brown fox jumps over the lazy dog. 0123456789
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Roundness", "How round corners are. All the way down is square.")}
            <Slider value={look.radius} min={0} max={2} onchange={(radius) => appearance({ radius })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Text size", "Larger or smaller text, without changing the rest (Interface size changes everything).")}
            <Slider value={look.font_scale} min={0.85} max={1.25} step={0.05} onchange={(font_scale) => appearance({ font_scale })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting(
              "Background picture",
              look.background_image ? (look.background_image.split(/[\\/]/).pop() ?? "Custom picture") : "Show a picture of your choice behind the interface.",
            )}
            {#if look.background_image}
              <img class="thumb" src={convertFileSrc(look.background_image)} alt="" />
              <button class="btn tertiary" onclick={() => setPreferences({ appearance: { background_image: null } })}><Icon name="close" />Remove</button>
            {/if}
            <button class="btn" onclick={() => run("pick_background")}><Icon name="picture" />{look.background_image ? "Change" : "Choose…"}</button>
          </div>
          <hr class="divider" />
          <div class="item" class:off={!look.background_image}>
            {@render setting("Picture blur", "Soften the background picture so it stays out of the way.")}
            <Slider value={look.image_blur} min={0} max={1} disabled={!look.background_image} onchange={(v) => appearance({ image_blur: v })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!look.background_image}>
            {@render setting("Picture dimming", "How much the interface tints the background picture.")}
            <Slider value={look.image_dim} min={0} max={1} disabled={!look.background_image} onchange={(v) => appearance({ image_dim: v })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Glass intensity", "How bright and defined glass panels look.")}
            <Slider value={look.glass} min={0.4} max={2} onchange={(v) => appearance({ glass: v })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("See-through window", "Let your desktop show through the background. Text and cards stay solid.")}
            <Switch on={look.see_through} onchange={(on) => setPreferences({ appearance: { see_through: on } })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!look.see_through}>
            {@render setting(
              "Background blur",
              "Frosted uses Windows' own live blur. Adjustable blurs what's behind the window at any strength (Pious stays out of screenshots while it's on). Diffused blends in a soft tint of your wallpaper.",
            )}
            <div class="row" style="gap: 6px">
              {#each ["Off", "Frosted", "Adjustable", "Diffused"] as Blur[] as blur (blur)}
                <button class="chip" class:on={look.blur === blur} disabled={!look.see_through} onclick={() => setPreferences({ appearance: { blur } })}>{blur}</button>
              {/each}
            </div>
          </div>
          <hr class="divider" />
          <div class="item" class:off={!look.see_through || (look.blur !== "Frosted" && look.blur !== "Adjustable")}>
            {@render setting(
              "Blur intensity",
              look.blur === "Adjustable" ? "How strongly what's behind the window is blurred." : "Lower values use Windows' lighter blur, higher ones its heavier frosted acrylic.",
            )}
            <Slider
              value={look.blur_strength}
              min={0}
              max={1}
              disabled={!look.see_through || (look.blur !== "Frosted" && look.blur !== "Adjustable")}
              onchange={(v) => appearance({ blur_strength: v })}
            />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!look.see_through}>
            {@render setting("Window opacity", "Lower values show more of what's behind the window.")}
            <Slider value={look.window_opacity} min={0.15} max={1} disabled={!look.see_through} onchange={(v) => appearance({ window_opacity: v })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Interface size", "Make everything in Pious bigger or smaller.")}
            <Slider value={prefs.ui_scale} min={0.8} max={1.3} step={0.05} oncommit={(v) => setPreferences({ ui_scale: Math.round(v * 100) / 100 })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Search blur", "How much the app blurs behind Ctrl K search and dialogs. All the way down turns it off.")}
            <Slider value={prefs.search_blur} min={0} max={1} onchange={(v) => throttled({ search_blur: v })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Reduce motion", "Switch pages, menus and dialogs instantly instead of animating them.")}
            <Switch on={prefs.reduce_motion} onchange={(on) => setPreferences({ reduce_motion: on })} />
          </div>
          <hr class="divider" />
          <div class="item" style="justify-content: flex-end">
            <button class="btn tertiary" disabled={isDefault} onclick={() => run("reset_appearance")}><Icon name="refresh" />Restore the original look</button>
          </div>
        </section>
      {:else if app.settingsTab === "overlay"}
        <span class="label">Pious over the game</span>
        <section class="glass list">
          <div class="item">
            {@render setting("Game overlay", `Press ${keyLabel(prefs.overlay.hotkey)} in a game to bring up Pious on top of it, like Steam's overlay.`)}
            <HotkeyInput value={prefs.overlay.hotkey} onchange={(hotkey) => setPreferences({ overlay: { hotkey } })} />
            <Switch on={prefs.overlay.enabled} onchange={(on) => setPreferences({ overlay: { enabled: on } })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.overlay.enabled}>
            {@render setting("Only while playing", `${keyLabel(prefs.overlay.hotkey)} opens the overlay only while a Roblox window is in front, and works normally everywhere else.`)}
            <Switch on={prefs.overlay.game_only} onchange={(on) => setPreferences({ overlay: { game_only: on } })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.overlay.enabled}>
            {@render setting(
              "Overlay blur",
              prefs.overlay.blur === "Live"
                ? "Windows' live blur: the game keeps moving behind it, at Windows' fixed strength."
                : prefs.overlay.blur === "Adjustable"
                  ? "A blurred picture of the game at the strength you choose."
                  : "No blur; the game is only dimmed.",
            )}
            <Select
              options={[
                { value: "Adjustable" as const, label: "Adjustable" },
                { value: "Live" as const, label: "Live" },
                { value: "Off" as const, label: "Off" },
              ]}
              value={prefs.overlay.blur}
              onchange={(blur) => setPreferences({ overlay: { blur } })}
              width="150px"
            />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.overlay.enabled || prefs.overlay.blur !== "Adjustable"}>
            {@render setting("Overlay blur strength", "How blurred the game is behind the overlay.")}
            <Slider value={prefs.overlay.blur_strength} min={0} max={1} disabled={prefs.overlay.blur !== "Adjustable"} onchange={(v) => throttled({ overlay: { blur_strength: v } })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.overlay.enabled}>
            {@render setting("Overlay dimming", "How dark the game gets behind the overlay.")}
            <Slider value={prefs.overlay.dim} min={0} max={0.9} onchange={(v) => throttled({ overlay: { dim: v } })} />
          </div>
          <hr class="divider" />
          <div class="item" class:off={!prefs.overlay.enabled}>
            {@render setting("Media controls", "A panel in the overlay for what's playing on your PC, like Spotify or a YouTube video: play, pause, skip and go back.")}
            <Switch on={prefs.overlay.media} onchange={(on) => setPreferences({ overlay: { media: on } })} />
          </div>
          {#each snap.plugins.filter((p) => p.enabled && p.overlay) as plugin (plugin.id)}
            {@const key = `plugin:${plugin.id}`}
            {@const widget = prefs.overlay.widgets[key] ?? { x: 0.36, y: 0.06, pinned: false, hidden: true }}
            <hr class="divider" />
            <div class="item" class:off={!prefs.overlay.enabled}>
              {@render setting(`${plugin.name} panel`, `A panel from the ${plugin.name} plugin in the overlay.`)}
              <Switch on={!widget.hidden} onchange={(on) => setPreferences({ overlay: { widgets: { [key]: { ...widget, hidden: !on } } } })} />
            </div>
          {/each}
        </section>
        <span class="label">Game stats on screen</span>
        <StatsOverlaySettings />
        <span class="label">Keys and mouse on screen</span>
        <InputOverlaySettings />
      {:else if app.settingsTab === "recording"}
        {#if !status.installed}
          <section class="glass setup">
            <div class="col grow" style="gap: 4px">
              <span class="item-title">Record and clip your games</span>
              <span class="meta">
                Pious records with FFmpeg, using your graphics card so games keep their frame rate. It's a free one-time download
                (about 115 MB).
              </span>
            </div>
            {#if status.installing}
              {@const [done, total] = status.installing}
              <div class="col" style="gap: 6px; width: 200px">
                <div class="progress"><div style="width: {total ? (done / total) * 100 : 5}%"></div></div>
                <span class="secondary">{bytes(done)}{total ? ` of ${bytes(total)}` : ""}</span>
              </div>
            {:else}
              <button class="btn primary" onclick={() => run("install_recorder")}><Icon name="download" />Set up recording</button>
            {/if}
          </section>
        {/if}

        <section class="glass list" class:off-all={!status.installed}>
          <div class="item">
            {@render setting(
              "Record",
              rec.record_hotkey === "F12"
                ? "F12 starts and stops a recording, in place of Roblox's own F12."
                : `${keyLabel(rec.record_hotkey)} starts and stops a recording. Use F12 to take over Roblox's own record key.`,
            )}
            <HotkeyInput value={rec.record_hotkey} onchange={(record_hotkey) => setPreferences({ recorder: { record_hotkey } })} />
            <Switch on={rec.recording} onchange={(on) => setPreferences({ recorder: { recording: on } })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting(
              "Clips",
              "While you play, Pious keeps the last few minutes so you can save what just happened, like ShadowPlay's instant replay.",
            )}
            <Switch on={rec.clips} onchange={(on) => setPreferences({ recorder: { clips: on } })} />
          </div>
          <div class="sub" class:off={!rec.clips}>
            <div class="item">
              {@render setting("Save a clip", `${keyLabel(rec.clip_hotkey)} saves the last ${rec.clip_seconds} seconds.`)}
              <HotkeyInput value={rec.clip_hotkey} onchange={(clip_hotkey) => setPreferences({ recorder: { clip_hotkey } })} />
              <Slider value={rec.clip_seconds} min={5} max={Math.min(rec.buffer_seconds, 300)} step={5} scale={1} unit="s" width="140px" onchange={(v) => recorder({ clip_seconds: Math.round(v) })} />
            </div>
            <div class="item">
              {@render setting(
                "Clip a length you choose",
                `${keyLabel(rec.manual_clip_hotkey)} asks how many seconds to save, counted back from the moment you pressed it.`,
              )}
              <HotkeyInput value={rec.manual_clip_hotkey} onchange={(manual_clip_hotkey) => setPreferences({ recorder: { manual_clip_hotkey } })} />
            </div>
            <div class="item">
              {@render setting("How far back", "The longest clip you can save. Longer uses a little more disk space while you play.")}
              <Slider value={rec.buffer_seconds} min={30} max={600} step={10} scale={1} unit="s" width="160px" onchange={(v) => recorder({ buffer_seconds: Math.round(v) })} />
            </div>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Hotkeys only while playing", "Recording keys work only while a Roblox window is in front, so F12 and friends stay normal everywhere else.")}
            <Switch on={rec.game_only} onchange={(on) => setPreferences({ recorder: { game_only: on } })} />
          </div>
        </section>

        <span class="label">Picture and sound</span>
        <section class="glass list" class:off-all={!status.installed}>
          <div class="item">
            {@render setting("Quality", "Higher looks sharper and makes bigger files.")}
            <div class="row" style="gap: 6px">
              {#each QUALITY as [name, q] (name)}
                <button class="chip" class:on={qualityPreset === name} onclick={() => setPreferences({ recorder: { rate_control: "Quality", quality: q } })}>{name}</button>
              {/each}
              {#if qualityPreset === "Custom"}<span class="chip on">Custom</span>{/if}
            </div>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Resolution", "Recording smaller than the game makes smaller files.")}
            <Select
              options={[
                { value: null as number | null, label: "Same as the game" },
                { value: 1440 as number | null, label: "1440p" },
                { value: 1080 as number | null, label: "1080p" },
                { value: 720 as number | null, label: "720p" },
                { value: 480 as number | null, label: "480p" },
              ]}
              value={rec.height}
              onchange={(height) => setPreferences({ recorder: { height } })}
              width="170px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Frame rate", "Frames per second in the video.")}
            <div class="row" style="gap: 6px">
              {#each [30, 60, 120, 144] as fps (fps)}
                <button class="chip" class:on={rec.fps === fps} onclick={() => setPreferences({ recorder: { fps } })}>{fps}</button>
              {/each}
              <input
                class="input small-number"
                type="number"
                min="10"
                max="240"
                value={rec.fps}
                title="Type any frame rate"
                onchange={(e) => setPreferences({ recorder: { fps: Math.min(240, Math.max(10, Math.round(Number(e.currentTarget.value) || 60))) } })}
              />
            </div>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Capture", "Just the game, or the whole screen it's on.")}
            <Select
              options={[
                { value: "GameWindow" as const, label: "The Roblox window" },
                { value: "GameMonitor" as const, label: "The whole screen" },
              ]}
              value={rec.target}
              onchange={(target) => setPreferences({ recorder: { target } })}
              width="190px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Game sound", "Only Roblox keeps Discord and music out of your videos.")}
            <Select
              options={[
                { value: "GameOnly" as const, label: "Only Roblox" },
                { value: "Everything" as const, label: "Everything on the PC" },
                { value: "Off" as const, label: "No sound" },
              ]}
              value={rec.game_audio}
              onchange={(game_audio) => setPreferences({ recorder: { game_audio } })}
              width="190px"
            />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Microphone", "Record your voice too.")}
            {#if rec.mic}
              <Select
                options={[{ value: null as string | null, label: "Windows default" }, ...mics.map(([id, name]) => ({ value: id as string | null, label: name }))]}
                value={rec.mic_device}
                onchange={(mic_device) => setPreferences({ recorder: { mic_device } })}
                width="220px"
              />
            {/if}
            <Switch on={rec.mic} onchange={(on) => setPreferences({ recorder: { mic: on } })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Save videos in", status.folder)}
            <button class="btn tertiary" onclick={() => run("open_path", { path: status.folder })}><Icon name="external" />Open</button>
            <button class="btn" onclick={() => run("pick_videos_folder")}><Icon name="folder" />Change…</button>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("File names", "Use {game}, {date}, {time} and {kind} (Clip or Recording).")}
            <input class="input" style="width: 230px" value={rec.file_name} onchange={(e) => setPreferences({ recorder: { file_name: e.currentTarget.value || "{game} {date} {time}" } })} />
            <Select
              options={[
                { value: "Mp4" as const, label: "MP4" },
                { value: "Mkv" as const, label: "MKV" },
                { value: "Mov" as const, label: "MOV" },
              ]}
              value={rec.container}
              onchange={(container) => setPreferences({ recorder: { container } })}
              width="90px"
            />
          </div>
        </section>

        <button class="disclosure" onclick={() => (advanced = !advanced)} aria-expanded={advanced}>
          <span class="chev" class:open={advanced}><Icon name="caret-right" size={12} /></span>Advanced (encoder, bitrate, audio)
        </button>
        {#if advanced}
          <section class="glass list" class:off-all={!status.installed}>
            <div class="item">
              {@render setting("Encoder", status.encoders ? "Graphics-card encoders barely touch your frame rate." : "Checking what this PC can use…")}
              <Select options={encoderChoices} value={rec.encoder} onchange={(encoder) => setPreferences({ recorder: { encoder } })} width="250px" />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Video format", "H.264 plays everywhere. HEVC and AV1 look better at the same size but need newer devices.")}
              <div class="row" style="gap: 6px">
                {#each [["H264", "H.264"], ["Hevc", "HEVC"], ["Av1", "AV1"]] as [codec, name] (codec)}
                  <button class="chip" class:on={rec.codec === codec} onclick={() => setPreferences({ recorder: { codec } })}>{name}</button>
                {/each}
              </div>
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting(
                "Rate control",
                rec.rate_control === "Quality"
                  ? "Constant quality: file size follows what's on screen (like OBS's CQP/CRF)."
                  : rec.rate_control === "Vbr"
                    ? "Variable bitrate around a target, never above the maximum."
                    : "Constant bitrate: predictable size, for strict upload limits.",
              )}
              <Select
                options={[
                  { value: "Quality" as const, label: "Constant quality" },
                  { value: "Vbr" as const, label: "Variable bitrate" },
                  { value: "Cbr" as const, label: "Constant bitrate" },
                ]}
                value={rec.rate_control}
                onchange={(rate_control) => setPreferences({ recorder: { rate_control } })}
                width="190px"
              />
            </div>
            {#if rec.rate_control === "Quality"}
              <div class="item">
                {@render setting("Quality level", "Lower numbers are sharper and bigger. 18–23 is visually close to lossless.")}
                <Slider value={rec.quality} min={0} max={51} step={1} scale={1} unit="" width="200px" onchange={(v) => recorder({ quality: Math.round(v) })} />
              </div>
            {:else}
              <div class="item">
                {@render setting("Bitrate", "Kilobits per second. 1080p60 looks great around 15,000–25,000.")}
                <Slider value={rec.bitrate_kbps} min={1000} max={150000} step={500} scale={1} unit=" kbps" width="160px" onchange={(v) => recorder({ bitrate_kbps: Math.round(v) })} />
              </div>
              {#if rec.rate_control === "Vbr"}
                <div class="item">
                  {@render setting("Maximum bitrate", "The most it can use in busy scenes.")}
                  <Slider value={rec.max_bitrate_kbps} min={1000} max={200000} step={500} scale={1} unit=" kbps" width="160px" onchange={(v) => recorder({ max_bitrate_kbps: Math.round(v) })} />
                </div>
              {/if}
            {/if}
            <hr class="divider" />
            <div class="item">
              {@render setting("Encoder speed", "Slower presets squeeze more quality out of each megabyte, at more graphics card work.")}
              <Select
                options={[
                  { value: "Fastest" as const, label: "Fastest" },
                  { value: "Fast" as const, label: "Fast" },
                  { value: "Balanced" as const, label: "Balanced" },
                  { value: "Quality" as const, label: "Quality" },
                  { value: "Best" as const, label: "Best quality" },
                ]}
                value={rec.speed}
                onchange={(speed) => setPreferences({ recorder: { speed } })}
                width="160px"
              />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Keyframe interval", "How often a full frame is stored. Shorter makes clips start closer to the exact moment.")}
              <Slider value={rec.keyframe_seconds} min={0.5} max={5} step={0.5} scale={1} unit="s" digits={1} width="140px" onchange={(v) => recorder({ keyframe_seconds: v })} />
            </div>
            <hr class="divider" />
            <div class="item" class:off={rec.codec === "H264"}>
              {@render setting("10-bit color", "Smoother gradients with HEVC or AV1.")}
              <Switch on={rec.ten_bit} disabled={rec.codec === "H264"} onchange={(on) => setPreferences({ recorder: { ten_bit: on } })} />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Show the mouse cursor", "Draw the cursor in videos.")}
              <Switch on={rec.cursor} onchange={(on) => setPreferences({ recorder: { cursor: on } })} />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Audio format", "AAC works everywhere; Opus sounds better at low bitrates.")}
              <Select
                options={[
                  { value: "Aac" as const, label: "AAC" },
                  { value: "Opus" as const, label: "Opus" },
                ]}
                value={rec.audio_codec}
                onchange={(audio_codec) => setPreferences({ recorder: { audio_codec } })}
                width="100px"
              />
              <Select
                options={[96, 128, 160, 192, 256, 320].map((k) => ({ value: k, label: `${k} kbps` }))}
                value={rec.audio_bitrate_kbps}
                onchange={(audio_bitrate_kbps) => setPreferences({ recorder: { audio_bitrate_kbps } })}
                width="120px"
              />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Game volume", "Louder or quieter than you hear it.")}
              <Slider value={rec.game_volume} min={0} max={2} onchange={(v) => recorder({ game_volume: v })} />
            </div>
            <hr class="divider" />
            <div class="item" class:off={!rec.mic}>
              {@render setting("Microphone volume", "Louder or quieter than it records.")}
              <Slider value={rec.mic_volume} min={0} max={3} disabled={!rec.mic} onchange={(v) => recorder({ mic_volume: v })} />
            </div>
            <hr class="divider" />
            <div class="item" class:off={!rec.mic}>
              {@render setting("Separate audio tracks", "Game and microphone on their own tracks, for editing.")}
              <Switch on={rec.separate_tracks} disabled={!rec.mic} onchange={(on) => setPreferences({ recorder: { separate_tracks: on } })} />
            </div>
            <hr class="divider" />
            <div class="item">
              {@render setting("Notices over the game", "A small note in the corner when a video is saved. Never shows up in videos.")}
              <Switch on={rec.notify} onchange={(on) => setPreferences({ recorder: { notify: on } })} />
            </div>
          </section>
        {/if}

        {#if status.installed}
          <div class="status meta">
            {#if status.recording_since !== null}
              <span class="rec-dot"></span>Recording
            {:else if status.capturing}
              <span class="live-dot"></span>Clip buffer running · {Math.round(status.buffered)}s kept
            {:else}
              Not capturing right now.
            {/if}
            {#if status.encoder}<span>· {ENCODER_NAMES[status.encoder] ?? status.encoder}</span>{/if}
            {#if status.error}<span class="err">· {status.error}</span>{/if}
            <span class="spacer"></span>
            {#if status.last_saved}
              <button class="link" onclick={() => run("open_path", { path: status.last_saved })}><Icon name="external" />Last video</button>
            {/if}
          </div>
        {/if}
      {:else if app.settingsTab === "keyboard"}
        <span class="label">While playing</span>
        <section class="glass list">
          <div class="item">
            {@render setting("Open the overlay", "Pious on top of the game.")}
            <HotkeyInput value={prefs.overlay.hotkey} onchange={(hotkey) => setPreferences({ overlay: { hotkey } })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Start or stop recording", rec.recording ? "On." : "Turn on recording in Recording to use it.")}
            <HotkeyInput value={rec.record_hotkey} onchange={(record_hotkey) => setPreferences({ recorder: { record_hotkey } })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Save a clip", rec.clips ? `The last ${rec.clip_seconds} seconds.` : "Turn on clips in Recording to use it.")}
            <HotkeyInput value={rec.clip_hotkey} onchange={(clip_hotkey) => setPreferences({ recorder: { clip_hotkey } })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Clip a length you choose", rec.clips ? "Asks how many seconds." : "Turn on clips in Recording to use it.")}
            <HotkeyInput value={rec.manual_clip_hotkey} onchange={(manual_clip_hotkey) => setPreferences({ recorder: { manual_clip_hotkey } })} />
          </div>
        </section>
        <span class="secondary">Macros, the auto-clicker and their hotkeys are in the Macros plugin's page (Settings → Plugins).</span>
        <div class="row">
          <span class="label grow">In Pious</span>
          {#if Object.keys(prefs.shortcuts).length}
            <button class="link" onclick={() => run("set_shortcuts", { shortcuts: {} })}><Icon name="history" />Reset all</button>
          {/if}
        </div>
        <section class="glass list">
          {#each SHORTCUTS as s, i (s.id)}
            {#if i > 0}<hr class="divider" />{/if}
            <div class="item short">
              <div class="col grow" style="gap: 1px">
                <span class="item-title">{s.label}</span>
                {#if clash(s.id)}<span class="meta warn">Also used for “{clash(s.id)!.label}”</span>{/if}
              </div>
              {#if s.id in prefs.shortcuts}
                <button class="icon-btn" title="Back to {s.keys ? keyLabel(s.keys) : 'off'}" aria-label="Reset" onclick={() => setShortcut(s.id, s.keys)}><Icon name="history" /></button>
              {/if}
              <HotkeyInput value={keysFor(s.id)} clearable onchange={(keys) => setShortcut(s.id, keys)} />
            </div>
          {/each}
          {#each FIXED as [keys, what] (keys)}
            <hr class="divider" />
            <div class="item short"><span class="item-title grow">{what}</span><span class="kbd fixed">{keys}</span></div>
          {/each}
        </section>
        <span class="secondary">Click a shortcut, then press the keys. Backspace turns it off.</span>
      {:else if app.settingsTab === "plugins"}
        <div class="row">
          <span class="label grow">Plugins</span>
          <button class="btn small" onclick={() => run("create_example_plugin")}><Icon name="add" />Example plugin</button>
          <button class="btn small" onclick={() => run("open_plugins_folder")}><Icon name="folder" />Plugins folder</button>
        </div>
        <div class="notice caution">
          <Icon name="shield" />Only add plugins you trust and have looked over. A plugin with broad permissions (input, run, full) can type, click,
          open programs and change Pious for you. Pious doesn't check third-party plugins and isn't responsible for what they do, including
          lost accounts or data.
        </div>
        {#if !snap.plugins.length}
          <div class="glass-base empty-plugins">
            <span class="tile-icon" style="width: 44px; height: 44px; border-radius: 50%"><Icon name="plugins" size={20} /></span>
            <div class="col" style="gap: 3px">
              <span class="item-title">No plugins yet</span>
              <span class="meta">A plugin is a folder with a plugin.json: a page, Roblox files (cursors, death sounds…), themes, icons or styles. Make the example to see how one works, then change it however you like.</span>
            </div>
          </div>
        {:else}
          <section class="glass list">
            {#each snap.plugins as p, i (p.id)}
              {#if i > 0}<hr class="divider" />{/if}
              <div class="item plugin">
                <span class="tile-icon" style="width: 36px; height: 36px"><Icon name={p.icon} size={17} /></span>
                <div class="col grow" style="gap: 2px">
                  <span class="item-title">{p.name} <span class="secondary">{p.author === "Pious" ? "Comes with Pious" : `${p.version}${p.author ? ` · ${p.author}` : ""}`}</span></span>
                  <span class="meta">{p.problem ?? (p.description || "No description.")}</span>
                  {#if p.brings.length && !p.problem}
                    <span class="row" style="gap: 4px; flex-wrap: wrap">{#each p.brings as b (b)}<span class="chip small">{b}</span>{/each}</span>
                  {/if}
                  {#if p.permissions.length}
                    <span class="secondary">Can {p.permissions.map((x) => PERMISSION_TEXT[x] ?? x).join(", ")}</span>
                  {/if}
                </div>
                {#if p.enabled && p.provides === "macros"}<button class="btn small" onclick={() => app.navigate({ name: "macros" })}>Open</button>{/if}
                <button class="icon-btn" title="Open its folder" aria-label="Open folder" onclick={() => run("open_path", { path: p.folder })}><Icon name="folder" /></button>
                <Switch on={p.enabled} disabled={!!p.problem} onchange={(on) => togglePlugin(p.id, on)} />
              </div>
            {/each}
          </section>
        {/if}
        <span class="secondary">
          Plugin pages run walled off from Pious and can only do what they list. Only turn on plugins you trust.
          <button class="link" onclick={() => app.navigate({ name: "help", doc: "PLUGINS" })}>How plugins work</button>
        </span>

        <span class="label">AI apps (MCP)</span>
        <section class="glass list">
          <div class="item">
            {@render setting(
              "Let AI apps use Pious",
              "Claude, ChatGPT, Codex, Cursor and other apps that speak the Model Context Protocol can see your games and friends, launch and record for you, and even play: look at a Roblox window, hear it, walk, type and click in it.",
            )}
            <Switch on={prefs.mcp.enabled} onchange={(enabled) => setPreferences({ mcp: { enabled } })} />
          </div>
          <div class="apps">
            {#each mcpApps as a (a.id)}
              <div class="app-card glass-base" class:on={a.connected}>
                <span class="col grow" style="gap: 1px; min-width: 0">
                  <span class="item-title">{a.name}</span>
                  <span class="secondary line" title={a.path ?? ""}>{a.connected ? "Connected" : "Not connected"}</span>
                </span>
                {#if a.connected}
                  <button class="btn small tertiary" onclick={() => connectApp(a.id, false)}>Remove</button>
                {:else}
                  <button class="btn small primary" onclick={() => connectApp(a.id, true)}><Icon name="link" />Connect</button>
                {/if}
              </div>
            {/each}
          </div>
          <span class="secondary pad-x">
            Connect adds Pious to that app's settings (a backup of the old file is kept next to it) and turns this on. Restart the app if it's open.
          </span>
          <div class="sub" class:off={!prefs.mcp.enabled}>
            <div class="item">
              {@render setting("Allow actions", "Off: AI apps can only look. On: they can also launch games, send messages, run macros and press keys, type and click in Roblox windows.")}
              <Switch on={prefs.mcp.allow_actions} disabled={!prefs.mcp.enabled} onchange={(allow_actions) => setPreferences({ mcp: { allow_actions } })} />
            </div>
            <div class="item">
              {@render setting("See and hear games", "AI apps can take pictures of Roblox windows and record a few seconds of a game's sound, if the AI can take them in.")}
              <Switch on={prefs.mcp.allow_senses ?? true} disabled={!prefs.mcp.enabled} onchange={(allow_senses) => setPreferences({ mcp: { allow_senses } })} />
            </div>
            <div class="item">
              {@render setting(
                "Start Pious when an AI app needs it",
                "Off: AI apps never start Pious; they say it isn't running until you open it. On: an AI app asking Pious to do something starts it in the tray. (Opening an AI app alone never starts Pious.)",
              )}
              <Switch on={prefs.mcp.start_on_demand ?? false} disabled={!prefs.mcp.enabled} onchange={(start_on_demand) => setPreferences({ mcp: { start_on_demand } })} />
            </div>
            <div class="item">
              {@render setting("Port", "Only this PC can connect.")}
              <input
                class="input"
                type="number"
                style="width: 110px"
                min="1024"
                max="65535"
                value={prefs.mcp.port}
                onchange={(e) => setPreferences({ mcp: { port: Math.min(65535, Math.max(1024, Number(e.currentTarget.value) || 47823)) } })}
              />
            </div>
            {#if snap.mcp.error}
              <div class="notice caution"><Icon name="warning" />{snap.mcp.error}</div>
            {:else if snap.mcp.url}
              <div class="notice"><span class="live"></span>Listening at {snap.mcp.url}</div>
            {/if}
            {#if mcpInfo}
              <div class="mcp-config">
                <div class="row">
                  <span class="item-title grow">ChatGPT</span>
                  <button class="btn small" onclick={() => copyText(chatgptUrl, "Connector address")}><Icon name="copy" />Copy address</button>
                </div>
                <span class="secondary">
                  ChatGPT only connects to servers on the internet. In ChatGPT's settings, turn on developer mode for connectors, then
                  add a connector with this address, made reachable through a tunnel like Cloudflare's. Anyone with the full address can use Pious, so
                  keep it private. The guide (docs/MCP.md) walks through it.
                </span>
                <button class="link" style="align-self: flex-start" onclick={() => (mcpManual = !mcpManual)}>
                  <Icon name={mcpManual ? "caret-down" : "caret-right"} size={12} />Set up another app by hand
                </button>
              </div>
            {/if}
            {#if mcpInfo && mcpManual}
              <div class="mcp-config">
                <div class="row">
                  <span class="item-title grow">For Claude Desktop and most apps</span>
                  <button class="btn small" onclick={() => copyText(claudeConfig, "Config")}><Icon name="copy" />Copy</button>
                </div>
                <pre class="selectable">{claudeConfig}</pre>
                <div class="row">
                  <span class="item-title grow">For apps that connect over HTTP</span>
                  <button class="btn small" onclick={() => copyText(httpConfig, "Config")}><Icon name="copy" />Copy</button>
                </div>
                <pre class="selectable">{httpConfig.replace(mcpInfo.token, "•".repeat(12))}</pre>
                <span class="secondary">The HTTP config holds a secret token (hidden here, included when copied). Keep it private.</span>
              </div>
            {/if}
          </div>
        </section>
      {:else}
        <section class="glass list">
          <div class="item">
            {@render setting("Pious", updateText)}
            {#if update.kind === "available"}
              <button class="btn tertiary" onclick={() => run("open_url", { url: update.release.page })}><Icon name="external" />What's new</button>
              <button class="btn primary" onclick={() => run("download_update")}><Icon name="download" />Download</button>
            {:else if update.kind === "downloading"}
              <div class="progress" style="width: 180px"><div style="width: {update.total ? (update.received / update.total) * 100 : 0}%"></div></div>
            {:else if update.kind === "ready"}
              <button class="btn primary" onclick={() => run("restart_to_update")}><Icon name="update" />Restart now</button>
            {:else}
              <button class="btn" disabled={update.kind === "checking"} onclick={() => run("check_for_update")}>
                <Icon name="refresh" />{update.kind === "checking" ? "Checking…" : update.kind === "failed" ? "Try again" : "Check for updates"}
              </button>
            {/if}
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Check automatically", "Look for a new version on GitHub each time Pious starts.")}
            <Switch on={prefs.auto_update} onchange={(on) => setPreferences({ auto_update: on })} />
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Your data", snap.data_dir)}
            <button class="btn" onclick={() => run("open_path", { path: snap.data_dir })}><Icon name="external" />Open</button>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Managed versions", snap.versions_dir)}
            <button class="btn" onclick={() => run("open_path", { path: snap.versions_dir })}><Icon name="external" />Open</button>
          </div>
          <hr class="divider" />
          <div class="item">
            {@render setting("Help", "Everything about Pious, its privacy policy and terms.")}
            <button class="btn" onclick={() => app.navigate({ name: "help" })}><Icon name="book-open" />Help</button>
            <button class="btn tertiary" onclick={() => app.navigate({ name: "help", doc: "PRIVACY" })}>Privacy</button>
            <button class="btn tertiary" onclick={() => app.navigate({ name: "help", doc: "TERMS" })}>Terms</button>
          </div>
        </section>
        <span class="label">Crash reports</span>
        <section class="glass list">
          <div class="item">
            {@render setting(
              "Keep crash reports",
              "When Pious or a Roblox window it started crashes, write what happened to a file on this PC (your user name and Roblox sign-ins are left out). Nothing is sent anywhere.",
            )}
            <button class="btn" onclick={() => run("open_crashes_folder")}><Icon name="folder" />Folder</button>
            <Switch on={prefs.crash_reports} onchange={(crash_reports) => setPreferences({ crash_reports })} />
          </div>
          {#each snap.crashes.slice(0, 8) as c (c.file)}
            <hr class="divider" />
            <div class="item">
              <span class="tile-icon" style="width: 32px; height: 32px"><Icon name={c.kind === "roblox" ? "game" : "warning"} size={15} /></span>
              <div class="col grow" style="gap: 1px; min-width: 0">
                <span class="item-title">{c.kind === "roblox" ? "Roblox" : "Pious"} · {new Date(c.at).toLocaleString()}</span>
                <span class="meta line" title={c.summary}>{c.summary || "No details"}</span>
              </div>
              <button class="btn small" onclick={() => openCrash(c.file)}>Open</button>
            </div>
            {#if crashText?.file === c.file}
              <pre class="crash selectable">{crashText.text}</pre>
              <div class="item" style="justify-content: flex-end">
                <button class="btn small tertiary" onclick={() => (crashText = null)}>Close</button>
                <button class="btn small" onclick={() => reportCrash(crashText!.text)}><Icon name="external" />Report on GitHub</button>
              </div>
            {/if}
          {/each}
          {#if snap.crashes.length}
            <hr class="divider" />
            <div class="item" style="justify-content: flex-end">
              <button class="btn small tertiary" onclick={() => run("clear_crashes")}><Icon name="remove" />Delete all</button>
            </div>
          {/if}
        </section>
        {#if stats}
          <span class="label">Your stats</span>
          <div class="stats stagger">
            {#each [["Played", hours(stats.play_seconds)], ["Sessions", String(stats.sessions)], ["Games", String(stats.games)], ["Launches", String(stats.launches)], ["Clips", String(stats.clips)], ["Recordings", String(stats.recordings)], ["Messages", String(stats.messages)], ["Server hops", String(stats.server_hops)], ["Macro runs", String(stats.macro_runs)], ["Auto-clicks", String(stats.clicks)]] as [label, value] (label)}
              <div class="glass-base stat"><span class="stat-value">{value}</span><span class="secondary">{label}</span></div>
            {/each}
          </div>
          {#if stats.top_games.length}
            <div class="glass-base top">
              <span class="item-title">Most played</span>
              {#each stats.top_games as [name, seconds], i (name)}
                <div class="row"><span class="secondary">{i + 1}</span><span class="grow line">{name}</span><span class="meta">{hours(seconds)}</span></div>
              {/each}
            </div>
          {/if}
          <span class="secondary">Kept on this PC only{stats.since ? `, since ${new Date(stats.since).toLocaleDateString()}` : ""}.</span>
        {/if}
        {#if releases.length}
          <span class="label">What's new</span>
          <section class="glass list">
            {#each releases as r, i (r.version)}
              {#if i > 0}<hr class="divider" />{/if}
              <button class="item release-head" onclick={() => (openRelease = openRelease === i ? -1 : i)}>
                <span class="item-title grow">Pious {r.version}{r.version === snap.current_version ? " · this version" : ""}</span>
                <span class="flip" class:open={openRelease === i}><Icon name="caret-down" /></span>
              </button>
              {#if openRelease === i}<div class="release-body"><ReleaseNotes release={r} /></div>{/if}
            {/each}
          </section>
        {/if}
        <div class="glass-base about">
          <span class="about-logo">{@html logo}</span>
          <div class="col grow" style="gap: 2px">
            <b>Pious</b>
            <span class="secondary">Version {snap.current_version}</span>
          </div>
          <span class="secondary">Your Roblox, all in one place</span>
        </div>
      {/if}
    </div>
  {/key}
</div>

<style>
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 8px;
    padding: 4px 0 12px;
  }
  .theme-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--r-md);
    border: 1px solid rgb(var(--surface) / 0.08);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .theme-card.on {
    border-color: rgb(var(--accent) / 0.8);
    box-shadow: 0 0 0 1px rgb(var(--accent) / 0.5);
  }
  .theme-card.broken {
    opacity: 0.55;
  }
  .theme-swatch {
    position: relative;
    width: 38px;
    height: 38px;
    flex: none;
    border-radius: var(--r-sm);
    border: 1px solid rgb(255 255 255 / 0.1);
  }
  .theme-swatch span {
    position: absolute;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    bottom: 5px;
  }
  .theme-swatch span:first-child {
    left: 5px;
  }
  .theme-swatch span:last-child {
    left: 19px;
  }
  .font-sample {
    padding: 0 0 12px;
    font-size: 15px;
    color: rgb(var(--muted));
  }
  .color-pick input {
    width: 30px;
    height: 26px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .crash {
    margin: 0 0 8px;
    padding: 10px 12px;
    max-height: 260px;
    overflow: auto;
    border-radius: var(--r-md);
    background: rgb(0 0 0 / 0.28);
    font-family: "Cascadia Code", Consolas, monospace;
    font-size: 11.5px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .settings {
    max-width: 880px;
    margin: 0 auto;
    width: 100%;
    gap: 14px;
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
  .short {
    min-height: 46px;
    padding: 7px 0;
  }
  .warn {
    color: #f5a524;
  }
  .kbd.fixed {
    min-width: 60px;
    text-align: center;
  }
  .plugin {
    align-items: flex-start;
  }
  .empty-plugins {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    border-radius: var(--r-md);
  }
  .live {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #30a46c;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .apps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px;
    padding: 2px 0 8px;
  }
  .app-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    animation: rise 240ms var(--ease) both;
    transition: border-color var(--fast);
  }
  .app-card.on {
    border-color: rgb(48 164 108 / 0.5);
  }
  .pad-x {
    display: block;
    padding-bottom: 8px;
  }
  .mcp-config {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 0 14px;
  }
  .mcp-config pre {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: rgb(0 0 0 / 0.25);
    font-family: "Cascadia Code", Consolas, monospace;
    font-size: 11.5px;
    overflow-x: auto;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 8px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px;
    border-radius: var(--r-md);
  }
  .stat-value {
    font-size: 20px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: var(--r-md);
  }
  .release-head {
    width: 100%;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .release-body {
    padding: 0 0 14px;
    animation: rise 240ms var(--ease) both;
  }
  .flip {
    display: inline-flex;
    transition: transform 240ms var(--ease);
  }
  .flip.open {
    transform: rotate(180deg);
  }
  .panel-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    animation: rise 260ms var(--ease);
  }
  .list {
    padding: 2px 16px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 0;
    min-height: 54px;
    transition: opacity var(--med);
  }
  .item.off,
  .sub.off,
  .off-all {
    opacity: 0.45;
    pointer-events: none;
  }
  .sub {
    padding-left: 16px;
    border-left: 2px solid rgb(var(--surface) / calc(0.06 * var(--glass)));
    margin: 0 0 6px 2px;
    transition: opacity var(--med);
  }
  .setup {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px;
  }
  .colors {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 0;
  }
  .swatch {
    width: 13px;
    height: 13px;
    border-radius: 4px;
    border: 1px solid rgb(255 255 255 / 0.2);
  }
  .preview {
    width: 160px;
    gap: 8px;
  }
  .big-swatch {
    height: 60px;
    border-radius: var(--r-md);
    border: 1px solid rgb(255 255 255 / 0.12);
  }
  .track {
    display: grid;
    grid-template-columns: 80px 1fr;
    align-items: center;
    gap: 12px;
  }
  .rail {
    width: 100%;
    height: 10px;
    border-radius: 99px;
    appearance: none;
    outline: none;
  }
  .rail.hue {
    background: linear-gradient(90deg, #e33, #ee3, #3e3, #3ee, #33e, #e3e, #e33);
  }
  .rail::-webkit-slider-thumb {
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: white;
    border: 2px solid rgb(0 0 0 / 0.4);
    cursor: pointer;
  }
  .thumb {
    width: 64px;
    height: 36px;
    object-fit: cover;
    border-radius: var(--r-sm);
  }
  .small-number {
    width: 72px;
    height: 26px;
    font-size: 12px;
  }
  .disclosure {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    border: none;
    background: none;
    color: rgb(var(--muted));
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
    padding: 6px 2px;
  }
  .disclosure:hover {
    color: rgb(var(--text));
  }
  .chev {
    display: inline-flex;
    transition: transform var(--med) var(--ease);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px;
  }
  .rec-dot,
  .live-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #e5484d;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .live-dot {
    background: rgb(var(--accent));
  }
  .err {
    color: rgb(var(--text));
  }
  .about {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px;
  }
  .about-logo {
    display: grid;
    width: 36px;
    height: 36px;
  }
  .about-logo :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
