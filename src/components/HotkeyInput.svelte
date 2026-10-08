<!-- Click, then press the keys you want (in combination). Esc cancels,
     Backspace clears. -->
<script lang="ts">
  import { fromEvent, keyLabel } from "../lib/keys";

  let {
    value,
    onchange,
    clearable = false,
    width = "170px",
  }: { value: string; onchange: (combo: string) => void; clearable?: boolean; width?: string } = $props();

  let listening = $state(false);
  let held = $state("");
  let button: HTMLButtonElement;

  function keydown(event: KeyboardEvent) {
    if (!listening) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      stop();
      return;
    }
    if (event.key === "Backspace" && clearable && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      onchange("");
      stop();
      return;
    }
    const combo = fromEvent(event);
    if (!combo) {
      // Only modifiers so far: show them while they're held.
      held = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Win"].filter(Boolean).join(" + ");
      return;
    }
    onchange(combo);
    stop();
  }

  function stop() {
    listening = false;
    held = "";
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
  onblur={() => stop()}
  title={listening ? "Press the keys, Esc to cancel" : "Click to change"}
>
  {#if listening}
    <span class="pulse-dot"></span><span class="line">{held ? `${held} + …` : "Press keys…"}</span>
  {:else}
    <span class="line">{keyLabel(value)}</span>
  {/if}
</button>

<style>
  .hotkey {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-weight: 600;
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }
  .hotkey.listening {
    border-color: rgb(var(--accent) / 0.65);
    box-shadow: 0 0 0 3px rgb(var(--accent) / 0.12);
    color: rgb(var(--muted));
  }
  .pulse-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(var(--accent));
    animation: pulse 1.2s ease-in-out infinite;
    flex: none;
  }
</style>
