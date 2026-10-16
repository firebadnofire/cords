<script lang="ts">
  import { ChevronDown, Globe, Hash, LockKeyhole, Plus, Server } from '@lucide/svelte';
  import { type Channel, type Identity, type Status } from '../model';

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
  let serverMenuOpen = false;

  function runServerAction(action: () => void) {
    serverMenuOpen = false;
    action();
  }
</script>

<div class="channel-navigation">
  <div class="community-title">
    <strong
      >{section === 'dms'
        ? 'Direct messages'
        : status.server_id && status.origin
          ? new URL(status.origin).hostname
          : 'Your servers'}</strong
    >
    {#if section === 'server' && status.server_id}
      <details class="surface-menu" bind:open={serverMenuOpen}>
        <summary aria-label="Server menu"><ChevronDown size={17} /></summary>
        <div class="context-menu inline-menu" role="menu">
          <button
            role="menuitem"
            disabled={!status.server_id || busy}
            on:click={() => runServerAction(openAdmin)}>Server settings</button
          >
          <button role="menuitem" on:click={() => runServerAction(openConnection)}
            >Connection and trust</button
          >
          <button
            role="menuitem"
            disabled={!status.server_id || busy}
            on:click={() => runServerAction(synchronize)}>Refresh and synchronize</button
          >
          <button
            role="menuitem"
            class="danger"
            disabled={!status.server_id || busy}
            on:click={() => runServerAction(leaveServer)}>Leave server…</button
          >
          <button
            role="menuitem"
            disabled={!status.server_id || busy}
            on:click={() => runServerAction(archiveServer)}>Archive server…</button
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
      </div>
    {:else if !status.server_id}
      <div class="sidebar-empty server-empty">
        <span class="empty-glyph"><Server size={22} /></span>
        <strong>A place for your communities</strong>
        <small>Connect to a Cords server to see its channels here.</small>
        <button class="sidebar-connect" on:click={openConnection}
          ><Plus size={16} /> Connect to server</button
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
            (channel.confidentiality_mode !== 'public' &&
              !channel.locally_archived &&
              !channel.members.some(
                (member) => member.authorization.value.device_id === status.device_id,
              ))}
          title={channel.members.some(
            (member) => member.authorization.value.device_id === status.device_id,
          ) ||
          channel.confidentiality_mode === 'public' ||
          channel.locally_archived
            ? `Open #${channel.name}`
            : 'Visible server channel; waiting to be added to its encrypted conversation'}
        >
          <Hash size={20} /><span
            >{channel.name}{channel.locally_archived
              ? ' · retired'
              : channel.confidentiality_mode === 'public'
                ? ' · public'
                : ''}</span
          >{#if channel.confidentiality_mode === 'public'}<Globe
              size={14}
            />{:else if !channel.members.some((member) => member.authorization.value.device_id === status.device_id)}<LockKeyhole
              size={14}
            />{/if}
        </button>
      {:else}
        <div class="sidebar-empty compact">
          <strong>{filter ? 'No matching channels' : 'No channels available'}</strong><small
            >{filter
              ? 'Try another channel name.'
              : 'Channels you can access will appear here.'}</small
          >
        </div>
      {/each}
    {/if}
    <div class="channel-sidebar-spacer"></div>
  </div>
</div>

{#snippet MessageSquareTextIcon()}<span class="empty-glyph">DM</span>{/snippet}
