<script lang="ts">
  import { who } from "../lib/format";
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { call, run } from "../lib/api";
  import type { QuickLoginStatus, QuickLoginUpdate, Uuid } from "../lib/types";
  import ModalFrame from "../components/ModalFrame.svelte";
  import Icon from "../components/Icon.svelte";

  let { reauth }: { reauth: Uuid | null } = $props();

  const snap = $derived(app.snap!);
  const reauthAccount = $derived(reauth ? snap.bootstrapper.accounts.find((a) => a.id === reauth) : null);

  let step = $state<"choose" | "quick" | "token" | "browser">("choose");
  // From the browser
  let searching = $state(false);
  let problems = $state<string[]>([]);
  let adding = $state<number | null>(null);
  const sessions = $derived(snap.browser_sessions);

  async function searchBrowsers() {
    step = "browser";
    searching = true;
    problems = [];
    try {
      const result = await call<{ found: number; problems: string[] }>("find_browser_sessions");
      problems = result.problems;
    } catch (e) {
      problems = [String(e)];
    } finally {
      searching = false;
    }
  }

  async function addFound(user: number) {
    adding = user;
    try {
      await call("add_browser_session", { user, reauth });
      if (reauth) app.modal = null;
    } catch (e) {
      problems = [String(e), ...problems];
    } finally {
      adding = null;
    }
  }
  // Quick Login
  let code = $state<string | null>(null);
  let status = $state<QuickLoginStatus>({ kind: "Pending" });
  let quickError = $state<string | null>(null);
  let finishing = $state(false);
  let poll: ReturnType<typeof setInterval> | undefined;
  // Token
  let token = $state("");
  let tokenError = $state<string | null>(null);
  let busy = $state(false);

  // Close once the sign-in window has added the account.
  let sawSignIn = false;
  $effect(() => {
    if (snap.sign_in) sawSignIn = true;
    else if (sawSignIn) app.modal = null;
  });

  onDestroy(() => clearInterval(poll));

  async function startQuick() {
    step = "quick";
    code = null;
    quickError = null;
    finishing = false;
    status = { kind: "Pending" };
    clearInterval(poll);
    try {
      code = await call<string>("quick_login_start");
      poll = setInterval(checkQuick, 3000);
    } catch (e) {
      quickError = String(e);
    }
  }

  async function checkQuick() {
    try {
      const update = await call<QuickLoginUpdate>("quick_login_check", { reauth });
      if (update.kind === "done") {
        clearInterval(poll);
        app.modal = null;
      } else if (update.kind === "expired") {
        clearInterval(poll);
        status = update.status;
        code = null;
      } else {
        status = update.status;
        finishing = update.status.kind === "Validated";
      }
    } catch (e) {
      clearInterval(poll);
      quickError = String(e);
      code = null;
    }
  }

  async function submitToken() {
    busy = true;
    tokenError = null;
    try {
      await call("add_session", { token, reauth });
      app.modal = null;
    } catch (e) {
      tokenError = String(e);
    } finally {
      busy = false;
    }
  }

  function browser(signUp: boolean) {
    run("open_sign_in", { signUp, reauth });
  }
</script>

