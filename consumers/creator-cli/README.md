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

Phase 01 scope was the `creator` CLI feature set only. **CRATES-CI-04**
added the Layer W section: a second build from the same staged package with
`--features creator,creator_ui,creator_ui_automation` (proving the GUI
wrapper compiles from packaged crates), then `test/creator-ui.test.js`
drives `rlvgl-creator --automation-headless --playit-port=0` through the
**unmodified** `playit/node` client (CRATES-CI-00 §7, INV-C7): the
`PLAYIT_READY tcp://127.0.0.1:<port>` handshake, `?` status advance,
`QE:/QB:` accesskit lookups by label (tags ARE labels, §7.4), a `T@Build`
tap that reveals the `Fonts Pack` menu entry, and one guarded `D` dump that
may degrade to `ERR: render-unavailable` on GPU-less CI (INV-C5). The
automation server executes each verb against an `egui_kittest` harness
hosting `CreatorApp` (`src/bin/creator_ui/automation.rs`); the wire codec
is reused from the `rlvgl-playit` crate so the verb vocabulary stays owned
there (INV-C3). Automation-driven test flows must stick to rfd-free UI
paths — menu entries that open native file/message dialogs
(`src/bin/creator_ui/commands.rs`) block the harness thread if clicked.

Env for the node test: `RLVGL_CREATOR_BIN` must point at a binary built
with `creator,creator_ui,creator_ui_automation`; the test stages a tempdir
containing a default `manifest.yml` and spawns the binary with that cwd
(automation mode never opens a manifest dialog — §7.5).
