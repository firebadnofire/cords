<script lang="ts">
  import CopyButton from './CopyButton.svelte';
  export let label: string;
  export let value: string;
  export let copyable = false;
  export let technical = false;
</script>

<div class="read-only-field">
  <span class="field-label">{label}</span>
  {#if technical}<code>{value || 'Unavailable'}</code>{:else}<span>{value || 'Unavailable'}</span
    >{/if}
  {#if copyable && value}<CopyButton {value} label="Copy {label.toLowerCase()}" />{/if}
</div>
