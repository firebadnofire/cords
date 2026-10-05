<script lang="ts">
  import { Fingerprint, HardDrive, KeyRound, LockKeyhole, Server, Users, X } from '@lucide/svelte';
  import type { Channel, Status } from '../model';
  export let channel: Channel | undefined;
  export let status: Status;
  export let manager: boolean;
  export let busy: boolean;
  export let close: () => void;
  export let removeDevice: (id: string) => void;
  export let addDevice: (id: string) => void;
  export let publish: () => void;
  let device = '';
</script>

<aside class="member-sidebar cords-details" aria-label="Conversation details">
  <header>
    <h2>Conversation details</h2>
    <button aria-label="Close details" on:click={close}><X size={18} /></button>
  </header>
  <section class="detail-section">
    <h3><LockKeyhole size={16} /> Encryption</h3>
    <p>
      MLS authenticates messages to device identities. Server trust and human verification remain
      separate.
    </p>
    {#if channel}<span class="detail-chip">Epoch {channel.epoch}</span>{/if}
  </section>
  <section class="detail-section">
    <h3><Server size={16} /> Relay server</h3>
    <strong>{new URL(status.origin || 'https://cords.invalid').hostname}</strong>
    <p class="detail-origin">{status.origin}</p>
  </section>
  <section class="detail-section">
    <h3><Fingerprint size={16} /> Server identity</h3>
    <code>{status.server_id || 'No trusted server'}</code>
  </section>
  {#if channel}<section class="detail-section">
      <h3><Users size={16} /> MLS devices</h3>
      {#each channel.members as member (member.authorization.value.device_id)}<div
          class="detail-person"
        >
          <span title={member.authorization.value.device_id}
            >{member.authorization.value.device_id.slice(-12)}</span
          >{#if manager && member.authorization.value.device_id !== status.device_id}<button
              class="danger"
              disabled={busy}
              on:click={() => removeDevice(member.authorization.value.device_id)}>Remove…</button
            >{/if}
        </div>{/each}{#if manager}<form
          class="detail-add"
          on:submit|preventDefault={() => {
            addDevice(device);
            device = '';
          }}
        >
          <label>Device ID<input bind:value={device} required /></label><button disabled={busy}
            >Approve MLS addition</button
          >
        </form>{/if}
    </section>{/if}
  <section class="detail-section">
    <h3><KeyRound size={16} /> Reachability</h3>
    <button disabled={!status.server_id || busy} on:click={publish}>Publish a KeyPackage</button
    ><small>Publish before another channel creator adds this installation.</small>
  </section>
  <section class="detail-section">
    <h3><HardDrive size={16} /> History &amp; retention</h3>
    <p>Displayed history is local. Server retention policy is not exposed by the current API.</p>
    <button class="dev-unimplemented" disabled aria-disabled="true" title="Not implemented yet"
      >Configure retention</button
    >
  </section>
</aside>
