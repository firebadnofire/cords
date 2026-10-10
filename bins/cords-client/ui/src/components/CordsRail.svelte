<script lang="ts">
  import { MessageSquareText, Plus, Settings2 } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import { tick } from 'svelte';
  import { dmEntry, type Preferences, type ServerListing, type Status } from '../model';

  export let status: Status | null;
  export let servers: ServerListing[];
  export let preferences: Preferences;
  export let section: 'server' | 'dms';
  export let select: (section: 'server' | 'dms') => void;
  export let connect: () => void;
  export let settings: () => void;
  export let selectServer: (serverId: string) => void;
  export let leaveServer: (serverId: string) => void;
  export let archiveServer: (serverId: string) => void;
  let context: { serverId: string; origin: string; x: number; y: number } | null = null;
  let menuElement: HTMLDivElement;
  async function openContext(event: MouseEvent, server: ServerListing) {
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    context = {
      serverId: server.server_id,
      origin: server.origin,
      x: Math.max(8, Math.min(event.clientX || bounds.right, window.innerWidth - 232)),
      y: Math.max(8, Math.min(event.clientY || bounds.top, window.innerHeight - 172)),
    };
    await tick();
    menuElement?.querySelector<HTMLButtonElement>('button')?.focus();
  }
  function menuKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      context = null;
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const items = Array.from(menuElement.querySelectorAll<HTMLButtonElement>('button'));
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? items.length - 1
          : (current + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
    items[next]?.focus();
  }
</script>

<svelte:window
  on:pointerdown={(event) => {
    if (!(event.target as HTMLElement).closest('.server-context-menu')) context = null;
  }}
  on:keydown={(event) => {
    if (event.key === 'Escape') context = null;
  }}
/>

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
      title={server.ownership_state === 'OWNER_LOCKDOWN'
        ? `${server.origin} — owner recovery required`
        : server.origin}
      aria-label={server.ownership_state === 'OWNER_LOCKDOWN'
        ? `Owner recovery required for ${server.origin}`
        : `Select server ${server.origin}`}
      on:click={() => selectServer(server.server_id)}
      on:contextmenu|preventDefault|stopPropagation={(event) => void openContext(event, server)}
      >{new URL(server.origin).hostname
        .slice(0, 1)
        .toUpperCase()}{#if server.ownership_state === 'OWNER_LOCKDOWN'}<span class="server-alert"
          >!</span
        >{/if}</button
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
{#if context}
  <div
    bind:this={menuElement}
    class="context-menu server-context-menu"
    role="menu"
    tabindex="-1"
    aria-label={`Actions for ${context.origin}`}
    style:left="{context.x}px"
    style:top="{context.y}px"
    on:keydown={menuKey}
  >
    <button
      role="menuitem"
      on:click={() => {
        if (context) selectServer(context.serverId);
        context = null;
      }}>Open server</button
    >
    <button
      role="menuitem"
      class="danger"
      on:click={() => {
        if (context) leaveServer(context.serverId);
        context = null;
      }}>Leave server…</button
    >
    <button
      role="menuitem"
      on:click={() => {
        if (context) archiveServer(context.serverId);
        context = null;
      }}>Archive server…</button
    >
  </div>
{/if}
