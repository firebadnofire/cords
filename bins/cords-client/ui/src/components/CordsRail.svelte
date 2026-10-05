<script lang="ts">
  import { MessageSquareText, Plus, Settings2 } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import { dmEntry, type Preferences, type Status } from '../model';

  export let status: Status | null;
  export let preferences: Preferences;
  export let section: 'server' | 'dms';
  export let select: (section: 'server' | 'dms') => void;
  export let connect: () => void;
  export let settings: () => void;
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
  {#if status?.server_id}
    <button
      class="server-button server-blue"
      class:selected={section === 'server'}
      title={status.origin}
      aria-label={`Select server ${status.origin}`}
      on:click={() => select('server')}
      >{new URL(status.origin).hostname.slice(0, 1).toUpperCase()}</button
    >
  {/if}
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
