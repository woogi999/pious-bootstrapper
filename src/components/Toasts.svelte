<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  const icons = { positive: "success", negative: "error", caution: "warning", active: "launch", neutral: "info" };
</script>

<div class="toasts">
  {#each app.toasts as toast (toast.id)}
    <div class="toast panel" class:leaving={toast.leaving} role="status">
      <Icon name={icons[toast.tone]} size={16} />
      <span class="grow">{toast.message}</span>
      <button class="icon-btn" aria-label="Dismiss" onclick={() => app.dismiss(toast.id)}><Icon name="close" size={12} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 120;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 340px;
    padding: 9px 8px 9px 12px;
    border-radius: var(--r-md);
    font-size: 12.5px;
    pointer-events: auto;
    animation: slide 240ms var(--ease);
    transition: opacity 220ms var(--ease), transform 220ms var(--ease);
  }
  .toast.leaving {
    opacity: 0;
    transform: translateX(24px);
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(24px);
    }
  }
</style>
