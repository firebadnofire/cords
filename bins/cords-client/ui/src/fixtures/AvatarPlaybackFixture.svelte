<script lang="ts">
  import { onMount } from 'svelte';
  import JxlCanvas from '../components/JxlCanvas.svelte';
  import { avatarMotion } from '../profiles';
  let images: string[] = [];
  let error = '';
  let disabled = false;
  let explicit = false;
  $: avatarMotion.set({ disabled, reduced: false });
  onMount(() => {
    void Promise.all(
      ['a', 'b'].map(async (name) => {
        const response = await fetch(`/fixtures/looping-${name}.jxl`);
        if (!response.ok) throw new Error(`Fixture image: ${response.status}`);
        return btoa(String.fromCharCode(...new Uint8Array(await response.arrayBuffer())));
      }),
    )
      .then((loaded) => {
        images = loaded;
      })
      .catch((caught) => {
        error = String(caught);
      });
  });
</script>

<main style="padding:32px">
  <h1>Actual bundled avatar playback</h1>
  <p>Development fixture. No account or server. Native acceptance is separate.</p>
  <label><input type="checkbox" bind:checked={disabled} />Disable animation</label>
  <label><input type="checkbox" bind:checked={explicit} />Explicit playback for review</label>
  <div style="display:flex;gap:32px;margin-top:24px">
    {#each images as image, index (image)}
      <figure>
        <div class="avatar" class:square={index === 1} style="width:160px;height:160px">
          <JxlCanvas {image} size={160} {explicit} />
        </div>
        <figcaption>{index === 0 ? 'Alice motion' : 'Bob motion'}</figcaption>
      </figure>
    {/each}
  </div>
  {#if error}<p role="alert">{error}</p>{/if}
</main>
