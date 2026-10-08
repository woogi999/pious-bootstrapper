<!-- Shows text in the font a tweak would give Roblox: a preset (downloaded
     or one of Roblox's own) or the user's file. -->
<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";

  let { preset, file }: { preset: string | null; file: string | null } = $props();

  let family = $state<string | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  const loaded = new Map<string, string>();
  let next = 0;

  $effect(() => {
    const wanted = preset ?? (file ? `file:${file}` : "roblox:BuilderSans-Medium.otf");
    load(wanted);
  });

  async function load(wanted: string) {
    error = null;
    const known = loaded.get(wanted);
    if (known) {
      family = known;
      return;
    }
    loading = true;
    const ticket = ++next;
    try {
      const path = wanted.startsWith("file:") ? wanted.slice(5) : await invoke<string>("font_preview", { preset: wanted });
      const name = `preview-${ticket}`;
      const face = new FontFace(name, `url("${convertFileSrc(path)}")`);
      await face.load();
      document.fonts.add(face);
      loaded.set(wanted, name);
      if (ticket === next) family = name;
    } catch (e) {
      if (ticket === next) error = String(e).includes("Install") ? String(e) : "Couldn't load this font to preview it.";
    } finally {
      if (ticket === next) loading = false;
    }
  }
</script>

<div class="preview glass-base" class:loading>
  {#if error}
    <span class="meta">{error}</span>
  {:else}
    {#key family}
      <div class="sample" style="font-family: {family ? `'${family}', ` : ''}system-ui">
        <span class="big">The quick brown fox jumps over the lazy dog</span>
        <span class="small">ABCDEFGHIJKLM nopqrstuvwxyz 0123456789 !?&amp;</span>
        <span class="chat"><b>[Server]</b> Welcome! Press <span class="key">E</span> to interact · Play · Shop · Settings</span>
      </div>
    {/key}
  {/if}
</div>

<style>
  .preview {
    padding: 14px 16px;
    border-radius: var(--r-md);
    min-height: 98px;
    transition: opacity var(--med);
  }
  .preview.loading {
    opacity: 0.55;
  }
  .sample {
    display: flex;
    flex-direction: column;
    gap: 6px;
    animation: rise 260ms var(--ease) both;
  }
  .big {
    font-size: 22px;
    line-height: 1.2;
  }
  .small {
    font-size: 14px;
    color: rgb(var(--muted));
  }
  .chat {
    font-size: 13px;
    color: rgb(var(--muted));
  }
  .key {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    padding: 0 4px;
    border-radius: 4px;
    background: rgb(var(--surface) / 0.12);
    color: rgb(var(--text));
  }
</style>
