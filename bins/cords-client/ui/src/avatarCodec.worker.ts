import { AvatarCodecRuntime } from './avatarCodecRuntime';
import { loadAvatarCodecs } from './avatarCodecs';
import { avatarWorkerMetadataBytes } from './avatarWorkerSession';

// Dedicated per-operation worker. Its supervisor lives outside synchronous WASM
// and terminates it on cancellation, decode timeout, lock or account switch.
let runtime: AvatarCodecRuntime | undefined;
let running = false;
let failed = false;
globalThis.onmessage = async ({ data }) => {
  if (running || failed) {
    globalThis.postMessage({ id: data?.id, error: 'Codec worker is busy or closed' });
    return;
  }
  running = true;
  try {
    if (!data || !Number.isSafeInteger(data.id) || typeof data.operation !== 'string')
      throw new Error('Invalid codec command');
    avatarWorkerMetadataBytes(data.value);
    // Open enters with its decode watchdog active; loading bundled modules is a
    // process phase. Each subsequent synchronous decode announces its own phase.
    if (data.operation === 'open' || data.operation === 'metadata' || data.operation === 'preview')
      globalThis.postMessage({ id: data.id, phase: 'process' });
    runtime ??= new AvatarCodecRuntime(await loadAvatarCodecs(), (phase) =>
      globalThis.postMessage({ id: currentId, phase }),
    );
    // The callback must refer to the currently executing command, not the first.
    currentId = data.id;
    const result = await runtime.command(
      data.operation,
      data.value,
      data.outputCapacity,
      data.input,
    );
    globalThis.postMessage(
      { id: data.id, ...result },
      { transfer: 'buffer' in result ? [result.buffer!] : [] },
    );
  } catch (error) {
    failed = true;
    globalThis.postMessage({ id: data?.id, error: String(error).slice(0, 4096) });
  } finally {
    running = false;
  }
};
let currentId = 0;
