import { AvatarDeadline, AvatarMemoryBudget, avatarLimits } from './avatarPolicy';

/** Narrow transport shared by a dedicated browser Worker and runtime tests. */
export interface AvatarWorkerPort {
  postMessage(message: unknown, transfer: ArrayBuffer[]): void;
  terminate(): void;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { message: string }) => void) | null;
  onmessageerror: (() => void) | null;
}

export type AvatarWorkerOperation = 'open' | 'metadata' | 'preview' | 'crop' | 'encode';

export interface AvatarWorkerResult {
  readonly value: unknown;
  readonly buffer: ArrayBuffer | undefined;
  /** Release the reservation only after the consumer drops every buffer view. */
  release(): void;
}

interface Pending {
  id: number;
  owner: object;
  capacity: number;
  timer: ReturnType<typeof setTimeout>;
  decoding: boolean;
  resolve: (result: AvatarWorkerResult) => void;
  reject: (reason: Error) => void;
}

const heapBytes = (256 + 128) * 1024 * 1024;
// Bindings, timing metadata and transfer plumbing also need headroom. The actual
// codec command must still bound its host allocations within this reservation.
// 16 MiB for bindings/metadata plus 48 MiB for explicitly bounded transient
// canvases, hash copies and verification buffers inside the worker.
const runtimeBytes = 64 * 1024 * 1024;

/** Binary data has exactly one explicit transferable slot. Bound metadata before
 * cloning it so nested typed arrays cannot evade the buffer reservation ledger. */
export function avatarWorkerMetadataBytes(value: unknown): number {
  let remaining = avatarLimits.metadataBytes;
  let nodes = 0;
  function visit(item: unknown, depth: number): void {
    if (++nodes > 16_000 || depth > 16) throw new Error('Profile-picture metadata is too complex');
    if (item === null || item === undefined) return;
    if (typeof item === 'boolean') remaining -= 8;
    else if (typeof item === 'number' && Number.isFinite(item)) remaining -= 16;
    else if (typeof item === 'string') remaining -= item.length * 2 + 64;
    else if (typeof item === 'bigint' && item >= 0n && item <= 0xffff_ffff_ffff_ffffn)
      remaining -= 16;
    else if (Array.isArray(item)) {
      if (item.length > 16_000) throw new Error('Profile-picture metadata is too complex');
      remaining -= item.length * 8 + 64;
      for (const child of item) visit(child, depth + 1);
    } else if (typeof item === 'object' && Object.getPrototypeOf(item) === Object.prototype) {
      remaining -= 64;
      for (const key of Object.keys(item)) {
        remaining -= key.length * 2 + 32;
        visit((item as Record<string, unknown>)[key], depth + 1);
      }
    } else
      throw new Error('Profile-picture metadata must not contain binary or unsupported values');
    if (remaining < 0) throw new Error('Profile-picture metadata exceeds 1 MiB');
  }
  visit(value, 0);
  return avatarLimits.metadataBytes - remaining;
}

/** Host-side watchdog lives outside the worker, so synchronous WASM cannot block
 * cancellation. One supervisor per active account; share its ledger with caches.
 * This controls command lifetimes, not the worker's internal codec allocations.
 */
export class AvatarWorkerSupervisor {
  private active: AvatarWorkerSession | undefined;

  constructor(
    private readonly createWorker: () => AvatarWorkerPort,
    readonly memory = new AvatarMemoryBudget(),
    private readonly clock: () => number = () => performance.now(),
  ) {}

  start(input: ArrayBuffer, deadline = new AvatarDeadline(this.clock)): AvatarWorkerSession {
    if (this.active) throw new Error('Another profile picture is still being processed');
    if (!input.byteLength || input.byteLength > avatarLimits.inputBytes)
      throw new Error('Profile-picture input must be between 1 and 50,000,000 bytes');
    const session = new AvatarWorkerSession(this.createWorker, this.memory, input, deadline, () => {
      this.active = undefined;
    });
    if (!session.disposed) this.active = session;
    return session;
  }

