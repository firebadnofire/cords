<script lang="ts">
  import {
    Activity,
    AlertTriangle,
    Bell,
    Bot,
    ClipboardList,
    Crown,
    KeyRound,
    Link,
    Shield,
    Trash2,
  } from '@lucide/svelte';
  import type { Identity, Status } from '../model';
  export let page:
    | 'Invites'
    | 'Moderation'
    | 'Audit Log'
    | 'Notifications'
    | 'Integrations'
    | 'Advanced'
    | 'Ownership';
  export let status: Status;
  export let identity: Identity | null;
  const pending = 'Not implemented yet';
</script>

{#if page === 'Invites'}
  <header class="admin-page-header">
    <div>
      <h1>Invites</h1>
      <p>Review capability links issued from this server’s HTTPS address.</p>
    </div>
    <button class="admin-primary dev-unimplemented" disabled aria-disabled="true" title={pending}
      >Create invite</button
    >
  </header>
  <div class="invite-card development-surface">
    <span class="development-label">DEVELOPMENT FORMAT EXAMPLE</span><code
      >{status.origin}/join/&lt;token&gt;#cords-server={status.server_id}</code
    >
    <p>No invite issuance or listing endpoint is available.</p>
    <footer>
      <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
        >Copy issued invite</button
      ><button class="dev-unimplemented danger" disabled aria-disabled="true" title={pending}
        >Revoke</button
      >
    </footer>
  </div>
{:else if page === 'Moderation'}
  <header class="admin-page-header">
    <div>
      <h1>Moderation</h1>
      <p>Membership safety controls without access to ordinary encrypted plaintext.</p>
    </div>
  </header>
  <div class="admin-stat-grid">
    <div><strong>—</strong><span>Banned members</span></div>
    <div><strong>—</strong><span>Active timeouts</span></div>
    <div><strong>—</strong><span>Open reports</span></div>
    <div><strong>—</strong><span>Recent actions</span></div>
  </div>
  <div class="admin-tabs">
    {#each ['Banned users', 'Timeouts', 'Reports', 'Moderator notes'] as tab (tab)}<button
        class="dev-unimplemented"
        disabled
        aria-disabled="true"
        title={pending}>{tab}</button
      >{/each}
  </div>
  <div class="admin-footnote">
    <AlertTriangle size={16} />No moderation API is exposed. User reports would need explicit
    disclosure of selected plaintext.
  </div>
{:else if page === 'Audit Log'}
  <header class="admin-page-header">
    <div>
      <h1>Audit log</h1>
      <p>Administrative events belong to an authoritative server feed, not message history.</p>
    </div>
  </header>
  <div class="settings-other">
    <ClipboardList size={30} />
    <h2>No audit API available</h2>
    <p>No fabricated administrative events are displayed.</p>
  </div>
  <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
    >Filter audit events</button
  >
{:else if page === 'Notifications'}
  <header class="admin-page-header">
    <div>
      <h1>Server notifications</h1>
      <p>Default notification presentation for new members.</p>
    </div>
  </header>
  {#each ['Default notification level', 'Moderation alerts', 'Welcome notifications'] as item (item)}<button
      class="dev-unimplemented settings-row-button"
      disabled
      aria-disabled="true"
      title={pending}><Bell size={16} />{item}</button
    >{/each}
{:else if page === 'Integrations'}
  <header class="admin-page-header">
    <div>
      <h1>Integrations</h1>
      <p>Connected services and server-side helpers.</p>
    </div>
    <button class="admin-primary dev-unimplemented" disabled aria-disabled="true" title={pending}
      >Add integration</button
    >
  </header>
  <div class="integration-card">
    <Bot size={25} />
    <div>
      <strong>No connected integrations</strong><span>The server exposes no integration API.</span>
    </div>
  </div>
  <div class="integration-card">
    <Activity size={25} />
    <div>
      <strong>Status helpers</strong><span>No runtime integration state is available.</span>
    </div>
    <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
      >Configure</button
    >
  </div>
{:else if page === 'Ownership'}
  <header class="admin-page-header">
    <div>
      <h1>Ownership</h1>
      <p>High-impact server lifecycle controls require explicit server authority.</p>
    </div>
  </header>
  <div class="owner-card">
    <Crown size={27} />
    <div>
      <span>Current account</span><strong>{status.account_id}</strong><small
        >{identity?.membership?.capabilities.includes('server.manage')
          ? 'Authenticated server owner'
          : 'Standard server membership'}</small
      >
    </div>
  </div>
  {#if identity?.membership?.capabilities.includes('server.manage')}
    <div class="admin-footnote">
      <Shield size={16} />This account can manage channel rosters across the server. Invite, ban,
      and destructive ownership operations remain unavailable until their authoritative APIs exist.
    </div>
  {:else}
    <div class="admin-footnote">
      <Shield size={16} />Ownership is established only during server bootstrap. An ordinary member
      cannot claim or inherit administrative authority here.
    </div>
  {/if}
  <section class="danger-zone">
    <h2>Danger zone</h2>
    <div>
      <span><strong>Leave server</strong><small>Remove this installation’s membership.</small></span
      ><button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
        >Leave server</button
      >
    </div>
    <div>
      <span
        ><strong>Delete server</strong><small>Permanently remove hosted server state.</small></span
      ><button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
        ><Trash2 size={15} />Delete server</button
      >
    </div>
  </section>
{:else}
  <header class="admin-page-header">
    <div>
      <h1>Advanced</h1>
      <p>Technical metadata and operator-owned configuration.</p>
    </div>
  </header>
  <div class="admin-form-grid">
    <label>Server ID<input readonly value={status.server_id} /></label><label
      >HTTPS origin<input readonly value={status.origin} /></label
    >
  </div>
  <button
    class="dev-unimplemented settings-row-button"
    disabled
    aria-disabled="true"
    title={pending}><Shield size={16} />Metadata retention</button
  ><button
    class="dev-unimplemented settings-row-button"
    disabled
    aria-disabled="true"
    title={pending}><Link size={16} />Admission policy</button
  >
  <div class="admin-footnote">
    <KeyRound size={16} />The persistent signing identity is distinct from TLS and conversation
    encryption.
  </div>
{/if}
