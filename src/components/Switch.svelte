<script lang="ts">
  let { on, onchange, disabled = false }: { on: boolean; onchange: (on: boolean) => void; disabled?: boolean } = $props();
</script>

<button class="switch" class:on {disabled} role="switch" aria-checked={on} aria-label="Toggle" onclick={() => onchange(!on)}>
  <span></span>
</button>

<style>
  .switch {
    position: relative;
    width: 36px;
    height: 20px;
    flex: none;
    border-radius: 99px;
    border: 1px solid rgb(var(--surface) / 0.12);
    background: rgb(var(--surface) / 0.08);
    cursor: pointer;
    transition: background var(--med) var(--ease), border-color var(--med);
  }
  .switch.on {
    background: rgb(var(--accent));
    border-color: transparent;
  }
  .switch:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: rgb(var(--text));
    transition: transform var(--med) var(--ease), background var(--med);
  }
  .on span {
    transform: translateX(16px);
    background: rgb(var(--accent-ink));
  }
</style>
