<!-- Join a private server straight from its link, like Minecraft's Direct
     Connect. Saving it is optional. -->
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { call, handleLaunch, run } from "../lib/api";
  import { accountLabel, who } from "../lib/format";
  import type { LaunchOutcome, ServerLinkInfo, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import Switch from "../components/Switch.svelte";

  const snap = $derived(app.snap!);
  let link = $state("");
  let account = $state<Uuid | null>(app.snap!.active_account);
  let save = $state(false);
  let info = $state<ServerLinkInfo | null>(null);
  let checking = $state(false);
  let error = $state<string | null>(null);
  let joining = $state(false);

  // A link to one public server (from "Copy server link"): place + server.
  const serverLink = $derived.by(() => {
    const place = /[?&]placeId=(\d+)/i.exec(link)?.[1];
    const job = /[?&]gameInstanceId=([0-9a-f-]{8,})/i.exec(link)?.[1];
    return place && job ? { place: Number(place), job } : null;
  });
  const looksLikeLink = $derived(!!serverLink || /privateServerLinkCode=|roblox\.com\/share\?/i.test(link));
  const accounts = $derived(snap.bootstrapper.accounts.map((a) => ({ value: a.id as Uuid | null, label: `${accountLabel(a)} (@${who(a.username)})` })));

  // Look the link up a moment after it's pasted.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const value = link.trim();
    clearTimeout(timer);
    info = null;
    error = null;
    if (!looksLikeLink || serverLink) return;
    timer = setTimeout(async () => {
      checking = true;
      try {
        const found = await call<ServerLinkInfo>("server_link_info", { link: value });
        if (link.trim() === value) info = found;
      } catch (e) {
        if (link.trim() === value) error = String(e);
      } finally {
        checking = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });

  async function join() {
    if (!looksLikeLink || joining) return;
    joining = true;
    try {
      if (serverLink) {
        const { place, job } = serverLink;
        const once = (force: boolean) => run<LaunchOutcome>("join_player", { place, job, account, name: null, force });
        const outcome = await once(false);
        app.modal = null;
        handleLaunch(outcome, () => once(true));
        return;
      }
      if (save) {
        await call("save_server", { form: { editing: null, game: null, name: "", link: link.trim(), server_id: "", notes: "" } }).catch((e) =>
          app.toast("caution", `Not saved: ${e}`),
        );
      }
      const once = (force: boolean) => run<LaunchOutcome>("join_link", { link: link.trim(), account, force });
      const outcome = await once(false);
      app.modal = null;
      handleLaunch(outcome, () => once(true));
    } finally {
      joining = false;
    }
  }
</script>

<ModalFrame
  title="Join a Server Link"
  subtitle="Paste a private server link, or a link someone copied to their server, to join it right away. Nothing is saved unless you want it to be."
  width={520}
>
  <div class="field">
    <span class="label">Server link</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input class="input" autofocus placeholder="https://www.roblox.com/share?code=…" bind:value={link} onkeydown={(e) => e.key === "Enter" && join()} />
  </div>
  {#if serverLink}
    <div class="notice"><Icon name="globe" /><span>A public server in place {serverLink.place}. You'll join that exact server if it still has room.</span></div>
  {:else if checking}
    <div class="notice"><Icon name="refresh" />Looking up the server…</div>
  {:else if info}
    <div class="notice">
      <Icon name="server" />
      <span>
        {#if info.name}<b>{info.name}</b>{:else}A private server{/if}{#if info.game} in <b>{info.game}</b>{/if}
      </span>
    </div>
  {:else if error}
    <div class="notice caution"><Icon name="warning" />{error}</div>
  {:else if link.trim() && !looksLikeLink}
    <div class="notice caution"><Icon name="warning" />That doesn't look like a server link.</div>
  {/if}
  <div class="field">
    <span class="label">Join as</span>
    <Select options={accounts} value={account} onchange={(v) => (account = v)} placeholder="Choose an account" />
  </div>
  {#if !serverLink}
    <div class="row" style="gap: 10px">
      <span class="grow meta">Also save it to Private Servers</span>
      <Switch on={save} onchange={(on) => (save = on)} />
    </div>
  {/if}
  {#snippet footer()}
    <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    <button class="btn primary" disabled={!looksLikeLink || joining || !account} onclick={join}><Icon name="play" />{joining ? "Joining…" : "Join"}</button>
  {/snippet}
</ModalFrame>
