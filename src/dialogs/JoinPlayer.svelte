<!-- Join anyone by username: username → user ID → presence (asked as one of
     your accounts) → their exact server. No friendship needed, as long as
     they let everyone join them. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { call, handleLaunch, run } from "../lib/api";
  import { accountLabel, who } from "../lib/format";
  import type { LaunchOutcome, PlayerFound, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";

  const snap = $derived(app.snap!);
  let username = $state("");
  let account = $state<Uuid | null>(app.snap!.active_account);
  let lookup = $state<{ kind: "idle" } | { kind: "loading" } | { kind: "found"; found: PlayerFound } | { kind: "failed"; error: string }>({
    kind: "idle",
  });

  const accounts = $derived(snap.bootstrapper.accounts.map((a) => ({ value: a.id as Uuid | null, label: `${accountLabel(a)} (@${who(a.username)})` })));

  async function find() {
    if (!username.trim()) return;
    if (!account) {
      lookup = { kind: "failed", error: "Choose which of your accounts should look them up." };
      return;
    }
    lookup = { kind: "loading" };
    try {
      lookup = { kind: "found", found: await call<PlayerFound>("find_player", { username, account }) };
    } catch (e) {
      lookup = { kind: "failed", error: String(e) };
    }
  }

  const where = $derived.by(() => {
    if (lookup.kind !== "found") return null;
    const { presence, game } = lookup.found;
    switch (presence.kind) {
      case "InGame": {
        const name = game?.name ?? presence.location;
        if (presence.job) return { icon: "play", tone: "", text: `Playing ${name}. You can join their server.`, exact: true, place: presence.place_id };
        if (presence.place_id)
          return {
            icon: "lock",
            tone: "caution",
            text: `Playing ${name}, but their join settings hide which server. You can still open the game.`,
            exact: false,
            place: presence.place_id,
          };
        return { icon: "lock", tone: "caution", text: "In a game, but their privacy settings hide which one.", exact: false, place: null };
      }
      case "Online":
        return { icon: "globe", tone: "", text: "Online on the website, not in a game.", exact: false, place: null };
      case "InStudio":
        return { icon: "cube", tone: "", text: "Building in Roblox Studio.", exact: false, place: null };
      default:
        return { icon: "power", tone: "", text: "Offline right now.", exact: false, place: null };
    }
  });

  async function join(exact: boolean) {
    if (lookup.kind !== "found" || lookup.found.presence.kind !== "InGame" || !lookup.found.presence.place_id) return;
    const { presence, game, user } = lookup.found;
    app.modal = null;
    app.toast("active", `Joining @${who(user.username)}…`);
    const args = { place: presence.place_id, job: exact ? presence.job : null, account, name: game?.name ?? null };
    const once = (force: boolean) => run<LaunchOutcome>("join_player", { ...args, force });
    handleLaunch(await once(false), () => once(true));
  }
</script>

<ModalFrame title="Join a player" subtitle="Join anyone by username, friend or not." width={500}>
  <div class="field">
    <span class="label">Player</span>
    <div class="row">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        autofocus
        placeholder="Their Roblox username"
        bind:value={username}
        oninput={() => (lookup = { kind: "idle" })}
        onkeydown={(e) => e.key === "Enter" && find()}
      />
      <button class="btn" disabled={!username.trim() || lookup.kind === "loading"} onclick={find}><Icon name="search" />Find</button>
    </div>
  </div>
  <div class="field"><span class="label">Join as</span><Select options={accounts} value={account} onchange={(v) => (account = v)} /></div>
  {#if lookup.kind === "idle"}
    <span class="secondary">
      Pious turns the username into a user ID, asks Roblox where they are, and joins that exact server. You don't need to be friends, as long
      as they let everyone join them.
    </span>
  {:else if lookup.kind === "loading"}
    <div class="notice"><Icon name="clock" />Looking for them…</div>
  {:else if lookup.kind === "failed"}
    <div class="notice negative"><Icon name="warning" />{lookup.error}</div>
  {:else if where}
    <div class="row" style="gap: 10px">
      <span class="tile-icon" style="width: 32px; height: 32px; border-radius: 50%"><Icon name="account" size={16} /></span>
      <div class="col">
        <span class="item-title">{who(lookup.found.user.display_name)}</span>
        <span class="secondary">@{who(lookup.found.user.username)} · ID {lookup.found.user.id}</span>
      </div>
    </div>
    <div class="notice {where.tone}"><Icon name={where.icon} />{where.text}</div>
  {/if}
  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    {#if where && !where.exact && where.place}
      <button class="btn" onclick={() => join(false)}><Icon name="play" />Open their game</button>
    {/if}
    <button class="btn primary" disabled={!where?.exact} onclick={() => join(true)}><Icon name="arrow-right" />Join their server</button>
  {/snippet}
</ModalFrame>
