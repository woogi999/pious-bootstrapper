<!-- A drop-down list. The menu is moved to <body> and positioned against
     the window, so no dialog or card around it can clip or offset it (a
     transform or backdrop-filter on any parent would otherwise make its
     fixed position relative to that parent). -->
<script lang="ts" generics="T">
  import Icon from "./Icon.svelte";

  let {
    options,
    value,
    onchange,
    placeholder = "Choose…",
    width = "100%",
  }: {
    options: { value: T; label: string }[];
    value: T;
    onchange: (value: T) => void;
    placeholder?: string;
    width?: string;
  } = $props();

  let open = $state(false);
  let field: HTMLButtonElement;
  let menu = $state({ x: 0, y: 0, width: 0 });

  const same = (a: T, b: T) => JSON.stringify(a) === JSON.stringify(b);
  const selected = $derived(options.find((o) => same(o.value, value)));

  function toggle() {
    if (!open) {
      const rect = field.getBoundingClientRect();
      const height = Math.min(options.length, 8) * 30 + 10;
      const up = rect.bottom + height + 8 > window.innerHeight;
      menu = { x: rect.left, y: up ? rect.top - height - 6 : rect.bottom + 6, width: rect.width };
    }
    open = !open;
  }

  function pick(option: { value: T }) {
    open = false;
    onchange(option.value);
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function key(event: KeyboardEvent) {
    if (open && event.key === "Escape") {
      event.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onkeydowncapture={key} />

<button bind:this={field} class="dropdown input" class:open style="width: {width}" onclick={toggle} type="button">
  <span class="line grow" class:placeholder={!selected}>{selected?.label ?? placeholder}</span>
  <span class="chevron"><Icon name="caret-down" size={13} /></span>
</button>

{#if open}
  <button
    use:portal
    class="backdrop"
    aria-label="Close"
    onclick={() => (open = false)}
    onwheel={(e) => {
      open = false;
      document.getElementById("page-scroll")?.scrollBy({ top: e.deltaY });
    }}
  ></button>
  <div use:portal class="menu panel" style="left: {menu.x}px; top: {menu.y}px; width: {menu.width}px">
    {#each options as option, i (i)}
      <button class="option" class:selected={same(option.value, value)} onclick={() => pick(option)}>
        <span class="line">{option.label}</span>
        {#if same(option.value, value)}<span class="dot"></span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .dropdown {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    cursor: pointer;
  }
  .placeholder {
    color: rgb(var(--faint));
  }
  .chevron {
    display: inline-flex;
    color: rgb(var(--muted));
    transition: transform var(--med) var(--ease);
  }
  .dropdown.open .chevron {
    transform: rotate(180deg);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: none;
    border: none;
  }
  .menu {
    position: fixed;
    z-index: 91;
    max-height: 250px;
    overflow-y: auto;
    padding: 5px;
    background: rgb(var(--panel));
    border-radius: var(--r-md);
    animation: fade 140ms var(--ease);
  }
  .option {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 11px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: rgb(var(--muted));
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }
  .option:hover {
    background: rgb(var(--surface) / 0.08);
    color: rgb(var(--text));
  }
  .option.selected {
    color: rgb(var(--text));
    font-weight: 600;
  }
  .dot {
    margin-left: auto;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgb(var(--accent));
    flex: none;
  }
</style>
