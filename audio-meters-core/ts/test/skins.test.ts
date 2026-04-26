// Cross-runtime validation of canonical skins under
// assets/audio-meters/skins/. Mirror of audio-meters-core/tests/skins.rs.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";

import { ALL_BALLISTICS } from "../src/ballistic.ts";

interface ScaleStub {
  id: string;
  compatible_ballistics: string[];
}

interface Palette {
  Safe: string;
  Nominal: string;
  Caution: string;
  Hot: string;
  Over: string;
}

interface Layout {
  orientation: string;
  aspect_ratio: number;
  led_count?: number;
  peak_hold_ms?: number;
}

interface Skin {
  $schema?: string;
  id: string;
  title: string;
  scale_id: string;
  default_ballistic: string;
  calibration_override?: { to: string; offset_db: number };
  meter_type: string;
  palette: Palette;
  secondary_colors?: Record<string, string>;
  layout: Layout;
  assets?: Record<string, string>;
}

const ALLOWED_BALLISTICS_SET = new Set<string>(
  ALL_BALLISTICS as readonly string[],
);
const ALLOWED_METER_TYPES = new Set([
  "bargraph",
  "needle",
  "numeric",
  "lufs_gauge",
]);
const ALLOWED_ORIENTATIONS = new Set(["horizontal", "vertical"]);
const ALLOWED_PALETTE_KEYS = new Set([
  "Safe",
  "Nominal",
  "Caution",
  "Hot",
  "Over",
]);
const ALLOWED_SECONDARY = new Set([
  "background",
  "frame",
  "scale_text",
  "minor_tick",
  "major_tick",
  "needle",
  "needle_pivot",
  "led_off",
  "peak_hold",
]);
const ALLOWED_TOPLEVEL = new Set([
  "$schema",
  "id",
  "title",
  "scale_id",
  "default_ballistic",
  "calibration_override",
  "meter_type",
  "palette",
  "secondary_colors",
  "layout",
  "assets",
]);

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ASSETS = join(__dirname, "..", "..", "..", "assets", "audio-meters");

function isHexColor(s: string): boolean {
  if (typeof s !== "string" || !s.startsWith("#")) return false;
  const hex = s.slice(1);
  return (
    (hex.length === 6 || hex.length === 8) &&
    /^[0-9a-fA-F]+$/.test(hex)
  );
}

function loadScales(): Map<string, ScaleStub> {
  const dir = join(ASSETS, "scales");
  const files = readdirSync(dir).filter((n) => n.endsWith(".json"));
  const map = new Map<string, ScaleStub>();
  for (const f of files) {
    const obj = JSON.parse(readFileSync(join(dir, f), "utf8")) as ScaleStub;
    map.set(obj.id, obj);
  }
  return map;
}

function validate(
  skin: Skin,
  fileStem: string,
  scales: Map<string, ScaleStub>,
): void {
  for (const k of Object.keys(skin)) {
    assert.ok(
      ALLOWED_TOPLEVEL.has(k),
      `${fileStem}: unknown top-level key \`${k}\``,
    );
  }
  assert.equal(skin.id, fileStem, `${fileStem}: id MUST match filename stem`);
  assert.ok(skin.title.length > 0, `${skin.id}: title required`);

  const scale = scales.get(skin.scale_id);
  assert.ok(
    scale !== undefined,
    `${skin.id}: scale_id \`${skin.scale_id}\` does not match any file under scales/`,
  );

  assert.ok(
    ALLOWED_BALLISTICS_SET.has(skin.default_ballistic),
    `${skin.id}: default_ballistic \`${skin.default_ballistic}\` not in §5 enum`,
  );
  if (!scale!.compatible_ballistics.includes(skin.default_ballistic)) {
    process.stderr.write(
      `[skins] note: ${skin.id} pairs ballistic \`${skin.default_ballistic}\` with scale \`${scale!.id}\` (advisory crossing)\n`,
    );
  }

  assert.ok(
    ALLOWED_METER_TYPES.has(skin.meter_type),
    `${skin.id}: meter_type \`${skin.meter_type}\` not in enum`,
  );

  // Palette: every required key present, hex format.
  for (const k of Object.keys(skin.palette)) {
    assert.ok(
      ALLOWED_PALETTE_KEYS.has(k),
      `${skin.id} palette[${k}]: not in §7 enum`,
    );
  }
  for (const k of ALLOWED_PALETTE_KEYS) {
    const v = skin.palette[k as keyof Palette];
    assert.ok(typeof v === "string", `${skin.id} palette[${k}]: missing`);
    assert.ok(
      isHexColor(v),
      `${skin.id} palette[${k}]: \`${v}\` not a valid hex colour`,
    );
  }

  if (skin.secondary_colors) {
    for (const [k, v] of Object.entries(skin.secondary_colors)) {
      assert.ok(
        ALLOWED_SECONDARY.has(k),
        `${skin.id}: secondary_colors key \`${k}\` not in schema`,
      );
      assert.ok(
        isHexColor(v),
        `${skin.id} secondary[${k}]: \`${v}\` not a valid hex colour`,
      );
    }
  }

  assert.ok(
    ALLOWED_ORIENTATIONS.has(skin.layout.orientation),
    `${skin.id}: layout.orientation \`${skin.layout.orientation}\` not in enum`,
  );
  assert.ok(
    skin.layout.aspect_ratio > 0 && skin.layout.aspect_ratio <= 100,
    `${skin.id}: aspect_ratio out of range (${skin.layout.aspect_ratio})`,
  );

  if (skin.meter_type === "bargraph") {
    assert.ok(
      typeof skin.layout.led_count === "number",
      `${skin.id}: bargraph must declare led_count`,
    );
    assert.ok(
      skin.layout.led_count! >= 4 && skin.layout.led_count! <= 256,
      `${skin.id}: led_count ${skin.layout.led_count} out of range`,
    );
  }

  if (typeof skin.layout.peak_hold_ms === "number") {
    assert.ok(
      skin.layout.peak_hold_ms >= 0 && skin.layout.peak_hold_ms <= 60_000,
      `${skin.id}: peak_hold_ms ${skin.layout.peak_hold_ms} out of range`,
    );
  }
}

const scales = loadScales();
const skinFiles = readdirSync(join(ASSETS, "skins"))
  .filter((n) => n.endsWith(".json"))
  .sort();

assert.ok(
  skinFiles.length >= 3,
  `expected ≥ 3 canonical skins, found ${skinFiles.length}`,
);

for (const file of skinFiles) {
  test(`skin loads + validates: ${file}`, () => {
    const skin = JSON.parse(
      readFileSync(join(ASSETS, "skins", file), "utf8"),
    ) as Skin;
    validate(skin, basename(file, extname(file)), scales);
  });
}
