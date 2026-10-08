<!-- The first-run tour: what Pious does and how to start, in a few slides. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import { app } from "../lib/state.svelte";
  import { setPreferences } from "../lib/api";
  import { keyLabel } from "../lib/keys";
  import { keysFor } from "../lib/shortcuts";
  import Icon from "../components/Icon.svelte";
  import logo from "../../assets/brand/logo.svg?raw";

  const prefs = $derived(app.snap!.bootstrapper.preferences);
  const hasAccount = $derived(app.snap!.bootstrapper.accounts.length > 0);
  let step = $state(0);
  let direction = $state(1);

  const SLIDES = [
    { icon: "", title: "Welcome to Pious", body: "Your Roblox games, accounts, private servers, friends and recordings, all in one place." },
    { icon: "accounts", title: "Bring your accounts", body: "Add each Roblox account once. Play picks the account, Roblox version and server you want, every time." },
    { icon: "game", title: "Your games, ready to play", body: "Add games from a link or by searching. Save private servers, hop servers, or join friends straight from Pious." },
    { icon: "launch", title: "Pious comes into the game", body: "" },
    { icon: "palette", title: "Make it yours", body: "" },
  ];

  function go(to: number) {
    direction = to > step ? 1 : -1;
    step = Math.max(0, Math.min(SLIDES.length - 1, to));
  }
  function finish(then?: () => void) {
    app.modal = null;
    then?.();
  }
  // However it's closed, don't show it again.
  onDestroy(() => {
    if (!app.snap?.bootstrapper.preferences.onboarded) setPreferences({ onboarded: true, seen_version: app.snap?.current_version ?? "" });
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "ArrowRight") go(step + 1);
    if (e.key === "ArrowLeft") go(step - 1);
  }}
/>

<div class="frame panel welcome">
  <button class="icon-btn skip" aria-label="Skip" title="Skip the tour" onclick={() => finish()}><Icon name="close" /></button>
  <div class="stage">
    {#key step}
      <div class="slide" in:fly={{ x: 40 * direction, duration: 320, easing: cubicOut }} out:fly={{ x: -40 * direction, duration: 200 }}>
        {#if step === 0}
          <span class="logo">{@html logo}</span>
        {:else}
          <span class="icon"><Icon name={SLIDES[step].icon} size={26} /></span>
        {/if}
        <h2>{SLIDES[step].title}</h2>
        {#if step === 3}
          <div class="keys stagger">
            <span><span class="kbd">{keyLabel(prefs.overlay.hotkey)}</span>opens the overlay over the game</span>
            <span><span class="kbd">{keyLabel(prefs.recorder.record_hotkey)}</span>records (turn it on in Settings → Recording)</span>
            <span><span class="kbd">{keyLabel(prefs.recorder.clip_hotkey)}</span>clips the last {prefs.recorder.clip_seconds} seconds</span>
            <span><span class="kbd">Plugins</span>Macros and an auto-clicker come with Pious: turn them on in Settings → Plugins</span>
          </div>
        {:else if step === 4}
          <div class="keys stagger">
            <span><span class="kbd">{keyLabel(keysFor("search"))}</span>searches everything</span>
            <span><span class="kbd">{keyLabel(keysFor("streamer"))}</span>hides names when you stream</span>
            <span><Icon name="pin-window" />The logo takes you home; change what it does in Settings</span>
            <span><Icon name="palette" />Colors, blur and a background picture in Settings → Appearance</span>
          </div>
        {:else}
          <p>{SLIDES[step].body}</p>
        {/if}
        {#if step === 1}
          <button class="btn primary" onclick={() => finish(() => (app.modal = { kind: "add_account", reauth: null }))}>
            <Icon name="add-account" />{hasAccount ? "Add another account" : "Add an account"}
          </button>
        {:else if step === 2}
          <button class="btn primary" onclick={() => finish(() => (app.modal = { kind: "add_game" }))}><Icon name="add" />Add a game</button>
        {/if}
      </div>
    {/key}
  </div>
  <footer>
    <div class="dots">
      {#each SLIDES as _, i (i)}
        <button class="dot" class:on={i === step} aria-label="Slide {i + 1}" onclick={() => go(i)}></button>
      {/each}
    </div>
    <span class="spacer"></span>
    {#if step > 0}<button class="btn tertiary" onclick={() => go(step - 1)}>Back</button>{/if}
    {#if step < SLIDES.length - 1}
      <button class="btn primary" onclick={() => go(step + 1)}>Next<Icon name="arrow-right" /></button>
    {:else}
      <button class="btn primary" onclick={() => finish()}><Icon name="check" />Start using Pious</button>
    {/if}
  </footer>
</div>

<style>
  .welcome {
    position: relative;
    width: 560px;
    max-width: calc(100vw - 48px);
    padding: 28px 28px 20px;
    border-radius: var(--r-xl);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .skip {
    position: absolute;
    top: 12px;
    right: 12px;
  }
  .stage {
    display: grid;
    min-height: 290px;
  }
  .slide {
    grid-area: 1 / 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    text-align: center;
  }
  .logo {
    display: grid;
    width: 84px;
    height: 84px;
    color: rgb(var(--text));
    animation: arrive 900ms cubic-bezier(0.22, 1, 0.36, 1) both;
    filter: drop-shadow(0 14px 30px rgb(0 0 0 / 0.45));
  }
  .logo :global(svg) {
    width: 100%;
    height: 100%;
  }
  @keyframes arrive {
    from {
      opacity: 0;
      transform: rotateY(180deg) scale(0.7);
    }
  }
  .icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgb(var(--surface) / 0.07);
    color: rgb(var(--text));
    animation: pop 360ms var(--ease) both;
  }
  h2 {
    margin: 0;
    font-size: 24px;
    font-weight: 800;
  }
  p {
    margin: 0;
    max-width: 400px;
    color: rgb(var(--muted));
    line-height: 1.55;
    font-size: 13.5px;
  }
  .keys {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
    text-align: left;
    color: rgb(var(--muted));
    font-size: 13px;
  }
  .keys > span {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .keys .kbd {
    min-width: 54px;
    text-align: center;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dots {
    display: flex;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    padding: 0;
    border: none;
    border-radius: 99px;
    background: rgb(var(--surface) / 0.2);
    cursor: pointer;
    transition: width 260ms var(--ease), background 200ms;
  }
  .dot.on {
    width: 22px;
    background: rgb(var(--accent));
  }
</style>