  /** Call before clearing account caches on lock or account switch. */
  cancel(): void {
    this.active?.dispose();
  }
}

export class AvatarWorkerSession {
  private readonly owner = {};
  private readonly deadline: AvatarDeadline;
  private readonly worker: AvatarWorkerPort;
  private readonly timer: ReturnType<typeof setTimeout>;
  private pending: Pending | undefined;
  private sequence = 0;
  private failure: Error | undefined;
  private input: ArrayBuffer | undefined;
  readonly opened: Promise<AvatarWorkerResult>;

  get disposed(): boolean {
    return this.failure !== undefined;
  }

  constructor(
    createWorker: () => AvatarWorkerPort,
    private readonly memory: AvatarMemoryBudget,
    input: ArrayBuffer,
    deadline: AvatarDeadline,
    private readonly onDispose: () => void,
  ) {
    // The file picker may supply a deadline created before reading the file.
    // Never start a fresh clock after an asynchronous file read or resize retry.
    this.deadline = deadline;
    this.deadline.remaining();
    this.input = input;
    this.memory.reserve(this.owner, heapBytes + runtimeBytes + input.byteLength);
    let worker: AvatarWorkerPort | undefined;
    try {
      worker = createWorker();
      const remaining = this.deadline.remaining();
      this.worker = worker;
      this.timer = setTimeout(() => this.stop(this.expired()), remaining);
    } catch (error) {
      worker?.terminate();
      this.memory.release(this.owner);
      throw error;
    }
    this.worker.onmessage = ({ data }) => this.receive(data);
    this.worker.onerror = ({ message }) =>
      this.stop(new Error(`Profile-picture worker failed: ${message}`));
    this.worker.onmessageerror = () =>
      this.stop(new Error('Profile-picture worker returned unreadable data'));
    // Initial input transfers once; its reservation remains live inside the
    // worker until termination, including all future decode/resize attempts.
    this.opened = this.request('open', null, 0);
  }

  request(
    operation: AvatarWorkerOperation,
    value: unknown,
    outputCapacity: number,
  ): Promise<AvatarWorkerResult> {
    if (this.failure) return Promise.reject(this.failure);
    if (this.pending)
      return Promise.reject(new Error('Wait for the current profile-picture command'));
    const owner = {};
    const decode = operation === 'open' || operation === 'preview' || operation === 'metadata';
    let milliseconds: number;
    try {
      const commandBytes = avatarWorkerMetadataBytes(value);
      if (!Number.isSafeInteger(outputCapacity) || outputCapacity < 0)
        throw new Error('Invalid profile-picture output reservation');
      milliseconds = decode ? this.deadline.decodeRemaining() : this.deadline.remaining();
      // Reserve response metadata before the worker creates/posts it, plus both
      // the caller's command metadata and its structured clone in the worker.
      this.memory.reserve(owner, outputCapacity + avatarLimits.metadataBytes + commandBytes * 2);
    } catch (error) {
      this.stop(error instanceof Error ? error : new Error(String(error)));
      return Promise.reject(this.failure);
    }
    return new Promise((resolve, reject) => {
      const id = ++this.sequence;
      const timer = setTimeout(
        () =>
          this.stop(
            new Error(
              decode
                ? 'Profile-picture decode exceeded its 15-second or remaining upload deadline'
                : this.expired().message,
            ),
          ),
        milliseconds,
      );
      this.pending = {
        id,
        owner,
        capacity: outputCapacity,
        timer,
        decoding: decode,
        resolve,
        reject,
      };
      try {
        const input = this.input;
        this.input = undefined;
        this.worker.postMessage(
          { id, operation, value, outputCapacity, input },
          input ? [input] : [],
        );
      } catch (error) {
        this.stop(new Error(`Cannot send profile-picture command: ${String(error)}`));
      }
    });
  }

