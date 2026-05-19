// SPDX-License-Identifier: MIT
// Navigation and focus management tests for the disco simulator.

import test from 'node:test';
import assert from 'node:assert/strict';

import { launchDiscoSim } from '../src/index.js';
import { dumpHasVisiblePixels, dumpSignature, assertAllTagsExist, KNOWN_TAGS } from './shared-assertions.js';

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

    // 2026-05-19: the disco-sim runtime has no opaque window-background
    // fill and the dashboard panel starts hidden (`refactor: dashboard
    // starts hidden` 504d56b, 2026-04-13), so the original (100, 100)
    // sample region is unrendered (zero pixels) regardless of which
    // hotkey fires. Retarget to the IconStrip slot 1 (Files) area on
    // the right edge: 'f' moves the focus highlight onto slot 1 (border
    // appears), and 'b' clears the IconStrip highlight (border
    // disappears). Both transitions touch slot 1's bounding region.
    // Playit's dump command caps width/height at 40. See
    // docs/concepts/DPR-01-A-disco-sim-triage.md (Pattern 1).
    // Slot 1 sits at y=87..147 (margin_top=17 + index*70). The focus
    // border is drawn 2 px inside the slot's top edge (y=87..89), so a
    // sample spanning y=80..99 catches the border-on/off transition.
    const stripSlot1 = { x: 740, y: 80, width: 40, height: 20, frames: 1 };

    // 'f' activates files — IconStrip focus highlight gains slot 1 border
    const beforeF = await session.dumpRect(stripSlot1);
    await session.keyDown('f');
    const afterF = await session.dumpRect(stripSlot1);
    assert.notEqual(dumpSignature(afterF), dumpSignature(beforeF));

    // 'b' opens settings wing on Backlight slot — IconStrip strip_slot
    // is cleared (focus moves into the wing) so slot 1 loses its border.
    const beforeB = await session.dumpRect(stripSlot1);
    await session.keyDown('b');
    const afterB = await session.dumpRect(stripSlot1);
    assert.notEqual(dumpSignature(afterB), dumpSignature(beforeB));
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
    // 2026-05-19: the disco-sim runtime has no opaque window-background
    // fill and the dashboard panel starts hidden (`refactor: dashboard
    // starts hidden` 504d56b, 2026-04-13). The original (100, 100)
    // sample region is unrendered (zero pixels) in all three panel
    // states. Sample IconStrip slot 1 (Files) area instead — slot 1
    // gains a focus border as 'f' fires (slot 0 -> slot 1) and loses it
    // as 'i' fires (slot 1 -> slot 2, plus info wing opens). Playit's
    // dump command caps width/height at 40. See
    // docs/concepts/DPR-01-A-disco-sim-triage.md (Pattern 1).
    // Slot 1 sits at y=87..147; the focus border occupies y=87..89, so
    // a sample spanning y=80..99 catches the border-on/off transition.
    const dumpArgs = { x: 740, y: 80, width: 40, height: 20, frames: 1 };

    // Settings panel (initial — IconStrip slot 0 focused, slot 1 idle)
    const sigSettings = dumpSignature(await session.dumpRect(dumpArgs));

    // Files panel (IconStrip slot 1 focused — gains border)
    await session.keyDown('f');
    const sigFiles = dumpSignature(await session.dumpRect(dumpArgs));

    // Info panel (info wing open + IconStrip slot 2 focused — slot 1
    // loses border)
    await session.keyDown('i');
    const sigInfo = dumpSignature(await session.dumpRect(dumpArgs));

    // All three should be distinct
    assert.notEqual(sigSettings, sigFiles, 'settings and files panels should differ');
    assert.notEqual(sigFiles, sigInfo, 'files and info panels should differ');
  } finally {
    await session.close();
  }
});
