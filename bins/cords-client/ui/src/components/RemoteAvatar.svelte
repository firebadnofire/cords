<script lang="ts">
  import { picture, type Picture } from '../model';
  import Avatar from './Avatar.svelte';
  export let value: Picture;
  export let text = 'C';
  export let size = 40;
  $: resolved = picture({ ...value, url: '' });
  $: unavailable = !!value.url && !value.data;
</script>

<span
  title={unavailable
    ? 'URL profile pictures are no longer supported. This user needs to upload a local profile picture.'
    : undefined}
>
  <Avatar value={resolved} {text} {size} />
</span>
