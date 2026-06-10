// SPDX-License-Identifier: MIT
// CRATES-CI-04 — node automation test for the Layer W playit server in
// rlvgl-creator (CRATES-CI-00 §7).
//
// Drives `rlvgl-creator --automation-headless` through playit/node
// UNMODIFIED (INV-C7): the same client that drives the in-tree disco-sim
// and the user-sim Consumer Project must launch and drive the GUI Wrapper.
// The binary path is injected via RLVGL_CREATOR_BIN (no silent fallback to
// an in-tree cargo run, which would defeat the gate).
//
// Widget addressing: playit tags ARE the egui/accesskit labels (§7.4) —
// the tags below are the literal menu strings from
// src/bin/creator_ui/menus.rs (the same labels Layer K asserts).
//
// The `D` dump verb is exercised once, guarded: a CI container without a
// usable wgpu adapter degrades to `ERR: render-unavailable` (documented in
// CRATES-CI-00 §7 / automation.rs); anything else is a failure. The
// accesskit verbs must work regardless (GPU-independent path).

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { launchDiscoSim } from '../../../playit/node/src/index.js';

const SCREEN = { width: 800, height: 480 };

// Same minimal manifest `rlvgl-creator init` produces (and that
// tests/creator_ui_kittest.rs writes for the Layer K harness).
const DEFAULT_MANIFEST = `# rlvgl-creator manifest v1
version: 1
packages: {}
groups: {}
features: {}
expose: {}
targets: []
assets: []
naming:
  prefixes: {}
  case: screaming_snake
`;

function binaryPath() {
  const bin = process.env.RLVGL_CREATOR_BIN;
  assert.ok(
    bin,
    'RLVGL_CREATOR_BIN must point at an rlvgl-creator built with ' +
      '--features creator,creator_ui,creator_ui_automation'
  );
  return bin;
}

// Automation mode needs a manifest and never opens a dialog (§7.5):
// stage a tempdir with a default manifest.yml and spawn with cwd there.
function makeWorkDir() {
  const dir = mkdtempSync(path.join(tmpdir(), 'rlvgl-creator-ui-test-'));
  writeFileSync(path.join(dir, 'manifest.yml'), DEFAULT_MANIFEST);
  // The asset watcher dirs `init` would create; harmless if unused.
  for (const sub of ['icons', 'fonts', 'media']) {
    mkdirSync(path.join(dir, sub), { recursive: true });
  }
  return dir;
}

async function launch() {
  const cwd = makeWorkDir();
  const session = await launchDiscoSim({
    binaryPath: binaryPath(),
    automationHeadless: true,
    screen: SCREEN,
    cwd
  });
  return { session, cwd };
}

test('creator UI automation: status advances and the menu tree is addressable', async () => {
  const { session, cwd } = await launch();
  try {
    // (a) STAT counters advance between `?` polls (node tick() pattern).
    const before = await session.status();
    const after = await session.tick();
    assert.ok(after.tickCount > before.tickCount, 'tickCount must advance');
    assert.ok(after.presentCount > before.presentCount, 'presentCount must advance');

    // (b) QE on real menu labels (src/bin/creator_ui/menus.rs — the same
    // chrome Layer K asserts) — tags ARE labels (§7.4).
    for (const label of ['Build', 'Assets', 'Deploy', 'Emulator', 'Qt']) {
      assert.equal(
        await session.widget(label).exists(),
        true,
        `missing menu label: ${label}`
      );
    }
    assert.equal(await session.widget('No Such Widget').exists(), false);

    // (c) QB returns sane on-screen bounds for the Build menu button.
    const bounds = await session.widget('Build').bounds();
    assert.ok(bounds, 'Build menu bounds must resolve');
    assert.ok(bounds.width > 0 && bounds.height > 0, 'Build bounds must be non-empty');
    assert.ok(
      bounds.x >= 0 &&
        bounds.y >= 0 &&
        bounds.x + bounds.width <= SCREEN.width &&
        bounds.y + bounds.height <= SCREEN.height,
      `Build bounds out of screen: ${JSON.stringify(bounds)}`
    );
  } finally {
    await session.close();
    rmSync(cwd, { recursive: true, force: true });
  }
});

test('creator UI automation: T@ opens the Build menu and reveals Fonts Pack', async () => {
  const { session, cwd } = await launch();
  try {
    // The menu entry is not in the tree until the group is opened —
    // same flow as Layer K's fonts_pack_dialog_opens_from_build_menu.
    assert.equal(await session.widget('Fonts Pack').exists(), false);

    await session.widget('Build').tap(); // QB -> center -> T@Build:<x>,<y>

    assert.equal(
      await session.widget('Fonts Pack').exists(),
      true,
      'Fonts Pack menu entry should appear after tapping Build'
    );
  } finally {
    await session.close();
    rmSync(cwd, { recursive: true, force: true });
  }
});

test('creator UI automation: D dump either renders or degrades cleanly', async () => {
  const { session, cwd } = await launch();
  try {
    // GPU-independent CI path: accept a well-formed dump OR the documented
    // render-unavailable degradation; anything else fails.
    let dump = null;
    try {
      dump = await session.dumpRect({ x: 0, y: 0, width: 6, height: 4, frames: 1 });
    } catch (error) {
      assert.ok(error instanceof Error, 'dump failure must be an Error');
      assert.equal(
        error.message,
        'render-unavailable',
        `unexpected dump failure: ${error.message}`
      );
    }
    if (dump !== null) {
      assert.equal(dump.frames.length, 1);
      assert.equal(dump.frames[0].length, 4);
      assert.ok(
        dump.frames[0].every((row) => row.length === 6),
        'each dump row must carry the requested width'
      );
    }

    // Either way, the accesskit verbs must still be serving.
    assert.equal(await session.widget('Build').exists(), true);
  } finally {
    await session.close();
    rmSync(cwd, { recursive: true, force: true });
  }
});
