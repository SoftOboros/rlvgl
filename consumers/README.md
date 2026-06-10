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
| `creator-cli/` | P-META/P-INCLUDE/P-RESOLVE on the root crate's `creator` path + the umbrella `simulator` feature (CRATES-CI-02a); CLI round-trip from the packaged file set | `smoke.sh` |
| `user-sim/` | `docs/CUSTOM-SIMULATOR.md` recipe from packaged crates: custom widget tree, `--headless`, playit `--automation-headless` handshake (INV-C7) + node tap→verify test, golden-PNG threshold check (INV-C4) | `gate_p.sh` (GATE=p staged / GATE=r crates.io) |
