<!-- The steps of a macro, each edited in place. Loops hold steps of their
     own, shown by this same list, indented. -->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { app } from "../lib/state.svelte";
  import type { MouseButton, Press, Step } from "../lib/types";
  import Icon from "./Icon.svelte";
  import KeyPicker from "./KeyPicker.svelte";
  import PointPicker from "./PointPicker.svelte";
  import Select from "./Select.svelte";
  import StepList from "./StepList.svelte";
  import Switch from "./Switch.svelte";

  let { steps, onchange, depth = 0 }: { steps: Step[]; onchange: (steps: Step[]) => void; depth?: number } = $props();

  // Steps have no IDs of their own; these keep the list's animation steady.
  let ids: number[] = [];
  let next = 0;
  const keyed = $derived.by(() => {
    while (ids.length < steps.length) ids.push(next++);
    ids.length = steps.length;
    return steps.map((step, i) => ({ step, id: ids[i] }));
  });

  const KINDS: { kind: Step["kind"]; icon: string; label: string; make: () => Step }[] = [
    { kind: "Key", icon: "keyboard", label: "Press a key", make: () => ({ kind: "Key", key: "KeyE", press: "Tap", hold_ms: 50 }) },
    { kind: "Text", icon: "text", label: "Type text", make: () => ({ kind: "Text", text: "", delay_ms: 20 }) },
    { kind: "Click", icon: "autoclick", label: "Click", make: () => ({ kind: "Click", button: "Left", press: "Tap", count: 1 }) },
    { kind: "Move", icon: "target", label: "Move the pointer", make: () => ({ kind: "Move", x: 0, y: 0, relative: false, duration_ms: 0 }) },
    { kind: "Scroll", icon: "sort", label: "Scroll", make: () => ({ kind: "Scroll", amount: -3, horizontal: false }) },
    { kind: "Wait", icon: "clock", label: "Wait", make: () => ({ kind: "Wait", ms: 500, random_ms: 0 }) },
    { kind: "Loop", icon: "refresh", label: "Repeat steps", make: () => ({ kind: "Loop", times: 5, steps: [] }) },
    { kind: "WaitPixel", icon: "eyedropper", label: "Wait for a color", make: () => ({ kind: "WaitPixel", x: 0, y: 0, color: "#FFFFFF", tolerance: 10, timeout_ms: 0 }) },
    { kind: "FocusRoblox", icon: "focus", label: "Bring Roblox forward", make: () => ({ kind: "FocusRoblox" }) },
    { kind: "Run", icon: "external", label: "Open a program or link", make: () => ({ kind: "Run", target: "" }) },
    { kind: "Comment", icon: "edit", label: "Note", make: () => ({ kind: "Comment", text: "" }) },
  ];
  const meta = (kind: Step["kind"]) => KINDS.find((k) => k.kind === kind)!;

  const PRESS: { value: Press; label: string }[] = [
    { value: "Tap", label: "Press" },
    { value: "Down", label: "Hold down" },
    { value: "Up", label: "Let go" },
  ];
  const BUTTONS: { value: MouseButton; label: string }[] = [
    { value: "Left", label: "Left button" },
    { value: "Right", label: "Right button" },
    { value: "Middle", label: "Middle button" },
    { value: "Back", label: "Back button" },
    { value: "Forward", label: "Forward button" },
  ];

  function set(i: number, patch: Partial<Step>) {
    const copy = steps.slice();
    copy[i] = { ...copy[i], ...patch } as Step;
    onchange(copy);
  }
  function move(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= steps.length) return;
    const copy = steps.slice();
    [copy[i], copy[j]] = [copy[j], copy[i]];
    [ids[i], ids[j]] = [ids[j], ids[i]];
    onchange(copy);
  }
  function remove(i: number) {
    ids.splice(i, 1);
    onchange(steps.filter((_, k) => k !== i));
  }
  function duplicate(i: number) {
    ids.splice(i + 1, 0, next++);
    const copy = steps.slice();
    copy.splice(i + 1, 0, structuredClone($state.snapshot(steps[i])) as Step);
    onchange(copy);
  }
  function add(event: MouseEvent) {
    app.openMenu(
      event,
      KINDS.filter((k) => depth < 3 || k.kind !== "Loop").map((k) => ({
        icon: k.icon,
        label: k.label,
        action: () => onchange([...steps, k.make()]),
      })),
    );
  }
  const num = (e: Event, min = 0) => Math.max(min, Math.round(Number((e.currentTarget as HTMLInputElement).value) || 0));
