<script lang="ts">
  import { Fingerprint, Settings2 } from '@lucide/svelte';
  import Avatar from './Avatar.svelte';
  import type { Identity, Preferences, Status } from '../model';
  export let status: Status;
  export let identity: Identity | null;
  export let preferences: Preferences;
  export let connected: boolean;
  export let openSettings: () => void;
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
    <button title="Identity and settings" aria-label="Identity and settings" on:click={openSettings}
      ><Settings2 size={20} /></button
    >
  </div>
</div>
