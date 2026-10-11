<script lang="ts">
  import { LockKeyhole, Send } from '@lucide/svelte';
  import RemoteAvatar from './RemoteAvatar.svelte';
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
  const messageWindow = 500;
  let visibleLimit = messageWindow;
  $: filtered = messages.filter((message) =>
    `${name(message.sender_device_id)} ${message.sender_device_id} ${message.body}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  $: visible = filtered.slice(-visibleLimit);
</script>

<main class="cords-conversation">
  <div class="cords-timeline" aria-label="Conversation messages">
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
      <div class="conversation-intro">
        <span class="intro-icon">C</span>
        <h1>Welcome to Cords</h1>
        <p>Select a channel or connect to your server.</p>
      </div>
    {:else}
      <div class="conversation-intro">
        <span class="intro-icon">#</span>
        <h1>#{channel.name}</h1>
        <p>
          {channel.confidentiality_mode === 'public'
            ? 'True public · signed messages · no end-to-end encryption'
            : `Encrypted conversation · MLS epoch ${channel.epoch}`}
        </p>
        {#if channel.locally_archived}<p>
            Retired channel · partial local archive · read-only
          </p>{/if}
        <div class="intro-note">
          {#if channel.confidentiality_mode === 'public'}The server and anyone with access can read,
            copy, and retain these messages. Sender identities are cryptographically verified. HTTPS
            protects transport.{:else}New devices receive current and future state, not earlier
            plaintext.{/if}
        </div>
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
              ><time datetime={new Date(message.client_timestamp * 1000).toISOString()}
                >{new Date(message.client_timestamp * 1000).toLocaleString()}</time
              >
              <details class="message-actions">
                <summary aria-label="Message actions">•••</summary>
                <div class="context-menu inline-menu">
                  <button on:click={() => navigator.clipboard.writeText(message.body)}
                    >Copy text</button
                  ><button on:click={() => navigator.clipboard.writeText(message.message_id)}
                    >Copy message ID</button
                  ><button
                    class="dev-unimplemented divided"
                    disabled
                    aria-disabled="true"
                    title="Not implemented yet">Reply</button
                  ><button
                    class="dev-unimplemented"
                    disabled
                    aria-disabled="true"
                    title="Not implemented yet">Edit message</button
                  ><button
                    class="dev-unimplemented"
                    disabled
                    aria-disabled="true"
                    title="Not implemented yet">Delete message</button
                  ><button
                    class="dev-unimplemented danger"
                    disabled
                    aria-disabled="true"
                    title="Not implemented yet">Report</button
                  >
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
  <form class="cords-composer" autocomplete="off" on:submit|preventDefault={send}>
    <button
      type="button"
      class="composer-tool dev-unimplemented"
      disabled
      aria-disabled="true"
      title="Not implemented yet">+</button
    >
    <label for="message-draft" class="sr-only">Message</label>
    <input
      id="message-draft"
      autocomplete="off"
      value={body}
      on:input={(event) => setBody(event.currentTarget.value)}
      disabled={!channel ||
        section === 'dms' ||
        busy ||
        revoked ||
        channel?.locally_archived ||
        channel?.requires_public_acknowledgement}
      placeholder={section === 'dms'
        ? 'Direct messaging is not available'
        : channel
          ? `Message #${channel.name}`
          : 'Select a channel'}
    />
    <button
      type="submit"
      disabled={!channel ||
        section === 'dms' ||
        busy ||
        revoked ||
        channel?.locally_archived ||
        channel?.requires_public_acknowledgement ||
        !body.trim()}
      title="Send message"
      aria-label="Send message"><Send size={19} /></button
    >
  </form>
</main>
