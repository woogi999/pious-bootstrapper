<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  function close() {
    app.menu = null;
  }

  // The wheel passes through to whatever is under the menu's backdrop
  // (it used to stop the page from scrolling until you clicked).
  function wheel(event: WheelEvent) {
    close();
    const under = document.elementsFromPoint(event.clientX, event.clientY).find((el) => !el.classList.contains("backdrop"));
    const scroller = under?.closest<HTMLElement>("#page-scroll, .scroll-list, .messages, .list, .body") ?? document.getElementById("page-scroll");
    scroller?.scrollBy({ top: event.deltaY, left: event.deltaX });
  }
</script>

{#if app.menu}
  <button class="backdrop" aria-label="Close menu" onclick={close} onwheel={wheel} oncontextmenu={(e) => (e.preventDefault(), close())}></button>
  <div class="menu panel" style="left: {app.menu.x}px; top: {app.menu.y}px" role="menu">
    {#each app.menu.items as item, i (i)}
      {#if item === "separator"}
        <hr class="divider" />
      {:else}
        <button
          class="item"
          class:danger={item.danger}
          role="menuitem"
          onclick={() => {
            close();
            item.action();
          }}
        >
          <Icon name={item.icon} size={14} />
          <span class="line">{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: none;
    border: none;
  }
  .menu {
    position: fixed;
    z-index: 101;
    width: 214px;
    padding: 5px;
    background: rgb(var(--panel));
    border-radius: var(--r-md);
    display: flex;
    flex-direction: column;
    gap: 1px;
    animation: pop 150ms var(--ease);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .menu hr {
    margin: 4px 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 30px;
    padding: 0 9px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: rgb(var(--text));
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }
  .item :global(.icon) {
    color: rgb(var(--muted));
  }
  .item:hover {
    background: rgb(var(--surface) / 0.08);
  }
  .item.danger {
    font-weight: 600;
  }
  .item.danger :global(.icon) {
    color: rgb(var(--text));
  }
</style>
