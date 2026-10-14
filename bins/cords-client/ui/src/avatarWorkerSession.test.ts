import { afterEach, describe, expect, it, vi } from 'vitest';
import { AvatarDeadline, AvatarMemoryBudget, avatarLimits } from './avatarPolicy';
import {
  AvatarWorkerSupervisor,
  avatarWorkerMetadataBytes,
  type AvatarWorkerPort,
} from './avatarWorkerSession';

class Port implements AvatarWorkerPort {
  onmessage: AvatarWorkerPort['onmessage'] = null;
  onerror: AvatarWorkerPort['onerror'] = null;
  onmessageerror: AvatarWorkerPort['onmessageerror'] = null;
  terminated = false;
  last: { id: number; input?: ArrayBuffer; outputCapacity: number } | undefined;
  postMessage(message: unknown, transfer: ArrayBuffer[]): void {
    this.last = structuredClone(message, { transfer }) as typeof this.last;
  }
  terminate(): void {
    this.terminated = true;
  }
  reply(buffer?: ArrayBuffer): void {
    this.onmessage?.({ data: { id: this.last?.id, buffer } });
  }
}

function setup(memory = new AvatarMemoryBudget()) {
  vi.useFakeTimers();
  let now = 0;
  const ports: Port[] = [];
  const supervisor = new AvatarWorkerSupervisor(
    () => {
      const port = new Port();
      ports.push(port);
      return port;
    },
    memory,
    () => now,
  );
  return {
    supervisor,
    memory,
    ports,
    advance(milliseconds: number) {
      now += milliseconds;
      vi.advanceTimersByTime(milliseconds);
    },
  };
}

afterEach(() => vi.useRealTimers());

