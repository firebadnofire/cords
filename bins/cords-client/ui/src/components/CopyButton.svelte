<script lang="ts">
  export let value: string;
  export let label = 'Copy';
  let pending = false;
  let copied = false;
  let error = '';
  $: if (value) {
    copied = false;
    error = '';
  }
  async function copy() {
    if (pending) return;
    pending = true;
    copied = false;
    error = '';
    try {
      await navigator.clipboard.writeText(value);
      copied = true;
    } catch {
      error = 'Could not copy. Select the text and copy it manually.';
    } finally {
      pending = false;
    }
  }
</script>

<span class="copy-control">
  <button type="button" disabled={pending || !value} on:click={copy}>
    {pending ? 'Copying…' : copied ? 'Copied' : label}
  </button>
  {#if copied}<span class="sr-only" role="status">{label}: copied to clipboard</span>{/if}
  {#if error}<span class="field-error" role="alert">{error}</span>{/if}
</span>
