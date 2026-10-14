<script lang="ts">
  import { onMount } from 'svelte';
  import {
    defaultPermissions,
    permissionGroups,
    type Role,
    type RoleCatalog,
    type RoleChange,
    type RoleRequest,
    type RoleReceipt,
  } from '../rolePolicy';
  export let load: () => Promise<RoleCatalog>;
  export let change: (request: RoleRequest) => Promise<RoleReceipt>;
  let catalog: RoleCatalog | null = null;
  let draft: Role | null = null;
  let original = '';
  let busy = false;
  let error = '';
  let notice = '';
  let confirmingDelete = false;
  let retry: RoleRequest | null = null;
  $: dirty = draft !== null && JSON.stringify(draft) !== original;
  $: builtin = draft?.id.startsWith('builtin:') ?? false;

  function select(role: Role) {
    draft = { ...role, permissions: [...role.permissions] };
    original = JSON.stringify(draft);
    confirmingDelete = false;
    notice = '';
  }
  async function refresh() {
    busy = true;
    error = '';
    try {
      const next = await load();
      catalog = next;
      const selected = next.roles.find((role) => role.id === draft?.id) ?? next.roles[0];
      if (selected) select(selected);
      else {
        draft = null;
        original = '';
      }
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  function create() {
    draft = {
      id: crypto.randomUUID(),
      name: '',
      color: '#4c9bd4',
      description: '',
      priority: 1,
      permissions: [],
      mentionable: false,
    };
    original = '';
    confirmingDelete = false;
    notice = '';
  }
  function permission(capability: string, enabled: boolean) {
    if (!draft) return;
    draft = {
      ...draft,
      permissions: enabled
        ? [...new Set([...draft.permissions, capability])].sort()
        : draft.permissions.filter((item) => item !== capability),
    };
  }
  async function submit(operation: RoleChange, previous?: RoleRequest) {
    if (!catalog) return;
    busy = true;
    error = '';
    notice = '';
    const request = previous ?? {
      expected_revision: catalog.revision,
      request_id: crypto.randomUUID(),
      change: operation,
    };
    retry = request;
    try {
      await change(request);
      retry = null;
      await refresh();
      if (!error) notice = 'Saved on the server. Role enforcement remains inactive.';
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  function reload() {
    retry = null;
    void refresh();
  }
  onMount(() => {
    void refresh();
  });
</script>

<header class="admin-page-header">
  <div>
    <h1>Roles</h1>
    <p>Stable roles, server-validated permissions, and explicit hierarchy.</p>
  </div>
  <button
    class="admin-primary"
    disabled={busy || !catalog?.roles.length || dirty || !!retry}
    on:click={create}>Create role</button
  >
</header>
{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if notice}<p role="status">{notice}</p>{/if}
{#if retry}
  <p>
    The result may be uncertain. Retry sends the identical request; reload checks current server
    state.
  </p>
  <button disabled={busy} on:click={() => retry && submit(retry.change, retry)}
    >Retry same request</button
  >
  <button disabled={busy} on:click={reload}>Reload server state</button>
{:else if !catalog && !busy}<button on:click={reload}>Retry loading roles</button>{/if}
{#if busy}<p role="status">Loading or saving role policy…</p>{/if}
{#if catalog}
  <div class="admin-footnote" role="note">
    <p>
      <strong>Role enforcement is not active.</strong> This editor saves the server's preparatory catalog
      only. Existing access is unchanged. Channel access, device approvals, and role notifications require
      the validated enforcement upgrade. Only the current owner can prepare roles.
    </p>
  </div>
  {#if !catalog.roles.length}
    <p>
      No role catalog has been initialized. Start with $default, $moderator, and $admin. This does
      not assign roles or grant access.
    </p>
    <button disabled={busy || !!retry} on:click={() => submit({ operation: 'initialize' })}
      >Initialize role catalog</button
    >
  {:else}
    <div class="role-workspace">
      <aside class="role-list" aria-label="Server roles">
        <span class="role-count">Policy revision {catalog.revision}</span>
        {#each [...catalog.roles].sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id)) as role (role.id)}
          <button
            class:active={draft?.id === role.id}
            disabled={busy || dirty || !!retry}
            on:click={() => select(role)}
          >
            <i style:background={role.color}></i><span
              ><strong>${role.name}</strong><small>Priority {role.priority}</small></span
            >
          </button>
        {/each}
      </aside>
      {#if draft}
        <form
          class="role-editor"
          on:submit|preventDefault={() => draft && submit({ operation: 'put_role', role: draft })}
        >
          <fieldset disabled={busy || !!retry}>
            <legend>{draft.name ? `$${draft.name}` : 'New role'}</legend>
            <p class="muted">
              $admin inherits $moderator. Priority does not inherit custom permissions. Ownership is
              separate.
            </p>
            <div class="admin-form-grid">
              <label
                >Role name (without $)<input
                  bind:value={draft.name}
                  required
                  maxlength="64"
                  readonly={builtin}
                /></label
              >
              <label>Color<input type="color" bind:value={draft.color} /></label>
              <label
                >Description<textarea bind:value={draft.description} maxlength="1000"
                ></textarea></label
              >
              <label
                >Priority<input
                  type="number"
                  bind:value={draft.priority}
                  min="0"
                  max="4294967295"
                  step="1"
                  required
                  readonly={draft.id === 'builtin:default'}
                /></label
              >
            </div>
            <label
              ><input type="checkbox" bind:checked={draft.mentionable} />Allow authorized senders to
              mention this role</label
            >
            {#each permissionGroups as group (group.name)}
              <section class="permission-group">
                <h3>{group.name}</h3>
                {#each group.permissions as [capability, label] (capability)}
                  <label class="permission-row"
                    ><span><strong>{label}</strong><small>{capability}</small></span>
                    <input
                      type="checkbox"
                      aria-label={label}
                      checked={draft.permissions.includes(capability)}
                      disabled={draft.id === 'builtin:default' &&
                        !defaultPermissions.has(capability)}
                      on:change={(event) => permission(capability, event.currentTarget.checked)}
                    />
                  </label>
                {/each}
              </section>
            {/each}
            <p class="muted">Role ID: <code>{draft.id}</code></p>
            <button type="submit" disabled={!dirty}>Save role</button>
            <button type="button" on:click={reload}>Cancel</button>
            {#if !builtin && catalog.roles.some((role) => role.id === draft?.id)}
              {#if confirmingDelete}
                <p>Delete this custom role and its stored assignments?</p>
                <button
                  type="button"
                  on:click={() => draft && submit({ operation: 'delete_role', role_id: draft.id })}
                  >Confirm delete role</button
                >
              {:else}<button type="button" on:click={() => (confirmingDelete = true)}
                  >Delete role</button
                >{/if}
            {/if}
          </fieldset>
        </form>
      {/if}
    </div>
  {/if}
{/if}

<style>
  fieldset {
    border: 0;
    padding: 0;
    min-width: 0;
  }
  legend {
    font-size: 1.2rem;
    font-weight: 700;
    margin-bottom: 0.6rem;
  }
  code {
    overflow-wrap: anywhere;
  }
  .permission-row input {
    width: auto;
    flex: 0 0 auto;
  }
  .muted {
    color: var(--text-muted, #b6bac3);
  }
</style>
