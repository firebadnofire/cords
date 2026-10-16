<script lang="ts">
  import { onMount } from 'svelte';
  export let title: string;
  export let close: () => void;
  export let navigation = false;
  let dialog: HTMLDialogElement;
  let narrow = false;
  onMount(() => {
    const previous = document.activeElement;
    const media = window.matchMedia(`(max-width: ${navigation ? 720 : 1100}px)`);
    function present() {
      narrow = media.matches;
      if (navigation && !narrow) {
        close();
        return;
      }
      if (dialog.open) dialog.close();
      if (narrow) dialog.showModal();
      else {
        dialog.show();
        // A desktop rail must not take focus from the conversation on mount.
        if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
      }
    }
    present();
    media.addEventListener('change', present);
    return () => {
      media.removeEventListener('change', present);
      if (dialog.open) dialog.close();
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  });
  function escape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || narrow || event.defaultPrevented) return;
    if (document.querySelector('dialog[open]:not(.side-surface)')) return;
    event.preventDefault();
    close();
  }
  function trapTab(event: KeyboardEvent) {
    if (!narrow || event.key !== 'Tab') return;
    const items = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), a[href], summary, [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((item) => item.getClientRects().length > 0);
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (
      event.shiftKey &&
      (document.activeElement === first || !items.includes(document.activeElement as HTMLElement))
    ) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window on:keydown={escape} />
<dialog
  bind:this={dialog}
  class="side-surface"
  class:navigation-surface={navigation}
  aria-label={title}
  on:cancel|preventDefault={close}
  on:keydown={trapTab}
  on:click={(event) => {
    if (!narrow || event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (
      event.clientX < bounds.left ||
      event.clientX > bounds.right ||
      event.clientY < bounds.top ||
      event.clientY > bounds.bottom
    )
      close();
  }}
>
  <slot />
</dialog>
