// Shared processing policy. Playback scheduling must never alter these exact
// source/encoded durations. WASM heaps are reservations, not extra allowances.
export const avatarLimits = Object.freeze({
  compressedBytes: 5_000_000,
  inputBytes: 50_000_000,
  frames: 1_000,
  dimension: 8_192,
  canvasPixels: 32_000_000,
  cumulativePixels: 100_000_000,
  metadataBytes: 1_048_576,
  memoryBytes: 512 * 1024 * 1024,
  processingMs: 120_000,
  decodeMs: 15_000,
  cycleSeconds: 120,
});

function integer(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`Invalid ${label}`);
  return value;
}

export function validateCompressedAvatar(bytes: number): void {
  if (integer(bytes, 'JXL size') === 0 || bytes > avatarLimits.compressedBytes)
    throw new Error('Encoded profile picture must be at most 5,000,000 bytes');
}

export function validateCanvas(width: number, height: number): number {
  integer(width, 'image width');
  integer(height, 'image height');
  if (!width || !height || width > avatarLimits.dimension || height > avatarLimits.dimension)
    throw new Error('Image dimensions exceed the supported limit');
  const pixels = width * height;
  if (pixels > avatarLimits.canvasPixels) throw new Error('Image canvas exceeds 32 million pixels');
  return pixels;
}

/** Count compositing steps too, including zero-duration frames. */
export class AvatarFrameBudget {
  private frames = 0;
  private pixels = 0;

  add(width: number, height: number): void {
    const pixels = validateCanvas(width, height);
    if (this.frames >= avatarLimits.frames || pixels > avatarLimits.cumulativePixels - this.pixels)
      throw new Error('Animation exceeds frame or cumulative decoded-pixel limits');
    this.frames += 1;
    this.pixels += pixels;
  }
}

/** One owner per backing allocation; reserve copies separately, transfer ownership. */
export class AvatarMemoryBudget {
  private allocations = new Map<object, number>();
  private total = 0;
  private maximum = 0;

  get used(): number {
    return this.total;
  }

  get peak(): number {
    return this.maximum;
  }

  reserve(owner: object, bytes: number): void {
    integer(bytes, 'allocation size');
    const previous = this.allocations.get(owner) ?? 0;
    const withoutPrevious = this.total - previous;
    if (bytes > avatarLimits.memoryBytes - withoutPrevious)
      throw new Error('Profile-picture processing exceeds the shared 512 MiB memory budget');
    this.allocations.set(owner, bytes);
    this.total = withoutPrevious + bytes;
    this.maximum = Math.max(this.maximum, this.total);
  }

  release(owner: object): void {
    this.total -= this.allocations.get(owner) ?? 0;
    this.allocations.delete(owner);
  }

  clear(): void {
    this.allocations.clear();
    this.total = 0;
  }
}

/** Reuse this object through every attempt. Time in the crop workflow also counts. */
export class AvatarDeadline {
  private readonly endsAt: number;

  constructor(private readonly clock: () => number = () => performance.now()) {
    this.endsAt = clock() + avatarLimits.processingMs;
  }

  remaining(): number {
    const remaining = this.endsAt - this.clock();
    if (!Number.isFinite(remaining) || remaining <= 0)
      throw new Error('Profile-picture processing exceeded 120 seconds; select the image again');
    return remaining;
  }

  decodeRemaining(): number {
    return Math.min(avatarLimits.decodeMs, this.remaining());
  }
}

export type ExactDelay = Readonly<{ numerator: bigint; denominator: bigint }>;
export type JxlTiming = Readonly<{
  ticksPerSecondNumerator: number;
  ticksPerSecondDenominator: number;
  durations: readonly number[];
}>;

function gcd(left: bigint, right: bigint): bigint {
  while (right) [left, right] = [right, left % right];
  return left;
}

const maximumU32 = 0xffff_ffffn;

/** Seconds, as exact fractions. GIF centiseconds/APNG fractions/WebP milliseconds
 * must enter here without floating-point conversion or browser minimum delays.
 * JXL sources may retain their existing timebase directly instead of rebasing.
 */
export function exactJxlTiming(delays: readonly ExactDelay[]): JxlTiming {
  if (!delays.length || delays.length > avatarLimits.frames)
    throw new Error('Invalid animation frame count');
  let denominator = 1n;
  for (const delay of delays) {
    if (delay.numerator < 0n || delay.denominator <= 0n)
      throw new Error('Invalid source animation timing');
    const reduced = delay.denominator / gcd(delay.numerator, delay.denominator);
    denominator = (denominator / gcd(denominator, reduced)) * reduced;
    // Bound BigInt growth before accepting malicious timing metadata. Such a
    // timebase cannot fit uint32 even after taking the GCD of reduced numerators.
    if (denominator > maximumU32)
      throw new Error('Animation timing cannot be represented exactly in JPEG XL');
  }
  const scaled = delays.map(({ numerator, denominator: divisor }) => {
    const factor = gcd(numerator, divisor);
    return (numerator / factor) * (denominator / (divisor / factor));
  });
  const common = scaled.reduce((current, value) => gcd(current, value), 0n);
  if (common === 0n)
    return {
      ticksPerSecondNumerator: 1,
      ticksPerSecondDenominator: 1,
      durations: scaled.map(() => 0),
    };
  const reduction = gcd(common, denominator);
  const numerator = denominator / reduction;
  const timeDenominator = common / reduction;
  const durations = scaled.map((value) => value / common);
  if (timeDenominator > maximumU32 || durations.some((value) => value > maximumU32))
    throw new Error('Animation timing cannot be represented exactly in JPEG XL');
  const total = scaled.reduce((current, value) => current + value, 0n);
  if (total > BigInt(avatarLimits.cycleSeconds) * denominator)
    throw new Error('Animation cycle exceeds 120 seconds');
  return {
    ticksPerSecondNumerator: Number(numerator),
    ticksPerSecondDenominator: Number(timeDenominator),
    durations: durations.map(Number),
  };
}

/** Always derive a retry from the original crop, never from an earlier resize. */
export function avatarResizeAttempts(width: number, height: number): readonly [number, number][] {
  validateCanvas(width, height);
  const sourceEdge = Math.max(width, height);
  let edge = Math.min(1024, sourceEdge);
  const floor = Math.min(64, edge);
  const attempts: [number, number][] = [];
  for (;;) {
    attempts.push([
      Math.max(1, Math.round((width * edge) / sourceEdge)),
      Math.max(1, Math.round((height * edge) / sourceEdge)),
    ]);
    if (edge === floor) return attempts;
    edge = Math.max(floor, Math.floor(edge * 0.8));
  }
}
