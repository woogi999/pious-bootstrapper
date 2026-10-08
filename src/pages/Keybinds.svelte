<!-- Keybinds: keys and mouse buttons that press something else while a
     Roblox window is in front, for every game, one account or one game.
     Game ones win over account ones, which win over the ones for every
     game. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import { accountLabel, plural } from "../lib/format";
  import { keyName } from "../lib/inputLayouts";
  import type { KeybindScope, KeybindSet, Keybinds } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import InputPicker from "../components/InputPicker.svelte";
  import Select from "../components/Select.svelte";
  import Switch from "../components/Switch.svelte";

  const snap = $derived(app.snap!);
  const keybinds = $derived(snap.bootstrapper.preferences.keybinds);
  const accounts = $derived(snap.bootstrapper.accounts);
  // One entry per place, library games first.
  const games = $derived(
    [
      ...new Map(
        [...snap.bootstrapper.games].sort((a, b) => Number(b.in_library) - Number(a.in_library)).map((g) => [g.place_id, g]),
      ).values(),
    ].sort((a, b) => a.name.localeCompare(b.name)),
  );

  /** Common remaps, offered when there are none yet. */
  const STARTERS: { label: string; from: string; to: string }[] = [
    { label: "Side button presses E", from: "MouseBack", to: "KeyE" },
    { label: "Other side button presses Q", from: "MouseForward", to: "KeyQ" },
    { label: "Caps Lock presses Shift (shift lock)", from: "CapsLock", to: "ShiftLeft" },
    { label: "Block the Windows key", from: "MetaLeft", to: "" },
  ];

  function save(next: Partial<Keybinds>) {
    setPreferences({ keybinds: { ...keybinds, ...next } });
  }
  function saveSet(id: string, change: (s: KeybindSet) => KeybindSet) {
    save({ sets: keybinds.sets.map((s) => (s.id === id ? change(s) : s)) });
  }
  function setBind(id: string, i: number, change: Partial<{ from: string; to: string }>) {
    saveSet(id, (s) => ({ ...s, binds: s.binds.map((b, j) => (j === i ? { ...b, ...change } : b)) }));
  }
  function addSet(scope: KeybindScope, binds = [{ from: "", to: "" }]) {
    save({ enabled: true, sets: [...keybinds.sets, { id: crypto.randomUUID(), scope, enabled: true, binds }] });
  }
  function removeSet(id: string) {
    save({ sets: keybinds.sets.filter((s) => s.id !== id) });
  }
  /** Adds a starter to the set for every game, making that set if needed. */
  function addStarter(from: string, to: string) {
    const global = keybinds.sets.find((s) => s.scope.kind === "Global");
    if (global) saveSet(global.id, (s) => ({ ...s, binds: [...s.binds.filter((b) => b.from), { from, to }] }));
    else addSet({ kind: "Global" }, [{ from, to }]);
  }

  const scopeName = (scope: KeybindScope) => {
    if (scope.kind === "Global") return "Every game";
    if (scope.kind === "Account") {
      const a = accounts.find((x) => x.id === scope.id);
      return a ? `Playing as ${accountLabel(a)}` : "An account that's gone";
    }
    return games.find((g) => g.place_id === scope.id)?.name ?? `Game ${scope.id}`;
  };
  const scopeHint = (scope: KeybindScope) =>
    scope.kind === "Global" ? "Any Roblox window" : scope.kind === "Account" ? "Windows Pious started for this account" : "Only in this game";
  const order = (s: KeybindSet) => (s.scope.kind === "Global" ? 0 : s.scope.kind === "Account" ? 1 : 2);
  const sets = $derived([...keybinds.sets].sort((a, b) => order(a) - order(b)));
  const total = $derived(keybinds.sets.reduce((n, s) => n + s.binds.filter((b) => b.from).length, 0));
  /** Keys picked more than once in a set: only the first one counts. */
  const repeats = (set: KeybindSet) => {
    const seen = new Set<string>();
    const twice = new Set<string>();
    for (const b of set.binds) if (b.from) (seen.has(b.from) ? twice : seen).add(b.from);
    return twice;
  };

  let adding = $state<"" | "account" | "game">("");
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Keybinds</h1>
      <span class="meta">
        {#if !keybinds.enabled}Off. Your keys do what Roblox expects.
        {:else if total}{plural(total, "key")} remapped while Roblox is in front
        {:else}Make a key or mouse button press something else in Roblox{/if}
      </span>
    </div>
    <span class="meta">{keybinds.enabled ? "On" : "Off"}</span>
    <Switch on={keybinds.enabled} onchange={(enabled) => save({ enabled })} />
  </div>

  {#if !sets.length}
    <EmptyState
      icon="keyboard"
      title="No keybinds yet"
      body="Press a side button for E, turn Caps Lock into shift lock, or block a key you keep hitting. They only work while a Roblox window is in front."
    >
      <button class="btn primary" onclick={() => addSet({ kind: "Global" })}><Icon name="add" />Add keybinds</button>
    </EmptyState>
    <span class="label">Or start with one of these</span>
    <div class="starters">
      {#each STARTERS as s (s.label)}
        <button class="glass starter" onclick={() => addStarter(s.from, s.to)}>
          <span class="kbd">{keyName(s.from)}</span>
          <Icon name="arrow-right" size={12} />
          <span class="kbd" class:none={!s.to}>{s.to ? keyName(s.to) : "Nothing"}</span>
          <span class="meta grow line">{s.label}</span>
          <Icon name="add" size={13} />
        </button>
      {/each}
    </div>
  {:else}
    <div class="sets" class:off={!keybinds.enabled}>
      {#each sets as set (set.id)}
        {@const twice = repeats(set)}
        <section class="glass set" class:paused={!set.enabled}>
          <div class="row head">
            <span class="scope-icon"><Icon name={set.scope.kind === "Global" ? "globe" : set.scope.kind === "Account" ? "account" : "game"} size={15} /></span>
            <div class="col grow" style="gap: 0">
              <span class="item-title line">{scopeName(set.scope)}</span>
              <span class="secondary line">{scopeHint(set.scope)} · {plural(set.binds.filter((b) => b.from).length, "key")}</span>
            </div>
            <Switch on={set.enabled} onchange={(enabled) => saveSet(set.id, (s) => ({ ...s, enabled }))} />
            <button class="icon-btn" title="Delete these keybinds" aria-label="Delete" onclick={() => removeSet(set.id)}><Icon name="remove" /></button>
          </div>
          <div class="binds">
            {#each set.binds as bind, i (i)}
              <div class="row bind" class:clash={twice.has(bind.from)}>
                <InputPicker value={bind.from} wheel placeholder="Pick a key" width="150px" onchange={(from) => setBind(set.id, i, { from })} />
                <span class="arrow"><Icon name="arrow-right" size={13} /></span>
                {#if bind.to === "" && bind.from}
                  <span class="blocked">Does nothing</span>
                {:else}
                  <InputPicker value={bind.to} placeholder="Presses…" width="150px" onchange={(to) => setBind(set.id, i, { to })} />
                {/if}
                <button class="chip" class:on={bind.to === "" && !!bind.from} title="Make this key do nothing in Roblox" onclick={() => setBind(set.id, i, { to: "" })}>
                  Block
                </button>
                {#if twice.has(bind.from)}<span class="warn" title="This key is picked twice here. Only the first one counts.">Used twice</span>{/if}
                <span class="spacer"></span>
                <button class="icon-btn" aria-label="Remove" onclick={() => saveSet(set.id, (s) => ({ ...s, binds: s.binds.filter((_, j) => j !== i) }))}>
                  <Icon name="close" size={13} />
                </button>
              </div>
            {/each}
          </div>
          <button class="link add" onclick={() => saveSet(set.id, (s) => ({ ...s, binds: [...s.binds, { from: "", to: "" }] }))}>
            <Icon name="add" size={13} />Add a key
          </button>
        </section>
      {/each}
    </div>
  {/if}

  <span class="label">Add keybinds for</span>
  <div class="row adders">
    <button class="btn" disabled={sets.some((s) => s.scope.kind === "Global")} onclick={() => addSet({ kind: "Global" })}>
      <Icon name="globe" />Every game
    </button>
    {#if adding === "account"}
      <Select
        options={accounts.filter((a) => !sets.some((s) => s.scope.kind === "Account" && s.scope.id === a.id)).map((a) => ({ value: a.id as string | null, label: accountLabel(a) }))}
        value={null}
        placeholder="Pick an account"
        onchange={(id) => id && (addSet({ kind: "Account", id }), (adding = ""))}
        width="220px"
      />
    {:else}
      <button class="btn" disabled={!accounts.length} onclick={() => (adding = "account")}><Icon name="account" />An account…</button>
    {/if}
    {#if adding === "game"}
      <Select
        options={games.filter((g) => !sets.some((s) => s.scope.kind === "Game" && s.scope.id === g.place_id)).map((g) => ({ value: g.place_id as number | null, label: g.name }))}
        value={null}
        placeholder="Pick a game"
        onchange={(id) => id && (addSet({ kind: "Game", id }), (adding = ""))}
        width="240px"
      />
    {:else}
      <button class="btn" disabled={!games.length} onclick={() => (adding = "game")}><Icon name="game" />A game…</button>
    {/if}
  </div>
  <span class="secondary">
    Keybinds for a game win over ones for an account, which win over ones for every game. Account and game keybinds apply to Roblox windows Pious
    started; ones for every game apply to any Roblox window.
  </span>
</div>

<style>
  .sets {
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: opacity var(--med);
  }
  .sets.off {
    opacity: 0.5;
  }
  .set {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px;
    animation: rise 220ms var(--ease) both;
    transition: opacity var(--med);
  }
  .set.paused .binds {
    opacity: 0.55;
  }
  .head {
    gap: 10px;
  }
  .scope-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    background: rgb(var(--accent) / 0.14);
    color: rgb(var(--accent));
  }
  .binds {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-left: 42px;
    transition: opacity var(--med);
  }
  .bind {
    gap: 8px;
    padding: 4px 6px;
    margin: 0 -6px;
    border-radius: 10px;
    transition: background var(--fast);
  }
  .bind:hover {
    background: rgb(var(--surface) / 0.04);
  }
  .bind.clash {
    background: rgb(250 190 80 / 0.08);
  }
  .warn {
    font-size: 11.5px;
    font-weight: 600;
    color: rgb(250 190 80);
  }
  .arrow {
    color: rgb(var(--faint));
  }
  .blocked {
    width: 150px;
    font-size: 12.5px;
    color: rgb(var(--faint));
    font-style: italic;
  }
  .add {
    align-self: flex-start;
    margin-left: 42px;
  }
  .adders {
    gap: 8px;
    flex-wrap: wrap;
  }
  .starters {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 8px;
  }
  .starter {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    text-align: left;
    color: rgb(var(--muted));
    animation: rise 220ms var(--ease) both;
    transition:
      transform var(--fast),
      background var(--fast);
  }
  .starter:hover {
    transform: translateY(-1px);
    color: rgb(var(--text));
  }
  .kbd {
    padding: 2px 7px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 700;
    background: rgb(var(--surface) / 0.1);
    color: rgb(var(--text));
    white-space: nowrap;
  }
  .kbd.none {
    font-style: italic;
    font-weight: 500;
  }
</style>
