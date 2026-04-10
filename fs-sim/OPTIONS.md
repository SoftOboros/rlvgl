<!--
OPTIONS.md - Cargo feature reference for the rlvgl-fs-sim crate.
-->
# rlvgl-fs-sim Options

`rlvgl-fs-sim` is a host-side block-device shim for filesystem testing and
simulation. It is a `std` crate by design.

## Default configuration

- Default features: none.
- Runtime model: host-only `std`.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `mmap` | Uses `memmap2` to memory-map the disk image instead of always seeking and reading through `std::fs::File`. | Host-only `std`. | Usually improves random I/O and reduces syscall overhead, but increases virtual-memory usage and depends on host mmap behavior. |
