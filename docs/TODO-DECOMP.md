<!--
docs/TODO-DECOMP.md - Work plan for rlvgl-decomp (palette + RLE codec)
-->

<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-decomp TODOs

This document tracks the outstanding work for the `rlvgl-decomp` crate: a
compact palette + RLE image format with short/long repeats and inline pixel
escapes. The crate targets no_std with `alloc` and operates on RGBA frames,
converting to/from RGB565 internally to match embedded display pipelines.

## Goals

- Provide a stable, documented compressed image format for rlvgl assets.
- Decode to RGBA quickly on embedded targets with minimal memory.
- Encode RGBA inputs efficiently for creator tooling; support single frames and sequences.
- Remain no_std; support `alloc` only.

## Format (Recap)

- Palette: up to `MAX_PALETTE` RGB565 entries (default 192) derived from frame histogram.
- Stream bytes:
  - `0xFF` (single inline): next 2 bytes RGB565; emit once.
  - `0xFE` (double inline): next 2 bytes RGB565; emit twice.
  - `0xFD` (long repeat): repeat most recent palette index color for `61 + next_byte` pixels (up to 316).
  - `0..(palette_len-1)`: palette index; emit once; sets the recent index.
  - `(palette_len)..(palette_len+60)`: short repeat; emit recent index `(byte - palette_len + 1)` times.

Notes:
- Encoder caps palette so short-repeat codes never collide with `0xFD`..`0xFF`.
- Decoder validates lengths and palette bounds; returns `Error::Truncated`/`SizeMismatch`.

## Work Items

- Decoder polish
  - [ ] Add streaming decode API (row-by-row) to limit peak memory.
  - [ ] Expose RGB565 output option to avoid RGBA expand on embedded.
  - [ ] Validate overflow/edge cases (empty palette, zero-sized images).

- Encoder improvements
  - [ ] Palette selection strategies: median-cut / k-means fallback to improve quality.
  - [ ] Run detection across rows (allow runs to continue over scanline boundaries optionally).
  - [ ] Mixed strategy for non-palette colors: small local palette vs inline pixels heuristic.
  - [ ] Tune long/short repeat thresholds; auto-split very long runs.
  - [ ] Add region-aware encoding (tiles) for better local reuse on complex images.

- Dictionary-based compression (next phase)
  - [ ] Build first-order dictionary: frequent 2–4 pixel tuples (RGB565) → codes.
  - [ ] Extend stream with dictionary section and escape keys (reserve below `0xF0`).
  - [ ] Encoder heuristic to choose RLE vs. dict hits per segment.
  - [ ] Backward compatibility flag in header to signal dict presence.

- Container/header
  - [ ] Define a minimal header: magic, version, width, height, format flags, palette length.
  - [ ] Bundle palette + stream (+ optional dictionary) as a single blob.
  - [ ] Little-endian, fixed-size header for easy parsing.

- Creator integration
  - [ ] Add rlvgl-creator CLI subcommand: `creator assets encode --format rle`.
  - [ ] Support sequences (APNG/Lottie): emit numbered frames or a simple multi-frame container.
  - [ ] Option for RGB565 target directly to skip RGBA round-trip.

- Testing & CI
  - [ ] Unit tests: round-trip small patterns (solid, checkerboard, gradients, long runs).
  - [ ] Fuzz stream decode (lengths, keys, palette bounds) under `std`.
  - [ ] Golden samples under `tests/` with fixture images.
  - [ ] Benchmarks (host): encode/decode throughput and size vs. PNG (sanity).

- Performance & memory
  - [ ] Avoid intermediate allocations during decode (provide caller-owned buffer API).
  - [ ] Optional SIMD path for RGBA<->RGB565 conversions on host builds.
  - [ ] Iterator-based encoder to reduce temporary histograms for large frames.

- Documentation
  - [ ] Public API docs with examples.
  - [ ] Format specification page (stable), include byte diagrams.
  - [ ] Creator usage docs and troubleshooting (color banding, palette sizing, thresholds).

## Nice-to-haves

- [ ] Lossy palette quantization knobs (dither options, palette size cap).
- [ ] Tile/strip encoding to speed partial redraws.
- [ ] Optional per-frame delta encoding for sequences.

## Acceptance

- Decoder: passes unit tests and decodes sample assets without errors.
- Encoder: produces smaller-than-RGBA blobs on typical UI assets; configurable palette size.
- Creator: can ingest PNG/APNG/Lottie and emit the container; basic docs in `docs/`.
- CI: builds on stable; `cargo fmt`, `clippy` clean; link checker ok.

