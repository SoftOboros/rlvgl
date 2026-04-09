# /pre-publish

Run the full pre-publish validation chain before committing changes to
publishable crates.  This mirrors what CI and `scripts/publish_changed.sh`
will enforce.

## Phases

| Phase | Command | Purpose |
|-------|---------|---------|
| 0 | `cargo fmt --all -- --check` | Format check |
| 1 | `RUSTFLAGS="" cargo clippy --workspace -- -D warnings` | Clippy (warnings = errors) |
| 2 | `RUSTFLAGS="" cargo test --workspace` | Unit + doc tests |
| 3 | `RUSTFLAGS="" cargo test -p rlvgl-playit` | playit standalone tests |
| 3b | `RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-playit` | playit no_std cross-compile |
| 3c | `cd playit && cargo package --list --allow-dirty && cd ..` | playit package listing |
| 4 | `RUSTFLAGS="" cargo test -p rlvgl-example-sim` | Simulator tests |
| 4b | `RUSTFLAGS="" cargo test --tests --features "creator" -p rlvgl` | Creator tests |
| 5 | `RUSTFLAGS="" cargo doc --workspace --no-deps` | Documentation build |
| 6 | `RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-example-disco --features cm7` | Embedded target build |
| 7 | `DRY_RUN=1 scripts/publish_changed.sh HEAD~1` | Publish dry run |

## Notes

- On macOS, use `RUSTFLAGS=""` to bypass the mold linker configuration.
- All phases must pass before committing.
- Stop on first failure and diagnose before continuing.

## Claude Code usage

```
/pre-publish
```

This runs all phases sequentially and reports a pass/fail summary table.
