import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { avatarMemory } from './avatarMemory';
import { avatarEditing, playAvatar } from './avatarPlayback';

class FakeWorker {
  static instances: FakeWorker[] = [];
  onmessage: ((event: { data: unknown }) => void) | undefined;
  onerror: ((event: { message: string }) => void) | undefined;
  terminate = vi.fn();
  postMessage = vi.fn();
  constructor() {
    FakeWorker.instances.push(this);
  }
  emit(data: unknown) {
    this.onmessage?.({ data });
  }
}
const disposers: (() => void)[] = [];
beforeEach(() => {
  vi.useFakeTimers();
  FakeWorker.instances = [];
  vi.stubGlobal('Worker', FakeWorker);
  avatarEditing.set(false);
});
afterEach(() => {
  for (const dispose of disposers.splice(0)) dispose();
  avatarEditing.set(false);
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
describe('shared bounded avatar playback', () => {
  it('reports worker startup failure without leaking a decoder slot', async () => {
    const baseline = avatarMemory.used;
    const fail = vi.fn();
    vi.stubGlobal(
      'Worker',
      class {
        constructor() {
          throw new Error('Worker blocked');
        }
      },
    );
    disposers.push(playAvatar('AQID', true, vi.fn(), fail));
    await Promise.resolve();
    expect(fail).toHaveBeenCalledWith(expect.stringContaining('Worker blocked'));
    expect(avatarMemory.used).toBe(baseline);
    vi.stubGlobal('Worker', FakeWorker);
    for (const image of ['BAUG', 'BwgJ', 'CgsM'])
      disposers.push(playAvatar(image, true, vi.fn(), vi.fn()));
    expect(FakeWorker.instances).toHaveLength(3);
  });
  it('shares one decoder and frame across repeated profile appearances', () => {
    const baseline = avatarMemory.used;
    const first = vi.fn(),
      second = vi.fn(),
      fail = vi.fn();
    const removeFirst = playAvatar('AQID', true, first, fail);
    disposers.push(removeFirst);
    const removeSecond = playAvatar('AQID', true, second, fail);
    disposers.push(removeSecond);
    expect(FakeWorker.instances).toHaveLength(1);
    const worker = FakeWorker.instances[0];
    worker.emit({
      phase: 'idle',
      width: 1,
      height: 1,
      pixels: new Uint8Array([1, 2, 3, 255]).buffer,
    });
    expect(first).toHaveBeenCalledOnce();
    expect(second).toHaveBeenCalledOnce();
    removeFirst();
    disposers.shift();
    expect(worker.terminate).not.toHaveBeenCalled();
    removeSecond();
    disposers.shift();
    expect(worker.terminate).toHaveBeenCalledOnce();
    expect(fail).not.toHaveBeenCalled();
    expect(avatarMemory.used).toBe(baseline);
  });
  it('releases decoder heaps before an editor starts and resumes fairly', () => {
    const baseline = avatarMemory.used;
    for (const image of ['AQID', 'BAUG', 'BwgJ', 'CgsM'])
      disposers.push(playAvatar(image, true, vi.fn(), vi.fn()));
    expect(FakeWorker.instances).toHaveLength(3);
    avatarEditing.set(true);
    expect(avatarMemory.used).toBe(baseline);
    expect(FakeWorker.instances.every((worker) => worker.terminate.mock.calls.length === 1)).toBe(
      true,
    );
    avatarEditing.set(false);
    expect(FakeWorker.instances).toHaveLength(6);
    vi.advanceTimersByTime(10000);
    expect(FakeWorker.instances).toHaveLength(9);
  });
  it('terminates a blocked decoder and rejects malformed frame dimensions', () => {
    const baseline = avatarMemory.used,
      fail = vi.fn();
    disposers.push(playAvatar('AQID', true, vi.fn(), fail));
    vi.advanceTimersByTime(15000);
    expect(fail).toHaveBeenCalledWith(expect.stringContaining('15 seconds'));
    expect(avatarMemory.used).toBe(baseline);
    disposers.push(playAvatar('BAUG', true, vi.fn(), fail));
    FakeWorker.instances[1].emit({ width: 1024, height: 1024, pixels: new ArrayBuffer(4) });
    expect(fail).toHaveBeenCalledWith('Invalid avatar playback frame');
    expect(avatarMemory.used).toBe(baseline);
  });
});
