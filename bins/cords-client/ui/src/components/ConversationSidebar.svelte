<script lang="ts">
  import { ChevronDown, Hash, LockKeyhole, Plus } from '@lucide/svelte';
  import { availability, type Channel, type Identity, type Status } from '../model';

  export let status: Status;
  export let identity: Identity | null;
  export let channels: Channel[];
  export let route: string;
  export let section: 'server' | 'dms';
  export let busy: boolean;
  export let selectChannel: (id: string) => void;
  export let createChannel: () => void;
  export let openAdmin: () => void;
  export let openConnection: () => void;
  export let synchronize: () => void;
  export let leaveServer: () => void;
  export let archiveServer: () => void;
  let filter = '';
  const directMessages = availability(false);
</script>

<div class="channel-navigation">
  <div class="community-title">
    <strong
      >{section === 'dms'
        ? 'Direct messages'
        : new URL(status.origin || 'https://cords.invalid').hostname}</strong
    >
    {#if section === 'server'}
      <details class="surface-menu">
        <summary aria-label="Server menu"><ChevronDown size={17} /></summary>
        <div class="context-menu inline-menu" role="menu">
          <button role="menuitem" disabled={!status.server_id || busy} on:click={openAdmin}
            >Server settings</button
          >
          <button role="menuitem" on:click={openConnection}>Connection and trust</button>
          <button role="menuitem" disabled={!status.server_id || busy} on:click={synchronize}
            >Refresh and synchronize</button
          >
          <button
            role="menuitem"
            class="danger"
            disabled={!status.server_id || busy}
            on:click={leaveServer}>Leave server…</button
          >
          <button role="menuitem" disabled={!status.server_id || busy} on:click={archiveServer}
            >Archive server…</button
          >
          <button
            class="dev-unimplemented divided"
            disabled
            aria-disabled="true"
            title="Not implemented yet">Invite people</button
          >
          <button
            class:dev-unimplemented={directMessages === 'development'}
            disabled
            aria-disabled="true"
            title="Not implemented yet">Notification settings</button
          >
        </div>
      </details>
    {/if}
  </div>
  <div class="channels-scroll">
    {#if section === 'dms'}
      <div class="sidebar-section-title">RECENT CONVERSATIONS</div>
      <div class="sidebar-empty">
        <MessageSquareTextIcon />
        <strong>No direct messages yet</strong>
        <small>The current client core does not expose two-account MLS conversations.</small>
        <button class="dev-unimplemented" disabled aria-disabled="true" title="Not implemented yet"
          >New direct message</button
        >
      </div>
    {:else}
      <div class="server-origin"><LockKeyhole size={14} /><span>{status.origin}</span></div>
      <label class="sidebar-search"
        ><span class="sr-only">Find a channel</span><input
          type="search"
          placeholder="Find a channel"
          bind:value={filter}
        /></label
      >
      <div class="group-title">
        <ChevronDown size={12} />TEXT CHANNELS
        <button
          aria-label="Create channel"
          disabled={!identity?.membership?.capabilities.includes('channel.create') ||
            busy ||
            status.ownership_state === 'OWNER_LOCKDOWN' ||
            status.burned}
          on:click={createChannel}><Plus size={15} /></button
        >
      </div>
      {#each channels.filter((channel) => channel.name
          .toLowerCase()
          .includes(filter.toLowerCase())) as channel (channel.channel_id)}
        <button
          class="channel-row"
          class:active={route === channel.channel_id}
          on:click={() => selectChannel(channel.channel_id)}
          disabled={busy ||
            !channel.members.some(
              (member) => member.authorization.value.device_id === status.device_id,
            )}
          title={channel.members.some(
            (member) => member.authorization.value.device_id === status.device_id,
          )
            ? `Open #${channel.name}`
            : 'Visible server channel; waiting to be added to its encrypted conversation'}
        >
          <Hash size={20} /><span>{channel.name}</span
          >{#if !channel.members.some((member) => member.authorization.value.device_id === status.device_id)}<LockKeyhole
              size={14}
            />{/if}
        </button>
      {:else}
        <div class="sidebar-empty compact"><small>No accessible channels.</small></div>
      {/each}
    {/if}
    <div class="channel-sidebar-spacer"></div>
  </div>
</div>

{#snippet MessageSquareTextIcon()}<span class="empty-glyph">DM</span>{/snippet}
