<!-- The custom title bar. Dragging anywhere but the buttons moves the window.
     In fullscreen it's hidden, leaving only a fullscreen toggle at the top right.
     Keep on top lives in Settings and its shortcut (Ctrl T). -->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { app, PAGE_TITLES } from "../lib/state.svelte";
  import { keyLabel } from "../lib/keys";
  import { keysFor, toggleFullscreen } from "../lib/shortcuts";
  import Icon from "./Icon.svelte";

  const win = getCurrentWindow();
  const title = $derived(
    app.page.name === "plugin" ? (app.snap?.plugins.find((p) => app.page.name === "plugin" && p.id === app.page.id)?.name ?? "Plugin") : PAGE_TITLES[app.page.name],
  );
  const hint = (id: string) => (keysFor(id) ? ` (${keyLabel(keysFor(id))})` : "");
</script>

{#if app.fullscreen}
  <button class="exit-full glass-elevated" aria-label="Exit fullscreen" title={"Exit fullscreen" + hint("fullscreen")} onclick={toggleFullscreen}>
    <Icon name="exit-fullscreen" size={14} />
  </button>
{:else}
  <div class="bar" data-tauri-drag-region>
    <span class="title" data-tauri-drag-region>Pious <span class="sep">·</span> {title}</span>
    <div class="controls">
      <button aria-label="Minimize" title="Minimize" onclick={() => win.minimize()}><Icon name="minimize" size={15} /></button>
      <button aria-label="Fullscreen" title={"Fullscreen" + hint("fullscreen")} onclick={toggleFullscreen}><Icon name="fullscreen" size={14} /></button>
      <button aria-label="Maximize" title={app.maximized ? "Restore" : "Maximize"} onclick={() => win.toggleMaximize()}>
        <Icon name={app.maximized ? "restore" : "maximize"} size={14} />
      </button>
      <button class="close" aria-label="Close" title="Close" onclick={() => invoke("close_window")}><Icon name="window-close" size={16} /></button>
    </div>
  </div>
{/if}

<style>
  .bar {
    /* Above the window's backdrop layer. */
    position: relative;
    display: flex;
    align-items: center;
    height: 34px;
    flex: none;
    background: rgb(var(--bg) / 0.35);
    border-bottom: 1px solid rgb(var(--surface) / 0.05);
  }
  .title {
    flex: 1;
    padding: 0 16px;
    font-size: 11.5px;
    font-weight: 600;
    color: rgb(var(--faint));
  }
  .sep {
    margin: 0 2px;
    opacity: 0.6;
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .controls button {
    display: grid;
    place-items: center;
    width: 44px;
    height: 100%;
    border: none;
    background: none;
    color: rgb(var(--muted));
    cursor: pointer;
    transition: background var(--fast), color var(--fast);
  }
  .controls button:hover {
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--text));
  }
  .exit-full {
    position: fixed;
    top: 10px;
    right: 12px;
    z-index: 60;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--r-md);
    color: rgb(var(--muted));
    cursor: pointer;
    opacity: 0.55;
    animation: rise 260ms var(--ease) both;
    transition: opacity var(--fast), color var(--fast), transform var(--fast) var(--ease);
  }
  .exit-full:hover {
    opacity: 1;
    color: rgb(var(--text));
  }
  .exit-full:active {
    transform: scale(0.94);
  }
  .controls .close:hover {
    background: #c42b1c;
    color: white;
  }
</style>
