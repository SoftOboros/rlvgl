<!--
OPTIONS.md - Cargo feature reference for the rlvgl-widgets crate.
-->
# rlvgl-widgets Options

`rlvgl-widgets` provides the built-in widget set that sits on top of
`rlvgl-core`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## What that means in practice

- Build behavior is stable across targets; there is no feature matrix to manage
  in this crate itself.
- Code size and runtime cost are driven by which widgets you instantiate, not
  by Cargo options.
- If you need codecs, simulator support, storage, or board integrations, those
  switches live in `rlvgl-core`, `rlvgl-platform`, or the consuming app crate.
