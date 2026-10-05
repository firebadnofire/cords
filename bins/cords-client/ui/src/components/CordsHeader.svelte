<script lang="ts">
  import { Hash, Info, MessageSquareText, Search, Settings2 } from '@lucide/svelte';
  import type { Channel } from '../model';
  export let section: 'server' | 'dms';
  export let channel: Channel | undefined;
  export let query: string;
  export let detailsVisible: boolean;
  export let setQuery: (value: string) => void;
  export let toggleDetails: () => void;
  export let openSettings: () => void;
</script>

<header class="channel-header">
  <div class="header-channel">
    <span class="header-channel-icon"
      >{#if section === 'dms'}<MessageSquareText size={21} />{:else}<Hash size={23} />{/if}</span
    >
    <strong>{section === 'dms' ? 'Direct messages' : (channel?.name ?? 'Cords')}</strong>
    <i></i>
    <span class="header-topic"
      >{section === 'dms'
        ? 'Private conversations between authorized devices'
        : channel
          ? `MLS epoch ${channel.epoch}`
          : 'Right on the wire'}</span
    >
  </div>
  <div class="header-tools">
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
    <button title="Cords settings" aria-label="Cords settings" on:click={openSettings}
      ><Settings2 size={20} /></button
    >
  </div>
</header>