describe('avatar worker watchdog and ownership', () => {
  it('keeps returned metadata in the ledger until its consumer releases it', async () => {
    const { supervisor, ports, memory } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    const value = { frames: [{ numerator: 0n, denominator: 100n }], label: 'animated' };
    ports[0].onmessage?.({ data: { id: ports[0].last?.id, value } });
    const result = await session.opened;
    session.dispose();
    expect(memory.used).toBe(avatarWorkerMetadataBytes(value));
    expect(memory.used).toBeGreaterThan(0);
    result.release();
    expect(result.value).toBeUndefined();
    expect(memory.used).toBe(0);
  });

  it('includes file-reading time when given the original upload deadline', async () => {
    const { supervisor, ports } = setup();
    let time = 0;
    const deadline = new AvatarDeadline(() => time);
    time = 119_999;
    const session = supervisor.start(new ArrayBuffer(1), deadline);
    const rejection = expect(session.opened).rejects.toThrow('120 seconds');
    time = 120_000;
    vi.advanceTimersByTime(1);
    await rejection;
    expect(ports[0].terminated).toBe(true);
  });

  it('cleans up factory and synchronous transfer failures before a new attempt', async () => {
    const memory = new AvatarMemoryBudget();
    const factory = vi.fn<() => AvatarWorkerPort>(() => {
      throw new Error('Cannot create worker');
    });
    const supervisor = new AvatarWorkerSupervisor(factory, memory);
    expect(() => supervisor.start(new ArrayBuffer(1))).toThrow('Cannot create worker');
    expect(memory.used).toBe(0);
    const failed = new Port();
    failed.postMessage = () => {
      throw new Error('Cannot transfer');
    };
    factory.mockReturnValueOnce(failed);
    const session = supervisor.start(new ArrayBuffer(1));
    await expect(session.opened).rejects.toThrow('Cannot transfer');
    expect(memory.used).toBe(0);
    const succeeding = new Port();
    factory.mockReturnValueOnce(succeeding);
    const replacement = supervisor.start(new ArrayBuffer(1));
    succeeding.reply();
    (await replacement.opened).release();
    supervisor.cancel();
  });

  it('times each streamed decode independently without extending the upload', async () => {
    const { supervisor, ports, advance } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    ports[0].reply();
    (await session.opened).release();
    const encoded = session.request('encode', null, 5_000_000);
    const rejection = expect(encoded).rejects.toThrow('15-second');
    advance(5_000);
    ports[0].onmessage?.({ data: { id: ports[0].last?.id, phase: 'decode' } });
    advance(10_000);
    ports[0].onmessage?.({ data: { id: ports[0].last?.id, phase: 'process' } });
    advance(5_000);
    ports[0].onmessage?.({ data: { id: ports[0].last?.id, phase: 'decode' } });
    advance(15_000);
    await rejection;
    expect(ports[0].terminated).toBe(true);
  });

  it('rejects binary data hidden in metadata rather than cloning unreserved frames', async () => {
    const { supervisor, ports, memory } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    ports[0].reply();
    (await session.opened).release();
    await expect(session.request('crop', { hidden: new Uint8Array(10) }, 0)).rejects.toThrow(
      'binary',
    );
    expect(memory.used).toBe(0);
  });

  it('transfers input once, reserves both heaps, and prohibits concurrent uploads', async () => {
    const { supervisor, memory, ports } = setup();
    const input = new ArrayBuffer(50);
    const session = supervisor.start(input);
    expect(input.byteLength).toBe(0);
    expect(ports[0].last?.input?.byteLength).toBe(50);
    expect(memory.used).toBe(401 * 1024 * 1024 + 50);
    expect(() => supervisor.start(new ArrayBuffer(1))).toThrow('Another');
    ports[0].reply();
    (await session.opened).release();
    session.dispose();
    expect(ports[0].terminated).toBe(true);
    expect(memory.used).toBe(0);
  });

  it('terminates a nonresponding decode from the host and permits a fresh upload', async () => {
    const { supervisor, memory, ports, advance } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    const rejection = expect(session.opened).rejects.toThrow('15-second');
    advance(15_000);
    await rejection;
    expect(ports[0].terminated).toBe(true);
    expect(memory.used).toBe(0);
    const replacement = supervisor.start(new ArrayBuffer(1));
    ports[1].reply();
    (await replacement.opened).release();
    supervisor.cancel();
  });

  it('expires during idle crop time and never renews the deadline on a retry', async () => {
    const { supervisor, ports, advance } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    ports[0].reply();
    (await session.opened).release();
    advance(110_000);
    const retry = session.request('encode', { width: 64 }, 5_000_000);
    const rejection = expect(retry).rejects.toThrow('120 seconds');
    advance(10_000);
    await rejection;
    expect(ports[0].terminated).toBe(true);
    await expect(session.request('preview', null, 1)).rejects.toThrow('120 seconds');
  });

  it('caps a later decode by remaining upload time', async () => {
    const { supervisor, ports, advance } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    ports[0].reply();
    (await session.opened).release();
    advance(119_999);
    const rejection = expect(session.request('preview', null, 8)).rejects.toThrow('120 seconds');
    advance(1);
    await rejection;
    expect(ports[0].terminated).toBe(true);
  });

  it('accounts for cache and held frame buffers, including after worker termination', async () => {
    const { supervisor, ports, memory } = setup();
    const cache = {};
    memory.reserve(cache, 80 * 1024 * 1024);
    const session = supervisor.start(new ArrayBuffer(1));
    ports[0].reply();
    (await session.opened).release();
    const preview = session.request('preview', null, 20 * 1024 * 1024);
    ports[0].reply(new ArrayBuffer(20 * 1024 * 1024));
    const held = await preview;
    await expect(session.request('preview', null, 20 * 1024 * 1024)).rejects.toThrow('512 MiB');
    expect(memory.used).toBe(100 * 1024 * 1024);
    expect(held.buffer?.byteLength).toBe(20 * 1024 * 1024);
    held.release();
    held.release();
    expect(held.buffer).toBeUndefined();
    expect(memory.used).toBe(80 * 1024 * 1024);
    memory.release(cache);
  });

  it('rejects an output beyond its reservation, crashes and unreadable messages', async () => {
    for (const failure of ['output', 'crash', 'message'] as const) {
      const { supervisor, ports, memory } = setup();
      const session = supervisor.start(new ArrayBuffer(1));
      const rejection = expect(session.opened).rejects.toThrow(/reserved|failed|unreadable/);
      if (failure === 'output') ports[0].reply(new ArrayBuffer(1));
      if (failure === 'crash') ports[0].onerror?.({ message: 'WASM allocation trap' });
      if (failure === 'message') ports[0].onmessageerror?.();
      await rejection;
      expect(ports[0].terminated).toBe(true);
      expect(memory.used).toBe(0);
    }
  });

  it('cancels on account lock and releases pending reservations exactly once', async () => {
    const { supervisor, ports, memory } = setup();
    const session = supervisor.start(new ArrayBuffer(1));
    const rejection = expect(session.opened).rejects.toThrow('cancelled');
    supervisor.cancel();
    supervisor.cancel();
    await rejection;
    expect(ports[0].terminated).toBe(true);
    expect(memory.used).toBe(0);
  });

  it('fails before creating workers if input or aggregate reservations exceed limits', () => {
    const { supervisor, ports, memory } = setup();
    expect(() => supervisor.start(new ArrayBuffer(avatarLimits.inputBytes + 1))).toThrow(
      '50,000,000',
    );
    memory.reserve({}, 128 * 1024 * 1024);
    expect(() => supervisor.start(new ArrayBuffer(1))).toThrow('512 MiB');
    expect(ports).toHaveLength(0);
  });
});
