<!-- How Pious starts Roblox (multi-instance, links, bootstrappers, the
     default version), or (part "presence") what Discord and your Roblox
     friends see. Both live on the Tweaks page. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { run, setPreferences } from "../lib/api";
  import type { VersionChoice } from "../lib/types";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Switch from "./Switch.svelte";

  const snap = $derived(app.snap!);
  const prefs = $derived(snap.bootstrapper.preferences);

  const versionChoices = $derived([
    { value: "Latest" as VersionChoice, label: snap.latest_installed ? `Latest installed · ${snap.version_titles[snap.latest_installed]}` : "Latest installed" },
    ...snap.bootstrapper.versions
      .filter((v) => snap.usable_versions.includes(v.hash))
      .map((v) => ({ value: { Specific: v.hash } as VersionChoice, label: `${snap.version_titles[v.hash]} (${v.hash.replace("version-", "").slice(0, 8)})` })),
  ]);

  function handlerName(command: string): string {
    const lower = command.toLowerCase();
    const bootstrapper = snap.bootstrappers.find((b) => lower.includes(b.name.toLowerCase()));
    if (bootstrapper) return bootstrapper.name;
    if (lower.includes("--roblox-link")) return "Pious";
    if (lower.includes("roblox")) return "Roblox";
    return "another program";
  }

  const linksDescription = $derived.by(() => {
    if (prefs.handle_roblox_links) {
      const previous = prefs.previous_handlers.find(([scheme]) => scheme === "roblox-player");
      return `Pressing Play on roblox.com starts Roblox through Pious, so every window shows up in Instances and can run alongside the others. Before Pious, links opened with ${previous ? handlerName(previous[1]) : "nothing"}.`;
    }
    const owner = snap.link_handler ? handlerName(snap.link_handler) : "nothing";
    return `What starts Roblox when you press Play on roblox.com. With Pious, each browser can be signed in to a different account and every game runs as its own instance. Right now: ${owner}.`;
  });

  const currentHandler = $derived.by(() => {
    if (prefs.handle_roblox_links) return "pious";
    const lower = (snap.link_handler ?? "").toLowerCase();
    const owner = snap.bootstrappers.find((b) => lower.includes(b.name.toLowerCase()));
    if (owner) return owner.name;
    return lower.includes("roblox") ? "roblox" : "";
  });
  const handlerChoices = $derived([
    { value: "pious", label: "Pious" },
    ...snap.bootstrappers.map((b) => ({ value: b.name, label: b.name })),
    { value: "roblox", label: "Roblox" },
    ...(currentHandler === "" ? [{ value: "", label: "Another program" }] : []),
  ]);
  const viaChoices = $derived([
    { value: null as string | null, label: "Directly (Pious)" },
    ...snap.bootstrappers.map((b) => ({ value: b.name as string | null, label: `Through ${b.name}` })),
  ]);

  // A bootstrapper only matters here while it actually starts games: it
  // opens Roblox links, or Pious launches through it.
  const presenceRival = $derived(
    snap.bootstrappers.find((b) => b.discord_presence && ((b.opens_links && !prefs.handle_roblox_links) || prefs.launch_via === b.name)),
  );
  const discordDescription = $derived(
    presenceRival
      ? `Show the game you're playing on your Discord profile. ${presenceRival.name} starts some of your games and shows its own, so Pious covers the games it starts itself.`
      : "Show the game you're playing on your Discord profile while Discord is open.",
  );
  const noPresence = $derived(!prefs.discord_presence);

  let { part }: { part: "launch" | "presence" } = $props();
</script>

{#if part === "launch"}

<section class="group">
  <span class="label">Launching</span>
  <div class="glass list">
    <SettingRow icon="copy" title="Multi-instance" description="Allow several Roblox clients to run at the same time.">
      <Switch on={prefs.multi_instance} onchange={(on) => setPreferences({ multi_instance: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="history"
      title="Remember last used setup"
      description="Play reuses the version and server you last launched each game with. The account comes from the sidebar unless you pin one to the game."
    >
      <Switch on={prefs.remember_last_used} onchange={(on) => setPreferences({ remember_last_used: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow icon="warning" title="Confirm before closing instances" description="Ask before closing a running Roblox instance.">
      <Switch on={prefs.confirm_close} onchange={(on) => setPreferences({ confirm_close: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow icon="link" title="Roblox links open with" description={linksDescription}>
      <Select options={handlerChoices} value={currentHandler} onchange={(target) => run("set_link_handler", { target })} width="220px" />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="launch"
      title="Pious starts games"
      description={prefs.launch_via
        ? `Through ${prefs.launch_via}, so its mods, FastFlags and features apply. Pious's own Tweaks are skipped.`
        : "Directly, with the version you chose and Pious's Tweaks. Pick a bootstrapper to use its mods and features instead."}
    >
      <Select options={viaChoices} value={prefs.launch_via} onchange={(v) => setPreferences({ launch_via: v })} width="220px" />
    </SettingRow>
    <hr class="divider" />
    <SettingRow icon="versions" title="Default Roblox version" description="Used when a game has no version of its own.">
      <Select options={versionChoices} value={prefs.default_version} onchange={(choice) => run("set_default_version", { choice })} placeholder="Choose a version" width="260px" />
    </SettingRow>
  </div>
</section>
{:else}

<section class="group">
  <span class="label">Discord Rich Presence</span>
  <div class="glass list">
    <SettingRow icon="chat" title="Show game activity" description={discordDescription}>
      <Switch on={prefs.discord_presence} onchange={(on) => setPreferences({ discord_presence: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="users"
      title="Allow activity joining"
      description="Adds a Join game button to your status, so anyone who sees it can join your server in one click. Public servers only; private servers stay private. Discord shows the button to others, not to you."
      off={noPresence}
    >
      <Switch on={prefs.discord_join} disabled={noPresence} onchange={(on) => setPreferences({ discord_join: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="accounts"
      title="Show Roblox account"
      description="Shows the avatar and name of the account you're playing as in the corner of your status."
      off={noPresence}
    >
      <Switch on={prefs.discord_account} disabled={noPresence} onchange={(on) => setPreferences({ discord_account: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="studio"
      title="Show Roblox Studio activity"
      description="Shows what you're building while Roblox Studio is open and no game is running."
      off={noPresence}
    >
      <Switch on={prefs.studio_presence} disabled={noPresence} onchange={(on) => setPreferences({ studio_presence: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="home"
      title="Show Pious while idle"
      description="Says Playing Pious while Pious is open and no game is, with a Get Pious button for friends."
      off={noPresence}
    >
      <Switch on={prefs.pious_presence} disabled={noPresence} onchange={(on) => setPreferences({ pious_presence: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow icon="edit" title="Activity name" description="What your Discord profile says you're playing." off={noPresence}>
      <Select
        options={[
          { value: "GameName" as const, label: "Playing <game name>" },
          { value: "Pious" as const, label: "Playing Pious" },
        ]}
        value={prefs.discord_display}
        onchange={(discord_display) => setPreferences({ discord_display })}
        width="210px"
      />
    </SettingRow>
  </div>
</section>

<section class="group">
  <span class="label">Roblox</span>
  <div class="glass list">
    <SettingRow
      icon="signal"
      title="Appear online"
      description="While Pious is open, your account shows as online to your Roblox friends, like it does with the Roblox app open. Uses the account you play as."
    >
      <Switch on={prefs.appear_online} onchange={(on) => setPreferences({ appear_online: on })} />
    </SettingRow>
  </div>
</section>
{/if}

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list {
    padding: 2px 14px;
  }
</style>
