<!-- A slider whose number can be clicked and typed into. `scale` turns the
     value into what's shown (100 for percentages). -->
<script lang="ts">
  let {
    value,
    min,
    max,
    step = 0.01,
    scale = 100,
    unit = "%",
    digits = 0,
    disabled = false,
    width = "200px",
    onchange,
    oncommit,
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    scale?: number;
    unit?: string;
    digits?: number;
    disabled?: boolean;
    width?: string;
    onchange?: (value: number) => void;
    /** Called once when dragging ends (or a typed value is entered). */
    oncommit?: (value: number) => void;
  } = $props();

  let editing = $state(false);
  let text = $state("");
  let field = $state<HTMLInputElement>();

  const shown = $derived((value * scale).toFixed(digits));

  function edit() {
    if (disabled) return;
    text = shown;
    editing = true;
    queueMicrotask(() => field?.select());
  }

  function commit() {
    if (!editing) return;
    editing = false;
    const typed = Number(text.replace(",", ".").replace(/[^\d.\-]/g, ""));
    if (!Number.isFinite(typed) || text.trim() === "") return;
    const next = Math.min(max, Math.max(min, typed / scale));
    onchange?.(next);
    oncommit?.(next);
  }
</script>

<div class="slider" class:disabled>
  <input
    type="range"
    {min}
    {max}
    {step}
    {value}
    {disabled}
    style="width: {width}"
    oninput={(e) => onchange?.(Number(e.currentTarget.value))}
    onchange={(e) => oncommit?.(Number(e.currentTarget.value))}
  />
  {#if editing}
    <input
      bind:this={field}
      class="number input"
      bind:value={text}
      onblur={commit}
      onkeydown={(e) => {
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") {
          e.stopPropagation();
          editing = false;
        }
      }}
    />
  {:else}
    <button class="number value" type="button" {disabled} title="Click to type a value" onclick={edit}>{shown}{unit}</button>
  {/if}
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .number {
    width: 58px;
    height: 26px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .value {
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    background: none;
    color: rgb(var(--muted));
    cursor: text;
    padding: 0 6px;
    transition: border-color var(--fast), background var(--fast), color var(--fast);
  }
  .value:hover:not(:disabled) {
    border-color: rgb(var(--surface) / calc(0.12 * var(--glass)));
    background: rgb(var(--surface) / calc(0.05 * var(--glass)));
    color: rgb(var(--text));
  }
  input.number {
    padding: 0 6px;
  }
  .disabled {
    opacity: 0.6;
  }
</style>
