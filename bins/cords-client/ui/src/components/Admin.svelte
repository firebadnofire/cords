<script lang="ts">
  import {
    ChevronDown,
    Hash,
    LockKeyhole,
    MoreHorizontal,
    Plus,
    Search,
    Server,
    Settings2,
    Users,
  } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import AdminUnavailable from './AdminUnavailable.svelte';
  import type { Channel, Contact, Identity, Status } from '../model';
  export let status: Status;
  export let identity: Identity | null;
  export let channels: Channel[];
  export let contacts: Contact[];
  export let close: () => void;
  export let create: (name: string) => Promise<void>;
  export let select: (id: string) => Promise<void>;
  type Page =
    | 'Overview'
    | 'Members'
    | 'Roles'
    | 'Channels'
    | 'Invites'
    | 'Moderation'
    | 'Audit Log'
    | 'Notifications'
    | 'Integrations'
    | 'Advanced'
    | 'Ownership';
  const groups: { title: string; pages: Page[] }[] = [
    { title: 'Server', pages: ['Overview', 'Members', 'Roles', 'Channels', 'Invites'] },
    { title: 'Community', pages: ['Moderation', 'Audit Log', 'Notifications', 'Integrations'] },
    { title: 'System', pages: ['Advanced', 'Ownership'] },
  ];
  let page: Page = 'Overview';
  let query = '';
  let name = '';
  let error = '';
  let busy = false;
  let selectedContact: Contact | null = null;
  let selectedChannel = channels[0]?.channel_id ?? '';
  const pending = 'Not implemented yet';
  async function createChannel() {
    busy = true;
    error = '';
    try {
      await create(name);
      name = '';
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  $: channel = channels.find((item) => item.channel_id === selectedChannel) ?? channels[0];
</script>

<Modal title="Server administration" {close} wide>
  <div class="settings-layout server-admin-layout">
    <aside class="settings-nav donor-settings-nav admin-sidebar">
      <div class="admin-server-heading">
        <span class="server-button server-blue"><Server size={21} /></span>
        <div>
          <strong>{new URL(status.origin).hostname}</strong><small>Server information</small>
        </div>
      </div>
      <div class="admin-privilege"><LockKeyhole size={14} />Capabilities determine access</div>
      {#each groups as group (group.title)}<section class="settings-menu-group">
          <h3>{group.title}</h3>
          {#each group.pages as item (item)}<button
              class:active={page === item}
              aria-current={page === item ? 'page' : undefined}
              on:click={() => {
                page = item;
                selectedContact = null;
              }}>{item}</button
            >{/each}
        </section>{/each}
      <div class="settings-sidebar-note">
        <Settings2 size={14} />Real state or explicit development gaps
      </div>
    </aside>
    <main class="settings-body admin-scroll">
      <div class="admin-page">
        {#if error}<p class="error" role="alert">{error}</p>{/if}
        {#if page === 'Overview'}
          <header class="admin-page-header">
            <div>
              <h1>Server overview</h1>
              <p>Public presentation and server-wide defaults.</p>
            </div>
            <button
              class="admin-primary dev-unimplemented"
              disabled
              aria-disabled="true"
              title={pending}>Save changes</button
            >
          </header>
          <div class="admin-hero-card">
            <div class="admin-server-icon">
              {new URL(status.origin).hostname.slice(0, 1).toUpperCase()}
            </div>
            <div>
              <strong>{new URL(status.origin).hostname}</strong><span>{status.origin}</span><small
                >{channels.length} accessible channels · {contacts.length} public device contacts</small
              >
            </div>
            <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
              >Change icon</button
            >
          </div>
          <div class="admin-form-grid">
            <label>Server name<input readonly value={new URL(status.origin).hostname} /></label
            ><label>Server identity<input readonly value={status.server_id} /></label><label
              >Description<textarea
                class="dev-unimplemented"
                disabled
                aria-disabled="true"
                title={pending}
                placeholder="Not exposed by the server API"
              ></textarea></label
            ><label
              >Join policy<select
                class="dev-unimplemented"
                disabled
                aria-disabled="true"
                title={pending}><option>Not exposed</option></select
              ></label
            >
          </div>
          <div class="admin-banner-preview development-surface">
            <div>
              <span>SERVER BANNER</span><strong
                >Presentation surface ready for authoritative metadata.</strong
              >
            </div>
            <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
              >Change banner</button
            >
          </div>
        {:else if page === 'Members'}
          <header class="admin-page-header">
            <div>
              <h1>Members</h1>
              <p>
                Public device contacts returned by this server. This is not presence or a complete
                membership directory.
              </p>
            </div>
            <button
              class="admin-primary dev-unimplemented"
              disabled
              aria-disabled="true"
              title={pending}>Invite member</button
            >
          </header>
          <div class="admin-toolbar">
            <label
              ><Search size={16} /><input
                type="search"
                placeholder="Search account or device"
                bind:value={query}
              /></label
            ><select class="dev-unimplemented" disabled aria-disabled="true" title={pending}
              ><option>All roles</option></select
            ><select class="dev-unimplemented" disabled aria-disabled="true" title={pending}
              ><option>Sort members</option></select
            >
          </div>
          <div class="member-table">
            <div class="member-table-head">
              <span>Device</span><span>Account</span><span>Generation</span><span>Status</span><span
              ></span>
            </div>
            {#each contacts.filter((contact) => JSON.stringify(contact.authorization.value)
                .toLowerCase()
                .includes(query.toLowerCase())) as contact (contact.authorization.value.device_id)}<button
                class="member-table-row"
                on:click={() => (selectedContact = contact)}
                ><div class="member-identity">
                  <span class="member-avatar"
                    >{contact.authorization.value.device_id.slice(-2)}</span
                  ><span
                    ><strong>{contact.authorization.value.device_id}</strong><small
                      >Authorized device</small
                    ></span
                  >
                </div>
                <code>{contact.authorization.value.account_id}</code><span
                  >{contact.authorization.value.generation}</span
                ><span class="member-status">Unknown</span><MoreHorizontal size={18} /></button
              >{:else}<div class="empty"><p>No matching public device contacts.</p></div>{/each}
          </div>
          {#if selectedContact}<div class="member-detail-card">
              <h2>Device authorization</h2>
              <code>{selectedContact.authorization.value.device_id}</code>
              <p>Account {selectedContact.authorization.value.account_id}</p>
              <p>
                Issued {new Date(
                  selectedContact.authorization.value.created_at * 1000,
                ).toLocaleString()}
              </p>
              <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
                >Manage server membership</button
              >
            </div>{/if}
          <div class="admin-footnote">
            <LockKeyhole size={16} />Server membership removal and MLS conversation removal are
            distinct transitions.
          </div>
        {:else if page === 'Roles'}
          <header class="admin-page-header">
            <div>
              <h1>Roles</h1>
              <p>Roles map display labels to stable capability strings.</p>
            </div>
            <button
              class="admin-primary dev-unimplemented"
              disabled
              aria-disabled="true"
              title={pending}>Create role</button
            >
          </header>
          <div class="role-workspace">
            <aside class="role-list">
              <span class="role-count">Current membership capabilities</span
              >{#each identity?.membership?.capabilities ?? [] as capability (capability)}<button
                  class="active"
                  ><i></i><span
                    ><strong>{capability}</strong><small>Granted to this device session</small
                    ></span
                  ></button
                >{:else}<div class="empty"><small>No capability data available.</small></div>{/each}
            </aside>
            <div class="role-editor">
              <div class="role-editor-head">
                <div class="role-icon">R</div>
                <div>
                  <h2>Role editor</h2>
                  <p>Role definitions and assignments are not exposed.</p>
                </div>
              </div>
              {#each ['General permissions', 'Moderation', 'Messaging', 'Voice'] as group (group)}<section
                  class="permission-group"
                >
                  <h3>{group}</h3>
                  <div class="permission-row">
                    <span
                      ><strong>Future capability controls</strong><small
                        >Requires an authoritative role API.</small
                      ></span
                    >
                    <div class="permission-choice">
                      <button
                        class="dev-unimplemented"
                        disabled
                        aria-disabled="true"
                        title={pending}>Deny</button
                      ><button
                        class="dev-unimplemented"
                        disabled
                        aria-disabled="true"
                        title={pending}>Neutral</button
                      ><button
                        class="dev-unimplemented"
                        disabled
                        aria-disabled="true"
                        title={pending}>Allow</button
                      >
                    </div>
                  </div>
                </section>{/each}
            </div>
          </div>
        {:else if page === 'Channels'}
          <header class="admin-page-header">
            <div>
              <h1>Channels</h1>
              <p>Organize encrypted routes and inspect current MLS state.</p>
            </div>
          </header>
          <div class="channel-admin-grid">
            <div class="channel-tree">
              <section>
                <h3>
                  <ChevronDown size={12} />TEXT CHANNELS
                  <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
                    ><Plus size={14} /></button
                  >
                </h3>
                {#each channels as item (item.channel_id)}<button
                    class:active={channel?.channel_id === item.channel_id}
                    on:click={() => (selectedChannel = item.channel_id)}
                    ><Hash size={16} /><span>{item.name}</span><MoreHorizontal size={15} /></button
                  >{/each}
              </section>
              <button
                class="create-category dev-unimplemented"
                disabled
                aria-disabled="true"
                title={pending}><Plus size={15} />Create category</button
              >
            </div>
            <div class="channel-editor">
              {#if channel}<h2>#{channel.name}</h2>
                <div class="admin-form-grid compact">
                  <label>Channel name<input readonly value={channel.name} /></label><label
                    >MLS epoch<input readonly value={channel.epoch} /></label
                  ><label
                    >Topic<textarea
                      class="dev-unimplemented"
                      disabled
                      aria-disabled="true"
                      title={pending}
                      placeholder="Not exposed"
                    ></textarea></label
                  >
                </div>
                <h3 class="section-label"><Users size={15} />Device membership</h3>
                {#each channel.members as member (member.authorization.value.device_id)}<div
                    class="override-row"
                  >
                    <span>{member.authorization.value.device_id}</span><small
                      >Generation {member.authorization.value.generation}</small
                    >
                  </div>{/each}<button
                  on:click={async () => {
                    await select(channel.channel_id);
                    close();
                  }}>Open conversation</button
                ><button
                  class="admin-danger-link dev-unimplemented"
                  disabled
                  aria-disabled="true"
                  title={pending}>Delete channel</button
                >{:else}<p>No channels available.</p>{/if}
            </div>
          </div>
          <form class="admin-create-channel" on:submit|preventDefault={createChannel}>
            <label>New encrypted channel<input maxlength="100" required bind:value={name} /></label
            ><button
              class="primary"
              disabled={busy || !identity?.membership?.capabilities.includes('channel.create')}
              >Create channel</button
            >
          </form>
        {:else}
          <AdminUnavailable {page} {status} {identity} />
        {/if}
      </div>
    </main>
  </div>
</Modal>
