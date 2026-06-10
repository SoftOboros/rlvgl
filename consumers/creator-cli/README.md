# consumers/creator-cli

Gate P/R harness for the "GUI-wrapper binary as built from crates" story
(CRATES-CI-00 §8.4). This is **not** a cargo project: `smoke.sh` builds the
`rlvgl-creator` binary *inside* the staged root-crate package dir
(`$STAGE_DIR/staged/rlvgl-<version>/`) — the same compilation that
`cargo install rlvgl --features creator` performs — proving
**P-META / P-INCLUDE / P-RESOLVE** on the root crate's creator path: the
packaged file set must contain everything the `creator` feature needs, and
its dependency features must resolve outside the workspace.

The staged patch table (`$STAGE_DIR/patch-config.toml`, produced by
`scripts/crates_ci_stage.sh`) is installed as the package's
`.cargo/config.toml` with the `rlvgl = { ... }` entry filtered out (a
package must not patch itself). After the build, `smoke.sh` runs a minimal
CLI round-trip mirroring `.github/workflows/creator-e2e.yml`:
`init` → `scan` → `convert` → `sync --out out`, against a PNG fixture
copied from the repo at run time, and asserts the expected outputs
(`icons/rlvgl-logo.raw`, `out/features.toml`, `out/rlvgl_index.rs`).

Phase 01 scope is the `creator` CLI feature set only. **CRATES-CI-04** adds
`--features creator,creator_ui` plus the Layer W playit handshake
(`--automation-headless --playit-port` per CRATES-CI-00 §7).
