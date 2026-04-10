<!--
OPTIONS.md - Cargo feature reference for the rlvgl-i18n crate.
-->
# rlvgl-i18n Options

`rlvgl-i18n` compiles locale JSON files into a compact embedded translation
blob and exposes runtime lookup helpers. The crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Most of the work happens at build time through the crate's code generation
  step.
- Runtime cost is driven by the size of the generated translation blob and the
  number of strings you ship, not by feature selection.
- Locale changes and external-blob loading are runtime API choices rather than
  Cargo feature choices.
