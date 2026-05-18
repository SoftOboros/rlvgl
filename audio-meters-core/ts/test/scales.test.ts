// Cross-runtime validation of the canonical scale set under
// assets/audio-meters/scales/. Mirror of audio-meters-core/tests/scales.rs;
// any check added here MUST also exist there (and vice versa) so the two
// runtimes agree on what a "valid scale" is.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";

import { ALL_BALLISTICS } from "../src/ballistic.ts";

interface RangeDb {
  min: number;
  max: number;
}

interface Pivot {
  value: number;
  label: string;
  input_dbfs: number;
}

interface CalibrationDefault {
  to: string;
  offset_db: number;
}

interface Ticks {
  majors: number[];
  minors_per_major_division: number;
  labels?: Record<string, string>;
}

interface Zone {
  from_db: number;
  to_db: number;
  color: string;
}

interface Scale {
  $schema?: string;
  id: string;
  label_units: string;
  range_db: RangeDb;
  pivot: Pivot;
  calibration_default?: CalibrationDefault;
  ticks: Ticks;
  zones: Zone[];
  compatible_ballistics: string[];
}

const ALLOWED_COLORS = new Set(["Safe", "Nominal", "Caution", "Hot", "Over"]);
const ALLOWED_BALLISTICS = new Set<string>(ALL_BALLISTICS as readonly string[]);
const ALLOWED_TOPLEVEL = new Set([
  "$schema",
  "id",
  "label_units",
  "range_db",
  "pivot",
  "calibration_default",
  "ticks",
  "zones",
  "compatible_ballistics",
]);

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const SCALES_DIR = join(
  __dirname,
  "..",
  "..",
  "..",
  "assets",
  "audio-meters",
  "scales",
);

function validate(scale: Scale, fileStem: string): void {
  // No unknown top-level keys.
  for (const key of Object.keys(scale)) {
    assert.ok(
      ALLOWED_TOPLEVEL.has(key),
      `${fileStem}: unknown top-level key \`${key}\``,
    );
  }

  assert.equal(
    scale.id,
    fileStem,
    `scale id \`${scale.id}\` MUST match filename stem \`${fileStem}\``,
  );

  assert.ok(
    scale.range_db.min < scale.range_db.max,
    `${scale.id}: range_db.min (${scale.range_db.min}) < range_db.max (${scale.range_db.max})`,
  );

  assert.ok(
    Number.isFinite(scale.pivot.input_dbfs),
    `${scale.id}: pivot.input_dbfs must be finite`,
  );
  assert.ok(
    Number.isFinite(scale.pivot.value),
    `${scale.id}: pivot.value must be finite`,
  );
  assert.ok(
    scale.pivot.value >= scale.range_db.min - 1e-3 &&
      scale.pivot.value <= scale.range_db.max + 1e-3,
    `${scale.id}: pivot.value (${scale.pivot.value}) must lie within range_db [${scale.range_db.min}, ${scale.range_db.max}]`,
  );

  // Majors strictly ascending, endpoints match range_db.
  assert.ok(scale.ticks.majors.length >= 2, `${scale.id}: need ≥ 2 majors`);
  for (let i = 1; i < scale.ticks.majors.length; i++) {
    assert.ok(
      scale.ticks.majors[i - 1] < scale.ticks.majors[i],
      `${scale.id}: ticks.majors must be strictly ascending`,
    );
  }
  const first = scale.ticks.majors[0];
  const last = scale.ticks.majors[scale.ticks.majors.length - 1];
  assert.ok(
    Math.abs(first - scale.range_db.min) < 1e-3,
    `${scale.id}: first major (${first}) should equal range_db.min (${scale.range_db.min})`,
  );
  assert.ok(
    Math.abs(last - scale.range_db.max) < 1e-3,
    `${scale.id}: last major (${last}) should equal range_db.max (${scale.range_db.max})`,
  );

  // Zones partition range without gap or overlap.
  assert.ok(scale.zones.length >= 1, `${scale.id}: at least one zone`);
  let prevTo = scale.range_db.min;
  scale.zones.forEach((z, i) => {
    assert.ok(
      ALLOWED_COLORS.has(z.color),
      `${scale.id} zone[${i}]: color \`${z.color}\` not in §7 enum`,
    );
    assert.ok(
      Math.abs(z.from_db - prevTo) < 1e-3,
      `${scale.id} zone[${i}]: from_db (${z.from_db}) must abut previous to_db (${prevTo})`,
    );
    assert.ok(
      z.to_db > z.from_db,
      `${scale.id} zone[${i}]: to_db (${z.to_db}) must exceed from_db (${z.from_db})`,
    );
    prevTo = z.to_db;
  });
  assert.ok(
    Math.abs(prevTo - scale.range_db.max) < 1e-3,
    `${scale.id}: last zone to_db (${prevTo}) must equal range_db.max (${scale.range_db.max})`,
  );

  // Ballistic identifiers known + unique.
  assert.ok(
    scale.compatible_ballistics.length >= 1,
    `${scale.id}: need ≥ 1 compatible_ballistics`,
  );
  const seen = new Set<string>();
  for (const b of scale.compatible_ballistics) {
    assert.ok(
      ALLOWED_BALLISTICS.has(b),
      `${scale.id}: ballistic \`${b}\` not in §5 enum`,
    );
    assert.ok(!seen.has(b), `${scale.id}: ballistic \`${b}\` duplicated`);
    seen.add(b);
  }

  // Labels reference declared majors.
  if (scale.ticks.labels) {
    for (const key of Object.keys(scale.ticks.labels)) {
      const parsed = Number(key);
      assert.ok(
        Number.isFinite(parsed),
        `${scale.id}: tick label key \`${key}\` not a number`,
      );
      const known = scale.ticks.majors.some((m) => Math.abs(m - parsed) < 1e-3);
      assert.ok(
        known,
        `${scale.id}: tick label key \`${key}\` not in majors`,
      );
    }
  }
}

const files = readdirSync(SCALES_DIR)
  .filter((n) => n.endsWith(".json"))
  .sort();

assert.ok(
  files.length >= 6,
  `expected ≥ 6 canonical scales under ${SCALES_DIR}, found ${files.length}`,
);

for (const file of files) {
  test(`scale loads + validates: ${file}`, () => {
    const text = readFileSync(join(SCALES_DIR, file), "utf8");
    const scale = JSON.parse(text) as Scale;
    const stem = basename(file, extname(file));
    validate(scale, stem);
  });
}
