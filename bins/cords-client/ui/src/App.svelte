<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { LockKeyhole } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import AccountPicker from './components/AccountPicker.svelte';
  import Modal from './components/Modal.svelte';
  import Settings from './components/Settings.svelte';
  import Admin from './components/Admin.svelte';
  import CordsRail from './components/CordsRail.svelte';
  import ConversationSidebar from './components/ConversationSidebar.svelte';
  import CordsHeader from './components/CordsHeader.svelte';
  import ConversationView from './components/ConversationView.svelte';
  import ConversationDetails from './components/ConversationDetails.svelte';
  import IdentityPanel from './components/IdentityPanel.svelte';
  import {
    defaults,
    preferences,
    canManage,
    selectedChannel,
    type Status,
    type Identity,
    type Channel,
    type Contact,
    type Message,
    type MembershipRequest,
    type View,
    type Preferences,
    type ServerListing,
  } from './model';

  let status: Status | null = null;
  let servers: ServerListing[] = [];
  type LocalAccount = { account_id: string; nickname: string; avatar_data: string };
  let accounts: LocalAccount[] = [];
  let selectedAccount = '';
  let activeAccount = '';
  let legacyVault = false;
  let identity: Identity | null = null;
  let prefs = defaults();
  let channels: Channel[] = [];
  let contacts: Contact[] = [];
  let membershipRequests: MembershipRequest[] = [];
  let messages: Message[] = [];
  let route = '';
  let section: 'server' | 'dms' = 'server';
  let overlay: 'connect' | 'settings' | 'admin' | 'create' | 'recovery' | null = null;
  let confirm: { title: string; description: string; execute: () => Promise<void> } | null = null;
  let origin = '';
  let removalPassword = '';
  let removingAccount = false;
  let name = '';
  let body = '';
  let query = '';
  let error = '';
  let connectionError = '';
  let claimCode = '';
  let recoveryCode = '';
  let connected = false;
  let busy = false;
  let refreshing = false;
  let details = true;
  let drafts: Record<string, string> = {};

  $: selected = selectedChannel(channels, route);
  $: manager = canManage(selected, status, identity);

  async function listAccounts() {
    const result = await invoke<{ accounts: LocalAccount[]; legacy_vault: boolean }>(
      'list_accounts',
    );
    accounts = result.accounts;
    legacyVault = result.legacy_vault;
  }
  async function finishUnlock(opened: Status, accountId: string) {
    status = opened;
    activeAccount = accountId || opened.account_id;
    selectedAccount = activeAccount;
    const view = await invoke<View>('conversation_view', { route: null });
    prefs = preferences(view.preferences);
    identity = view.identity;
    origin = opened.origin;
    servers = await action<ServerListing[]>('servers');
    if (opened.burned) {
      error = 'This identity has been burned. Failed server notices will retry; it cannot resume messaging.';
    } else if (opened.ownership_state === 'OWNER_LOCKDOWN') {
      overlay = null;
    } else if (opened.server_id && opened.ownership_state === 'CLAIMED') {
      try {
        await action('authenticate');
        channels = await action<Channel[]>('channels');
      } catch (caught) {
        if (!/CORDS_APPROVAL_PENDING|CORDS_JOIN_REJECTED/.test(String(caught))) throw caught;
        error = String(caught).includes('CORDS_JOIN_REJECTED')
          ? 'Your membership request was rejected. Contact the server owner.'
          : 'Your membership request is awaiting owner approval. Try again after approval.';
        overlay = 'connect';
      }
    } else overlay = 'connect';
  }
  function clearSensitiveUi() {
    status = null;
    identity = null;
    prefs = defaults();
    channels = [];
    servers = [];
    contacts = [];
    membershipRequests = [];
    messages = [];
    route = '';
    body = '';
    drafts = {};
    name = '';
    query = '';
    removalPassword = '';
    origin = '';
    claimCode = '';
    recoveryCode = '';
    connected = false;
    connectionError = '';
    error = '';
    overlay = null;
    confirm = null;
  }
  async function returnToPicker(actionName: 'lock' | 'switch' | 'sign_out') {
    const previous = await invoke<string | null>('session_control', { action: actionName });
    clearSensitiveUi();
    activeAccount = '';
    selectedAccount = actionName === 'switch' ? '' : (previous ?? selectedAccount);
    await listAccounts();
  }
  async function unlockAccount(accountId: string, password: string) {
    const opened = await invoke<Status>('unlock_account', { accountId, password });
    await finishUnlock(opened, accountId);
  }
  async function createAccount(nickname: string, password: string, allowWeak: boolean) {
    const opened = await invoke<Status>('create_account', { nickname, password, allowWeak });
    await listAccounts();
    await finishUnlock(opened, opened.account_id);
  }
  async function migrateAccount(
    nickname: string,
    currentPassword: string,
    password: string,
    allowWeak: boolean,
  ) {
    const opened = await invoke<Status>('migrate_legacy_account', {
      nickname,
      currentPassword: currentPassword || null,
      newPassword: password,
      allowWeak,
    });
    await listAccounts();
    await finishUnlock(opened, opened.account_id);
  }

  async function action<T>(kind: string, fields: Record<string, unknown> = {}): Promise<T> {
    return invoke<T>('conversation_action', { action: { kind, ...fields } });
  }
  async function refresh() {
    if (!status || refreshing) return;
    refreshing = true;
    try {
      const view = await invoke<View>('conversation_view', { route: route || null });
      const oldGeneration = status?.ownership_generation;
      const oldServer = status?.server_id;
      status = view.status;
      if (oldGeneration !== status.ownership_generation || oldServer !== status.server_id) {
        servers = await action<ServerListing[]>('servers');
      }
      identity = view.identity;
      messages = view.messages;
      connected = view.connected;
      connectionError = view.error ?? '';
    } catch (caught) {
      const message = String(caught);
      if (/unlock/i.test(message)) {
        clearSensitiveUi();
        activeAccount = '';
        await listAccounts();
      } else error = message;
    } finally {
      refreshing = false;
    }
  }
  async function run(operation: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await operation();
      await refresh();
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  async function connect() {
    await run(async () => {
      status = await action<Status>('trust', { origin });
      servers = await action<ServerListing[]>('servers');
      if (status.ownership_state === 'CLAIMED') {
        status = await action<Status>('authenticate');
        channels = await action<Channel[]>('channels');
        overlay = null;
        section = 'server';
      } else if (status.ownership_state === 'OWNER_LOCKDOWN') {
        overlay = null;
      }
    });
  }
  async function selectServer(serverId: string) {
    await run(async () => {
      status = await action<Status>('select_server', { server_id: serverId });
      route = '';
      messages = [];
      channels = [];
      contacts = [];
      section = 'server';
      if (status.ownership_state === 'OWNER_LOCKDOWN') {
        overlay = 'recovery';
      } else if (!status.burned && status.ownership_state === 'CLAIMED') {
        status = await action<Status>('authenticate');
        channels = await action<Channel[]>('channels');
      }
      servers = await action<ServerListing[]>('servers');
    });
  }
  function removeServer(serverId: string) {
    overlay = null;
    confirm = {
      title: 'Remove server and erase its data?',
      description: 'Cords will send a root-signed departure, wait for a verified server confirmation, then erase this server’s cached messages, MLS state, and local server data.',
      execute: async () => {
        try {
          await action('remove_server', { server_id: serverId, local_only: false });
          channels = []; route = ''; messages = [];
          servers = await action<ServerListing[]>('servers');
        } catch (caught) {
          const message = String(caught);
          if (/CORDS_SERVER_KEY_CHANGED|connect|timeout|timed out|dns|network/i.test(message)) {
            confirm = {
              title: 'Delete only the local server data?',
              description: `The signed departure could not be verified: ${message}. Local deletion will erase cached messages and MLS state, but remote membership removal was NOT verified.`,
              execute: async () => {
                await action('remove_server', { server_id: serverId, local_only: true });
                channels = []; route = ''; messages = [];
                servers = await action<ServerListing[]>('servers');
                error = 'Server deleted locally. Remote membership removal was not verified.';
              },
            };
          } else throw caught;
        }
      },
    };
  }
  function archiveServer(serverId: string) {
    overlay = null;
    confirm = {
      title: 'Archive this server?',
      description: 'Cords will attempt a signed departure, then remove the active connection and MLS state regardless of the response. The archive keeps only its URL, fingerprint, and departure status.',
      execute: async () => {
        const result = await action<{ remote_confirmed: boolean; warning: string | null }>('archive_server', { server_id: serverId });
        channels = []; route = ''; messages = [];
        servers = await action<ServerListing[]>('servers');
        if (result.warning) error = result.warning;
      },
    };
  }
  function burnIdentity() {
    overlay = null;
    confirm = {
      title: 'Permanently burn this identity?',
      description: `This is for compromise or account deletion only. Cords will sign burn notices for ${servers.length} known server records. Failed deliveries remain pending; unknown historical servers cannot be reached. This account cannot resume messaging. There is no undo.`,
      execute: async () => {
        const result = await action<{ confirmed: number; pending: string[] }>('burn_identity');
        status = (await invoke<View>('conversation_view', { route: null })).status;
        if (result.pending.length) error = `Identity burned locally. Delivery still pending for: ${result.pending.join(', ')}`;
      },
    };
  }
  async function recoverOwner() {
    await run(async () => {
      const code = recoveryCode;
      recoveryCode = '';
      status = await action<Status>('recover_owner', { code });
      servers = await action<ServerListing[]>('servers');
      channels = await action<Channel[]>('channels');
      overlay = null;
    });
  }
  async function selectChannel(id: string) {
    await run(async () => {
      await action('join', { route: id });
      drafts[route] = body;
      route = id;
      body = drafts[id] ?? '';
      section = 'server';
      channels = await action<Channel[]>('channels');
    });
  }
  async function createChannel(channelName: string) {
    const id = await action<string>('create', { name: channelName });
    channels = await action<Channel[]>('channels');
    route = id;
    section = 'server';
    body = '';
  }
  async function openAdmin() {
    await run(async () => {
      contacts = await action<Contact[]>('members');
      channels = await action<Channel[]>('channels');
      membershipRequests = identity?.membership?.capabilities.includes('server.manage')
        ? await action<MembershipRequest[]>('membership_requests')
        : [];
      overlay = 'admin';
    });
  }
  async function decideMembership(device: string, approve: boolean) {
    await action(approve ? 'approve_membership' : 'reject_membership', { device });
    membershipRequests = await action<MembershipRequest[]>('membership_requests');
    contacts = await action<Contact[]>('members');
  }
  async function savePreferences(value: Preferences) {
    const normalized = preferences(value);
    await action('preferences', { value: normalized });
    prefs = normalized;
  }
  async function claimOwnership(code: string) {
    status = await action<Status>('claim_ownership', { code });
    claimCode = '';
    channels = await action<Channel[]>('channels');
    overlay = null;
    section = 'server';
  }
  function revoke() {
    confirm = {
      title: 'Revoke this device?',
      description:
        'This signs a real account-authorized revocation. This installation will no longer authenticate. Delivered history cannot be erased remotely. There is no undo or enrollment recovery flow in this build.',
      execute: async () => {
        await action('revoke');
        await refresh();
      },
    };
  }
  function removeDevice(id: string) {
    confirm = {
      title: 'Remove this channel device?',
      description: `Device ${id} will lose server delivery and be removed by an MLS commit. This is channel removal, not account revocation or a server ban.`,
      execute: async () => {
        await action('remove', { route, device: id });
        channels = await action<Channel[]>('channels');
      },
    };
  }
  onMount(() => {
    void listAccounts().catch((caught) => (error = String(caught)));
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  });
  let activityPending = false;
  function activity() {
    if (!status || activityPending) return;
    activityPending = true;
    void invoke('record_activity').finally(() => {
      window.setTimeout(() => (activityPending = false), 1000);
    });
  }
</script>

<svelte:head><title>Cords — Right on the wire</title></svelte:head>
<svelte:window on:pointerdown={activity} on:keydown={activity} />

{#if !status}
  <div data-theme={prefs.theme}>
    <AccountPicker
      {accounts}
      bind:selected={selectedAccount}
      {legacyVault}
      unlock={unlockAccount}
      create={createAccount}
      migrate={migrateAccount}
      assess={(password, nickname) => invoke('check_password', { password, nickname })}
    />
    {#if error}<div class="startup-global-error error" role="alert">{error}</div>{/if}
  </div>
{:else}
  <div
    class="app-shell"
    data-theme={prefs.theme}
    class:compact={prefs.compact}
    class:reduced-motion={prefs.reducedMotion}
  >
    <div class="app-workspacebar">
      <span class="workspace-mark">C</span> Cords
      <small>{connected ? 'connected' : 'local state'}</small>
    </div>
    <div class="app-layout">
      <CordsRail
        {status}
        {servers}
        preferences={prefs}
        {section}
        select={(next) => {
          section = next;
          query = '';
        }}
        connect={() => (overlay = 'connect')}
        settings={() => (overlay = 'settings')}
        selectServer={(serverId) => void selectServer(serverId)}
      />
      <aside class="left-column">
        <ConversationSidebar
          {status}
          {identity}
          {channels}
          {route}
          {section}
          {busy}
          selectChannel={(id) => void selectChannel(id)}
          createChannel={() => (overlay = 'create')}
          openAdmin={() => void openAdmin()}
          openConnection={() => (overlay = 'connect')}
          synchronize={() =>
            void run(async () => {
              channels = await action<Channel[]>('channels');
              await action('synchronize');
            })}
        />
        <IdentityPanel
          {status}
          {identity}
          preferences={prefs}
          {connected}
          openSettings={() => (overlay = 'settings')}
          switchAccount={() => void returnToPicker('switch')}
          removeAccount={() => (removingAccount = true)}
        />
      </aside>
      <div class="conversation-column">
        <CordsHeader
          {section}
          channel={selected}
          {query}
          detailsVisible={details}
          setQuery={(value) => (query = value)}
          toggleDetails={() => (details = !details)}
          openSettings={() => (overlay = 'settings')}
        />
        {#if error || connectionError}<div role="alert" class="connection-error">
            <strong>Action or connection failed</strong><span>{error || connectionError}</span>
          </div>{/if}
        <ConversationView
          {section}
          channel={selected}
          {messages}
          {status}
          preferences={prefs}
          {query}
          {body}
          {busy}
          revoked={identity?.revoked ?? false}
          setBody={(value) => {
            body = value;
            drafts[route] = value;
          }}
          send={() =>
            void run(async () => {
              await action('send', { route, body });
              body = '';
              drafts[route] = '';
            })}
        />
      </div>
      {#if details}<ConversationDetails
          channel={selected}
          {status}
          {manager}
          {busy}
          close={() => (details = false)}
          {removeDevice}
          addDevice={(device) =>
            void run(async () => {
              await action('add', { route, device });
              channels = await action<Channel[]>('channels');
            })}
          publish={() =>
            void run(async () => {
              await action('publish');
            })}
        />{/if}
    </div>

    {#if overlay === 'settings'}<Settings
        value={prefs}
        {identity}
        {status}
        {servers}
        close={() => (overlay = null)}
        save={savePreferences}
        lockNow={() => returnToPicker('lock')}
        {revoke}
        {removeServer}
        {archiveServer}
        {burnIdentity}
        designateSuccessor={(accountId) => action<string>('designate_successor', { account_id: accountId })}
        acceptSuccessor={(designationHash) => action<void>('accept_successor', { designation_hash: designationHash })}
      />{/if}
    {#if overlay === 'admin'}<Admin
        {status}
        {identity}
        {channels}
        {contacts}
        {membershipRequests}
        close={() => (overlay = null)}
        create={createChannel}
        select={selectChannel}
        decideMembership={decideMembership}
      />{/if}
    {#if overlay === 'connect'}<Modal title="Connection and trust" close={() => (overlay = null)}
        ><div class="modal-body connect-dialog">
          <p>
            Cords validates HTTPS, verifies signed discovery, and pins the server identity returned
            by this URL. A later identity change is refused.
          </p>
          <form on:submit|preventDefault={connect}>
            <label
              >HTTPS origin<input
                type="url"
                placeholder="https://your-server.example:4848"
                bind:value={origin}
                required
              /></label
            ><button class="primary" disabled={busy || identity?.revoked}>Trust and continue</button
            >
          </form>
          {#if status?.ownership_state === 'UNCLAIMED'}
            <section class="settings-card ownership-claim">
              <h2>Claim this unclaimed server</h2>
              <p>
                Enter the one-time code generated by the server. It appears only in the initial
                bootstrap log, or after an operator runs the local rotate command.
              </p>
              <form
                on:submit|preventDefault={() =>
                  void run(async () => {
                    const code = claimCode;
                    claimCode = '';
                    await claimOwnership(code);
                  })}
              >
                <label
                  >One-time claim code<input
                    bind:value={claimCode}
                    required
                    minlength="43"
                    maxlength="43"
                    spellcheck="false"
                    autocomplete="off"
                  /></label
                ><button class="primary" disabled={busy || identity?.revoked}>Claim server</button>
              </form>
            </section>
          {/if}
          <div class="account-banner">
            <LockKeyhole size={18} /><span>Each trusted server keeps its own pin, session, MLS state, and encrypted cache.</span>
          </div>
          {#if error}<p role="alert" class="error">{error}</p>{/if}
        </div></Modal
      >{/if}
    {#if overlay === 'recovery'}<Modal title="Owner recovery required" close={() => (overlay = null)}>
        <form class="modal-body" on:submit|preventDefault={recoverOwner}>
          <p>This server is in OWNER_LOCKDOWN. Messages and membership changes are paused. The one-time recovery code is available only to the actual server operator through the server’s local administrative command or logs—not to ordinary members.</p>
          <p>An incorrect code will not trigger another prompt. Ask the operator to generate and share a fresh code only if you are the intended new owner.</p>
          <label>Operator recovery code<input type="password" bind:value={recoveryCode} minlength="43" maxlength="43" autocomplete="off" spellcheck="false" required /></label>
          <button class="primary" disabled={busy || recoveryCode.length !== 43}>Claim recovered ownership</button>
          {#if error}<p role="alert" class="error">{error}</p>{/if}
        </form>
      </Modal>{/if}
    {#if overlay === 'create'}<Modal title="Create encrypted channel" close={() => (overlay = null)}
        ><form
          class="modal-body"
          on:submit|preventDefault={() =>
            void run(async () => {
              await createChannel(name);
              name = '';
              overlay = null;
            })}
        >
          <label>Channel name<input bind:value={name} required maxlength="100" /></label><button
            class="primary"
            disabled={busy}>Create encrypted channel</button
          >{#if error}<p role="alert" class="error">{error}</p>{/if}
        </form></Modal
      >{/if}
    {#if removingAccount}<Modal
        title="Remove Account From Device"
        close={() => (removingAccount = false)}
      >
        <form
          class="modal-body"
          on:submit|preventDefault={() =>
            void run(async () => {
              await invoke('remove_local_account', {
                accountId: activeAccount,
                password: removalPassword,
              });
              removalPassword = '';
              removingAccount = false;
              clearSensitiveUi();
              activeAccount = '';
              selectedAccount = '';
              await listAccounts();
            })}
        >
          <p>
            This deletes only this device’s encrypted local vault and key material. It does not
            delete the identity elsewhere, revoke any device, or burn the account root key.
          </p>
          <label
            >Confirm with the local storage password<input
              type="password"
              bind:value={removalPassword}
              autocomplete="current-password"
              required
            /></label
          >
          <button class="danger" disabled={!removalPassword}>Remove local account</button>
        </form>
      </Modal>{/if}
    {#if confirm}<Modal
        title={confirm.title}
        close={() => {
          if (!busy) confirm = null;
        }}
        ><div class="modal-body">
          <p>{confirm.description}</p>
          {#if error}<p class="error" role="alert">{error}</p>{/if}
          <div class="actions">
            <button disabled={busy} on:click={() => (confirm = null)}>Cancel</button><button
              class="danger"
              disabled={busy}
              on:click={() =>
                void run(async () => {
                  const current = confirm;
                  await current?.execute();
                  if (confirm === current) confirm = null;
                })}>Confirm action</button
            >
          </div>
        </div></Modal
      >{/if}
  </div>
{/if}
