# consumers/user-sim — crates-built custom simulator (CRATES-CI-02)

Consumer Project per `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` §8.1–8.3:
proves that a downstream user can build a **working simulator purely from
registry crates**, following the `docs/CUSTOM-SIMULATOR.md` recipe — own
resolution (480x320), own widget tree (tagged `user.root` / `user.title` /
`user.button`), not the built-in demo — and that the **playit automation
surface is reachable from published crates** (INV-C7: the unmodified
`playit/node` client launches and drives this binary exactly like the
in-tree disco-sim).

Workspace-detached (empty `[workspace]` stanza, INV-C2: registry source
only). Built only under the Gate P (staged) or Gate R (crates.io) harness;
a bare `cargo build` outside the harness is expected to fail to resolve
until the requested versions exist on crates.io.

## Run it

```bash
bash consumers/user-sim/gate_p.sh        # Gate P (requires scripts/crates_ci_stage.sh first)
GATE=r bash consumers/user-sim/gate_p.sh # Gate R (real crates.io)
```

The gate builds the binary, asserts no rlvgl-* crate resolved from the
registry (Gate P honesty check), runs the in-binary golden-PNG comparison,
and runs `test/user-sim.test.js` through `playit/node`.

## Binary modes

| Flags | Behaviour |
|---|---|
| *(none)* | windowed wgpu run |
| `--headless=<path>` | one offscreen frame as ASCII art |
| `--png-path=<file>` | one PNG frame via `WgpuDisplay::headless` (CPU only) |
| `--golden-check=<file>` | render + compare against a golden PNG; prints `GOLDEN_CHECK mean_abs_diff=… threshold=3`; exits nonzero past threshold (INV-C4: explicit threshold, never bit-exact) |
| `--automation-headless --playit-port=<n>` | playit TCP server, `PLAYIT_READY tcp://127.0.0.1:<port>` handshake, no window/GPU |

## Adapting it downstream

Copy `Cargo.toml` + `src/main.rs`; replace `build_ui()` with your own
widget tree and tag the nodes you want addressable over the wire
(`WidgetNode::with_tag`). Dependency choices, deliberately user-realistic:

- `rlvgl-core` + `rlvgl-widgets` + `rlvgl-platform = { features =
  ["simulator", "fontdue"] }` — the three layers a custom simulator
  actually consumes. NOT the umbrella `rlvgl = { features = ["simulator"] }`
  crate: as of 0.2.2 that feature is unbuildable from the packaged crate
  set because it pulls `rlvgl-app-disco-demo`, whose package is missing the
  `.rle` icon assets it `include_bytes!`s from outside its crate root
  (P-INCLUDE; found by this consumer's first Gate P run, 2026-06-10).
  `fontdue` is required for visible Label/Button text — without it the
  blitter's `draw_text` is a stub.
- `rlvgl-playit = { version = "0.2", features = ["std"] }` — taken directly
  because the umbrella crate links but does not re-export it; `std` enables
  `TcpServerTransport`.

## Regenerating the golden

After an intentional rendering change, from a Gate-P-built tree:

```bash
consumers/user-sim/target/debug/crates-ci-user-sim \
  --png-path=consumers/user-sim/golden/user-sim.png
```

then commit the new `golden/user-sim.png`.
