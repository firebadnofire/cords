<script lang="ts">
  import { blankPicture, type Picture } from '../model';
  import { onMount } from 'svelte';
  import { publicProfiles, loadProfile } from '../profiles';
  import JxlCanvas from './JxlCanvas.svelte';
  export let value: Picture = blankPicture();
  export let text = 'C';
  export let color = '#477da3';
  export let size = 40;
  let error = '';
  $: canonical = value.account ? $publicProfiles[value.account] : undefined;
  $: shape = canonical?.state.agreed?.avatar?.shape ?? value.shape;
  async function refresh() {
    if (value.account)
      try {
        await loadProfile(value.account);
        error = '';
      } catch (caught) {
        error = String(caught);
      }
  }
  $: if (value.account) void refresh();
  onMount(() => {
    const timer = setInterval(() => {
      void refresh();
    }, 15000);
    return () => clearInterval(timer);
  });
</script>

<span
  class="avatar"
  class:square={shape === 'square'}
  title={error || undefined}
  style:width="{size}px"
  style:height="{size}px"
  style:background={color}
  aria-hidden="true"
>
  {#if canonical?.image && canonical.state.agreed?.avatar}
    <JxlCanvas image={canonical.image} {size} />
  {:else if canonical?.state.agreed}{text.slice(0, 3)}
  {:else if value.data}<img
      src={value.data}
      alt=""
      style:object-position="{value.x}% {value.y}%"
      style:transform="scale({value.zoom})"
      style:transform-origin="{value.x}% {value.y}%"
    />{:else}{text.slice(0, 3)}{/if}
</span>
