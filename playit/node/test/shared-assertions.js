// SPDX-License-Identifier: MIT
// Shared test helpers for disco demo playit automation tests.

import assert from 'node:assert/strict';

export const KNOWN_TAGS = [
  'disco.root',
  'disco.dashboard',
  'disco.subtitle',
  'disco.footer',
  'disco.events',
  'disco.main.settings',
  'disco.main.files',
  'disco.main.info',
  'disco.settings.audio',
  'disco.settings.camera',
  'disco.settings.display',
  'disco.settings.locale',
  'disco.settings.backlight',
  'disco.info.diagnostics',
  'disco.info.live_stats',
  'disco.info.star_crawl',
  'disco.info.audio_scope',
];

export function dumpHasVisiblePixels(dump) {
  return dump.frames.some((frame) =>
    frame.some((row) => row.some((pixel) => pixel !== 0))
  );
}

export function dumpSignature(dump) {
  return JSON.stringify(dump.frames);
}

export async function assertAllTagsExist(session, tags = KNOWN_TAGS) {
  for (const tag of tags) {
    const exists = await session.widget(tag).exists();
    assert.equal(exists, true, `missing tag: ${tag}`);
  }
}
