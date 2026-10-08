<!-- A chat with one friend through Roblox's chat, signed in as `account`.
     Polls for new messages while open. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { app } from "../lib/state.svelte";
  import { call } from "../lib/api";
  import { accountLabel, who } from "../lib/format";
  import { emojiShortcodes } from "../lib/emoji";
  import type { ChatMessage, Friend, Uuid } from "../lib/types";
  import Icon from "./Icon.svelte";

  let {
    account,
    friend,
    onclose,
    compact = false,
  }: { account: Uuid; friend: Friend; onclose: () => void; compact?: boolean } = $props();

  const me = $derived(app.snap?.bootstrapper.accounts.find((a) => a.id === account));

  let conversation = $state<string | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let draft = $state("");
  let sending = $state(false);
  let error = $state<string | null>(null);
  let opening = $state(true);
  let scroller = $state<HTMLElement>();
  let poll: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    try {
      conversation = await call<string>("open_chat", { account, user: friend.id });
      await load(true);
      poll = setInterval(() => load(false), 4000);
    } catch (e) {
      error = String(e);
    } finally {
      opening = false;
    }
  });
  onDestroy(() => clearInterval(poll));

  async function load(scroll: boolean) {
    if (!conversation) return;
    try {
      const next = await call<ChatMessage[]>("chat_messages", { account, conversation });
      const atBottom = !scroller || scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
      if (JSON.stringify(next) !== JSON.stringify(messages)) {
        messages = next;
        if (scroll || atBottom) {
          await tick();
          scroller?.scrollTo({ top: scroller.scrollHeight });
        }
      }
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function send() {
    const text = draft.trim();
    if (!text || !conversation || sending) return;
    sending = true;
    try {
      await call("send_chat", { account, conversation, text });
      draft = "";
      await load(true);
    } catch (e) {
      error = String(e);
    } finally {
      sending = false;
    }
  }
</script>

<div class="chat" class:compact>
  <header class="row">
    <span class="face">{#if friend.avatar}<img src={friend.avatar} alt="" />{:else}{friend.display_name.charAt(0)}{/if}</span>
    <div class="col grow" style="gap: 0">
      <span class="item-title line">{who(friend.display_name)}</span>
      <span class="secondary line">@{who(friend.username)}</span>
    </div>
    <button class="icon-btn" aria-label="Close chat" onclick={onclose}><Icon name="close" /></button>
  </header>
  <div class="messages" bind:this={scroller}>
    {#if opening}
      <span class="meta center">Opening the chat…</span>
    {:else if error && !messages.length}
      <span class="meta center">{error}</span>
    {:else if !messages.length}
      <span class="meta center">No messages yet. Say hi!</span>
    {:else}
      {#each messages as m (m.id)}
        <div class="bubble pop-in" class:mine={m.sender === me?.user_id} title={m.sent ? new Date(m.sent).toLocaleString() : ""}>{m.content}</div>
      {/each}
    {/if}
  </div>
  {#if error && messages.length}<span class="secondary pad">{error}</span>{/if}
  <form
    class="compose"
    onsubmit={(e) => {
      e.preventDefault();
      send();
    }}
  >
    <input
      class="input"
      placeholder="Message {who(friend.display_name)}"
      bind:value={draft}
      use:emojiShortcodes={app.snap?.bootstrapper.preferences.emoji_shortcodes.in_app ?? true}
      disabled={!conversation}
      maxlength="500"
    />
    <button class="btn primary" type="submit" disabled={!draft.trim() || sending || !conversation} aria-label="Send"><Icon name="send" /></button>
  </form>
  {#if !compact}<span class="secondary hint">Sent through Roblox chat as {me ? accountLabel(me) : "your account"}. Roblox filters messages as usual.</span>{/if}
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  header {
    gap: 10px;
    padding: 10px 12px;
    border-bottom: 1px solid rgb(var(--surface) / calc(0.07 * var(--glass)));
  }
  .face {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    background: rgb(var(--surface) / 0.08);
    font-weight: 700;
  }
  .face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
  }
  .compact .messages {
    max-height: 260px;
    min-height: 140px;
  }
  .center {
    margin: auto;
    text-align: center;
  }
  .pop-in {
    animation: rise 240ms var(--ease) both;
  }
  .bubble {
    align-self: flex-start;
    max-width: 82%;
    padding: 7px 11px;
    border-radius: 14px 14px 14px 4px;
    background: rgb(var(--surface) / calc(0.08 * var(--glass)));
    font-size: 13px;
    user-select: text;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .bubble.mine {
    align-self: flex-end;
    border-radius: 14px 14px 4px 14px;
    background: rgb(var(--accent));
    color: rgb(var(--accent-ink));
  }
  .compose {
    display: flex;
    gap: 8px;
    padding: 10px 12px 8px;
  }
  .compose .btn {
    height: 34px;
  }
  .pad {
    padding: 0 12px;
  }
  .hint {
    padding: 0 12px 10px;
    font-size: 11px;
  }
</style>
