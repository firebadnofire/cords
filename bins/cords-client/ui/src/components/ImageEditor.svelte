<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { importImage } from '../images';
  import { blankPicture, type Picture } from '../model';
  import Avatar from './Avatar.svelte';
  export let value: Picture;
  export let title: string;
  let url = '';
  let error = '';
  let busy = false;
  async function load(bytes: Uint8Array) {
    value = { ...blankPicture(), data: await importImage(bytes) };
  }
  async function file(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (!file) return;
    busy = true;
    error = '';
    try {
      if (file.size > 5 * 1024 * 1024) throw new Error('Choose an image up to 5 MiB');
      await load(new Uint8Array(await file.arrayBuffer()));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  async function remote() {
    busy = true;
    error = '';
    try {
      const data = await invoke<string>('load_image_url', { url });
      await load(Uint8Array.from(atob(data), (c) => c.charCodeAt(0)));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="image-editor" aria-label={title}>
  <h2>{title}</h2>
  <Avatar {value} size={120} />
  <label
    >Browse image<input
      type="file"
      accept="image/png,image/jpeg,image/webp,image/svg+xml"
      on:change={file}
      disabled={busy}
    /></label
  >
  <label
    >HTTPS image URL<input
      type="url"
      bind:value={url}
      placeholder="https://example.org/avatar.webp"
    /></label
  >
  <p class="hint">
    Loading contacts this host directly. Profile pictures are normalized before being shared in your
    signed user card; SVG scripts, references, fonts and CSS are rejected.
  </p>
  <div class="actions">
    <button disabled={busy || !url} on:click={remote}>{busy ? 'Loading…' : 'Load URL'}</button
    ><button disabled={busy} on:click={() => (value = blankPicture())}>Use initials / letter</button
    >
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if value.data}<label
      >Shape<select bind:value={value.shape}
        ><option value="circle">Circle</option><option value="square">Square</option></select
      ></label
    ><label>Horizontal position<input type="range" min="0" max="100" bind:value={value.x} /></label
    ><label>Vertical position<input type="range" min="0" max="100" bind:value={value.y} /></label
    ><label>Zoom<input type="range" min="1" max="4" step="0.05" bind:value={value.zoom} /></label
    ><button on:click={() => (value = { ...value, x: 50, y: 50, zoom: 1 })}>Reset crop</button>{/if}
</section>
