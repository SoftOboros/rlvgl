<!--
rlvgl-decomp/README.md - RLE decoder/encoder for rlvgl splash format.
-->

# rlvgl-decomp

Core compressed image format utilities for rlvgl.

This crate provides a compact run‑length format with a palette and inline pixel
escape codes, plus a basic encoder that builds a palette and emits a short/long
repeat stream. Both operate on RGBA frames and convert to/from RGB565 internally
to match embedded display pipelines.

Features:
- No-std compatible (uses `alloc`).
- Decoder for the RLE format (palette + byte stream → RGBA).
- Encoder from RGBA → palette (RGB565) + byte stream using repeat/dictionary.

The format is a starting point for creator tooling to convert inputs (e.g.,
PNG/APNG/Lottie frames) into a compact representation consumable by rlvgl.
