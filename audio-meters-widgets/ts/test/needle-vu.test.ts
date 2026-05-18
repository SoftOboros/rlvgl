// Headless tests for NeedleVuCore. Mirror of the Rust-side needle
// tests in widgets/src/meters/needle.rs.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { NEEDLE_HALF_ARC_RAD, NeedleVuCore } from "../src/needle-vu-core.ts";
import { dbfsToScaleUnits, type Scale, type Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const SCALE = JSON.parse(
  readFileSync(join(ASSETS, "scales/vu_broadcast.json"), "utf8"),
) as Scale;
const SKIN = JSON.parse(
  readFileSync(join(ASSETS, "skins/broadcast_classic_needle.json"), "utf8"),
) as Skin;

test("rejects skins with non-needle meter_type", () => {
  const bargraphSkin = JSON.parse(
    readFileSync(
      join(ASSETS, "skins/broadcast_classic_bargraph.json"),
      "utf8",
    ),
  ) as Skin;
  assert.throws(
    () => new NeedleVuCore({ scale: SCALE, skin: bargraphSkin }),
    /meter_type 'bargraph'/,
  );
});

test("angle at floor is at left of arc", () => {
  const core = new NeedleVuCore({ scale: SCALE, skin: SKIN });
  const a = core.needleAngleRad();
  assert.ok(
    a < 0 && a > -NEEDLE_HALF_ARC_RAD - 1e-3,
    `expected angle near -half_arc, got ${a}`,
  );
});

test("angle at top of scale pegs at right of arc", () => {
  const core = new NeedleVuCore({ scale: SCALE, skin: SKIN });
  for (let i = 0; i < 1000; i++) {
    core.update(50.0, 1 / 60);
  }
  const a = core.needleAngleRad();
  assert.ok(
    Math.abs(a - NEEDLE_HALF_ARC_RAD) < 1e-3,
    `expected angle ≈ +half_arc, got ${a}`,
  );
});

test("angle at pivot input matches pivot value mapping", () => {
  const core = new NeedleVuCore({ scale: SCALE, skin: SKIN });
  for (let i = 0; i < 1000; i++) {
    core.update(SCALE.pivot.input_dbfs, 1 / 60);
  }
  const sv = dbfsToScaleUnits(SCALE, core.readingDbValue());
  const frac =
    (sv - SCALE.range_db.min) /
    (SCALE.range_db.max - SCALE.range_db.min);
  const expected = -NEEDLE_HALF_ARC_RAD + frac * 2 * NEEDLE_HALF_ARC_RAD;
  assert.ok(
    Math.abs(core.needleAngleRad() - expected) < 5e-3,
    `needle angle ${core.needleAngleRad()} ≠ pivot expected ${expected}`,
  );
});

test("draw paints background + needle line + pivot (no ticks)", () => {
  const core = new NeedleVuCore({ scale: SCALE, skin: SKIN });
  let opCount = 0;
  let textCount = 0;
  core.draw(
    {
      fillRect() {
        opCount++;
      },
      drawText() {
        textCount++;
      },
    },
    0,
    0,
    320,
    200,
  );
  // 1 background + (length+1) needle steps + 1 pivot dot.
  // length = floor(200 * 0.95) = 190, so 191 needle ops.
  assert.equal(opCount, 1 + 191 + 1);
  assert.equal(textCount, 0, "ticks default off");
});

test("showTicks paints one label per major", () => {
  const core = new NeedleVuCore({ scale: SCALE, skin: SKIN, showTicks: true });
  const labels: string[] = [];
  core.draw(
    {
      fillRect() {},
      drawText(_x, _y, text) {
        labels.push(text);
      },
    },
    0,
    0,
    320,
    200,
  );
  assert.equal(labels.length, SCALE.ticks.majors.length);
  assert.ok(
    labels.includes("0"),
    `expected '0' label among ticks, got ${JSON.stringify(labels)}`,
  );
});
