<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { importImage } from '../images';
  import { blankPicture, type Picture } from '../model';

  export let value: Picture;
  export let title: string;
  export let source: 'local' | 'url' = 'local';
  let url = value.url;
  let error = '';
  let busy = false;
  let drag: { pointer: number; clientX: number; clientY: number; x: number; y: number } | null =
    null;
  const clamp = (number: number, min: number, max: number) => Math.max(min, Math.min(max, number));

  async function local(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    busy = true;
    error = '';
    try {
      if (!file.size || file.size > 5 * 1024 * 1024)
        throw new Error('Choose an image up to 5 MiB for the local preview');
      const data = await importImage(new Uint8Array(await file.arrayBuffer()));
      value = { ...blankPicture(), data };
      url = '';
    } catch (caught) {
      error = String(caught);
    } finally {
      input.value = '';
      busy = false;
    }
  }

  async function remote() {
    busy = true;
    error = '';
    try {
      const data = await invoke<string>('load_image_url', { url });
      value = {
        ...blankPicture(),
        url,
        data: await importImage(Uint8Array.from(atob(data), (c) => c.charCodeAt(0))),
      };
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  function startDrag(event: PointerEvent) {
    if (!value.data) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = {
      pointer: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
      x: value.x,
      y: value.y,
    };
  }
  function moveDrag(event: PointerEvent) {
    if (!drag || drag.pointer !== event.pointerId) return;
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    value = {
      ...value,
      x: clamp(drag.x - ((event.clientX - drag.clientX) / bounds.width) * 100, 0, 100),
      y: clamp(drag.y - ((event.clientY - drag.clientY) / bounds.height) * 100, 0, 100),
    };
  }
  function endDrag(event: PointerEvent) {
    if (drag?.pointer === event.pointerId) drag = null;
  }
  function adjustPosition(x: number, y: number) {
    value = { ...value, x: clamp(value.x + x, 0, 100), y: clamp(value.y + y, 0, 100) };
  }
  function cropKey(event: KeyboardEvent) {
    const distance = event.shiftKey ? 5 : 1;
    if (event.key === 'ArrowLeft') adjustPosition(-distance, 0);
    else if (event.key === 'ArrowRight') adjustPosition(distance, 0);
    else if (event.key === 'ArrowUp') adjustPosition(0, -distance);
    else if (event.key === 'ArrowDown') adjustPosition(0, distance);
    else return;
    event.preventDefault();
  }
</script>

<section class="image-editor" aria-label={title}>
  <h2>{title}</h2>
  {#if source === 'local'}
    <label
      >Choose a local image<input
        type="file"
        disabled={busy}
        accept=".png,.apng,.jpg,.jpeg,.webp,.gif,.jxl,.svg,image/png,image/jpeg,image/webp,image/gif,image/jxl,image/svg+xml"
        on:change={local}
      /></label
    >
    <p class="hint">
      This saves a private local preview. Shared JPEG XL upload is still being integrated. Animated
      images are rejected rather than converted to a still image.
    </p>
  {:else}
    <label
      >HTTPS image URL<input
        type="url"
        bind:value={url}
        placeholder="https://example.org/avatar.webp"
      /></label
    >
    <p class="hint">
      Loading contacts this host directly and may reveal your IP address. This Jewel image stays in
      encrypted local preferences and is not published in your user card. SVG scripts, references,
      fonts and CSS are rejected.
    </p>
  {/if}
  <div class="actions">
    {#if source === 'url'}
      <button disabled={busy || !url} on:click={remote}>{busy ? 'Loading…' : 'Load URL'}</button
      >{/if}<button
      disabled={busy}
      on:click={() => {
        url = '';
        value = blankPicture();
      }}>Use initials / letter</button
    >
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if value.data}<div class="crop-workspace">
      <button
        type="button"
        class="crop-viewport"
        class:square={value.shape === 'square'}
        class:dragging={drag !== null}
        aria-label="Interactive image crop. Drag to reposition; use arrow keys for fine adjustment."
        on:pointerdown={startDrag}
        on:pointermove={moveDrag}
        on:pointerup={endDrag}
        on:pointercancel={endDrag}
        on:keydown={cropKey}
        on:wheel|preventDefault={(event) =>
          (value = { ...value, zoom: clamp(value.zoom - event.deltaY * 0.002, 1, 4) })}
      >
        <img
          src={value.data}
          alt=""
          draggable="false"
          style:object-position="{value.x}% {value.y}%"
          style:transform="scale({value.zoom})"
          style:transform-origin="{value.x}% {value.y}%"
        />
        <span class="crop-grid" aria-hidden="true"></span>
      </button>
      <p class="hint">Drag the image to frame it. Scroll over the preview or use zoom below.</p>
    </div>
    <label
      >Shape<select bind:value={value.shape}
        ><option value="circle">Circle</option><option value="square">Square</option></select
      ></label
    ><label>Zoom<input type="range" min="1" max="4" step="0.05" bind:value={value.zoom} /></label
    ><button on:click={() => (value = { ...value, x: 50, y: 50, zoom: 1 })}>Reset crop</button>{/if}
</section>
