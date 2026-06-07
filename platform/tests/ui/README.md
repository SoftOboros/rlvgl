# Trybuild Compile-Fail Fixtures

Each `*.rs` here is a deliberately-failing program. The matching
`*.stderr` snapshot records the exact `rustc` diagnostic — `trybuild`
runs the fixture and asserts the diagnostic still matches.

## Naming convention

| Prefix | What the fixture proves |
|--------|--------------------------|
| `inflight_*.rs` | DMA `InFlight` / `BorrowedForDma` borrow contracts |
| `scanout_*.rs` | `Scanout` / `FrontBuffer` / `BackBuffer` access rules |
| `mmio_*.rs` | `MmioAddr<T>` and other `unsafe`-construction guards |

The harness `tests/discipline_compile.rs` globs by these prefixes.

## Regenerating snapshots

After a Rust toolchain bump the diagnostic wording may shift. Refresh
the `*.stderr` files in a single PR with no other changes:

```bash
TRYBUILD=overwrite cargo test -p rlvgl-platform --test discipline_compile
```

Review the diff and confirm the new stderr still pinpoints the same
violation before committing.
