<!--
OPTIONS.md - Cargo feature reference for the rlvgl-decomp crate.
-->
# rlvgl-decomp Options

`rlvgl-decomp` provides the compact RLE codec used for splash screens and other
preprocessed RGBA assets. The crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Code size and runtime cost are fixed by the codec implementation and the
  assets you encode or decode.
- This crate is a good fit for embedded builds that want predictable startup
  asset decode without pulling in heavyweight image decoders.
