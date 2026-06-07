// Two-pass gated integrated loudness per ITU-R BS.1770-4 §5.1.
// TS port of audio-meters-core/src/integrated.rs.
//
// Where the Rust type uses a const-generic `N`, the TS class takes
// `windowSize` as a constructor argument and stores its ring in a
// Float32Array. The arithmetic (gate constants, two-pass structure)
// matches the Rust reference; cross-runtime parity is verified at
// the unit-test level.

/** Absolute-gate threshold per BS.1770-4. Same constant as `LufsI` in ballistic.ts. */
export const ABSOLUTE_GATE_DB = -70.0;
/** Relative-gate offset, in LU below the absolute-gated mean. */
export const RELATIVE_GATE_OFFSET_LU = 10.0;
/** Display floor when no samples survive the absolute gate. */
export const NEG_INFINITY_FLOOR_DB = -120.0;

/** Two-pass gated integrated loudness with a sliding window. */
export class RelativelyGatedLufsI {
  readonly windowSize: number;
  private ring: Float32Array;
  private head: number = 0;
  private count: number = 0;
  private lastReadingDb: number = NEG_INFINITY_FLOOR_DB;

  constructor(windowSize: number) {
    if (!Number.isInteger(windowSize) || windowSize < 1) {
      throw new Error(
        `RelativelyGatedLufsI: windowSize must be a positive integer (got ${windowSize})`,
      );
    }
    this.windowSize = windowSize;
    this.ring = new Float32Array(windowSize);
    this.ring.fill(NEG_INFINITY_FLOOR_DB);
  }

  reset(): void {
    this.ring.fill(NEG_INFINITY_FLOOR_DB);
    this.head = 0;
    this.count = 0;
    this.lastReadingDb = NEG_INFINITY_FLOOR_DB;
  }

  /** Number of samples currently in the ring (saturates at `windowSize`). */
  len(): number {
    return this.count;
  }

  isEmpty(): boolean {
    return this.count === 0;
  }

  capacity(): number {
    return this.windowSize;
  }

  readingDb(): number {
    return this.lastReadingDb;
  }

  /**
   * Push a per-frame dBFS sample and recompute the doubly-gated
   * mean. `dt` is currently unused but is part of the API surface
   * for parity with `BallisticState.update`.
   */
  update(dbfs: number, _dt: number): number {
    const clean = !Number.isFinite(dbfs)
      ? NEG_INFINITY_FLOOR_DB
      : dbfs < NEG_INFINITY_FLOOR_DB
        ? NEG_INFINITY_FLOOR_DB
        : dbfs;

    this.ring[this.head] = clean;
    this.head = (this.head + 1) % this.windowSize;
    if (this.count < this.windowSize) {
      this.count += 1;
    }

    // Pass 1: absolute-gated mean of linear power.
    let absSum = 0;
    let absCount = 0;
    for (let i = 0; i < this.count; i++) {
      const x = this.ring[i];
      if (x >= ABSOLUTE_GATE_DB) {
        absSum += Math.pow(10, x / 10);
        absCount += 1;
      }
    }
    if (absCount === 0) {
      this.lastReadingDb = NEG_INFINITY_FLOOR_DB;
      return this.lastReadingDb;
    }
    const absMeanPower = absSum / absCount;
    const absMeanDb = 10 * Math.log10(absMeanPower);

    // Pass 2: relative-gated mean. Threshold floors at the absolute
    // gate (BS.1770-4 §5.1: relative gate must not relax absolute).
    const relGate = Math.max(absMeanDb - RELATIVE_GATE_OFFSET_LU, ABSOLUTE_GATE_DB);
    let relSum = 0;
    let relCount = 0;
    for (let i = 0; i < this.count; i++) {
      const x = this.ring[i];
      if (x >= relGate) {
        relSum += Math.pow(10, x / 10);
        relCount += 1;
      }
    }
    if (relCount === 0) {
      // Pathological — fall back to absolute-gated mean.
      this.lastReadingDb = absMeanDb;
    } else {
      const relMeanPower = relSum / relCount;
      this.lastReadingDb = 10 * Math.log10(relMeanPower);
    }
    return this.lastReadingDb;
  }
}