  dispose(): void {
    this.stop(new Error('Profile-picture processing was cancelled'));
  }

  private expired(): Error {
    return new Error('Profile-picture processing exceeded 120 seconds; select the image again');
  }

  private receive(data: unknown): void {
    if (this.failure) return;
    const pending = this.pending;
    if (
      !pending ||
      !data ||
      typeof data !== 'object' ||
      !('id' in data) ||
      data.id !== pending.id
    ) {
      this.stop(new Error('Profile-picture worker returned an unexpected response'));
      return;
    }
    if ('error' in data) {
      if (
        Object.keys(data).some((key) => key !== 'id' && key !== 'error') ||
        typeof data.error !== 'string' ||
        data.error.length > 4096
      ) {
        this.stop(new Error('Profile-picture worker returned an invalid error'));
        return;
      }
      this.stop(new Error(`Profile-picture processing failed: ${String(data.error)}`));
      return;
    }
    // Long encode commands stream through many decodes. The worker must report
    // entry/exit around each synchronous decode so this external timer can stop
    // a stuck codec; these phase messages never renew the upload deadline.
    if ('phase' in data) {
      if (
        Object.keys(data).some((key) => key !== 'id' && key !== 'phase') ||
        (data.phase !== 'decode' && data.phase !== 'process') ||
        (data.phase === 'decode') === pending.decoding
      ) {
        this.stop(new Error('Profile-picture worker returned an invalid processing phase'));
        return;
      }
      try {
        const decoding = data.phase === 'decode';
        const milliseconds = decoding ? this.deadline.decodeRemaining() : this.deadline.remaining();
        clearTimeout(pending.timer);
        pending.decoding = decoding;
        pending.timer = setTimeout(
          () =>
            this.stop(
              new Error(
                decoding
                  ? 'Profile-picture decode exceeded its 15-second or remaining upload deadline'
                  : this.expired().message,
              ),
            ),
          milliseconds,
        );
      } catch {
        this.stop(this.expired());
      }
      return;
    }
    if (Object.keys(data).some((key) => key !== 'id' && key !== 'buffer' && key !== 'value')) {
      this.stop(new Error('Profile-picture worker returned unexpected payload fields'));
      return;
    }
    const buffer = 'buffer' in data ? data.buffer : undefined;
    if (
      (buffer !== undefined && !(buffer instanceof ArrayBuffer)) ||
      (buffer?.byteLength ?? 0) > pending.capacity
    ) {
      this.stop(new Error('Profile-picture worker exceeded its reserved output buffer'));
      return;
    }
    let metadataBytes: number;
    try {
      this.deadline.remaining();
      metadataBytes = avatarWorkerMetadataBytes('value' in data ? data.value : undefined);
    } catch (error) {
      this.stop(error instanceof Error ? error : new Error(String(error)));
      return;
    }
    clearTimeout(pending.timer);
    this.pending = undefined;
    this.memory.reserve(pending.owner, (buffer?.byteLength ?? 0) + metadataBytes);
    const result = {
      value: 'value' in data ? data.value : undefined,
      buffer,
      release: () => {
        result.buffer = undefined;
        result.value = undefined;
        this.memory.release(pending.owner);
      },
    };
    pending.resolve(result);
  }

  private stop(reason: Error): void {
    if (this.failure) return;
    this.failure = reason;
    clearTimeout(this.timer);
    this.worker.onmessage = this.worker.onerror = this.worker.onmessageerror = null;
    this.worker.terminate();
    this.input = undefined;
    this.memory.release(this.owner);
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.memory.release(this.pending.owner);
      this.pending.reject(reason);
      this.pending = undefined;
    }
    // Successfully returned buffers belong to their consumers until release;
    // terminating a worker must not erase those outstanding reservations.
    this.onDispose();
  }
}
