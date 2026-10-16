<script lang="ts">
  import { ChevronUp, Fingerprint, Settings2, Shuffle } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import { tick } from 'svelte';
  import type { Identity, Preferences, Status } from '../model';
  export let status: Status;
  export let identity: Identity | null;
  export let preferences: Preferences;
  export let openSettings: () => void;
  export let switchAccount: () => void;
  export let removeAccount: () => void;
  let menu = false;
  let panel: HTMLDivElement;
  let trigger: HTMLButtonElement;
  async function toggleMenu() {
    menu = !menu;
    if (menu) {
      await tick();
      panel.querySelector<HTMLButtonElement>('.profile-menu button')?.focus();
    }
  }
  function menuKey(event: KeyboardEvent) {
    if (!menu) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      menu = false;
      trigger.focus();
    } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const items = Array.from(panel.querySelectorAll<HTMLButtonElement>('.profile-menu button'));
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      items[
        event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? items.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length
      ]?.focus();
    }
  }
</script>

<svelte:window
  on:pointerdown={(event) => {
    if (menu && !panel.contains(event.target as Node)) menu = false;
  }}
  on:keydown={menuKey}
/>
<div bind:this={panel} class="bottom-panels identity-bottom">
  {#if identity?.revoked}<div class="identity-status" role="status">
      <Fingerprint size={17} /><span>Device revoked</span>
    </div>{/if}
  <div class="user-panel">
    <Avatar
      value={{ ...preferences.avatar, account: status.account_id }}
      text={preferences.displayName.slice(0, 2)}
      size={40}
    />
    <div class="user-identity">
      <strong>{preferences.displayName || 'You'}</strong><small title={status.device_id}
        >Account unlocked</small
      >
    </div>
    <button
      bind:this={trigger}
      title="Account menu"
      aria-label="Account menu"
      aria-expanded={menu}
      aria-haspopup="menu"
      on:click={toggleMenu}><ChevronUp size={18} /></button
    >
    {#if menu}<div class="profile-menu" role="menu" aria-label="Account actions">
        <button
          role="menuitem"
          on:click={() => {
            menu = false;
            openSettings();
          }}><Settings2 size={16} />Settings</button
        >
        <button
          role="menuitem"
          on:click={() => {
            menu = false;
            switchAccount();
          }}><Shuffle size={16} />Switch Account</button
        >
        <button
          role="menuitem"
          class="danger"
          on:click={() => {
            menu = false;
            removeAccount();
          }}>Remove Account From Device…</button
        >
      </div>{/if}
  </div>
</div>
