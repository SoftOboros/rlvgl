# KI2C-06 — Kria I2C Integration As-Built Handoff

**Status:** Complete 2026-07-15. Accepted by deterministic gates and the local
Llama tactical judge. KI2C-07 software preparation is unblocked, but physical
hardware conformance remains blocked on the prerequisites in §14.

## 0. Authority Policy

This phase composes only topology admitted by KI2C-00 and public APIs accepted
in KI2C-02 through KI2C-05. It adds no new device register claims. Memory Alpha
notebook 57 remains the project evidence root. Its MCP endpoint returned HTTP
502 during implementation, then recovered during compression; KI2C-01 through
KI2C-06 are now embedded as artifacts 88–92 and 87, respectively.

## 1. Purpose

Bind the transport-agnostic leaf drivers to the three stable Kria logical bus
roles without encoding Linux adapter enumeration, duplicating a physical bus,
or weakening the PCM3168A readiness contract. Provide allocation-free smoke
diagnostics that retain both the board endpoint and the leaf driver's exact
result.

## 2. Outcome

`devices/rlvgl-kria-i2c` now owns the complete software integration boundary:

- `PhysicalBusMap<P>` stores caller-supplied identifiers for PS I2C0, PS I2C1,
  and the PL front-panel controller.
- `KriaI2cBuses<Ps0, Ps1, Pl>` owns one backend per physical controller and
  wraps each once in a `RefCell`.
- Typed factories lend STTS22H from PS I2C1 and VEML3235SL, PTN3460, and
  PCM3168A from the same PL backend.
- `ProbeDiagnostic<T, E>` owns the stable endpoint and exact leaf `Result`.
- The optional Linux module opens a complete physical mapping and attributes
  an open failure to its logical bus while preserving `LinuxI2CError`.

PS I2C0 remains owned and releasable for the evidence-gated EEPROM, but this
phase does not invent an EEPROM driver.

## 3. Glossary

| Term | Meaning |
|---|---|
| Physical identifier | Caller-owned value naming one concrete controller, such as a Linux device-node path or BSP controller ID. |
| Bus bundle | The three backend-owning `KriaI2cBuses` cells. |
| Lent driver | A leaf driver owning a temporary `RefCellDevice` handle into one bundle cell. |
| Smoke diagnostic | One leaf probe result paired with its admitted `DeviceEndpoint`; not physical-conformance proof by itself. |

## 4. Logical-to-Physical Mapping

`PhysicalBusMap<P>` is generic and contains no `/dev/i2c-*` constants. Its
`new` constructor requires all three mappings, and `get` resolves only the
stable `LogicalBus` enum. Applications therefore own device-tree discovery,
adapter enumeration, and deployment-specific naming.

The Linux-only `open_mapped` function opens controllers in PS I2C0, PS I2C1,
then PL order. `OpenError::bus` identifies the failed logical controller and
`OpenError::error` borrows the original Linux backend error. A partial bundle
is never returned.

## 5. Ownership and Sharing Contract

| Factory | Backend cell | Fitted address | Returned state |
|---|---|---:|---|
| `stts22h` | PS I2C1 | `0x38` | STTS22H driver |
| `veml3235sl` | PL front panel | `0x10` | VEML3235SL driver |
| `ptn3460` | PL front panel | `0x20` | PTN3460 driver |
| `pcm3168a` | PL front panel | `0x44` | PCM3168A `Unverified` driver |

Each factory constrains only the backend it uses. `RefCellDevice` serializes a
complete transaction on the shared PL cell. Drivers are scoped borrows of the
bundle; `release` consumes the bundle only after those borrows end and returns
the three original backends in PS0, PS1, PL order.

This adapter is single-threaded. Interrupt, multicore, or executor-shared
consumers must choose an appropriate stronger `embedded-hal-bus` adapter in a
future evidence-backed integration.

## 6. PCM3168A Readiness Boundary

The integration crate does not create `HardwareReady`. `pcm3168a` returns the
leaf driver's default `Unverified` typestate, on which no bus operations are
available. A board application must attest stable rails, released reset, and
synchronized clocks before calling `into_ready`; only a `Ready` driver can be
passed to `probe_pcm3168a`.

This keeps GPIO, clock, mute, settling, and electrical-voltage ownership at the
board layer where those facts can actually be observed.

## 7. Structured Diagnostics

`ProbeDiagnostic<T, E>` is allocation-free and provides:

- `endpoint` for the stable logical bus and seven-bit address;
- `is_ok` for summary reporting;
- `result` for a borrowed successful probe value or leaf error; and
- `into_result` for recovering the original owned leaf result.

The four probe helpers do not translate or erase errors. STTS22H and
VEML3235SL retain identity-check results; PTN3460 retains its
configuration-magic health value; PCM3168A retains its reset-state health
value. The latter two remain health checks rather than silicon-identity claims.

## 8. Verification Evidence

The following gates passed:

```text
cargo fmt -p rlvgl-kria-i2c
cargo test -p rlvgl-kria-i2c
cargo clippy -p rlvgl-kria-i2c --all-targets -- -D warnings
cargo check -p rlvgl-kria-i2c --target thumbv7em-none-eabihf
cargo clippy -p rlvgl-kria-i2c --target aarch64-unknown-linux-gnu --features linux -- -D warnings
cargo doc -p rlvgl-kria-i2c --no-deps
git diff --check
```

