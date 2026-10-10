<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { importImage } from '../images';
  import { picture, type Picture } from '../model';
  import Avatar from './Avatar.svelte';

  export let value: Picture;
  export let text = 'C';
  export let size = 40;
  let resolved = value;
  let requested = '';

  async function resolve(url: string) {
    if (!url || value.data) {
      resolved = value;
      return;
    }
    if (requested === url) return;
    requested = url;
    try {
      const encoded = await invoke<string>('load_image_url', { url });
      const data = await importImage(
        Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0)),
      );
      if (requested === url) resolved = { ...value, data };
    } catch {
      if (requested === url) resolved = picture({ ...value, data: '' });
    }
  }

  $: void resolve(value.url);
</script>

<Avatar value={resolved} {text} {size} />