{#snippet option(icon: string, title: string, body: string, recommended: boolean, action: () => void)}
  <button class="option glass-base" onclick={action}>
    <span class="tile-icon" style="width: 42px; height: 42px"><Icon name={icon} size={20} /></span>
    <span class="col grow" style="gap: 3px">
      <span class="row" style="gap: 8px"><span class="item-title">{title}</span>{#if recommended}<span class="badge">Recommended</span>{/if}</span>
      <span class="meta">{body}</span>
    </span>
    <Icon name="caret-right" size={14} />
  </button>
{/snippet}

{#if step === "choose"}
  <ModalFrame
    title={reauth ? "Sign in again" : "Add Account"}
    subtitle={reauthAccount ? `Your session for @${who(reauthAccount.username)} expired. Sign in to keep launching with it.` : undefined}
    width={540}
  >
    {#if snap.sign_in}
      <div class="notice"><Icon name="browser" />
        {snap.sign_in.verifying ? "Signed in — adding your account…" : "Finish signing in in the Roblox window. Your password goes only to Roblox."}
      </div>
    {/if}
    <span class="meta">How would you like to continue?</span>
    {@render option(
      "browser",
      "Sign in with Roblox",
      "Opens roblox.com in a private window. Use your username, email or phone with your password, a one-time code, a passkey, or any other Roblox sign-in method.",
      true,
      () => browser(false),
    )}
    {@render option(
      "globe",
      "Use your browser's sign-in",
      "Already signed in to roblox.com in Edge, Chrome, Firefox, Brave or Opera? Pious picks that up. Nothing is shared but the account you choose.",
      false,
      searchBrowsers,
    )}
    {@render option(
      "phone",
      "Quick Login with another device",
      "Get a code here and approve it in the Roblox app or on roblox.com where you're already signed in.",
      false,
      startQuick,
    )}
    {#if !reauth}
      {@render option(
        "add-account",
        "Create account",
        "New to Roblox? Sign up on roblox.com. You're added automatically when you finish.",
        false,
        () => browser(true),
      )}
    {/if}
    <div class="row" style="gap: 8px">
      <Icon name="key" size={13} />
      <span class="secondary">Advanced:</span>
      <button class="link" onclick={() => (step = "token")}>Use a session token<Icon name="arrow-right" /></button>
    </div>
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (app.modal = null)}>Cancel</button>
    {/snippet}
  </ModalFrame>
{:else if step === "browser"}
  <ModalFrame title="Sign in from your browser" subtitle="Accounts signed in to roblox.com in your web browsers." width={540}>
    {#if searching}
      <div class="col stagger" style="gap: 8px">
        {#each [0, 1] as i (i)}<div class="glass-base found-skeleton pulse"></div>{/each}
      </div>
      <span class="meta">Looking through your browsers…</span>
    {:else if sessions.length}
      <div class="col stagger" style="gap: 8px">
        {#each sessions as s (s.user_id)}
          <div class="glass-base found">
            <span class="tile-icon" style="width: 38px; height: 38px"><Icon name="account" size={18} /></span>
            <div class="col grow" style="gap: 1px">
              <span class="item-title line">{who(s.display_name)} <span class="secondary">@{who(s.username)}</span></span>
              <span class="secondary">Signed in on {s.browser}</span>
            </div>
            {#if s.added && !reauth}
              <span class="badge">Added</span>
            {:else}
              <button class="btn small primary" disabled={adding !== null} onclick={() => addFound(s.user_id)}>
                <Icon name="add-account" />{adding === s.user_id ? "Adding…" : reauth ? "Use" : "Add"}
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="notice"><Icon name="info" />No Roblox sign-ins were found. Sign in on roblox.com in your browser, then look again.</div>
    {/if}
    {#each problems as problem (problem)}
      <div class="notice caution"><Icon name="warning" />{problem}</div>
    {/each}
    <div class="notice"><Icon name="lock" />Pious reads only the Roblox sign-in, keeps it in your system's credential vault, and never sends it anywhere but Roblox.</div>
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (step = "choose")}><Icon name="back" />Back</button>
      <span class="spacer"></span>
      <button class="btn" disabled={searching} onclick={searchBrowsers}><Icon name="refresh" />Look again</button>
      <button class="btn primary" onclick={() => (app.modal = null)}><Icon name="check" />Done</button>
    {/snippet}
  </ModalFrame>
{:else if step === "quick"}
  <ModalFrame title="Sign in with Quick Login" subtitle="Pious never sees your password." width={540}>
    {#if code}
      <div class="code">
        {#each code.split("") as c, i (i)}<span class="glass">{c}</span>{/each}
      </div>
    {:else if !quickError && status.kind === "Pending"}
      <div class="meta" style="text-align: center; padding: 16px">Requesting a code from Roblox…</div>
    {/if}
    <div class="col" style="gap: 8px">
      {#each ["On a phone or computer where you're signed in to Roblox, open roblox.com/crossdevicelogin.", "Enter the code shown here.", "Confirm the sign-in. Pious will finish automatically."] as line, i (i)}
        <div class="row" style="gap: 10px">
          <span class="step">{i + 1}</span>
          <span class="meta">{line}</span>
        </div>
      {/each}
    </div>
    {#if quickError}
      <div class="notice negative"><Icon name="warning" />{quickError}</div>
    {:else if finishing}
      <div class="notice"><Icon name="clock" />Approved — finishing sign-in…</div>
    {:else if status.kind === "Linked"}
      <div class="notice"><Icon name="info" />Code entered{status.account ? ` for @${status.account}` : ""}. Confirm the sign-in on your device.</div>
    {:else if status.kind === "Cancelled"}
      <div class="notice negative"><Icon name="error" />The sign-in was cancelled.</div>
    {:else if status.kind === "Expired"}
      <div class="notice caution"><Icon name="clock" />That code expired.</div>
    {:else}
      <div class="notice"><Icon name="clock" />Waiting for you to enter the code…</div>
    {/if}
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (clearInterval(poll), (step = "choose"))}><Icon name="back" />Back</button>
      <span class="spacer"></span>
      <button class="btn" onclick={() => run("open_url", { url: "https://www.roblox.com/crossdevicelogin" })}>
        <Icon name="external" />Open Roblox
      </button>
      {#if !code && !finishing}
        <button class="btn primary" onclick={startQuick}><Icon name="refresh" />Get a new code</button>
      {/if}
    {/snippet}
  </ModalFrame>
{:else}
  <ModalFrame title="Use a session token" subtitle="For advanced users who already have a signed-in session." width={540}>
    <div class="field">
      <span class="label">Session token</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        type="password"
        autofocus
        placeholder="Paste your .ROBLOSECURITY value"
        bind:value={token}
        oninput={() => (tokenError = null)}
        onkeydown={(e) => e.key === "Enter" && submitToken()}
      />
    </div>
    <div class="notice">
      <Icon name="lock" />The token is verified with Roblox and stored only in your system's credential vault. It's never shown again. Treat
      it like a password and never share it.
    </div>
    {#if tokenError}<div class="notice negative"><Icon name="warning" />{tokenError}</div>{/if}
    {#snippet footer()}
      <button class="btn tertiary" onclick={() => (step = "choose")}><Icon name="back" />Back</button>
      <span class="spacer"></span>
      <button class="btn primary" disabled={busy} onclick={submitToken}><Icon name="sign-in" />{busy ? "Verifying…" : "Verify & add"}</button>
    {/snippet}
  </ModalFrame>
{/if}

<style>
  .option {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    text-align: left;
    cursor: pointer;
    color: inherit;
    transition: background var(--fast), border-color var(--fast);
  }
  .option:hover {
    background: rgb(var(--surface) / 0.06);
    border-color: rgb(var(--surface) / 0.14);
  }
  .found {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-md);
  }
  .found-skeleton {
    height: 58px;
    border-radius: var(--r-md);
  }
  .code {
    display: flex;
    justify-content: center;
    gap: 7px;
  }
  .code span {
    width: 40px;
    padding: 8px 0;
    text-align: center;
    font-weight: 700;
    font-size: 26px;
  }
  .step {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    flex: none;
    border-radius: 50%;
    font-size: 11px;
    font-weight: 600;
    color: rgb(var(--accent));
    background: rgb(var(--accent) / 0.12);
  }
</style>
