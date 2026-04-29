// Smoke test: every bargraph skin under assets/audio-meters/skins/
// successfully wraps an LedBargraphCore against its bound scale, and
// renders without errors at a representative size.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { LedBargraphCore } from "../src/led-bargraph-core.ts";
import type { Scale, Skin } from "../src/skin.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

const skinFiles = readdirSync(join(ASSETS, "skins"))
  .filter((n) => n.endsWith(".json"))
  .sort();

for (const file of skinFiles) {
  const skin = JSON.parse(
    readFileSync(join(ASSETS, "skins", file), "utf8"),
  ) as Skin;

  if (skin.meter_type !== "bargraph") continue;

  test(`bargraph skin renders: ${skin.id}`, () => {
    const scale = JSON.parse(
      readFileSync(join(ASSETS, "scales", `${skin.scale_id}.json`), "utf8"),
    ) as Scale;
    const core = new LedBargraphCore({ scale, skin });
    let opCount = 0;
    core.draw(
      {
        fillRect() {
          opCount++;
        },
      },
      0,
      0,
      48,
      256,
    );
    // Background + led_count segments. The peak pip is suppressed
    // when peak is at floor (silent meter).
    const expected = 1 + (skin.layout.led_count ?? 0);
    assert.equal(
      opCount,
      expected,
      `${skin.id}: got ${opCount} draw ops, expected ${expected}`,
    );
  });
}
