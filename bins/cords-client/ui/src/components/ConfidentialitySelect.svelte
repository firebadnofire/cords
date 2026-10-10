<script lang="ts">
  import type { ConfidentialityMode } from '../model';
  export let mode: ConfidentialityMode = 'encrypted';
</script>

<label
  >Permanent confidentiality mode
  <select bind:value={mode}>
    <option value="encrypted">End-to-end encrypted</option>
    <option value="public">True public</option>
  </select>
</label>
{#if mode === 'encrypted'}
  <p>Messages are encrypted so the server cannot read their contents.</p>
{:else}
  <p>
    <strong>Messages are not end-to-end encrypted.</strong> The server and anyone with access to the
    messages can read, copy, and retain them. Sender identities remain cryptographically verified.
  </p>
  <p>
    Useful for large communities, public announcements, and discussions that do not require
    confidentiality.
  </p>
{/if}
<p>
  This selection is permanent. Changing confidentiality requires replacing the channel with a new
  identity.
</p>
