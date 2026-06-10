# consumers/ — Gate P/R Consumer Projects

Consumer Projects per `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` §8:
workspace-detached (empty `[workspace]` stanza), registry-source-only
projects that consume rlvgl **as packaged crates** — never via `path`
dependencies or `[patch]` routes into the workspace (INV-C2; the extracted
package dirs under `target/crates-ci/staged/` injected by the Gate P
harness are the packaged file set and are explicitly allowed). They are
NOT members of the root workspace and are built only under the Gate P
(staged, pre-publish) or Gate R (real crates.io, post-publish) harness —
the crate versions they request may not exist on crates.io yet, so a bare
`cargo build` outside the harness is expected to fail to resolve.

| Consumer | What it proves | Entry point |
|---|---|---|
| `lib-smoke/` | P-RESOLVE on `rlvgl-core` (`png`+`fontdue`, the 9bee2f9 class) + `rlvgl-widgets`, linked from packaged crates | `gate_p.sh` (GATE=p staged / GATE=r crates.io) |
| `creator-cli/` | P-META/P-INCLUDE/P-RESOLVE on the root crate's `creator` path + the umbrella `simulator` feature (CRATES-CI-02a); CLI round-trip from the packaged file set | `smoke.sh` (Gate P) / `gate_r.sh` (Gate R) |
| `user-sim/` | `docs/CUSTOM-SIMULATOR.md` recipe from packaged crates: custom widget tree, `--headless`, playit `--automation-headless` handshake (INV-C7) + node tap→verify test, golden-PNG threshold check (INV-C4) | `gate_p.sh` (GATE=p staged / GATE=r crates.io) |

## Gate R (registry truth)

Gate R (CRATES-CI-05, `.github/workflows/gate-r.yml`) runs the same
consumers against **real crates.io** — after every successful Publish run
and on a daily schedule. To run locally:

```sh
GATE=r bash consumers/lib-smoke/gate_p.sh   # removes .cargo/config.toml, builds from crates.io
GATE=r bash consumers/user-sim/gate_p.sh    # ditto, plus golden-PNG + playit/node checks
bash consumers/creator-cli/gate_r.sh        # cargo install rlvgl --features creator + CLI round-trip
```

For `creator-cli`, Gate R is **not** smoke.sh against a staged package —
it is the literal end-user path: `cargo install rlvgl --features creator`
(unlocked, the way users type it) into a throwaway `--root`, then the
same init → scan → convert → sync round-trip as `smoke.sh`.
`GATE_R_FEATURES` overrides the feature list (default `creator`; do not
add `creator_ui` until a 0.2.x that builds it is the latest published).

The consumers request version `"0.2"`, so Gate R resolves the **latest
published 0.2.x**. Until v0.2.2 publishes, that is 0.2.1 — whose
simulator path is known-broken (CRATES-CI-00 §15, P-INCLUDE) — so the
user-sim Gate R step is expected red. No `continue-on-error` is used:
a red Gate R is the truthful state of the registry (§12, no silent skip).
