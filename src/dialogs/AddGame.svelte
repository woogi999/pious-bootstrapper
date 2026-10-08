<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { call, run } from "../lib/api";
  import type { GameInfo, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";

  let input = $state("");
  let lookup = $state<{ kind: "idle" } | { kind: "loading" } | { kind: "found"; info: GameInfo } | { kind: "failed"; error: string }>({
    kind: "idle",
  });

  async function look() {
    if (!input.trim() || lookup.kind === "loading") return;
    lookup = { kind: "loading" };
    try {
      lookup = { kind: "found", info: await call<GameInfo>("look_up_game", { input }) };
    } catch (error) {
      lookup = { kind: "failed", error: String(error) };
    }
  }

  async function save() {
    if (lookup.kind !== "found") return look();
    const id = await run<Uuid>("add_game", { info: lookup.info });
    app.modal = null;
    if (id) app.navigate({ name: "game", id });
  }
</script>

<ModalFrame title="Add Game" subtitle="Save a Roblox experience to your library." width={520}>
  <div class="field">
    <span class="label">Game</span>
    <div class="row">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        autofocus
        placeholder="Place ID or roblox.com/games/… link"
        bind:value={input}
        oninput={() => (lookup = { kind: "idle" })}
        onkeydown={(e) => e.key === "Enter" && (lookup.kind === "found" ? save() : look())}
      />
      <button class="btn" disabled={!input.trim() || lookup.kind === "loading"} onclick={look}><Icon name="search" />Look up</button>
    </div>
  </div>
  {#if lookup.kind === "idle"}
    <span class="secondary">Tip: copy the link from the game's page on roblox.com.</span>
  {:else if lookup.kind === "loading"}
    <div class="notice"><Icon name="clock" />Looking up the game on Roblox…</div>
  {:else if lookup.kind === "failed"}
    <div class="notice negative"><Icon name="warning" />{lookup.error}</div>
  {:else}
    <div class="glass found">
      <span class="tile-icon" style="width: 44px; height: 44px"><Icon name="game" size={20} /></span>
      <div class="col grow">
        <span class="item-title">{lookup.info.name}</span>
        <span class="meta">{lookup.info.developer ?? "Roblox"} · Place {lookup.info.place_id}</span>
      </div>
      <Icon name="success" size={18} />
    </div>
  {/if}
  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    <button class="btn primary" disabled={lookup.kind !== "found"} onclick={save}><Icon name="add" />Add to Library</button>
  {/snippet}
</ModalFrame>

<style>
  .found {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
  }
</style>
