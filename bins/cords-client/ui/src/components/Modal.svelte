<script lang="ts">
  import { onMount } from 'svelte';
  export let title: string;
  export let close: () => void;
  export let wide = false;
  export let dismissible = true;
  let dialog: HTMLDialogElement;
  onMount(() => {
    const previous = document.activeElement;
    dialog.showModal();
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  });
</script>

<dialog
  bind:this={dialog}
  class:wide
  on:cancel|preventDefault={() => dismissible && close()}
  aria-label={title}
>
  <header class="modal-heading">
    <h2>{title}</h2>
    <button class="icon-button" aria-label="Close {title}" disabled={!dismissible} on:click={close}
      >✕</button
    >
  </header>
  <slot />
</dialog>
