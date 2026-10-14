<script lang="ts">
  import { onMount } from 'svelte';
  import type { AccountRoles, RoleRequest, RoleReceipt } from '../rolePolicy';
  export let account: string;
  export let load: (account: string) => Promise<AccountRoles>;
  export let change: (request: RoleRequest) => Promise<RoleReceipt>;
  let state: AccountRoles | null = null;
  let busy = false;
  let error = '';
  let notice = '';
  let retry: RoleRequest | null = null;
  async function refresh() {
    busy = true;
    error = '';
    try {
      const next = await load(account);
      if (next.account_id !== account)
        throw new Error('Account role response does not match this member');
      state = next;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  async function save(request: RoleRequest) {
    busy = true;
    error = '';
    notice = '';
    retry = request;
    try {
      await change(request);
      retry = null;
      await refresh();
      if (!error) notice = 'Assignment saved. Role enforcement remains inactive.';
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  function toggle(role: string, assigned: boolean) {
    if (!state) return;
    void save({
      expected_revision: state.catalog.revision,
      request_id: crypto.randomUUID(),
      change: {
        operation: assigned ? 'revoke_role' : 'assign_role',
        account_id: account,
        role_id: role,
      },
    });
  }
  function reload() {
    retry = null;
    notice = '';
    void refresh();
  }
  onMount(() => {
    void refresh();
  });
</script>

<section aria-label="Account roles">
  <h3>Account roles</h3>
  <p>Assignments belong to this account, not just the selected device.</p>
  <p role="note">
    Role enforcement is not active. These saved assignments do not grant channel access or MLS
    membership.
  </p>
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if busy}<p role="status">Loading or saving assignments…</p>{/if}
  {#if retry}
    <p>The outcome may be uncertain. Retry sends the same request.</p>
    <button disabled={busy} on:click={() => retry && save(retry)}>Retry assignment</button>
  {/if}
  <button disabled={busy} on:click={reload}>Reload assignments</button>
  {#if state}
    <p>Policy revision {state.catalog.revision}</p>
    {#if !state.catalog.roles.length}
      <p>The owner must initialize the catalog in Roles first.</p>
    {:else}
      <ul>
        {#each state.catalog.roles as role (role.id)}
          <li>
            <span>${role.name}</span>
            {#if role.id === 'builtin:default'}
              <span>Implicit for admitted accounts</span>
            {:else}
              <span>{state.role_ids.includes(role.id) ? 'Assigned' : 'Not assigned'}</span>
              <button
                disabled={busy || !!retry}
                on:click={() => toggle(role.id, state!.role_ids.includes(role.id))}
              >
                {state.role_ids.includes(role.id) ? 'Remove' : 'Assign'} ${role.name}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  ul {
    list-style: none;
    padding: 0;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0;
  }
  li > span:first-child {
    min-width: 8rem;
    font-weight: 600;
  }
</style>
