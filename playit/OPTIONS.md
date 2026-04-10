<!--
OPTIONS.md - Cargo feature reference for the rlvgl-playit crate.
-->
# rlvgl-playit Options

`rlvgl-playit` is the transport-agnostic automation and test driver used by
the simulator, UEFI runtime, and embedded serial workflows.

## Default configuration

- Default features: none.
- Runtime model: `no_std` without an allocator by default.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `alloc` | Enables heap-backed helpers while staying out of the full standard library. | Still `no_std`, but now expects an allocator. | Small code-size increase; useful when recordings or buffers are easier to manage on the heap. |
| `std` | Enables standard-library integrations such as the TCP transport and test-friendly host paths. | Host-oriented. Also implies `alloc`. | Moderate compile-time and binary-size increase, but required for socket-based automation. |