</script>

<div class="steps" class:nested={depth > 0}>
  {#each keyed as { step, id }, i (id)}
    <div class="step glass-base" animate:flip={{ duration: 220 }}>
      <span class="n">{i + 1}</span>
      <span class="kind"><Icon name={meta(step.kind).icon} size={14} /></span>
      <div class="body">
        {#if step.kind === "Key"}
          <Select options={PRESS} value={step.press} onchange={(press) => set(i, { press })} width="120px" />
          <KeyPicker value={step.key} onchange={(key) => set(i, { key })} />
          {#if step.press === "Tap"}
            <label class="unit">for <input class="input num" type="number" min="0" value={step.hold_ms} onchange={(e) => set(i, { hold_ms: num(e) })} /> ms</label>
          {/if}
        {:else if step.kind === "Text"}
          <input class="input grow" placeholder="Text to type" value={step.text} onchange={(e) => set(i, { text: e.currentTarget.value })} />
          <label class="unit"><input class="input num" type="number" min="0" value={step.delay_ms} onchange={(e) => set(i, { delay_ms: num(e) })} /> ms per letter</label>
        {:else if step.kind === "Click"}
          <Select options={PRESS} value={step.press} onchange={(press) => set(i, { press })} width="120px" />
          <Select options={BUTTONS} value={step.button} onchange={(button) => set(i, { button })} width="150px" />
          {#if step.press === "Tap"}
            <label class="unit"><input class="input num" type="number" min="1" value={step.count} onchange={(e) => set(i, { count: num(e, 1) })} /> time(s)</label>
          {/if}
        {:else if step.kind === "Move"}
          <Select
            options={[
              { value: false, label: "To a spot" },
              { value: true, label: "By an amount" },
            ]}
            value={step.relative}
            onchange={(relative) => set(i, { relative })}
            width="140px"
          />
          <label class="unit">X <input class="input num" type="number" value={step.x} onchange={(e) => set(i, { x: Number(e.currentTarget.value) || 0 })} /></label>
          <label class="unit">Y <input class="input num" type="number" value={step.y} onchange={(e) => set(i, { y: Number(e.currentTarget.value) || 0 })} /></label>
          {#if !step.relative}<PointPicker onpick={(x, y) => set(i, { x, y })} />{/if}
          <label class="unit">over <input class="input num" type="number" min="0" value={step.duration_ms} onchange={(e) => set(i, { duration_ms: num(e) })} /> ms</label>
        {:else if step.kind === "Scroll"}
          <label class="unit"><input class="input num" type="number" value={step.amount} onchange={(e) => set(i, { amount: Number(e.currentTarget.value) || 0 })} /> notches</label>
          <span class="secondary">(negative scrolls down)</span>
          <label class="unit">Sideways <Switch on={step.horizontal} onchange={(horizontal) => set(i, { horizontal })} /></label>
        {:else if step.kind === "Wait"}
          <label class="unit"><input class="input num" type="number" min="0" value={step.ms} onchange={(e) => set(i, { ms: num(e) })} /> ms</label>
          <label class="unit">plus up to <input class="input num" type="number" min="0" value={step.random_ms} onchange={(e) => set(i, { random_ms: num(e) })} /> ms at random</label>
        {:else if step.kind === "Loop"}
          <label class="unit">Repeat <input class="input num" type="number" min="0" value={step.times} onchange={(e) => set(i, { times: num(e) })} /> times</label>
          <span class="secondary">{step.times === 0 ? "(until stopped)" : "(0 = until stopped)"}</span>
        {:else if step.kind === "WaitPixel"}
          <label class="unit">X <input class="input num" type="number" value={step.x} onchange={(e) => set(i, { x: Number(e.currentTarget.value) || 0 })} /></label>
          <label class="unit">Y <input class="input num" type="number" value={step.y} onchange={(e) => set(i, { y: Number(e.currentTarget.value) || 0 })} /></label>
          <label class="unit">is <span class="swatch" style="background: {step.color}"></span><input class="input hex" value={step.color} onchange={(e) => set(i, { color: e.currentTarget.value.trim() })} /></label>
          <PointPicker label="Pick spot and color" onpick={(x, y, color) => set(i, { x, y, color: color ?? step.color })} />
          <label class="unit">± <input class="input num" type="number" min="0" max="255" value={step.tolerance} onchange={(e) => set(i, { tolerance: Math.min(255, num(e)) })} /></label>
          <label class="unit">give up after <input class="input num" type="number" min="0" value={step.timeout_ms} onchange={(e) => set(i, { timeout_ms: num(e) })} /> ms</label>
        {:else if step.kind === "FocusRoblox"}
          <span class="meta">Brings a Roblox window to the front.</span>
        {:else if step.kind === "Run"}
          <input class="input grow" placeholder="A program, file or https:// link" value={step.target} onchange={(e) => set(i, { target: e.currentTarget.value })} />
        {:else if step.kind === "Comment"}
          <input class="input grow note" placeholder="A note for yourself" value={step.text} onchange={(e) => set(i, { text: e.currentTarget.value })} />
        {/if}
      </div>
      <div class="tools">
        <button class="icon-btn" aria-label="Move up" title="Move up" disabled={i === 0} onclick={() => move(i, -1)}><Icon name="up" size={13} /></button>
        <button class="icon-btn" aria-label="Move down" title="Move down" disabled={i === steps.length - 1} onclick={() => move(i, 1)}><Icon name="down" size={13} /></button>
        <button class="icon-btn" aria-label="Duplicate" title="Duplicate" onclick={() => duplicate(i)}><Icon name="copy" size={13} /></button>
        <button class="icon-btn" aria-label="Remove" title="Remove" onclick={() => remove(i)}><Icon name="remove" size={13} /></button>
      </div>
      {#if step.kind === "Loop"}
        <div class="inner">
          <StepList steps={step.steps} depth={depth + 1} onchange={(inner) => set(i, { steps: inner })} />
        </div>
      {/if}
    </div>
  {/each}
  <button class="add" onclick={add}><Icon name="add" size={14} />{depth ? "Add a step inside" : "Add a step"}</button>
</div>

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .step {
    display: grid;
    grid-template-columns: 22px 26px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 10px;
    border-radius: var(--r-md);
    animation: rise 240ms var(--ease) both;
  }
  .n {
    font-size: 11px;
    font-weight: 700;
    color: rgb(var(--faint));
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .kind {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    background: rgb(var(--surface) / 0.07);
    color: rgb(var(--muted));
  }
  .body {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    min-width: 0;
  }
  .unit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: rgb(var(--muted));
    white-space: nowrap;
  }
  .num {
    width: 74px;
    height: 30px;
  }
  .hex {
    width: 90px;
    height: 30px;
    font-family: ui-monospace, monospace;
  }
  .swatch {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1px solid rgb(var(--surface) / 0.2);
  }
  .note {
    font-style: italic;
  }
  .tools {
    display: flex;
    gap: 2px;
    opacity: 0.55;
    transition: opacity var(--fast);
  }
  .step:hover > .tools {
    opacity: 1;
  }
  .tools .icon-btn:disabled {
    opacity: 0.3;
    pointer-events: none;
  }
  .inner {
    grid-column: 2 / -1;
    padding-left: 10px;
    border-left: 2px solid rgb(var(--surface) / 0.08);
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
    padding: 7px 12px;
    border: 1px dashed rgb(var(--surface) / 0.18);
    border-radius: var(--r-md);
    background: none;
    color: rgb(var(--muted));
    font: inherit;
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
    transition: border-color var(--fast), color var(--fast), background var(--fast);
  }
  .add:hover {
    border-color: rgb(var(--accent) / 0.5);
    color: rgb(var(--text));
    background: rgb(var(--surface) / 0.03);
  }
  .nested .add {
    padding: 5px 10px;
  }
</style>
