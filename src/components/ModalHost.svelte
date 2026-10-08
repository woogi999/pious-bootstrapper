<script lang="ts">
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import { app, type Modal } from "../lib/state.svelte";
  import AddAccount from "../dialogs/AddAccount.svelte";
  import AddGame from "../dialogs/AddGame.svelte";
  import JoinPlayer from "../dialogs/JoinPlayer.svelte";
  import JoinLink from "../dialogs/JoinLink.svelte";
  import Launch from "../dialogs/Launch.svelte";
  import ServerForm from "../dialogs/ServerForm.svelte";
  import SmallDialogs from "../dialogs/SmallDialogs.svelte";
  import ServerBrowser from "../dialogs/ServerBrowser.svelte";
  import RobloxSettings from "../dialogs/RobloxSettings.svelte";
  import Welcome from "../dialogs/Welcome.svelte";
  import WhatsNew from "../dialogs/WhatsNew.svelte";

  // The last dialog stays drawn while it animates out.
  let last = $state<Modal | null>(null);
  $effect.pre(() => {
    if (app.modal) last = app.modal;
  });
</script>

{#if app.modal && last}
  {@const modal = last}
  <div class="layer">
    <button class="scrim" aria-label="Close dialog" onclick={() => (app.modal = null)} transition:fade|global={{ duration: 200 }}></button>
    <div class="center">
      {#key modal}
        <div class="slot" in:fly|global={{ y: 14, duration: 260, easing: cubicOut }} out:fade|global={{ duration: 140 }}>
        {#if modal.kind === "add_game"}
          <AddGame />
        {:else if modal.kind === "server"}
          <ServerForm editing={modal.editing} game={modal.game} />
        {:else if modal.kind === "add_account"}
          <AddAccount reauth={modal.reauth} />
        {:else if modal.kind === "launch"}
          <Launch {modal} />
        {:else if modal.kind === "join_player"}
          <JoinPlayer />
        {:else if modal.kind === "join_link"}
          <JoinLink />
        {:else if modal.kind === "servers"}
          <ServerBrowser game={modal.game} />
        {:else if modal.kind === "roblox_settings"}
          <RobloxSettings account={modal.account} />
        {:else if modal.kind === "welcome"}
          <Welcome />
        {:else if modal.kind === "whats_new"}
          <WhatsNew />
        {:else}
          <SmallDialogs {modal} />
        {/if}
        </div>
      {/key}
    </div>
  </div>
{/if}

<style>
  .layer {
    position: fixed;
    inset: 34px 0 0 0;
    z-index: 80;
  }
  .scrim {
    position: absolute;
    inset: 0;
    border: none;
    background: rgb(0 0 0 / 0.5);
    backdrop-filter: blur(calc(var(--search-blur) * 10px));
  }
  .slot {
    grid-area: 1 / 1;
  }
  .center {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .center :global(.frame) {
    pointer-events: auto;
  }
</style>
