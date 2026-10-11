<script lang="ts">
  import { Fingerprint, HardDrive, KeyRound, LockKeyhole, Server, Users, X } from '@lucide/svelte';
  import { memberName, memberPicture, type Contact, type Channel, type Status } from '../model';
  import RemoteAvatar from './RemoteAvatar.svelte';
  export let contacts: Contact[] = [];
  $: candidates = contacts.filter(
    (contact) =>
      !channel?.members.some(
        (member) => member.authorization.value.device_id === contact.authorization.value.device_id,
      ),
  );
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
    <h3><LockKeyhole size={16} /> Confidentiality · permanent</h3>
    <p>
      {#if channel?.confidentiality_mode === 'public'}True public: signed plaintext messages
        protected by HTTPS in transit. The server can read and retain them.{:else}End-to-end
        encrypted with MLS. The server cannot read message contents.{/if} A different confidentiality
      mode requires a new replacement channel.
    </p>
    {#if channel && channel.confidentiality_mode !== 'public'}<span class="detail-chip"
        >Epoch {channel.epoch}</span
      >{/if}
    {#if channel?.confidentiality_mode === 'public'}<p>
        All admitted server members with read/write permission can participate. No MLS invitation is
        needed.
      </p>{/if}
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
  {#if channel && channel.confidentiality_mode !== 'public' && !channel.locally_archived}<section
      class="detail-section"
    >
      <h3><Users size={16} /> Channel members</h3>
      <p>
        Each installation joins separately. A new member must join this server and publish a
        KeyPackage first.
      </p>
      {#each channel.members as member (member.authorization.value.device_id)}<div
          class="detail-person"
        >
          <span title={member.authorization.value.device_id}
            >{memberName(
              contacts.find(
                (contact) =>
                  contact.authorization.value.device_id === member.authorization.value.device_id,
              ),
              member.authorization.value.device_id.slice(-12),
            )}</span
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
          <label
            >Add a member<select bind:value={device} required>
              <option value="" disabled>Choose a server member</option>
              {#each candidates as contact (contact.authorization.value.device_id)}<option
                  value={contact.authorization.value.device_id}
                  >{memberName(contact, 'Unnamed member')} · {contact.authorization.value.device_id.slice(
                    -8,
                  )}</option
                >{/each}
            </select></label
          >
          {#if device}{@const candidate = candidates.find(
              (contact) => contact.authorization.value.device_id === device,
            )}<RemoteAvatar
              value={memberPicture(candidate)}
              text={memberName(candidate, '?').slice(0, 2)}
              size={28}
            /><small>Device {device}</small>{/if}
          <button disabled={busy || !device}>Add to encrypted channel</button>
        </form>{/if}
    </section>{/if}
  <section class="detail-section">
    <h3><KeyRound size={16} /> Join encrypted channels</h3>
    <button disabled={!status.server_id || busy} on:click={publish}>Publish a KeyPackage</button
    ><small
      >Publish your device key, then ask the channel creator to add you using Channel members above.
      Newly added devices receive future messages, not earlier history.</small
    >
  </section>
  <section class="detail-section">
    <h3><HardDrive size={16} /> History &amp; retention</h3>
    <p>Displayed history is local. Server retention policy is not exposed by the current API.</p>
    <button class="dev-unimplemented" disabled aria-disabled="true" title="Not implemented yet"
      >Configure retention</button
    >
  </section>
</aside>
