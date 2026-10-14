import { describe, expect, it } from 'vitest';
import {
  AvatarDeadline,
  AvatarFrameBudget,
  AvatarMemoryBudget,
  avatarLimits,
  avatarResizeAttempts,
  exactJxlTiming,
  validateCompressedAvatar,
} from './avatarPolicy';

describe('avatar processing policy', () => {
  it('enforces the exact compressed boundary before Base64', () => {
    expect(() => validateCompressedAvatar(4_999_999)).not.toThrow();
    expect(() => validateCompressedAvatar(5_000_000)).not.toThrow();
    expect(() => validateCompressedAvatar(5_000_001)).toThrow('5,000,000');
    expect(() => validateCompressedAvatar(0)).toThrow();
    expect(() => validateCompressedAvatar(NaN)).toThrow();
    expect(4 * Math.ceil(5_000_000 / 3)).toBe(6_666_668);
  });

  it('accounts for simultaneous heaps, codec buffers and copies under one cap', () => {
    const budget = new AvatarMemoryBudget();
    const jxlHeap = {},
      sourceHeap = {},
      hostBuffers = {},
      copy = {};
    budget.reserve(jxlHeap, 256 * 1024 * 1024);
    budget.reserve(sourceHeap, 128 * 1024 * 1024);
    budget.reserve(hostBuffers, 128 * 1024 * 1024);
    expect(budget.used).toBe(avatarLimits.memoryBytes);
    expect(() => budget.reserve(copy, 1)).toThrow('512 MiB');
    expect(budget.used).toBe(avatarLimits.memoryBytes);
    budget.release(hostBuffers);
    budget.release(hostBuffers);
    budget.reserve(copy, 128 * 1024 * 1024);
    expect(budget.peak).toBe(avatarLimits.memoryBytes);
    expect(() => budget.reserve(sourceHeap, 128 * 1024 * 1024 + 1)).toThrow();
    expect(budget.used).toBe(avatarLimits.memoryBytes);
    budget.clear();
    expect(budget.used).toBe(0);
  });

  it('bounds zero-duration compositing steps by count and full-canvas pixels', () => {
    const count = new AvatarFrameBudget();
    for (let i = 0; i < 1000; i++) count.add(1, 1);
    expect(() => count.add(1, 1)).toThrow('frame');
    const pixels = new AvatarFrameBudget();
    for (let i = 0; i < 3; i++) pixels.add(5000, 5000);
    expect(() => pixels.add(5000, 5000)).not.toThrow();
    expect(() => pixels.add(1, 1)).toThrow('pixel');
  });

  it('shares one deadline across every resize/decode attempt and time spent cropping', () => {
    let now = 100;
    const deadline = new AvatarDeadline(() => now);
    now += 110_000;
    expect(deadline.decodeRemaining()).toBe(10_000);
    now += 9_999;
    expect(deadline.remaining()).toBe(1);
    now += 1;
    expect(() => deadline.remaining()).toThrow('120 seconds');
    expect(() => deadline.decodeRemaining()).toThrow('120 seconds');
  });

  it('preserves exact APNG rational durations and zero steps without milliseconds', () => {
    const delays = [
      { numerator: 0n, denominator: 100n },
      { numerator: 1n, denominator: 60n },
      { numerator: 1n, denominator: 7n },
      { numerator: 1n, denominator: 60n },
    ];
    const timing = exactJxlTiming(delays);
    expect(timing.durations).toEqual([0, 7, 60, 7]);
    for (let index = 0; index < delays.length; index++) {
      const delay = delays[index];
      expect(
        BigInt(timing.durations[index]) *
          BigInt(timing.ticksPerSecondDenominator) *
          delay.denominator,
      ).toBe(delay.numerator * BigInt(timing.ticksPerSecondNumerator));
    }
  });

  it('retains every zero-duration frame even for an all-zero source', () => {
    const result = exactJxlTiming(Array(5).fill({ numerator: 0n, denominator: 100n }));
    expect(result.durations).toEqual([0, 0, 0, 0, 0]);
  });

  it('rejects excessive and unrepresentable timing without normalizing it', () => {
    expect(() => exactJxlTiming([{ numerator: 121n, denominator: 1n }])).toThrow('120 seconds');
    expect(() => exactJxlTiming([{ numerator: 1n, denominator: 0n }])).toThrow('Invalid');
    expect(() =>
      exactJxlTiming([
        { numerator: 1n, denominator: 65_521n },
        { numerator: 1n, denominator: 65_519n },
        { numerator: 1n, denominator: 65_513n },
      ]),
    ).toThrow('exactly');
  });

  it('generates aspect-preserving attempts without upscaling and stops at the floor', () => {
    const attempts = avatarResizeAttempts(2048, 1024);
    expect(attempts[0]).toEqual([1024, 512]);
    expect(attempts.at(-1)).toEqual([64, 32]);
    expect(avatarResizeAttempts(30, 15)).toEqual([[30, 15]]);
    expect(avatarResizeAttempts(1024, 1).at(-1)).toEqual([64, 1]);
  });
});
