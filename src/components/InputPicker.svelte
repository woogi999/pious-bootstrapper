<!-- One key, mouse button or wheel notch: click, then press, click or
     scroll. Shows the name of what was picked. -->
<script lang="ts">
  import { keyName } from "../lib/inputLayouts";

  let {
    value,
    onchange,
    wheel = false,
    placeholder = "Pick",
    width = "130px",
  }: { value: string; onchange: (code: string) => void; wheel?: boolean; placeholder?: string; width?: string } = $props();

  let listening = $state(false);
  let button = $state<HTMLButtonElement>();
  // The click that starts listening isn't itself picked.
  let armed = false;

  const MOUSE = ["MouseLeft", "MouseMiddle", "MouseRight", "MouseBack", "MouseForward"];

  function done(code: string) {
    onchange(code);
    listening = false;
    button?.blur();
  }

  function start() {
    listening = !listening;
    armed = false;
    if (listening) setTimeout(() => (armed = true), 0);
  }

  function keydown(event: KeyboardEvent) {
    if (!listening) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.code === "Escape") listening = false;
    else done(event.code);
  }

  function mousedown(event: MouseEvent) {
    if (!listening || !armed) return;
    event.preventDefault();
    event.stopPropagation();
    done(MOUSE[event.button] ?? "MouseLeft");
  }

  function scroll(event: WheelEvent) {
    if (!listening || !wheel) return;
    event.preventDefault();
    done(event.deltaY < 0 ? "WheelUp" : "WheelDown");
  }
</script>

<svelte:window onkeydowncapture={keydown} onmousedowncapture={mousedown} />

<button
  bind:this={button}
  type="button"
  class="hotkey input"
  class:listening
  style="width: {width}"
  onclick={start}
  onblur={() => (listening = false)}
  onwheel={scroll}
  oncontextmenu={(e) => listening && e.preventDefault()}
  title={listening ? "Press a key, click a mouse button" + (wheel ? " or scroll" : "") : "Click, then press a key or a mouse button"}
>
  {#if listening}Press or click…{:else if value}{keyName(value)}{:else}<span class="secondary">{placeholder}</span>{/if}
</button>
