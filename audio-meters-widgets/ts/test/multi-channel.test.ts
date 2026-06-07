// Tests for MultiChannel<C>. Mirror of the Rust-side
// widgets/src/meters/multi_channel.rs tests.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore } from "../src/led-bargraph-core.ts";
import {
  MultiChannel,
  splitHorizontalN,
  splitVerticalN,
} from "../src/multi-channel.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const VU = JSON.parse(
  readFileSync(join(ASSETS, "scales/vu_broadcast.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/broadcast_classic_bargraph.json"), "utf8"),
) as Skin;
const DIGITAL = JSON.parse(
  readFileSync(join(ASSETS, "scales/digital_peak.json"), "utf8"),
) as Scale;
const DIGITAL_SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/digital_studio_bargraph.json"), "utf8"),
) as Skin;

test("splitHorizontalN partitions exactly", () => {
  const parts = splitHorizontalN({ x: 0, y: 0, w: 100, h: 50 }, 4, 4);
  assert.equal(parts.length, 4);
  assert.equal(parts[0].x, 0);
  const last = parts[3];
  assert.equal(last.x + last.w, 100);
  for (const r of parts) assert.ok(r.w > 0);
});

test("splitVerticalN partitions exactly", () => {
  const parts = splitVerticalN({ x: 10, y: 20, w: 200, h: 600 }, 2, 6);
  assert.equal(parts.length, 6);
  assert.equal(parts[0].y, 20);
  const last = parts[5];
  assert.equal(last.y + last.h, 620);
  for (const r of parts) assert.ok(r.h > 0);
});

test("rejects empty channel array", () => {
  assert.throws(
    () => new MultiChannel({ x: 0, y: 0, w: 10, h: 10 }, 0, []),
    /at least one channel/,
  );
});

test("5.1 surround forwards distinct levels per channel", () => {
  const outer = { x: 0, y: 0, w: 480, h: 320 };
  const surround = MultiChannel.fromHorizontalFactory(
    outer,
    4,
    6,
    (_idx, b) =>
      new LedBargraphCore({
        scale: VU,
        skin: SKIN,
      }),
  );
  assert.equal(surround.channelCount(), 6);
  const inputs = [-30, -28, -10, -45, -34, -34];
  for (let i = 0; i < 120; i++) surround.updateN(inputs, 1 / 60);
  // Centre (idx 2) loud; Ls (idx 4) quiet — centre should peg the meter.
  assert.ok(
    surround.channel(2).readingDbValue() > surround.channel(4).readingDbValue() + 5,
    `centre should exceed Ls by ≥ 5 dB; got centre=${surround.channel(2).readingDbValue()}, Ls=${surround.channel(4).readingDbValue()}`,
  );
});

test("graphic EQ 8 bands respects per-band sparse updates", () => {
  const outer = { x: 0, y: 0, w: 320, h: 200 };
  const eq = MultiChannel.fromHorizontalFactory(
    outer,
    2,
    8,
    (_idx, _b) =>
      new LedBargraphCore({
        scale: DIGITAL,
        skin: DIGITAL_SKIN,
      }),
  );
  for (let f = 0; f < 120; f++) {
    for (let band = 0; band < 8; band++) {
      const dbfs = band % 2 === 0 ? -10 : -40;
      eq.updateAt(band, dbfs, 1 / 60);
    }
  }
  for (let band = 0; band < 8; band++) {
    const r = eq.channel(band).readingDbValue();
    if (band % 2 === 0) {
      assert.ok(r > -20, `even band ${band} should be loud, got ${r}`);
    } else {
      assert.ok(r < -20, `odd band ${band} should be quiet, got ${r}`);
    }
  }
});

test("draw paints all child sub-bounds", () => {
  const outer = { x: 0, y: 0, w: 96, h: 200 };
  const mc = MultiChannel.fromHorizontalFactory(
    outer,
    2,
    3,
    (_idx, _b) =>
      new LedBargraphCore({
        scale: VU,
        skin: SKIN,
      }),
  );
  let count = 0;
  mc.draw({
    fillRect() {
      count++;
    },
  });
  // Each LedBargraphCore: 1 background + N segments, no peak pip
  // when silent. 3 children → 3 * (1 + N).
  const perChild = 1 + (SKIN.layout.led_count ?? 0);
  assert.equal(count, 3 * perChild);
});

test("reset floors all channels", () => {
  const outer = { x: 0, y: 0, w: 96, h: 200 };
  const mc = MultiChannel.fromHorizontalFactory(
    outer,
    2,
    3,
    (_idx, _b) =>
      new LedBargraphCore({
        scale: VU,
        skin: SKIN,
      }),
  );
  mc.updateN([-5, -5, -5], 1 / 60);
  const before = mc.channel(0).readingDbValue();
  mc.reset();
  const after = mc.channel(0).readingDbValue();
  assert.ok(after < before, "reset should drop reading toward floor");
});
