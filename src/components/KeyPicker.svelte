<!-- One key (not a combination): click, then press it. -->
<script lang="ts">
  import { keyLabel } from "../lib/keys";

  let { value, onchange, width = "120px" }: { value: string; onchange: (code: string) => void; width?: string } = $props();

  let listening = $state(false);
  let button: HTMLButtonElement;

  function keydown(event: KeyboardEvent) {
    if (!listening) return;
    event.preventDefault();
    event.stopPropagation();
    onchange(event.code);
    listening = false;
    button?.blur();
  }
</script>

<svelte:window onkeydowncapture={keydown} />

<button
  bind:this={button}
  type="button"
  class="hotkey input"
  class:listening
  style="width: {width}"
  onclick={() => (listening = !listening)}
  onblur={() => (listening = false)}
  title={listening ? "Press a key" : "Click, then press a key"}
>
  {#if listening}<span class="dot"></span>Press a key…{:else}{keyLabel(value) || "Pick a key"}{/if}
</button>

<style>
  .hotkey {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    font-weight: 600;
    font-size: 12.5px;
  }
  .hotkey.listening {
    border-color: rgb(var(--accent) / 0.65);
    box-shadow: 0 0 0 3px rgb(var(--accent) / 0.12);
    color: rgb(var(--muted));
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(var(--accent));
    animation: pulse 1.2s ease-in-out infinite;
  }
</style>
