<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { AvatarDeadline } from '../avatarPolicy';
  import { AvatarWorkerSupervisor, type AvatarWorkerPort } from '../avatarWorkerSession';
  import type { AvatarMetadata, AvatarGeometry } from '../avatarCodecRuntime';
  import type { ProfileAvatar } from '../profiles';
  import { registerAvatarProcessing } from '../profiles';
  import { avatarEditing } from '../avatarPlayback';
  import JxlCanvas from './JxlCanvas.svelte';
  import { avatarMemory } from '../avatarMemory';
  export let selected: { avatar: ProfileAvatar | null; image: string | null } | undefined =
    undefined;
  export let busy = false;
  let metadata: AvatarMetadata | undefined;
  let preview: HTMLCanvasElement;
  let x = 50,
    y = 50,
    zoom = 1,
    output = 1024;
  let shape: 'circle' | 'square' = 'circle';
  let error = '';
  let notice = '';
  let playback = false;
  const supervisor = new AvatarWorkerSupervisor(
    () =>
      new Worker(new URL('../avatarCodec.worker.ts', import.meta.url), {
        type: 'module',
      }) as unknown as AvatarWorkerPort,
    avatarMemory,
  );
  let session: ReturnType<AvatarWorkerSupervisor['start']> | undefined;
  const selectedMemory = {};
  $: avatarMemory.reserve(selectedMemory, selected?.image ? selected.image.length * 2 : 0);
  function cancel() {
    supervisor.cancel();
    session = undefined;
    metadata = undefined;
    busy = false;
    avatarEditing.set(false);
  }
  registerAvatarProcessing(cancel);
  onDestroy(() => {
    cancel();
    avatarMemory.release(selectedMemory);
  });
  function geometry(): AvatarGeometry {
    if (!metadata) throw new Error('Select an image first');
    const side = Math.max(1, Math.floor(Math.min(metadata.width, metadata.height) / zoom));
    const out = Math.min(output, side, 1024);
    return {
      x: Math.round(((metadata.width - side) * x) / 100),
      y: Math.round(((metadata.height - side) * y) / 100),
      width: side,
      height: side,
      outWidth: out,
      outHeight: out,
    };
  }
  async function cropPreview() {
    if (!session || busy) return;
    busy = true;
    try {
      const crop = geometry();
      const edge = Math.min(256, crop.outWidth);
      const result = await session.request(
        'preview',
        { ...crop, outWidth: edge, outHeight: edge },
        edge * edge * 4,
      );
      try {
        preview.width = edge;
        preview.height = edge;
        preview
          .getContext('2d')!
          .putImageData(new ImageData(new Uint8ClampedArray(result.buffer!), edge, edge), 0, 0);
      } finally {
        result.release();
      }
    } catch (caught) {
      error = String(caught);
      cancel();
    } finally {
      busy = false;
    }
  }
  async function choose(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (!file) return;
    cancel();
    selected = undefined;
    error = '';
    notice = '';
    playback = false;
    const deadline = new AvatarDeadline();
    try {
      if (!file.size || file.size > 50_000_000)
        throw new Error('Choose an image up to 50,000,000 bytes.');
      avatarEditing.set(true);
      busy = true;
      await tick(); // Playback consumers release heaps/canvases before processing.
      session = supervisor.start(await file.arrayBuffer(), deadline);
      const result = await session.opened;
      metadata = result.value as AvatarMetadata;
      result.release();
      output = Math.min(1024, metadata.width, metadata.height);
      x = y = 50;
      zoom = 1;
      busy = false;
      await tick();
      await cropPreview();
    } catch (caught) {
      error = String(caught);
      cancel();
    } finally {
      busy = false;
    }
  }
  async function process() {
    if (!session || busy) return;
    busy = true;
    error = '';
    try {
      const crop = geometry();
      const result = await session.request('encode', crop, 5_000_000);
      let bytes: Uint8Array;
      try {
        const length = (result.value as { length: number }).length;
        if (!Number.isInteger(length) || length < 1 || length > 5_000_000)
          throw new Error('Invalid encoded image length');
        avatarMemory.reserve(selectedMemory, length);
        bytes = new Uint8Array(result.buffer!, 0, length).slice();
      } finally {
        result.release();
      }
      // Release decoder/encoder heaps before creating Base64 or starting playback.
      cancel();
      avatarMemory.reserve(selectedMemory, bytes!.length * 6);
      const digest = new Uint8Array(
        await crypto.subtle.digest('SHA-256', bytes! as Uint8Array<ArrayBuffer>),
      );
      const base64 = (data: Uint8Array) => {
        let text = '';
        for (let offset = 0; offset < data.length; offset += 8192)
          text += String.fromCharCode(...data.subarray(offset, offset + 8192));
        return btoa(text);
      };
      selected = {
        avatar: {
          sha256: base64(digest).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, ''),
          bytes: bytes!.length,
          width: crop.outWidth,
          height: crop.outHeight,
          shape,
        },
        image: base64(bytes!),
      };
      notice = `Ready to review: ${crop.outWidth} × ${crop.outHeight}, ${bytes!.length.toLocaleString()} bytes. All frames were verified losslessly.`;
    } catch (caught) {
      error = `${String(caught)}. Select the image again to retry, using a smaller output size if it exceeds 5 MB.`;
      avatarMemory.release(selectedMemory);
      cancel();
    } finally {
      busy = false;
    }
  }
  function remove() {
    cancel();
    selected = { avatar: null, image: null };
    notice = 'Picture removal will be published when you save your profile.';
  }
