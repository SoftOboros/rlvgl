// Tests for StereoPair generic composite. Mirror of Rust-side
// widgets/src/meters/stereo.rs tests.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore } from "../src/led-bargraph-core.ts";
import { StereoPair, splitHorizontal } from "../src/stereo.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const SCALE = JSON.parse(
  readFileSync(join(ASSETS, "scales/vu_broadcast.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/broadcast_classic_bargraph.json"), "utf8"),
) as Skin;

test("splitHorizontal divides outer with a gap", () => {
  const [l, r] = splitHorizontal({ x: 0, y: 0, w: 100, h: 50 }, 4);
  assert.equal(l.x, 0);
  assert.equal(l.w, 48);
  assert.equal(r.x, 52);
  assert.equal(r.w, 48);
});

test("StereoPair forwards independent updates per channel", () => {
  const left = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  const right = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  const pair = new StereoPair({ x: 0, y: 0, w: 80, h: 320 }, 4, left, right);
  for (let i = 0; i < 120; i++) {
    pair.updateStereo(-30, -10, 1 / 60);
  }
  // Right is louder, should read higher.
  assert.ok(
    pair.right.readingDbValue() > pair.left.readingDbValue() + 5,
    `right (${pair.right.readingDbValue()}) should exceed left (${pair.left.readingDbValue()})`,
  );
});

test("StereoPair draws both children", () => {
  const left = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  const right = new LedBargraphCore({ scale: SCALE, skin: SKIN });
  const pair = new StereoPair({ x: 0, y: 0, w: 80, h: 320 }, 4, left, right);
  let count = 0;
  pair.draw({
    fillRect() {
      count++;
    },
  });
  // Each child: 1 background + N segments. Two children → 2*(1+N).
  const expected = 2 * (1 + (SKIN.layout.led_count ?? 0));
  assert.equal(count, expected);
});
