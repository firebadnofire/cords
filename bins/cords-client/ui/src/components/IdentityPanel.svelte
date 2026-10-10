<script lang="ts">
  import { Fingerprint, LockKeyhole, LogOut, Settings2, Shuffle } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import type { Identity, Preferences, Status } from '../model';
  export let status: Status;
  export let identity: Identity | null;
  export let preferences: Preferences;
  export let connected: boolean;
  export let openSettings: () => void;
  export let lockAccount: () => void;
  export let switchAccount: () => void;
  export let signOut: () => void;
  export let removeAccount: () => void;
  let menu = false;
</script>

<div class="bottom-panels identity-bottom">
  <div class="identity-status">
    <Fingerprint size={17} /><span
      >{identity?.revoked
        ? 'Device revoked'
        : connected
          ? 'Identity authenticated'
          : 'Identity unlocked'}</span
    >
  </div>
  <div class="user-panel">
    <Avatar value={preferences.avatar} text={preferences.displayName.slice(0, 2)} size={34} />
    <div class="user-identity">
      <strong>{preferences.displayName || 'You'}</strong><small title={status.device_id}
        >Device {status.device_id.slice(-8)}</small
      >
    </div>
    <button
      title="Account menu"
      aria-label="Account menu"
      aria-expanded={menu}
      on:click={() => (menu = !menu)}><Settings2 size={20} /></button
    >
    {#if menu}<div class="profile-menu">
        <button on:click={openSettings}><Settings2 size={16} />Settings</button>
        <button on:click={lockAccount}><LockKeyhole size={16} />Lock Account</button>
        <button on:click={switchAccount}><Shuffle size={16} />Switch Account</button>
        <button on:click={signOut}><LogOut size={16} />Sign Out</button>
        <button class="danger" on:click={removeAccount}>Remove Account From Device…</button>
      </div>{/if}
  </div>
</div>
