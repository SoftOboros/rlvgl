// Generic multi-channel meter composite. Mirror of
// widgets/src/meters/multi_channel.rs. Foundation for stereo (N=2),
// 5.1 surround (N=6), graphic EQ (N=8/16/32), and any other
// fixed-channel-count layout.
//
// `N` is a runtime count on the TS side (no const generics). Caller
// passes a pre-built array of children; the container partitions the
// outer rect, forwards updates, and draws all children.

import type { DrawSink } from "./led-bargraph-core.ts";
import type { MeterCoreLike, Rect } from "./stereo.ts";

/** Split `outer` horizontally into `n` equally-sized children with `gap` px between. */
export function splitHorizontalN(outer: Rect, gap: number, n: number): Rect[] {
  if (n < 1) throw new Error("splitHorizontalN requires n >= 1");
  const totalGap = gap * (n - 1);
  const cellW = Math.max(1, Math.floor((outer.w - totalGap) / n));
  return Array.from({ length: n }, (_, i) => {
    const x = outer.x + i * (cellW + gap);
    const w = i === n - 1 ? outer.x + outer.w - x : cellW;
    return { x, y: outer.y, w: Math.max(1, w), h: outer.h };
  });
}

/** Split `outer` vertically. Useful for stacked horizontal layouts. */
export function splitVerticalN(outer: Rect, gap: number, n: number): Rect[] {
  if (n < 1) throw new Error("splitVerticalN requires n >= 1");
  const totalGap = gap * (n - 1);
  const cellH = Math.max(1, Math.floor((outer.h - totalGap) / n));
  return Array.from({ length: n }, (_, i) => {
    const y = outer.y + i * (cellH + gap);
    const h = i === n - 1 ? outer.y + outer.h - y : cellH;
    return { x: outer.x, y, w: outer.w, h: Math.max(1, h) };
  });
}

/**
 * Generic multi-channel composite. Holds N children of the same
 * meter family and forwards per-channel dBFS values.
 */
export class MultiChannel<C extends MeterCoreLike> {
  readonly bounds: Rect;
  readonly gap: number;
  readonly channels: readonly C[];
  readonly childBounds: readonly Rect[];

  constructor(bounds: Rect, gap: number, channels: C[]);
  constructor(
    bounds: Rect,
    gap: number,
    channels: C[],
    childBounds: Rect[],
  );
  constructor(
    bounds: Rect,
    gap: number,
    channels: C[],
    childBounds?: Rect[],
  ) {
    if (channels.length < 1) {
      throw new Error("MultiChannel requires at least one channel");
    }
    this.bounds = bounds;
    this.gap = gap;
    this.channels = channels;
    this.childBounds =
      childBounds ?? splitHorizontalN(bounds, gap, channels.length);
    if (this.childBounds.length !== channels.length) {
      throw new Error(
        `childBounds length ${this.childBounds.length} != channels length ${channels.length}`,
      );
    }
  }

  /** Build by horizontal partition + factory. */
  static fromHorizontalFactory<C extends MeterCoreLike>(
    bounds: Rect,
    gap: number,
    n: number,
    make: (idx: number, childBounds: Rect) => C,
  ): MultiChannel<C> {
    const childBounds = splitHorizontalN(bounds, gap, n);
    const channels = childBounds.map((b, i) => make(i, b));
    return new MultiChannel(bounds, gap, channels, childBounds);
  }

  /** Build by vertical partition + factory. */
  static fromVerticalFactory<C extends MeterCoreLike>(
    bounds: Rect,
    gap: number,
    n: number,
    make: (idx: number, childBounds: Rect) => C,
  ): MultiChannel<C> {
    const childBounds = splitVerticalN(bounds, gap, n);
    const channels = childBounds.map((b, i) => make(i, b));
    return new MultiChannel(bounds, gap, channels, childBounds);
  }

  channelCount(): number {
    return this.channels.length;
  }

  channel(idx: number): C {
    return this.channels[idx];
  }

  /** Advance every channel: dbfs[i] drives channels[i]. */
  updateN(dbfs: readonly number[], dt: number): void {
    if (dbfs.length !== this.channels.length) {
      throw new Error(
        `updateN expected ${this.channels.length} samples, got ${dbfs.length}`,
      );
    }
    for (let i = 0; i < this.channels.length; i++) {
      this.channels[i].update(dbfs[i], dt);
    }
  }

  /** Advance one channel only (sparse update path). */
  updateAt(idx: number, dbfs: number, dt: number): void {
    this.channels[idx].update(dbfs, dt);
  }

  reset(): void {
    for (const ch of this.channels) {
      ch.reset?.();
    }
  }

  /** Paint every child at its sub-bounds. */
  draw(sink: DrawSink): void {
    for (let i = 0; i < this.channels.length; i++) {
      const b = this.childBounds[i];
      this.channels[i].draw(sink, b.x, b.y, b.w, b.h);
    }
  }
}
