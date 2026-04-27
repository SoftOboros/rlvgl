// Headless tests for LedBargraphCore. Asserts the same draw-call
// shape as the Rust-side widget tests in widgets/src/meters/bargraph.rs:
// 1 background fill + N segment fills, peak pip when in range.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore, type DrawSink } from "../src/led-bargraph-core.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

interface Op {
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
}

function recorder(): DrawSink & { ops: Op[] } {
  const ops: Op[] = [];
  return {
    ops,
    fillRect(x, y, w, h, color) {
      ops.push({ x, y, w, h, color });
    },
  };
}

function loadJson<T>(rel: string): T {
  return JSON.parse(readFileSync(join(ASSETS, rel), "utf8")) as T;
}

const SCALE = loadJson<Scale>("scales/vu_broadcast.json");
const SKIN = loadJson<Skin>("skins/broadcast_classic_bargraph.json");
const N = SKIN.layout.led_count!;

test("draws 1 background + N segments when silent", () => {
  const core = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  const sink = recorder();
  core.draw(sink, 0, 0, 32, 256);
  // Silent meter: peak is at floor so the peak pip is suppressed.
  assert.equal(sink.ops.length, 1 + N);
  // First op is the background covering the whole rect.
  assert.equal(sink.ops[0].x, 0);
  assert.equal(sink.ops[0].y, 0);
  assert.equal(sink.ops[0].w, 32);
  assert.equal(sink.ops[0].h, 256);
});

test("rejects skins with non-bargraph meter_type", () => {
  const needleSkin: Skin = loadJson("skins/broadcast_classic_needle.json");
  assert.throws(
    () => new LedBargraphCore({ scale: SCALE, skin: needleSkin }),
    /meter_type 'needle'/,
  );
});

test("lit segment count grows with sustained signal", () => {
  const core = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  // Drive sustained -20 dBFS (= 0 VU on broadcast scale) for ~2 s
  // — VU should settle close to 0 VU.
  for (let i = 0; i < 120; i++) {
    core.update(-20.0, 1 / 60);
  }
  const sink = recorder();
  core.draw(sink, 0, 0, 32, 256);
  // Bg + N segments. peak == reading on a steady tone, so peak pip
  // overlays a lit segment but does not change the count.
  // Count how many segments use a *non-led-off* colour.
  const ledOff = SKIN.secondary_colors!.led_off!;
  const litOps = sink.ops.slice(1).filter((o) => o.color !== ledOff);
  assert.ok(
    litOps.length >= Math.floor(N * 0.6),
    `expected ≥ 60% lit at 0 VU sustained, got ${litOps.length}/${N}`,
  );
});

test("peak pip persists after signal drops", () => {
  const core = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  // Inject one frame at 0 dBFS (well past the top of the scale),
  // then go silent.
  core.update(0, 1 / 60);
  for (let i = 0; i < 6; i++) {
    core.update(-120, 1 / 60); // hold time ≥ 1 frame, well within 1.2 s
  }
  // Reading has decayed; peak should still be high.
  const reading = core.readingDbValue();
  const peak = core.peakDbValue();
  assert.ok(
    peak > reading + 5,
    `peak (${peak}) should remain well above reading (${reading}) during dwell`,
  );
});

test("setBallistic resets ballistic + peak state", () => {
  const core = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  core.update(-1, 1 / 60);
  const before = core.readingDbValue();
  core.setBallistic("DigitalPeak");
  assert.ok(
    core.readingDbValue() < before,
    `after setBallistic, reading should drop back to floor`,
  );
});
