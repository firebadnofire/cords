<script lang="ts">
  import {
    Hash,
    Info,
    LockKeyhole,
    Menu,
    MessageSquareText,
    Search,
    Settings2,
    Users,
  } from '@lucide/svelte';
  import type { Channel } from '../model';
  export let section: 'server' | 'dms';
  export let channel: Channel | undefined;
  export let query: string;
  export let detailsVisible: boolean;
  export let setQuery: (value: string) => void;
  export let toggleDetails: () => void;
  export let membersVisible = false;
  export let membersAvailable = false;
  export let toggleMembers: () => void = () => undefined;
  export let openSettings: () => void;
  export let navigationVisible = false;
  export let toggleNavigation: () => void = () => undefined;
</script>

<header class="channel-header">
  <button
    class="navigation-toggle icon-button"
    aria-label="Channel navigation"
    aria-expanded={navigationVisible}
    on:click={toggleNavigation}><Menu size={21} /></button
  >
  <div class="header-channel">
    <span class="header-channel-icon"
      >{#if section === 'dms'}<MessageSquareText size={21} />{:else if channel}<Hash
          size={23}
        />{:else}<LockKeyhole size={21} />{/if}</span
    >
    <strong>{section === 'dms' ? 'Direct messages' : (channel?.name ?? 'Your workspace')}</strong>
    <i></i>
    <span class="header-topic"
      >{section === 'dms'
        ? 'Private conversations between authorized devices'
        : channel
          ? channel.confidentiality_mode === 'public'
            ? 'True public · signed plaintext'
            : 'End-to-end encrypted'
          : 'Right on the wire'}</span
    >
  </div>
  <div class="header-tools">
    {#if membersAvailable}<button
        title="Server members"
        aria-label="Server members"
        aria-pressed={membersVisible}
        on:click={toggleMembers}><Users size={21} /></button
      >{/if}
    {#if channel}
      <label class="header-search"
        ><input
          value={query}
          on:input={(event) => setQuery(event.currentTarget.value)}
          placeholder="Search messages"
          aria-label="Search displayed messages"
        /><Search size={17} /></label
      >
      <button
        title="Conversation details"
        aria-label="Conversation details"
        aria-pressed={detailsVisible}
        on:click={toggleDetails}><Info size={21} /></button
      >
    {/if}
    <button title="Cords settings" aria-label="Cords settings" on:click={openSettings}
      ><Settings2 size={20} /></button
    >
  </div>
</header>
