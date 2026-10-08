<!-- A plugin's page, walled off in a frame of its own. It can only talk to
     Pious through messages, and only for what its manifest asked for. -->
<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { app, type Page } from "../lib/state.svelte";
  import { handleLaunch, play, run } from "../lib/api";
  import type { LaunchOutcome } from "../lib/types";
  import EmptyState from "../components/EmptyState.svelte";

  let { id }: { id: string } = $props();

  const snap = $derived(app.snap!);
  const plugin = $derived(snap.plugins.find((p) => p.id === id && p.enabled));
  const src = $derived(plugin?.page ? convertFileSrc(plugin.page) : null);
  let frame = $state<HTMLIFrameElement>();
  let subscribed = false;

  const NEEDS: Record<string, string> = {
    snapshot: "read",
    launch: "launch",
    join: "launch",
    toast: "notify",
    navigate: "navigate",
    runMacro: "macros",
    openLink: "links",
  };

  /** What a plugin with "read" sees: no tokens, no file paths. */
  function view() {
    const s = app.snap!;
    return {
      games: s.bootstrapper.games.filter((g) => g.in_library).map((g) => ({ id: g.id, name: g.name, place_id: g.place_id, favorite: g.favorite, last_played: g.last_played })),
      accounts: s.bootstrapper.accounts.map((a) => ({ id: a.id, username: a.username, display_name: a.display_name, nickname: a.alias })),
      friends: s.friends.list.map((f) => ({ id: f.id, username: f.username, display_name: f.display_name, status: f.status, playing: f.location, place_id: f.place_id })),
      running: s.instances.map((i) => ({ id: i.id, game: s.bootstrapper.games.find((g) => g.id === i.game)?.name ?? null, started: i.started })),
      macros: s.bootstrapper.preferences.macros.map((m) => ({ name: m.name })),
    };
  }

  function theme() {
    const style = getComputedStyle(document.documentElement);
    const vars: Record<string, string> = {};
    for (const name of ["--bg", "--text", "--muted", "--faint", "--accent", "--accent-ink", "--surface", "--panel"]) {
      vars[name] = style.getPropertyValue(name).trim();
    }
    return vars;
  }

  function post(message: unknown) {
    frame?.contentWindow?.postMessage(message, "*");
  }

  async function handle(method: string, args: unknown[]): Promise<unknown> {
    const p = plugin!;
    const needed = NEEDS[method];
    if (!needed) throw new Error(`Pious has no ${method}.`);
    if (!p.permissions.includes(needed)) throw new Error(`${p.name} didn't ask to ${needed === "read" ? "see your data" : needed}.`);
    switch (method) {
      case "snapshot":
        return view();
      case "launch":
        await play(String(args[0]));
        return true;
      case "join": {
        const once = (force: boolean) => run<LaunchOutcome>("join_player", { place: Number(args[0]), job: null, account: null, name: null, force });
        handleLaunch(await once(false), () => once(true));
        return true;
      }
      case "toast":
        app.toast("neutral", `${p.name}: ${String(args[0]).slice(0, 200)}`);
        return true;
      case "navigate":
        app.navigate({ name: String(args[0]) } as Page);
        return true;
      case "runMacro": {
        const macro = app.snap!.bootstrapper.preferences.macros.find((m) => m.name.toLowerCase() === String(args[0]).toLowerCase());
        if (!macro) throw new Error("No macro by that name.");
        await invoke("run_macro", { id: macro.id });
        return true;
      }
      case "openLink":
        await invoke("open_url", { url: String(args[0]) });
        return true;
    }
    return null;
  }

  function onmessage(event: MessageEvent) {
    if (!frame || event.source !== frame.contentWindow || !plugin) return;
    const m = event.data;
    if (!m || typeof m !== "object") return;
    if (m.pious === "ready") {
      post({ pious: "theme", vars: theme() });
    } else if (m.pious === "call") {
      handle(String(m.method), Array.isArray(m.args) ? m.args : [])
        .then((value) => post({ pious: "reply", id: m.id, value }))
        .catch((e) => post({ pious: "reply", id: m.id, error: String(e?.message ?? e) }));
    } else if (m.pious === "subscribe" && m.event === "snapshot" && plugin.permissions.includes("read")) {
      subscribed = true;
    }
  }

  // Live updates for plugins that asked for them.
  $effect(() => {
    app.snap;
    if (subscribed) post({ pious: "event", event: "snapshot", value: view() });
  });
</script>

<svelte:window {onmessage} />

{#if !plugin}
  <EmptyState icon="plugins" title="This plugin is off" body="Turn it on in Settings → Plugins." />
{:else if !src}
  <EmptyState icon="plugins" title="{plugin.name} has no page" body="It works in the background." />
{:else}
  {#key src}
    <iframe bind:this={frame} {src} title={plugin.name} sandbox="allow-scripts allow-forms allow-popups" class="frame"></iframe>
  {/key}
{/if}

<style>
  .frame {
    width: 100%;
    height: calc(100vh - 130px);
    border: none;
    border-radius: var(--r-lg);
    background: transparent;
    animation: rise 300ms var(--ease) both;
  }
</style>
