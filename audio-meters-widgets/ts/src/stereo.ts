// Stereo composition for any audio-meter core. Mirror of
// widgets/src/meters/stereo.rs.
//
// Any object with `update(dbfs, dt)` and `draw(sink, x, y, w, h)` can
// be paired into a stereo container. The first-party cores
// (LedBargraphCore, NeedleVuCore, NumericPeakCore) all match this
// shape.

import type { DrawSink } from "./led-bargraph-core.ts";

/** Object compatible with a meter core's headless API. */
export interface MeterCoreLike {
  update(dbfs: number, dt: number): unknown;
  draw(sink: DrawSink, x: number, y: number, w: number, h: number): void;
  reset?(): void;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Split an outer rect horizontally into two equally-sized child rects
 * separated by `gap` pixels. Mirrors Rust's `split_horizontal`.
 */
export function splitHorizontal(outer: Rect, gap: number): [Rect, Rect] {
  const half = Math.max(1, Math.floor((outer.w - gap) / 2));
  const left: Rect = { x: outer.x, y: outer.y, w: half, h: outer.h };
  const right: Rect = {
    x: outer.x + half + gap,
    y: outer.y,
    w: outer.w - half - gap,
    h: outer.h,
  };
  return [left, right];
}

/** Generic stereo container. Instantiate with two pre-built cores. */
export class StereoPair<C extends MeterCoreLike> {
  readonly bounds: Rect;
  readonly gap: number;
  readonly left: C;
  readonly right: C;
  readonly leftBounds: Rect;
  readonly rightBounds: Rect;

  constructor(bounds: Rect, gap: number, left: C, right: C) {
    this.bounds = bounds;
    this.gap = gap;
    this.left = left;
    this.right = right;
    [this.leftBounds, this.rightBounds] = splitHorizontal(bounds, gap);
  }

  updateStereo(leftDbfs: number, rightDbfs: number, dt: number): void {
    this.left.update(leftDbfs, dt);
    this.right.update(rightDbfs, dt);
  }

  reset(): void {
    this.left.reset?.();
    this.right.reset?.();
  }

  /** Paint both children at their respective sub-bounds. */
  draw(sink: DrawSink): void {
    this.left.draw(
      sink,
      this.leftBounds.x,
      this.leftBounds.y,
      this.leftBounds.w,
      this.leftBounds.h,
    );
    this.right.draw(
      sink,
      this.rightBounds.x,
      this.rightBounds.y,
      this.rightBounds.w,
      this.rightBounds.h,
    );
  }
}