Six tests pass across the integration, shared-bus, and topology suites. The
three KI2C-06 tests prove caller-owned mappings; separate PS routing; exact
shared-PL transaction order and corrected PTN3460 address; explicit PCM
readiness; endpoint-bearing success values; and preservation of a leaf
identity failure.

The AArch64 Linux gate is compile-only. It proves API and dependency
compatibility, not that any particular device node maps to a board controller.

A focused host graph check covering `rlvgl-core`, `rlvgl-widgets`,
`rlvgl-platform`, `rlvgl-ui`, `rlvgl-api`, and all six KI2C crates passed. A
separate `cargo check --workspace` attempt reached and compiled the KI2C crates
but failed in unchanged `rlvgl-platform` Linux evdev/fbdev modules on the
Windows host: workspace feature unification enabled `linux_fbdev`, whose source
uses `std::os::unix` and Unix-only `libc` APIs without a target-OS guard. That
pre-existing cross-host gate is outside KI2C scope and was not modified.

## 9. Model-Loop Record

The initial local `qwen2.5-coder:7b` request exceeded the 55-second transport
bound. Revision one returned a partial diff that implemented none of the
required integration types and proposed unrelated, invalid Linux error
variants. The final bounded revision added only duplicated documentation and
again omitted every required API. The frozen tests were never relaxed.

The primary executor escalation changed only `src/lib.rs`; the manifest and
fixed integration tests remained outside the candidate path. All §8 gates
then passed. The first local `llama3.1:8b` response said `ACCEPT` but
inconsistently attached a blocking finding to a nonexistent
`get_adapter_path` method. A constrained adjudication retry returned `ACCEPT`
with an empty blocking array. Its repeated informational reference to that
nonexistent symbol was discarded as non-actionable; the compiler-enforced
missing-documentation gate and exact source were authoritative.

## 10. Reconciliation

KI2C-01 exposed individual shared handles and one Linux path opener. KI2C-06
retains both APIs and adds the backend-owning bundle, typed leaf factories,
complete mapping opener, and diagnostics. This is an additive integration,
not a second I2C trait or a replacement leaf-driver abstraction.

The original topology notation for PTN3460 was corrected in KI2C-04. All
integration transactions use the normalized `embedded-hal` seven-bit address
`0x20`, not NXP's eight-bit write-address byte `0x40`.

## 11. Non-Goals

- Hard-coded Linux adapter numbers or device-tree discovery.
- EEPROM or KSZ9897S admission without missing board evidence.
- GPIO, rail, reset, clock, mute, or interrupt ownership.
- Multithreaded, interrupt-safe, multicore, or asynchronous bus sharing.
- Automatic configuration writes during construction or probing.
- Treating host mock transactions or cross-compilation as hardware evidence.

## 12. Acceptance Checklist

- [x] Complete generic logical-to-physical mapping with no fixed paths.
- [x] One owned backend per physical controller.
- [x] STTS22H routed to PS I2C1.
- [x] VEML3235SL, PTN3460, and PCM3168A routed through one shared PL cell.
- [x] PCM3168A readiness remains an explicit board-owned attestation.
- [x] Allocation-free diagnostics retain endpoint, values, and leaf errors.
- [x] Host, lint, no-std, Linux cross-target, docs, and diff gates pass.
- [x] Independent Llama verdict `ACCEPT` with no blocking findings.

## 13. Artifact Hashes

SHA-256 at acceptance:

```text
95CFA1CB158B38E3AA0DD56F2829AB8CCF1CA3D4D2A4F571BB71C3393807A437  devices/rlvgl-kria-i2c/Cargo.toml
3CDFF93CFAE34061E8B66DF9E126B9B88011CEDDC81150ACDC7B48743E1DE2FF  devices/rlvgl-kria-i2c/src/lib.rs
5C911F5108086AB03799ADDAEA4E908DE6C1D85A400B12CA525ECF02C98E1CD8  devices/rlvgl-kria-i2c/tests/integration.rs
8D28901CA88C70014BC9782FBBEADA1FB487060CFCA17799B42F89CD63623A92  devices/rlvgl-kria-i2c/tests/shared_bus.rs
BF12694057FB504B32566A98569A9C52A85D59A743331FFFA045D0CB49096C71  devices/rlvgl-kria-i2c/tests/topology.rs
```

## 14. Unblocks and Deferred Work

- **Unblocked:** KI2C-07 software-side bring-up command/example design and
  versioned conformance-log schema.
- **Hardware-blocked:** physical smoke/conformance requires the Kria board,
  board revision, deployed bitstream/device tree, actual logical-to-physical
  controller mapping, and permission to observe the buses.
- **PCM hardware-blocked:** readiness requires observable rail, reset, and
  synchronous-clock ownership. The reported PL pull-up-voltage risk must be
  measured or otherwise resolved before an I2C success is called safe.
- **Evidence-gated:** EEPROM U6 and KSZ9897S remain excluded.
- **Memory Alpha:** KI2C-06 is embedded as artifact 87; KI2C-01 through
  KI2C-05 are embedded as artifacts 88 through 92. The HTTP 502 backfill
  obligation is closed.

No KI2C-07 document may claim on-board success from the host mocks or Linux
cross-compilation recorded here.

## 15. Change Log

- **2026-07-15 — Completed.** Implemented the explicit three-controller
  bundle, typed shared-bus leaf factories, caller-owned physical mapping,
  structured diagnostics, and Linux mapped opener after bounded local-Qwen
  exhaustion. All deterministic gates passed and the local Llama judge
  returned `ACCEPT` with no blocking findings after adjudication.
