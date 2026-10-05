<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { KeyRound, LockKeyhole } from '@lucide/svelte';
  import { onMount } from 'svelte';
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
    type View,
    type Preferences,
  } from './model';

  let status: Status | null = null;
  let identity: Identity | null = null;
  let prefs = defaults();
  let channels: Channel[] = [];
  let contacts: Contact[] = [];
  let messages: Message[] = [];
  let route = '';
  let section: 'server' | 'dms' = 'server';
  let overlay: 'connect' | 'settings' | 'admin' | 'create' | null = null;
  let confirm: { title: string; description: string; execute: () => Promise<void> } | null = null;
  let origin = '';
  let passphrase = '';
  let name = '';
  let body = '';
  let query = '';
  let error = '';
  let connectionError = '';
  let connected = false;
  let busy = false;
  let refreshing = false;
  let details = true;
  const drafts: Record<string, string> = {};

  $: selected = selectedChannel(channels, route);
  $: manager = canManage(selected, status, identity);

  async function action<T>(kind: string, fields: Record<string, unknown> = {}): Promise<T> {
    return invoke<T>('conversation_action', { action: { kind, ...fields } });
  }
  async function refresh() {
    if (!status || refreshing) return;
    refreshing = true;
    try {
      const view = await invoke<View>('conversation_view', { route: route || null });
      status = view.status;
      identity = view.identity;
      messages = view.messages;
      connected = view.connected;
      connectionError = view.error ?? '';
    } catch (caught) {
      error = String(caught);
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
  async function unlock() {
    const material = passphrase || null;
    passphrase = '';
    await run(async () => {
      status = await invoke<Status>('open_client', { passphrase: material });
      const view = await invoke<View>('conversation_view', { route: null });
      prefs = preferences(view.preferences);
      identity = view.identity;
      origin = status.origin;
      if (status.server_id) {
        await action('authenticate');
        channels = await action<Channel[]>('channels');
      } else overlay = 'connect';
    });
  }
  async function connect() {
    await run(async () => {
      await action('trust', { origin });
      status = await action<Status>('authenticate');
      channels = await action<Channel[]>('channels');
      overlay = null;
      section = 'server';
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
      overlay = 'admin';
    });
  }
  async function savePreferences(value: Preferences) {
    const normalized = preferences(value);
    await action('preferences', { value: normalized });
    prefs = normalized;
  }
  async function claimOwnership(code: string) {
    await action('claim_ownership', { code });
    await refresh();
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
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  });
</script>

<svelte:head><title>Cords — Right on the wire</title></svelte:head>

{#if !status}
  <div class="startup-shell" data-theme={prefs.theme}>
    <main class="startup-surface">
      <section class="startup-card">
        <div class="startup-mark"><LockKeyhole size={30} /></div>
        <p class="eyebrow">Portable identity</p>
        <h1>Unlock this installation</h1>
        <p>
          Your account and device keys stay on this device. Unlock local encrypted state before
          connecting to a server.
        </p>
        <form on:submit|preventDefault={unlock}>
          <label
            >Local storage passphrase<input
              type="password"
              autocomplete="current-password"
              bind:value={passphrase}
            /></label
          >
          <p class="hint">
            Leave empty to use the operating-system credential store. A new installation is
            initialized on first unlock.
          </p>
          <button class="primary" disabled={busy}
            ><KeyRound size={17} />{busy ? 'Opening…' : 'Unlock or initialize'}</button
          >
        </form>
        {#if error}<div role="alert" class="error">
            <strong>Could not open this installation</strong>
            <p>{error}</p>
          </div>{/if}
      </section>
    </main>
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
        preferences={prefs}
        {section}
        select={(next) => {
          section = next;
          query = '';
        }}
        connect={() => (overlay = 'connect')}
        settings={() => (overlay = 'settings')}
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
        close={() => (overlay = null)}
        save={savePreferences}
        {revoke}
      />{/if}
    {#if overlay === 'admin'}<Admin
        {status}
        {identity}
        {channels}
        {contacts}
        close={() => (overlay = null)}
        create={createChannel}
        select={selectChannel}
        generateOwnershipCode={() => action<string>('ownership_code')}
        {claimOwnership}
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
            ><button class="primary" disabled={busy || identity?.revoked}
              >Trust and authenticate</button
            >
          </form>
          <div class="account-banner">
            <LockKeyhole size={18} /><span
              >This build supports one pinned server per installation. Existing encrypted state is
              never reset to switch servers.</span
            >
          </div>
          {#if error}<p role="alert" class="error">{error}</p>{/if}
        </div></Modal
      >{/if}
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
                  await confirm?.execute();
                  confirm = null;
                })}>Confirm action</button
            >
          </div>
        </div></Modal
      >{/if}
  </div>
{/if}
