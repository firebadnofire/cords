<script lang="ts">
  import { remoteImage } from '../remoteImages';
  import { picture, type Picture } from '../model';
  import Avatar from './Avatar.svelte';

  export let value: Picture;
  export let text = 'C';
  export let size = 40;
  let resolved = value;
  let requested = '';
  let generation = 0;
  let cachedData = '';
  let failed = false;

  async function resolve(next: Picture) {
    if (!next.url || next.data) {
      generation += 1;
      requested = '';
      cachedData = '';
      failed = false;
      resolved = next;
      return;
    }
    resolved = picture({ ...next, data: next.url === requested ? cachedData : '' });
    if (requested === next.url) return;
    requested = next.url;
    cachedData = '';
    failed = false;
    const current = ++generation;
    try {
      const data = await remoteImage(next.url);
      if (current === generation) {
        cachedData = data;
        resolved = picture({ ...value, data });
      }
    } catch {
      if (current === generation) {
        failed = true;
        resolved = picture({ ...value, data: '' });
      }
    }
  }

  $: void resolve(value);
</script>

<span title={failed ? 'Profile image unavailable' : undefined}
  ><Avatar value={resolved} {text} {size} /></span
>
