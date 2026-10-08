<script lang="ts">
  import { app, sortOf, viewOf } from "../lib/state.svelte";
  import SortSelect from "../components/SortSelect.svelte";
  import Tip from "../components/Tip.svelte";
  import { setPreferences } from "../lib/api";
  import AccountCard from "../components/AccountCard.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import ViewToggle from "../components/ViewToggle.svelte";

  const snap = $derived(app.snap!);
  const library = $derived(snap.bootstrapper);
  const list = $derived(viewOf("accounts") === "List");
  const playing = $derived(library.accounts.filter((a) => snap.instances.some((i) => i.account === a.id)).length);
  const needsSignIn = $derived(library.accounts.filter((a) => a.needs_sign_in).length);
  const sort = $derived(sortOf("accounts", "default"));
  const label = (a: (typeof library.accounts)[number]) => a.alias ?? a.display_name;
  const accounts = $derived(
    [...library.accounts].sort((a, b) => {
      if (sort === "name") return label(a).localeCompare(label(b));
      if (sort === "added") return a.added_at.localeCompare(b.added_at);
      if (sort === "recent") return (b.last_used ?? "").localeCompare(a.last_used ?? "");
      return (
        Number(library.preferences.default_account === b.id) - Number(library.preferences.default_account === a.id) ||
        (b.last_used ?? "").localeCompare(a.last_used ?? "") ||
        a.added_at.localeCompare(b.added_at)
      );
    }),
  );
</script>

<div class="page">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Accounts</h1>
      <span class="meta">
        {library.accounts.length === 0
          ? "Your Roblox profiles"
          : library.accounts.length === 1
            ? "1 account"
            : `${library.accounts.length} accounts${playing ? ` · ${playing} playing now` : ""}`}
      </span>
    </div>
    <SortSelect
      page="accounts"
      value={sort}
      options={[
        { value: "default", label: "Default first" },
        { value: "recent", label: "Recently used" },
        { value: "name", label: "Name" },
        { value: "added", label: "Date added" },
      ]}
    />
    <ViewToggle page="accounts" />
    <button class="btn primary" onclick={() => (app.modal = { kind: "add_account", reauth: null })}><Icon name="add-account" />Add Account</button>
  </div>

  {#if !library.accounts.length}
    <EmptyState icon="accounts" title="No accounts yet" body="Add an account to start using Pious.">
      <button class="btn primary" onclick={() => (app.modal = { kind: "add_account", reauth: null })}><Icon name="add-account" />Add Account</button>
    </EmptyState>
  {:else}
    <Tip id="accounts">The account in the sidebar is the one Play uses. Each account can keep its own Roblox settings (graphics, volume, keys).</Tip>
    {#if needsSignIn}
      <div class="notice caution">
        <Icon name="warning" />{needsSignIn === 1
          ? "One account's session expired. Sign it in again to keep launching with it."
          : `${needsSignIn} accounts need to sign in again.`}
      </div>
    {/if}
    {#if list}
      <div class="list-rows">
        {#each accounts as account (account.id)}<AccountCard {account} variant="row" />{/each}
      </div>
    {:else}
      <div class="grid" style="--card: 256px">
        {#each accounts as account (account.id)}<AccountCard {account} />{/each}
      </div>
    {/if}
    <div class="notice"><Icon name="lock" />Sessions are kept in your system's secure credential vault. Pious never stores passwords.</div>
  {/if}
</div>
