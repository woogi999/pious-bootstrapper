<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "../lib/state.svelte";
  import { call } from "../lib/api";
  import type { PrivateServer, ServerLinkInfo, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";

  let { editing, game: initialGame }: { editing: PrivateServer | null; game: Uuid | null } = $props();

  const snap = $derived(app.snap!);
  const libraryGames = $derived(snap.bootstrapper.games.filter((g) => g.in_library || g.id === initialGame));

  // The form edits its own copy, starting from the server being edited.
  const start = untrack(() => ({ editing, initialGame }));
  let game = $state<Uuid | null>(start.editing?.game_id ?? start.initialGame ?? null);
  let name = $state(start.editing?.name ?? "");
  let link = $state(start.editing?.link ?? "");
  let serverId = $state(start.editing?.server_id ?? "");
  let notes = $state(start.editing?.notes ?? "");
  let error = $state<string | null>(null);
  let saving = $state(false);

  // The link tells us the server's name and game (looked up as it's pasted).
  let info = $state<ServerLinkInfo | null>(null);
  let looking = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const value = link.trim();
    clearTimeout(timer);
    info = null;
    if (!/privateServerLinkCode=|roblox\.com\/share\?/i.test(value)) return;
    timer = setTimeout(async () => {
      looking = true;
      try {
        const found = await call<ServerLinkInfo>("server_link_info", { link: value });
        if (link.trim() !== value) return;
        info = found;
        // Pick the game for them when it's in the library.
        if (!game && found.place_id) {
          const match = app.snap!.bootstrapper.games.find((g) => g.place_id === found.place_id);
          if (match) game = match.id;
        }
      } catch {
        // The name is only a nicety; saving still works.
      } finally {
        looking = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });
  const gameChoices = $derived([
    { value: null as Uuid | null, label: info?.game ? `${info.game} (from the link)` : "The link's game" },
    ...libraryGames.map((g) => ({ value: g.id as Uuid | null, label: g.name })),
  ]);

  async function save() {
    if (saving) return;
    saving = true;
    try {
      await call("save_server", { form: { editing: editing?.id ?? null, game, name, link, server_id: serverId, notes } });
      app.modal = null;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<ModalFrame
  title={editing ? "Edit Private Server" : "Add Private Server"}
  subtitle="Save a private server so you can join it in one click."
  width={540}
>
  <div class="field">
    <span class="label">Game</span>
    <Select options={gameChoices} value={game} onchange={(v) => (game = v)} />
  </div>
  <div class="field">
    <span class="label">Server name</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="input"
      autofocus
      placeholder={looking ? "Looking up the server's name…" : info?.name ? `${info.name} (its own name)` : "Leave empty to use the server's own name"}
      bind:value={name}
    />
  </div>
  <div class="field">
    <span class="label">Private server link</span>
    <input class="input" placeholder="https://www.roblox.com/share?code=…" bind:value={link} oninput={() => (error = null)} />
  </div>
  <div class="field">
    <span class="label">Server ID (optional)</span>
    <input class="input" placeholder="Optional" bind:value={serverId} />
  </div>
  <div class="field">
    <span class="label">Notes</span>
    <input class="input" placeholder="Anything you want to remember" bind:value={notes} onkeydown={(e) => e.key === "Enter" && save()} />
  </div>
  {#if error}<div class="notice negative"><Icon name="warning" />{error}</div>{/if}
  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    <button class="btn primary" disabled={saving} onclick={save}><Icon name={editing ? "check" : "add"} />{saving ? "Saving…" : editing ? "Save changes" : "Save server"}</button>
  {/snippet}
</ModalFrame>
