<script lang="ts">
  import { onMount } from 'svelte';
  import type { RoleAuditEntry, RoleAuditPage } from '../rolePolicy';
  export let load: (after: number) => Promise<RoleAuditPage>;
  let entries: RoleAuditEntry[] = [];
  let after = 0;
  let head = 0;
  let loaded = false;
  let busy = false;
  let error = '';
  const labels: Record<string, string> = {
    initialize: 'Initialized role catalog',
    put_role: 'Saved role',
    delete_role: 'Deleted role',
    assign_role: 'Assigned role',
    revoke_role: 'Removed role assignment',
  };
  async function refresh(reset: boolean) {
    busy = true;
    error = '';
    const cursor = reset ? 0 : after;
    try {
      const page = await load(cursor);
      if (page.after_revision !== cursor) throw new Error('Audit cursor does not match request');
      entries = reset ? page.entries : [...entries, ...page.entries];
      after = page.entries.at(-1)?.revision ?? cursor;
      head = page.head_revision;
      loaded = true;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void refresh(true);
  });
</script>

<header class="admin-page-header">
  <div>
    <h1>Role change history</h1>
    <p>Committed role and assignment changes, ordered by policy revision.</p>
  </div>
  <button disabled={busy} on:click={() => refresh(true)}>Refresh history</button>
</header>
<p>
  This log covers role administration only. It is signed by this server; it is not independent proof
  against a compromised server.
</p>
{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if busy}<p role="status">Loading role history…</p>{/if}
{#if loaded && !entries.length}<p>No committed role changes.</p>{/if}
<ol aria-label="Role change history">
  {#each entries as entry (entry.revision)}
    <li>
      <strong>Revision {entry.revision} · {labels[entry.operation] ?? entry.operation}</strong>
      <dl>
        <dt>Actor device</dt>
        <dd><code>{entry.actor_device_id}</code></dd>
        <dt>Target</dt>
        <dd><code>{entry.target}</code></dd>
        <dt>Recorded at</dt>
        <dd>{new Date(entry.committed_at * 1000).toLocaleString()}</dd>
      </dl>
    </li>
  {/each}
</ol>
{#if after < head}<button disabled={busy} on:click={() => refresh(false)}>Load more changes</button
  >{/if}

<style>
  ol {
    list-style: none;
    padding: 0;
  }
  li {
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.4rem 1rem;
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
