<!-- The little notice over the game: a recording indicator and short notes
     like "Clip saved". It can't be clicked and never shows up in videos. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Icon from "./components/Icon.svelte";

  type Note = { id: number; kind: string; text: string };
  let notes = $state<Note[]>([]);
  let recording = $state<number | null>(null);
  let next = 0;
  let idle: ReturnType<typeof setTimeout> | undefined;

  function settle() {
    clearTimeout(idle);
    idle = setTimeout(() => {
      if (!notes.length && recording === null) invoke("hide_hud");
    }, 400);
  }

  onMount(() => {
    const offNote = listen<{ kind: string; text: string }>("hud", (event) => {
      const { kind, text } = event.payload;
      if (kind === "recording") {
        recording = 0;
        return;
      }
      if (kind === "saved" || kind === "error") recording = null;
      // "Saving…" is replaced by what happened.
      notes = [...notes.filter((n) => n.kind !== "saving"), { id: ++next, kind, text }];
      const id = next;
      if (kind !== "saving") {
        setTimeout(() => {
          notes = notes.filter((n) => n.id !== id);
          settle();
        }, 2600);
      }
    });
    const offTick = listen<number>("hud-recording", (event) => (recording = event.payload));
    return () => {
      offNote.then((f) => f());
      offTick.then((f) => f());
    };
  });

  const time = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
</script>

<div class="hud">
  {#if recording !== null}
    <div class="pill rec"><span class="dot"></span>REC {time(recording)}</div>
  {/if}
  {#each notes as note (note.id)}
    <div class="pill" class:error={note.kind === "error"}>
      <Icon name={note.kind === "error" ? "warning" : note.kind === "saving" ? "clip" : note.kind === "macro" ? "macros" : "success"} size={14} />{note.text}
    </div>
  {/each}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .hud {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    padding: 4px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border-radius: 99px;
    background: rgb(12 12 14 / 0.86);
    border: 1px solid rgb(255 255 255 / 0.12);
    color: #f2f2f3;
    font-weight: 700;
    font-size: 13px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
    animation: pop 260ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .pill.error {
    border-color: rgb(229 72 77 / 0.6);
  }
  .rec {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.04em;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #e5484d;
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-8px) scale(0.96);
    }
  }
</style>
