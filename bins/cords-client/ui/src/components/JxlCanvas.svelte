<script lang="ts">
  import { onMount } from 'svelte';
  import { avatarMotion } from '../profiles';
  import { playAvatar, avatarEditing, type AvatarPixels } from '../avatarPlayback';
  import { avatarMemory } from '../avatarMemory';
  export let image: string;
  export let size = 40;
  export let explicit = false;
  let canvas: HTMLCanvasElement;
  let visible = false,
    hidden = false,
    osReduced = false;
  let error = '';
  let release: (() => void) | undefined;
  const canvasMemory = {};
  function paint(frame: AvatarPixels) {
    const edge = Math.min(256, Math.ceil(size * devicePixelRatio));
    // Keep only the displayed canvas per consumer. The shared worker retains at
    // most one small display frame per active image, never a decoded animation.
    const width = Math.max(
      1,
      Math.round(frame.width * Math.min(1, edge / Math.max(frame.width, frame.height))),
    );
    const height = Math.max(1, Math.round((frame.height * width) / frame.width));
    avatarMemory.reserve(canvasMemory, width * height * 4 * 2);
    const source = new Uint8ClampedArray(frame.pixels);
    const pixels = new Uint8ClampedArray(width * height * 4);
    for (let y = 0; y < height; y++)
      for (let x = 0; x < width; x++) {
        const from =
          (Math.min(frame.height - 1, Math.floor(((y + 0.5) * frame.height) / height)) *
            frame.width +
            Math.min(frame.width - 1, Math.floor(((x + 0.5) * frame.width) / width))) *
          4;
        pixels.set(source.subarray(from, from + 4), (y * width + x) * 4);
      }
    canvas.width = width;
    canvas.height = height;
    canvas.getContext('2d')?.putImageData(new ImageData(pixels, width, height), 0, 0);
  }
  function update(source: string, animate: boolean, eligible: boolean) {
    release?.();
    release = undefined;
    if (!canvas) return;
    if (source && eligible)
      release = playAvatar(source, animate, paint, (reason) => {
        error = reason;
      });
    else {
      canvas.width = 0;
      canvas.height = 0;
      avatarMemory.release(canvasMemory);
    }
  }
  $: update(
    image,
    explicit || !($avatarMotion.disabled || $avatarMotion.reduced || osReduced),
    visible && !hidden && !$avatarEditing,
  );
  onMount(() => {
    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
    });
    observer.observe(canvas);
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    const reduced = () => {
      osReduced = motion.matches;
    };
    reduced();
    motion.addEventListener('change', reduced);
    const visibility = () => {
      hidden = document.hidden;
    };
    visibility();
    document.addEventListener('visibilitychange', visibility);
    return () => {
      observer.disconnect();
      motion.removeEventListener('change', reduced);
      document.removeEventListener('visibilitychange', visibility);
      release?.();
      avatarMemory.release(canvasMemory);
    };
  });
</script>

<canvas
  bind:this={canvas}
  title={error || undefined}
  aria-hidden="true"
  style:width="100%"
  style:height="100%"
></canvas>
