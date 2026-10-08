<!-- What Pious does while you play: anti-AFK, rejoining, the server's
     location and emoji shortcodes. Lives on the Tweaks page. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Switch from "./Switch.svelte";

  const prefs = $derived(app.snap!.bootstrapper.preferences);
</script>

<section class="group">
  <span class="label">While you play</span>
  <div class="glass list">
    <SettingRow
      icon="clock"
      title="Anti-AFK"
      description="Keeps every Roblox window from being kicked after 20 idle minutes. Each window gets a key press on schedule, sent only once you've stopped typing for a few seconds, then focus goes straight back to what you were using."
    >
      <Switch on={prefs.anti_afk.enabled} onchange={(on) => setPreferences({ anti_afk: { enabled: on } })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow icon="clock" title="Nudge every" description="How often an idle window gets its nudge, and what it is." off={!prefs.anti_afk.enabled}>
      <Select
        options={[3, 6, 9, 11, 13, 15, 18].map((m) => ({ value: m, label: `${m} minutes` }))}
        value={prefs.anti_afk.minutes}
        onchange={(minutes) => setPreferences({ anti_afk: { minutes } })}
        width="150px"
      />
      <Select
        options={[
          { value: "Walk" as const, label: "Step (W, then S)" },
          { value: "Jump" as const, label: "Jump (Space)" },
          { value: "Zoom" as const, label: "Zoom (I, then O)" },
        ]}
        value={prefs.anti_afk.action}
        onchange={(action) => setPreferences({ anti_afk: { action } })}
        width="200px"
      />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="refresh"
      title="Rejoin when disconnected"
      description="If a game kicks you or loses connection, Pious closes the error and joins the same game again with the same account. Leaving on purpose and teleports are left alone."
    >
      <Switch on={prefs.auto_rejoin} onchange={(on) => setPreferences({ auto_rejoin: on })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="map-pin"
      title="Server location"
      description="When you join a server, Pious says where it is (like Ashburn, Virginia, US) and shows it in Instances. It asks ipinfo.io about the server's address, never yours."
    >
      <Switch on={prefs.server_location} onchange={(on) => setPreferences({ server_location: on })} />
    </SettingRow>
  </div>
</section>

<section class="group">
  <span class="label">Emoji shortcodes</span>
  <div class="glass list">
    <SettingRow
      icon="emoji"
      title="In Pious's chats"
      description="Type a colon and a name, like :sob:, to pick an emoji from a list (Enter or Tab picks it, like on Discord). A full :name: turns into its emoji by itself."
    >
      <Switch on={prefs.emoji_shortcodes.in_app} onchange={(on) => setPreferences({ emoji_shortcodes: { in_app: on } })} />
    </SettingRow>
    <hr class="divider" />
    <SettingRow
      icon="emoji"
      title="In Roblox"
      description="The same while you type in Roblox's chat: a list shows over the game (up and down to choose, Enter or Tab to pick), and :name: swaps itself for the emoji. Pious watches what you type only while a Roblox window is in front."
    >
      <Switch on={prefs.emoji_shortcodes.in_roblox} onchange={(on) => setPreferences({ emoji_shortcodes: { in_roblox: on } })} />
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
</style>
