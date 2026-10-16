<script lang="ts">
  import { onMount } from 'svelte';
  import { RefreshCw, X } from '@lucide/svelte';
  import type { AccountDirectoryPage, DirectoryAccount } from '../model';
  import { memberPicture, blankPicture } from '../model';
  import RemoteAvatar from './RemoteAvatar.svelte';
  import { publicProfiles } from '../profiles';

  export let serverId: string;
  export let load: (after: string) => Promise<AccountDirectoryPage>;
  export let close: () => void;
  let accounts: DirectoryAccount[] = [];
  let next: string | null = null;
  let busy = false;
  let error = '';
  let filter = '';
  let disposed = false;
  let expanded = '';
  // Device cards remain review candidates until canonical profiles activate.
  // Only use unanimous presentation; device order cannot resolve a conflict.
  function presentation(account: DirectoryAccount, profiles = $publicProfiles) {
    const canonical = profiles[account.account_id]?.state.agreed;
    const cards = account.devices.map((device) => device.user_card?.value);
    const agreed =
      cards[0] &&
      cards.every(
        (card) =>
          card &&
          card.nickname === cards[0]?.nickname &&
          JSON.stringify(card.avatar) === JSON.stringify(cards[0]?.avatar),
      );
    return {
      name:
        canonical?.display_name ??
        (agreed ? cards[0]!.nickname : `Account ${account.account_id.slice(0, 8)}`),
      picture: {
        ...(agreed ? memberPicture(account.devices[0]) : blankPicture()),
        account: account.account_id,
      },
      conflict: !canonical && !agreed && cards.some(Boolean),
    };
  }
  async function fetchPage(reset = false) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      const page = await load(reset ? '' : (next ?? ''));
      if (disposed) return;
      if (page.server_id !== serverId) throw new Error('Server changed while loading members.');
      const combined = reset ? page.accounts : [...accounts, ...page.accounts];
      accounts = [...new Map(combined.map((account) => [account.account_id, account])).values()];
      next = page.next_after;
    } catch (caught) {
      if (!disposed) {
        // Do not keep showing a directory after a failed authoritative refresh.
        if (reset) {
          accounts = [];
          next = null;
          expanded = '';
        }
        const reason = String(caught);
        error = /HTTP 404/.test(reason)
          ? 'This server needs an update to support the account directory.'
          : `Could not load server members: ${reason}`;
      }
    } finally {
      if (!disposed) busy = false;
    }
  }
  onMount(() => {
    void fetchPage(true);
    return () => {
      disposed = true;
    };
  });
  $: visible = accounts.filter((account) =>
    `${presentation(account).name} ${account.account_id}`
      .toLowerCase()
      .includes(filter.toLowerCase()),
  );
</script>

<aside class="members-panel" aria-label="Server members">
  <header class="members-heading">
    <h2>Members</h2>
    <button
      class="icon-button"
      aria-label="Refresh members"
      disabled={busy}
      on:click={() => void fetchPage(true)}><RefreshCw size={17} /></button
    >
    <button class="icon-button" aria-label="Close members" on:click={close}><X size={19} /></button>
  </header>
  <label class="members-search"
    ><span class="sr-only">Search loaded members</span><input
      type="search"
      placeholder="Find a member"
      bind:value={filter}
    /></label
  >
  <div class="members-list" aria-busy={busy}>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <h3>Members · {accounts.length}{next ? '+' : ''}</h3>
    {#each visible as account (account.account_id)}
      {@const profile = presentation(account, $publicProfiles)}
      <button
        class="member-list-row"
        aria-expanded={expanded === account.account_id}
        on:click={() => (expanded = expanded === account.account_id ? '' : account.account_id)}
      >
        <RemoteAvatar value={profile.picture} text={profile.name.slice(0, 2)} size={36} />
        <span
          ><strong>{profile.name}</strong>{#if profile.conflict}<small>Profile needs review</small
            >{/if}</span
        >
      </button>
      {#if expanded === account.account_id}
        <div class="member-summary">
          <strong>{profile.name}</strong><code>{account.account_id}</code>
          <details>
            <summary>Devices ({account.devices.length})</summary>
            {#each account.devices as device (device.authorization.value.device_id)}<code
                >{device.authorization.value.device_id}</code
              >{/each}
          </details>
        </div>
      {/if}
    {:else}{#if !busy && !error}<p class="hint">
          {filter ? 'No matching loaded members.' : 'No admitted accounts returned.'}
        </p>{/if}{/each}
    {#if busy}<p role="status">Loading members…</p>{/if}
    {#if next}<button disabled={busy} on:click={() => void fetchPage()}>Load more members</button
      >{/if}
  </div>
</aside>
