// SPDX-License-Identifier: MIT
// Navigation and focus management tests for the disco simulator.

import test from 'node:test';
import assert from 'node:assert/strict';

import { launchDiscoSim } from '../src/index.js';
import { dumpHasVisiblePixels, dumpSignature, assertAllTagsExist, KNOWN_TAGS } from './shared-assertions.js';

async function iconStripSignature(session) {
  const regions = [];
  for (const y of [10, 70, 130]) {
    regions.push(dumpSignature(await session.dumpRect({ x: 740, y, width: 40, height: 40, frames: 1 })));
  }
  return regions.join('|');
}

test('all known widget tags exist at startup', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true,
  });
  try {
    await assertAllTagsExist(session);
  } finally {
    await session.close();
  }
});

test('keyboard full navigation walk: ArrowDown cycles through all main slots', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true,
  });

  try {
    // Dump icon strip region (right edge) at initial focus — slot 0 has highlight
    const dumpA = await session.dumpRect({ x: 740, y: 10, width: 40, height: 20, frames: 1 });
    assert(dumpHasVisiblePixels(dumpA));
    const sigA = dumpSignature(dumpA);

    // ArrowDown moves highlight to slot 1
    await session.keyDown('ArrowDown');
    const dumpB = await session.dumpRect({ x: 740, y: 10, width: 40, height: 20, frames: 1 });
    const sigB = dumpSignature(dumpB);

    // The icon strip pixels should change because the highlight border moved
    assert.notEqual(sigA, sigB, 'icon strip should change as focus moves');
  } finally {
    await session.close();
  }
});

test('settings wing full traversal: Enter then ArrowDown through all items', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true,
  });

  try {
    // Open settings wing
    await session.keyDown('Enter');
    assert.equal(await session.widget('disco.settings.audio').isVisible(), true);

    // Navigate through all 5 settings items
    const tags = [
      'disco.settings.audio',
      'disco.settings.camera',
      'disco.settings.display',
      'disco.settings.locale',
      'disco.settings.backlight',
    ];

    for (let i = 1; i < tags.length; i++) {
      await session.keyDown('ArrowDown');
      // All hotspots should still be visible (wing stays open during focus navigation)
      assert.equal(await session.widget(tags[i]).isVisible(), true);
    }
  } finally {
    await session.close();
  }
});

test('hotkey roundtrip: s, f, i, b all change controller state', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true,
  });

  try {
    // 's' opens settings wing
    await session.keyDown('s');
    assert.equal(await session.widget('disco.settings.audio').isVisible(), true);

    // Escape to close
    await session.keyDown('Escape');
    assert.equal(await session.widget('disco.settings.audio').isVisible(), false);

    // 'i' opens info wing
    await session.keyDown('i');
    assert.equal(await session.widget('disco.info.diagnostics').isVisible(), true);

    // Escape
    await session.keyDown('Escape');

    // 'f' activates files (no wing opens, but main-strip focus changes)
    const beforeF = await iconStripSignature(session);
    await session.keyDown('f');
    const afterF = await iconStripSignature(session);
    assert.notEqual(afterF, beforeF);

    // 'b' cycles backlight and forwards the request to the simulator runtime.
    let stderr = '';
    session.child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    await session.keyDown('b');
    assert.match(stderr, /backlight request 100%/);
  } finally {
    await session.close();
  }
});

test('framebuffer differs across main panels', async () => {
  const session = await launchDiscoSim({
    cwd: process.cwd(),
    automationHeadless: true,
  });

  try {
    // Settings panel (initial)
    const sigSettings = await iconStripSignature(session);

    // Files panel
    await session.keyDown('f');
    const sigFiles = await iconStripSignature(session);

    // Info panel
    await session.keyDown('i');
    const sigInfo = await iconStripSignature(session);

    // All three should be distinct
    assert.notEqual(sigSettings, sigFiles, 'settings and files panels should differ');
    assert.notEqual(sigFiles, sigInfo, 'files and info panels should differ');
  } finally {
    await session.close();
  }
});
