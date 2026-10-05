<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { connectionLabel, type ConnectionState } from './status';

  type InspectedServer = {
    origin: string;
    serverId: string;
    serverName: string;
    protocol: number;
    features: string[];
    joinPolicy: string[];
    signatureStatus: 'valid';
    trustStatus: 'not_pinned';
  };

  type DemoChannel = 'welcome' | 'architecture' | 'security';

  const demoChannels = {
    welcome: {
      eyebrow: '# welcome',
      title: 'Right on the wire',
      heading: 'Welcome to the Cords foundation',
      description:
        'This screen proves the desktop shell and authenticated discovery boundary. The conversation below is local fixture content—not a server message history.',
      messages: [
        {
          initials: 'AR',
          color: 'violet',
          author: 'Architecture',
          label: 'Phase 0',
          body: 'The server relays policy and ciphertext. Authorized clients retain plaintext.',
        },
        {
          initials: 'TS',
          color: 'blue',
          author: 'Trust state',
          label: 'Local inspection',
          body: 'Signed discovery proves consistency with the presented server key. Pinning begins in Phase 1 and is not implied here.',
        },
      ],
    },
    architecture: {
      eyebrow: '# architecture',
      title: 'Client-held plaintext',
      heading: 'A narrow trust boundary',
      description:
        'Cords separates the desktop client, independently hosted servers, and encrypted conversation state.',
      messages: [
        {
          initials: 'CL',
          color: 'violet',
          author: 'Desktop client',
          label: 'Trusted endpoint',
          body: 'The client owns account keys, device keys, decrypted messages, and local search data.',
        },
        {
          initials: 'SV',
          color: 'blue',
          author: 'Cords server',
          label: 'Policy and delivery',
          body: 'The server controls membership and routing while message bodies remain encrypted.',
        },
      ],
    },
    security: {
      eyebrow: '# security',
      title: 'Verify before trusting',
      heading: 'TLS plus persistent identity',
      description:
        'Discovery validates normal HTTPS and a signed server identity before a server can be trusted by the client.',
      messages: [
        {
          initials: 'TLS',
          color: 'violet',
          author: 'Transport',
          label: 'Web PKI',
          body: 'The native client requires HTTPS certificate validation for server discovery.',
        },
        {
          initials: 'ID',
          color: 'blue',
          author: 'Server identity',
          label: 'Signed metadata',
          body: 'The server signing key is separate from TLS and becomes a persistent pin after user approval.',
        },
      ],
    },
  } as const;

  let origin = 'https://localhost:4848';
  let state: ConnectionState = 'idle';
  let server: InspectedServer | null = null;
  let error = '';
  let activeChannel: DemoChannel = 'welcome';
  let originInput: HTMLInputElement;
  $: channel = demoChannels[activeChannel];

  function selectChannel(channelName: DemoChannel) {
    activeChannel = channelName;
  }

  function startServerDiscovery() {
    activeChannel = 'welcome';
    originInput.focus();
    originInput.select();
  }

  async function inspectServer() {
    state = 'connecting';
    server = null;
    error = '';
    try {
      server = await invoke<InspectedServer>('inspect_server', { origin });
      state = 'connected';
    } catch (reason) {
      error = typeof reason === 'string' ? reason : 'The server inspection failed.';
      state = 'failed';
    }
  }
</script>

<svelte:head>
  <meta name="description" content="Cords authenticated discovery demonstration" />
</svelte:head>

<main class="app-shell">
  <nav class="server-rail" aria-label="Servers">
    <div class="brand" aria-label="Cords">C</div>
    <button
      class="server-dot selected"
      type="button"
      aria-label="Local UI demo"
      title="Return to welcome"
      on:click={() => selectChannel('welcome')}>LD</button
    >
    <button
      class="add-server"
      type="button"
      aria-label="Add server"
      title="Inspect another server"
      on:click={startServerDiscovery}>+</button
    >
  </nav>

  <aside class="channel-panel">
    <header>
      <p class="eyebrow">Cords</p>
      <h1>{server?.serverName ?? 'Server discovery'}</h1>
    </header>

    <section class="connect-card" aria-labelledby="connect-title">
      <h2 id="connect-title">Inspect a server</h2>
      <p>HTTPS and signed metadata are both verified by the native client.</p>
      <form on:submit|preventDefault={inspectServer}>
        <label for="origin">Server origin</label>
        <input
          id="origin"
          bind:this={originInput}
          bind:value={origin}
          type="url"
          required
          spellcheck="false"
          autocomplete="url"
          placeholder="https://example.com:4848"
        />
        <button class="primary" type="submit" disabled={state === 'connecting'}>
          {state === 'connecting' ? 'Inspecting…' : 'Inspect server'}
        </button>
      </form>
    </section>

    <section class="channels" aria-labelledby="channels-title">
      <div class="section-heading">
        <h2 id="channels-title">Text channels</h2>
        <span>Demo</span>
      </div>
      {#each Object.keys(demoChannels) as channelName (channelName)}
        <button
          class:active={activeChannel === channelName}
          class="channel"
          type="button"
          aria-pressed={activeChannel === channelName}
          on:click={() => selectChannel(channelName as DemoChannel)}
          ><span>#</span> {channelName}</button
        >
      {/each}
    </section>

    <footer>
      <span class:online={state === 'connected'} class="status-dot"></span>
      <div>
        <strong>{connectionLabel(state)}</strong>
        <small>{server ? `Protocol v${server.protocol}` : 'Phase 0'}</small>
      </div>
    </footer>
  </aside>

  <section class="content-panel" aria-live="polite">
    <header class="conversation-header">
      <div>
        <p class="eyebrow">{channel.eyebrow}</p>
        <h2>{channel.title}</h2>
      </div>
      <span class="demo-badge">Local UI demo · no messaging</span>
    </header>

    <div class="timeline">
      <div class="intro-mark" aria-hidden="true">C</div>
      <h3>{channel.heading}</h3>
      <p class="intro-copy">{channel.description}</p>

      {#each channel.messages as message (message.author)}
        <article class="message">
          <div
            class:violet={message.color === 'violet'}
            class:blue={message.color === 'blue'}
            class="avatar"
          >
            {message.initials}
          </div>
          <div>
            <div class="message-meta">
              <strong>{message.author}</strong><time>{message.label}</time>
            </div>
            <p>{message.body}</p>
          </div>
        </article>
      {/each}

      {#if server}
        <article class="result-card success">
          <div class="result-title">
            <span aria-hidden="true">✓</span>
            <div><strong>{server.serverName}</strong><small>{server.origin}</small></div>
          </div>
          <dl>
            <div>
              <dt>Signature</dt>
              <dd>{server.signatureStatus}</dd>
            </div>
            <div>
              <dt>Trust</dt>
              <dd>{server.trustStatus.replace('_', ' ')}</dd>
            </div>
            <div>
              <dt>Protocol</dt>
              <dd>v{server.protocol}</dd>
            </div>
            <div>
              <dt>Server ID</dt>
              <dd class="fingerprint">{server.serverId}</dd>
            </div>
          </dl>
        </article>
      {:else if error}
        <article class="result-card error" role="alert">
          <strong>Server inspection failed</strong>
          <p>{error}</p>
        </article>
      {/if}
    </div>

    <div class="composer" aria-disabled="true">
      <span>Messaging is not implemented in Phase 0</span>
      <button type="button" disabled aria-label="Send message">↑</button>
    </div>
  </section>
</main>
