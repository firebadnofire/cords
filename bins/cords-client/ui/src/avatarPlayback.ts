// Three 128 MiB decoder heaps, 16 MiB runtime each, 32 MiB compressed media,
// and 16 MiB display canvases stay below the shared 512 MiB ceiling. Editing
// suspends all playback before reserving its 448 MiB processing heaps/runtime.
import { writable } from 'svelte/store';
import { avatarMemory } from './avatarMemory';
export const avatarEditing = writable(false);
type Slot = { start: () => void; stop: () => void };
const waiting: Slot[] = [];
const active = new Set<Slot>();
// Rotate scarce decoder slots fairly while each canvas retains its last frame.
// Paused/offscreen consumers release their slot immediately.
let rotation: ReturnType<typeof setTimeout> | undefined;
let suspended = false;
function fill() {
  while (!suspended && active.size < 3 && waiting.length) {
    const slot = waiting.shift()!;
    active.add(slot);
    slot.start();
  }
  clearTimeout(rotation);
  if (!suspended && waiting.length && active.size)
    rotation = setTimeout(() => {
      for (const slot of active) {
        slot.stop();
        waiting.push(slot);
      }
      active.clear();
      fill();
    }, 10000);
}
avatarEditing.subscribe((value) => {
  suspended = value;
  if (value) {
    for (const slot of active) {
      slot.stop();
      waiting.push(slot);
    }
    active.clear();
  } else fill();
});
export function playbackSlot(start: () => void, stop: () => void) {
  const slot = { start, stop };
  waiting.push(slot);
  fill();
  return () => {
    const index = waiting.indexOf(slot);
    if (index >= 0) waiting.splice(index, 1);
    if (active.delete(slot)) stop();
    fill();
  };
}

export type AvatarPixels = { width: number; height: number; pixels: ArrayBuffer };
type Consumer = { paint: (frame: AvatarPixels) => void; fail: (error: string) => void };
type Playback = {
  consumers: Set<Consumer>;
  last?: AvatarPixels;
  release?: () => void;
  worker?: Worker;
  timer?: ReturnType<typeof setTimeout>;
  done: boolean;
  memory: object;
  frameMemory: object;
};
const jobs = new Map<string, Map<boolean, Playback>>();

/** One decoder per distinct image/motion policy, shared by every message header,
 * member row and profile using it. No full-resolution decoded frame cache. */
export function playAvatar(
  image: string,
  animate: boolean,
  paint: Consumer['paint'],
  fail: Consumer['fail'],
) {
  let policies = jobs.get(image);
  if (!policies) {
    policies = new Map();
    jobs.set(image, policies);
  }
  let job = policies.get(animate);
  const consumer = { paint, fail };
  if (!job) {
    job = { consumers: new Set(), done: false, memory: {}, frameMemory: {} };
    policies.set(animate, job);
  }
  const current = job;
  current.consumers.add(consumer);
  if (current.last) paint(current.last);
  const stop = () => {
    current.worker?.terminate();
    current.worker = undefined;
    clearTimeout(current.timer);
    avatarMemory.release(current.memory);
  };
  const error = (message: string) => {
    for (const subscriber of current.consumers) subscriber.fail(message);
    stop();
    current.done = true;
    current.release?.();
    current.release = undefined;
    // A constructor/Base64 failure can occur before playbackSlot returns its
    // disposer. Release that newly assigned slot at the next microtask as well.
    queueMicrotask(() => {
      if (current.done) {
        current.release?.();
        current.release = undefined;
      }
    });
  };
  if (!current.release && !current.done)
    current.release = playbackSlot(() => {
      stop();
      try {
        current.worker = new Worker(new URL('./avatarPlayback.worker.ts', import.meta.url), {
          type: 'module',
        });
      } catch (caught) {
        error(`Unable to start profile-picture playback: ${String(caught)}`);
        return;
      }
      const arm = () => {
        clearTimeout(current.timer);
        current.timer = setTimeout(
          () => error('Profile-picture decode exceeded 15 seconds'),
          15000,
        );
      };
      arm();
      current.worker.onmessage = ({ data }) => {
        try {
          if (data.error) return error(data.error);
          if (data.phase === 'decode') arm();
          if (data.phase === 'idle' || data.done) clearTimeout(current.timer);
          if (data.pixels) {
            if (
              !Number.isInteger(data.width) ||
              !Number.isInteger(data.height) ||
              data.width < 1 ||
              data.height < 1 ||
              data.width > 256 ||
              data.height > 256 ||
              !(data.pixels instanceof ArrayBuffer) ||
              data.pixels.byteLength !== data.width * data.height * 4
            )
              return error('Invalid avatar playback frame');
            avatarMemory.reserve(current.frameMemory, data.pixels.byteLength * 2);
            current.last = { width: data.width, height: data.height, pixels: data.pixels };
            for (const subscriber of current.consumers) subscriber.paint(current.last);
          }
          if (data.done) {
            current.done = true;
            stop();
            current.release?.();
            current.release = undefined;
          }
        } catch (caught) {
          error(String(caught));
        }
      };
      current.worker.onerror = (event) => error(event.message);
      try {
        if (image.length > 6_666_668) throw new Error('Profile picture exceeds 5 MB');
        avatarMemory.reserve(current.memory, 144 * 1024 * 1024 + Math.ceil((image.length * 3) / 4));
        const decoded = new Uint8Array(
          (image.length / 4) * 3 - (image.endsWith('==') ? 2 : image.endsWith('=') ? 1 : 0),
        );
        for (let offset = 0; offset < image.length; offset += 8192) {
          const part = atob(image.slice(offset, offset + 8192));
          for (let index = 0; index < part.length; index++)
            decoded[(offset / 4) * 3 + index] = part.charCodeAt(index);
        }
        const input = decoded.buffer;
        current.worker.postMessage({ input, animate, size: 256 }, [input]);
      } catch (caught) {
        error(String(caught));
      }
    }, stop);
  return () => {
    current.consumers.delete(consumer);
    if (!current.consumers.size) {
      current.release?.();
      stop();
      avatarMemory.release(current.frameMemory);
      policies?.delete(animate);
      if (!policies?.size) jobs.delete(image);
    }
  };
}
