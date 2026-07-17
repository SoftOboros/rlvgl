# KI2C-01 — Support Substrate As-Built Handoff

**Status:** Complete 2026-07-15. Accepted by deterministic gates and the local
Llama tactical judge. KI2C-02 is unblocked.

## 0. Authority Policy

This is the lossless compressed handoff for KI2C-01 under
`KI2C-00-CONCEPTS.md`. KI2C-00 remains normative. Source files, fixed tests,
command results, and the hashes in §13 are the full-fidelity implementation
handles; this summary does not replace them.

## 1. Purpose

Establish the reusable support needed before any device register behavior:
workspace crates, a strict `embedded-hal` 1.0 transaction recorder, stable Kria
logical-bus names and fitted addresses, a proven shared-bus handle, and an
optional Linux kernel-I2C adapter.

## 2. Outcome

Two crates landed in the workspace:

- `rlvgl-i2c-test-support` — unpublished, `no_std`, allocation-free strict
  transaction expectations for all later leaf-driver tests.
- `rlvgl-kria-i2c` — publishable, `no_std` by default, containing logical bus
  roles, fitted endpoints, `RefCellDevice` sharing, and an optional Linux
  `I2cdev` opener.

No device register driver landed in this phase.

## 3. Glossary

| Term | Meaning in the as-built substrate |
|---|---|
| Strict recorder | `MockI2c`, which consumes only exact ordered transaction matches. |
| Fitted endpoint | A `DeviceEndpoint` admitted by KI2C-00 hardware evidence. |
| Shared handle | An `embedded_hal_bus::i2c::RefCellDevice` created by `share_i2c`. |
| Linux backend | `linux_embedded_hal::I2cdev`, opened from a caller-supplied path. |

## 4. Source-of-Truth Map

| Concern | Source |
|---|---|
| Recorder API/behavior | `devices/rlvgl-i2c-test-support/src/lib.rs` |
| Recorder acceptance | `devices/rlvgl-i2c-test-support/tests/strict_recorder.rs` |
| Logical buses/endpoints/adapters | `devices/rlvgl-kria-i2c/src/lib.rs` |
| Corrected topology acceptance | `devices/rlvgl-kria-i2c/tests/topology.rs` |
| Complete shared transactions | `devices/rlvgl-kria-i2c/tests/shared_bus.rs` |
| Dependency/version selection | Both crate manifests |

## 5. Implemented Contract — Recorder

`ExpectedTransaction` borrows an ordered slice of `ExpectedOperation` values.
`MockI2c` implements `ErrorType<Error = MockError>` and
`I2c<SevenBitAddress>` through one authoritative `transaction` method.

It matches address, operation count, operation kind, write bytes, and read
length before advancing. It supplies admitted read response bytes, preserves
the current expectation after a mismatch, consumes a matched injected failure,
and reports unconsumed expectations from `done`.

## 6. Implemented Contract — Topology

`LogicalBus` contains exactly `PsI2c0`, `PsI2c1`, and `PlFrontPanelI2c`.
Fitted endpoint constants encode:

| Constant | Bus | Address |
|---|---|---:|
| `STTS22H` | `PsI2c1` | `0x38` |
| `EEPROM_U6` | `PsI2c0` | `0x50` |
| `VEML3235SL` | `PlFrontPanelI2c` | `0x10` |
| `PTN3460` | `PlFrontPanelI2c` | `0x20` |
| `PCM3168A` | `PlFrontPanelI2c` | `0x44` |

KSZ9897S is intentionally absent.

KI2C-04 expanded NXP's protocol examples and established that the documented
`0x40`/`0xc0` values are eight-bit address bytes. The topology therefore uses
the normalized DEV_CFG-low seven-bit address `0x20`; see §15.

## 7. Implemented Contract — Backends

`share_i2c` creates single-threaded `RefCellDevice` handles. Each handle
implements the ordinary leaf-driver I2C contract and locks the underlying bus
for a complete transaction. Interrupt/thread sharing remains an integration-
specific choice of a stronger `embedded-hal-bus` adapter.

The `linux` feature enables `linux::open(path)` only on Linux targets. Callers
map logical buses to paths; no `/dev/i2c-N` value is embedded in the crate.

## 8. Verification Evidence

The following gates passed:

```text
cargo fmt --manifest-path devices/rlvgl-i2c-test-support/Cargo.toml -- --check
cargo fmt --manifest-path devices/rlvgl-kria-i2c/Cargo.toml -- --check
cargo test -p rlvgl-i2c-test-support -p rlvgl-kria-i2c
cargo clippy -p rlvgl-i2c-test-support -p rlvgl-kria-i2c --all-targets -- -D warnings
cargo check -p rlvgl-i2c-test-support -p rlvgl-kria-i2c --target thumbv7em-none-eabihf
cargo clippy -p rlvgl-kria-i2c --features linux --target aarch64-unknown-linux-gnu -- -D warnings
cargo doc -p rlvgl-i2c-test-support -p rlvgl-kria-i2c --no-deps
cargo doc -p rlvgl-kria-i2c --features linux --target aarch64-unknown-linux-gnu --no-deps
git diff --check
```