</script>

<section class="profile-image-editor" aria-label="Profile picture editor">
  <h2>Profile picture</h2>
  <p>
    PNG, JPEG, WebP, GIF, APNG or JPEG XL. Animation, transparency and exact timing are preserved.
  </p>
  <label
    >Choose a local image<input
      type="file"
      accept=".png,.apng,.jpg,.jpeg,.webp,.gif,.jxl"
      on:change={choose}
      disabled={busy}
    /></label
  >
  {#if metadata}
    <div class="profile-crop">
      <canvas bind:this={preview} class:square={shape === 'square'} aria-label="Crop preview"
      ></canvas>
      <div class="profile-crop-controls">
        <label
          >Horizontal position<input
            type="range"
            min="0"
            max="100"
            bind:value={x}
            on:change={cropPreview}
            disabled={busy}
          /></label
        >
        <label
          >Vertical position<input
            type="range"
            min="0"
            max="100"
            bind:value={y}
            on:change={cropPreview}
            disabled={busy}
          /></label
        >
        <label
          >Zoom<input
            type="range"
            min="1"
            max="4"
            step="0.01"
            bind:value={zoom}
            on:change={cropPreview}
            disabled={busy}
          /></label
        >
        <label
          >Output size<input
            type="number"
            min="1"
            max={Math.min(1024, metadata.width, metadata.height)}
            bind:value={output}
            disabled={busy}
          /></label
        >
        <label
          >Shape<select bind:value={shape} disabled={busy}
            ><option value="circle">Circle</option><option value="square">Square</option></select
          ></label
        >
        <button class="primary" disabled={busy} on:click={process}
          >{busy ? 'Processing…' : 'Process and review all frames'}</button
        >
        <button on:click={cancel}>Cancel processing</button>
      </div>
    </div>
  {/if}
  {#if selected?.image}
    <div class="profile-animation-review" class:square={selected.avatar?.shape === 'square'}>
      <JxlCanvas image={selected.image} size={160} explicit={playback} />
    </div>
    <button on:click={() => (playback = !playback)}
      >{playback ? 'Use motion preference' : 'Play animation for review'}</button
    >
  {/if}
  <button on:click={remove} disabled={busy}>Use initials / remove picture</button>
  {#if busy}<p role="status">Processing the original frames…</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>
