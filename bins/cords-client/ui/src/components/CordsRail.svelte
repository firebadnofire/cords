<script lang="ts">
  import { MessageSquareText, Plus, Settings2 } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import { dmEntry, type Preferences, type ServerListing, type Status } from '../model';

  export let status: Status | null;
  export let servers: ServerListing[];
  export let preferences: Preferences;
  export let section: 'server' | 'dms';
  export let select: (section: 'server' | 'dms') => void;
  export let connect: () => void;
  export let settings: () => void;
  export let selectServer: (serverId: string) => void;
</script>

<nav class="server-rail" aria-label="Servers and direct messages">
  {#if preferences.jewelAction === 'none'}
    <div class="cords-brand" title="Cords Jewel">
      <Avatar
        value={preferences.jewel}
        text={preferences.jewelText}
        color={preferences.jewelColor}
        size={46}
      />
    </div>
  {:else}
    <button
      class:active={preferences.jewelAction === 'dms' && section === 'dms'}
      class="cords-brand"
      aria-label={preferences.jewelAction === 'dms' ? 'Open direct messages' : 'Open settings'}
      on:click={() => (preferences.jewelAction === 'dms' ? select('dms') : settings())}
    >
      <Avatar
        value={preferences.jewel}
        text={preferences.jewelText}
        color={preferences.jewelColor}
        size={46}
      />
    </button>
  {/if}

  {#if dmEntry(preferences.jewelAction) === 'separate'}
    <div class="rail-divider"></div>
    <button
      class="server-button dm-rail-button"
      class:selected={section === 'dms'}
      title="Direct messages"
      aria-label="Direct messages"
      on:click={() => select('dms')}><MessageSquareText size={22} /></button
    >
  {/if}

  <div class="rail-divider"></div>
  {#each servers.filter((server) => !server.archived) as server (server.server_id)}
    <button
      class="server-button server-blue"
      class:selected={section === 'server' && status?.server_id === server.server_id}
      class:server-lockdown={server.ownership_state === 'OWNER_LOCKDOWN'}
      title={server.ownership_state === 'OWNER_LOCKDOWN' ? `${server.origin} — owner recovery required` : server.origin}
      aria-label={server.ownership_state === 'OWNER_LOCKDOWN' ? `Owner recovery required for ${server.origin}` : `Select server ${server.origin}`}
      on:click={() => selectServer(server.server_id)}
      >{new URL(server.origin).hostname.slice(0, 1).toUpperCase()}{#if server.ownership_state === 'OWNER_LOCKDOWN'}<span class="server-alert">!</span>{/if}</button
    >
  {/each}
  <button
    class="server-button rail-action"
    title="Connect to server"
    aria-label="Connect to server"
    on:click={connect}
  >
    <Plus size={25} />
  </button>
  <div class="rail-spacer"></div>
  <button
    class="server-button rail-settings"
    title="User settings"
    aria-label="User settings"
    on:click={settings}
  >
    <Settings2 size={20} />
  </button>
</nav>