Host result: five recorder tests, one shared-bus test, and two topology tests
passed. The two Rust cross targets were installed through `rustup`.

## 9. Model-Loop Record

The fixed candidate path was `rlvgl-i2c-test-support/src/lib.rs`. The local
`qwen2.5-coder:7b` executor produced an initial candidate and two bounded
revisions. All three used obsolete or invented `embedded-hal` types and failed
reactive compilation. The fixed tests were not relaxed.

After the local executor rung was exhausted, the primary executor replaced
only the candidate file. All §8 gates then passed. The local `llama3.1:8b`
judge returned `ACCEPT`. Its informational suggestions to collapse diagnostic
errors and replace `RefCellDevice` were not applied: precise mismatches are the
recorder's purpose, and the selected adapter is the upstream HAL team's
documented single-threaded `no_std` mechanism.

## 10. Reconciliation

The root workspace contains generated state-machine Rust that is intentionally
not normalized by the current rustfmt. A root `cargo fmt --all` would rewrite
those out-of-scope generated artifacts. KI2C therefore runs manifest-targeted
format checks for every in-scope crate and confirms a clean scoped diff. No
generated artifact remains modified.

Dependency verification selected `embedded-hal-bus` 0.3.0 and
`linux-embedded-hal` 0.4.1. Both implement `embedded-hal` 1.0 and are available
under MIT or Apache-2.0 terms compatible with this workspace.

## 11. Non-Goals

- Device register maps or behavior.
- Linux device-tree discovery policy beyond a caller-supplied path.
- Interrupt/thread shared-bus policy.
- EEPROM or KSZ9897S admission.
- Hardware I2C mutation.

## 12. Acceptance Checklist

- [x] Both crates are workspace members.
- [x] Strict ordered transaction behavior is covered by fixed tests.
- [x] Corrected PS split and collision-free shared PL bus are covered.
- [x] Three logical device handles complete independent transactions.
- [x] Default surfaces are documented and `no_std`.
- [x] Linux feature compiles for AArch64 Linux without fixed adapter numbers.
- [x] Format, test, clippy, cross-check, docs, and diff gates pass.
- [x] Independent local tactical judge verdict is `ACCEPT`.

## 13. Artifact Hashes

SHA-256 at acceptance:

```text
145938DBAF209DAE3D17275CFF368C2414F09DD4B8DBE6C41F03D0A4591139D2  devices/rlvgl-i2c-test-support/src/lib.rs
13FD755561636FFC9DE5826F74DA80A4250342C76956FF8DF20E21C1727400A6  devices/rlvgl-i2c-test-support/tests/strict_recorder.rs
3CDFF93CFAE34061E8B66DF9E126B9B88011CEDDC81150ACDC7B48743E1DE2FF  devices/rlvgl-kria-i2c/src/lib.rs
8D28901CA88C70014BC9782FBBEADA1FB487060CFCA17799B42F89CD63623A92  devices/rlvgl-kria-i2c/tests/shared_bus.rs
BF12694057FB504B32566A98569A9C52A85D59A743331FFFA045D0CB49096C71  devices/rlvgl-kria-i2c/tests/topology.rs
```

## 14. Unblocks and Deferred Work

- **Unblocked:** KI2C-02 STTS22H driver and its bounded PUT.
- **Deferred:** board device-node discovery, interrupt/thread adapters, and
  hardware smoke testing remain in KI2C-06/07.
- **Carry-forward obligation:** every leaf crate uses this recorder for exact
  register transactions and error preservation.
- **Memory Alpha:** as-built artifact 88 is embedded in notebook 57.

## 15. Change Log

- **2026-07-15 — KI2C-06 integration amendment.** Extended the Kria crate's
  accepted KI2C-01 substrate with the three-backend owner, typed leaf
  factories, generic physical mapping, structured diagnostics, and Linux
  mapped opener. The original topology/shared-bus tests remain unchanged and
  green; refreshed the evolving Kria source hash. See
  `KI2C-06-KRIA-INTEGRATION.md` for the complete integration hashes and gates.
- **2026-07-15 — KI2C-04 address-format amendment.** NXP AN11128 protocol
  examples proved the earlier `0x40` value was an eight-bit write-address byte.
  Changed the `embedded-hal` topology endpoint and shared-bus fixtures to the
  normalized seven-bit `0x20`, reran every support gate, and refreshed hashes.
- **2026-07-15 — Completed.** Reactive gates passed after bounded local-Qwen
  failure and primary-executor escalation; local Llama judge accepted the
  artifact. Full paths, hashes, checks, and next obligations recorded above.
