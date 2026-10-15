<script lang="ts">
  import { ArrowRight, Hash, LockKeyhole, Send, Server } from '@lucide/svelte';
  import RemoteAvatar from './RemoteAvatar.svelte';
  import CopyButton from './CopyButton.svelte';
  import {
    memberName,
    memberPicture,
    type Contact,
    type Channel,
    type Message,
    type Preferences,
    type Status,
  } from '../model';
  export let contacts: Contact[] = [];
  function sender(id: string) {
    return contacts.find((contact) => contact.authorization.value.device_id === id);
  }
  function name(id: string) {
    return id === status.device_id ? preferences.displayName : memberName(sender(id), id);
  }
  export let section: 'server' | 'dms';
  export let channel: Channel | undefined;
  export let messages: Message[];
  export let status: Status;
  export let preferences: Preferences;
  export let query: string;
  export let body: string;
  export let busy: boolean;
  export let revoked: boolean;
  export let setBody: (value: string) => void;
  export let send: () => void;
  export let openConnection: () => void = () => undefined;
  const messageWindow = 500;
  let visibleLimit = messageWindow;
  $: filtered = messages.filter((message) =>
    `${name(message.sender_device_id)} ${message.sender_device_id} ${message.body}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  $: visible = filtered.slice(-visibleLimit);
  $: restriction = revoked
    ? 'This device is revoked. Sending is unavailable.'
    : channel?.locally_archived
      ? 'This retired channel is a read-only local archive.'
      : channel?.requires_public_acknowledgement
        ? 'Review and acknowledge the public channel notice before sending.'
        : '';
</script>

<main class="cords-conversation">
  <div class="cords-timeline" class:workspace-empty={!channel} aria-label="Conversation messages">
    {#if section === 'dms'}
      <div class="conversation-intro">
        <span class="intro-icon"><LockKeyhole size={25} /></span>
        <h1>Direct messages</h1>
        <p>Private two-account MLS conversations will appear here.</p>
        <div class="intro-note">
          The navigation and conversation surface are present, but the current core does not expose
          DM creation or delivery.
        </div>
      </div>
    {:else if !channel}
      <div class="workspace-welcome">
        <div class="welcome-art" aria-hidden="true">
          <span class="welcome-orbit"></span><span class="welcome-mark">C<span></span></span>
        </div>
        <span class="eyebrow">RIGHT ON THE WIRE</span>
        <h1>{status.server_id ? 'Find your conversation.' : 'Your people. Your servers.'}</h1>
        <p>
          {status.server_id
            ? 'Choose a channel from the sidebar to pick up the conversation.'
            : 'A familiar place to talk, with an identity that belongs to you. Connect to a community to get started.'}
        </p>
        {#if !status.server_id}
          <button class="primary welcome-connect" on:click={openConnection}
            >Connect to a server <ArrowRight size={18} /></button
          >
        {/if}
        <div class="welcome-context">
          {#if status.server_id}<Hash size={21} />
            <div>
              <strong>{new URL(status.origin).hostname}</strong><span
                >Your server is selected. Channel access depends on your membership.</span
              >
            </div>
          {:else}<Server size={21} />
            <div>
              <strong>Independent communities</strong><span
                >Connect directly using your server’s HTTPS address.</span
              >
            </div>{/if}
        </div>
        <div class="welcome-privacy">
          <LockKeyhole size={14} /><span>Your account is unlocked on this device.</span>
        </div>
      </div>
    {:else}
      <div class="conversation-intro" class:has-messages={messages.length > 0}>
        <span class="intro-icon">#</span>
        <h1>#{channel.name}</h1>
        <p>
          {channel.confidentiality_mode === 'public'
            ? 'True public · signed messages · no end-to-end encryption'
            : 'End-to-end encrypted conversation'}
        </p>
        {#if channel.locally_archived}<p>
            Retired channel · partial local archive · read-only
          </p>{/if}
        {#if channel.confidentiality_mode === 'public' || !messages.length}<div class="intro-note">
            {#if channel.confidentiality_mode === 'public'}The server and anyone with access can
              read, copy, and retain these messages. Sender identities are cryptographically
              verified. HTTPS protects transport.{:else}New devices receive current and future
              state, not earlier plaintext.{/if}
          </div>{/if}
      </div>
      <div class="timeline-divider"><span>Local history</span></div>
      {#if filtered.length > visibleLimit}
        <button class="load-earlier" on:click={() => (visibleLimit += messageWindow)}
          >Show {Math.min(messageWindow, filtered.length - visibleLimit)} earlier messages</button
        >
      {/if}
      {#each visible as message (message.message_id)}
        <article class="cords-message">
          <RemoteAvatar
            value={message.sender_device_id === status.device_id
              ? preferences.avatar
              : memberPicture(sender(message.sender_device_id))}
            text={message.sender_device_id === status.device_id
              ? preferences.displayName.slice(0, 2)
              : name(message.sender_device_id).slice(0, 2)}
            size={42}
          />
          <div class="message-content">
            <div class="message-meta">
              <strong
                title={message.sender_device_id}
                class:message-own={message.sender_device_id === status.device_id}
                >{message.sender_device_id === status.device_id
                  ? preferences.displayName
                  : name(message.sender_device_id)}</strong
              ><time
                datetime={new Date(message.client_timestamp * 1000).toISOString()}
                title={new Date(message.client_timestamp * 1000).toLocaleString()}
                >{new Date(message.client_timestamp * 1000).toLocaleString(undefined, {
                  month: 'short',
                  day: 'numeric',
                  hour: 'numeric',
                  minute: '2-digit',
                })}</time
              >
              <details class="message-actions">
                <summary aria-label="Message actions">•••</summary>
                <div class="context-menu inline-menu">
                  <CopyButton value={message.body} label="Copy text" />
                  <CopyButton value={message.message_id} label="Copy message ID" />
                </div>
              </details>
            </div>
            <p>{message.body}</p>
          </div>
        </article>
      {:else}
        <div class="no-results">
          {query ? `No messages match “${query}”.` : 'No messages in local history yet.'}
        </div>
      {/each}
    {/if}
  </div>
  {#if channel && section === 'server'}
    {#if restriction}<p class="composer-restriction" id="composer-restriction" role="status">
        <LockKeyhole size={15} />{restriction}
      </p>{/if}
    <form class="cords-composer" autocomplete="off" on:submit|preventDefault={send}>
      <label for="message-draft" class="sr-only">Message</label>
      <input
        id="message-draft"
        autocomplete="off"
        aria-describedby={restriction ? 'composer-restriction' : undefined}
        value={body}
        on:input={(event) => setBody(event.currentTarget.value)}
        disabled={busy ||
          revoked ||
          channel?.locally_archived ||
          channel?.requires_public_acknowledgement}
        placeholder={`Message #${channel.name}`}
      />
      <button
        type="submit"
        disabled={busy ||
          revoked ||
          channel?.locally_archived ||
          channel?.requires_public_acknowledgement ||
          !body.trim()}
        title="Send message"
        aria-label="Send message"><Send size={19} /></button
      >
    </form>
  {/if}
</main>
