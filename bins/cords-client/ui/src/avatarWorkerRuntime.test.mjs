import { afterEach, expect, it, vi } from 'vitest';
import { Worker as NodeWorker } from 'node:worker_threads';
import { AvatarWorkerSupervisor } from './avatarWorkerSession';
afterEach(() => vi.useRealTimers());

it('terminates a real worker stuck inside synchronous WebAssembly', async () => {
  vi.useFakeTimers();
  let started;
  const ready = new Promise((resolve) => {
    started = resolve;
  });
  let exited;
  const stopped = new Promise((resolve) => {
    exited = resolve;
  });
  const worker = new NodeWorker(
    `
      const { parentPort } = require('node:worker_threads');
      parentPort.once('message', () => {
        const binary = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,
          7,8,1,4,104,97,110,103,0,0,10,9,1,7,0,3,64,12,0,11,11]);
        const instance = new WebAssembly.Instance(new WebAssembly.Module(binary));
        parentPort.postMessage('started');
        instance.exports.hang();
      });`,
    { eval: true },
  );
  const port = {
    onmessage: null,
    onerror: null,
    onmessageerror: null,
    postMessage: (message, transfer) => worker.postMessage(message, transfer),
    terminate: () => {
      void worker.terminate();
    },
  };
  worker.on('message', () => started());
  worker.on('exit', () => exited());
  worker.on('error', (error) => port.onerror?.({ message: error.message }));
  const supervisor = new AvatarWorkerSupervisor(() => port);
  const session = supervisor.start(new ArrayBuffer(1));
  const rejection = expect(session.opened).rejects.toThrow('15-second');
  try {
    await ready;
    vi.advanceTimersByTime(15_000);
    await rejection;
    await stopped;
    expect(supervisor.memory.used).toBe(0);
  } finally {
    supervisor.cancel();
    await worker.terminate();
  }
});
