// Headless tests for LufsGaugeStrictCore. Mirror of the Rust-side
// widgets/src/meters/lufs_gauge_strict.rs tests.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LufsGaugeCore } from "../src/lufs-gauge-core.ts";
import { LufsGaugeStrictCore } from "../src/lufs-gauge-strict-core.ts";
import { dbfsToScaleUnits, type Scale, type Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const SCALE = JSON.parse(
  readFileSync(join(ASSETS, "scales/lufs_ebu_r128.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/lufs_ebu_r128_gauge.json"), "utf8"),
) as Skin;

test("rejects skins with non-lufs-gauge meter_type", () => {
  const bargraphSkin = JSON.parse(
    readFileSync(
      join(ASSETS, "skins/broadcast_classic_bargraph.json"),
      "utf8",
    ),
  ) as Skin;
  assert.throws(
    () =>
      new LufsGaugeStrictCore({
        scale: SCALE,
        skin: bargraphSkin,
        windowSize: 128,
      }),
    /meter_type 'bargraph'/,
  );
});

test("at-target reading colours as Nominal", () => {
  const g = new LufsGaugeStrictCore({
    scale: SCALE,
    skin: SKIN,
    windowSize: 128,
  });
  for (let i = 0; i < 1000; i++) g.update(-23, 1 / 60);
  const lu = Math.abs(
    dbfsToScaleUnits(SCALE, g.integratedDbValue()) - SCALE.pivot.value,
  );
  assert.ok(
    lu < 0.5,
    `strict gauge at target should be within 0.5 LU, got ${lu} LU`,
  );
});

test("strict gauge lifts above streaming when quiet passages present", () => {
  const strict = new LufsGaugeStrictCore({
    scale: SCALE,
    skin: SKIN,
    windowSize: 256,
  });
  const streaming = new LufsGaugeCore({ scale: SCALE, skin: SKIN });
  for (let f = 0; f < 256; f++) {
    const dbfs = f % 5 === 0 ? -45 : -23;
    strict.update(dbfs, 1 / 60);
    streaming.update(dbfs, 1 / 60);
  }
  assert.ok(
    strict.integratedDbValue() > streaming.integratedDbValue(),
    `strict (${strict.integratedDbValue()}) should exceed streaming (${streaming.integratedDbValue()})`,
  );
  assert.ok(
    Math.abs(strict.integratedDbValue() - -23) < 0.2,
    `strict should track loud passage near -23, got ${strict.integratedDbValue()}`,
  );
});

test("draws 1 background + 3 text lines", () => {
  const g = new LufsGaugeStrictCore({
    scale: SCALE,
    skin: SKIN,
    windowSize: 128,
  });
  let rects = 0;
  let texts = 0;
  g.draw(
    {
      fillRect() {
        rects++;
      },
      drawText() {
        texts++;
      },
    },
    0,
    0,
    240,
    120,
  );
  assert.equal(rects, 1);
  assert.equal(texts, 3);
});
