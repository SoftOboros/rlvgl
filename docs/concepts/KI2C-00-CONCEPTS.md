# KI2C-00 — Kria I2C Device and Backend Concepts

**Status:** Ratified 2026-07-15. Normative for the KI2C initiative. KI2C-06 is
complete; KI2C-07 hardware conformance remains gated by §7 and the physical
prerequisites recorded in `KI2C-06-KRIA-INTEGRATION.md` §14.

**Execution context:** This document is the durable technical context for the
planned local-model expand/implement/test/compress loop. The loop uses the
Qwen coder as executor and Llama as judge, but model policy is not part of the
device API and MUST NOT leak into a plan-under-test.

## 0. Authority Policy

| Concern | Authority | KI2C relationship |
|---|---|---|
| Board topology and fitted devices | Memory Alpha notebook `kria` (notebook 57), especially artifacts 71, 72, 73, 74, and 75 | Hardware claims below are admitted only when supported by these artifacts or their cited source documents. |
| Device register behavior | Source datasheet stored in notebook 57 | Each behavior phase MUST expand the relevant datasheet evidence before writing a register map. Notebook summaries alone are not register-level authority. |
| RLVGL I2C boundary | `platform/src/ft5336.rs`, `platform/src/wm8994.rs`, and `platform/Cargo.toml` at RLVGL commit `b67d799bc151d74b5ff5e6c420fc82bd4698fb59` | New leaf drivers use the same `embedded-hal` 1.0 blocking I2C contract. |
| RLVGL public and embedded discipline | `CLAUDE.md` and `AGENTS.md` | `no_std`, typed hardware APIs, public documentation, and spec-before-code gates apply. |
| Local executor/judge policy | Parent repository `docs/closedclaw/CLOSEDCLAW-13-ROLE-PARTITIONED-EXECUTOR.md` | Governs bounded model turns, role separation, escalation, and evidence routing; it does not govern Rust APIs. |

If new schematic or BOM evidence contradicts §2, this document MUST be amended
before a driver or board integration encodes the new claim.

## 1. Purpose

Create reusable Rust crates for the I2C-controlled devices fitted to the Kria
display/audio board and make them directly consumable by RLVGL platform code.
The device crates remain transport- and board-agnostic. A separate Kria
integration crate binds logical board buses to concrete backends, including a
Linux I2C backend when the controller is exposed through the kernel.

The work is deliberately multi-pass. Each pass expands full-fidelity Memory
Alpha evidence for one bounded device, implements one reviewable surface,
runs deterministic Rust gates, and compresses the accepted result into a
lossless handoff for the next pass.

## 2. Problem Statement and Admitted Hardware

The board has three distinct logical I2C buses. Earlier notes that placed the
STTS22H and EEPROM on one PS bus, or left the light sensor unresolved, are
superseded by notebook artifacts 74 and 75.

| Logical bus | Board signals | Device | 7-bit address | Disposition |
|---|---|---|---:|---|
| `PsI2c1` | `PS_MIO32` SCL, `PS_MIO33` SDA | STTS22H U35 temperature sensor | `0x38` | Confirmed; first driver phase. |
| `PsI2c0` | `PS_MIO46` SCL, `PS_MIO47` SDA | EEPROM U6 | `0x50` | Presence and address confirmed; driver blocked on exact part number and protocol. |
| `PlFrontPanelI2c` | FPGA `PL_I2C` through PCA9306 | VEML3235SL U4 ambient-light sensor | `0x10` | Confirmed. |
| `PlFrontPanelI2c` | FPGA `PL_I2C` through PCA9306 | PTN3460 U7 eDP-to-LVDS bridge | `0x20` | Confirmed DEV_CFG-low strap. NXP's `0x40` notation is the eight-bit write-address byte; normalized here for `embedded-hal`. |
| `PlFrontPanelI2c` | FPGA `PL_I2C` through PCA9306 | PCM3168A U2 audio codec | `0x44` | Confirmed; I2C control plane only. |
| Unconfirmed | Datasheet-supported mode only | KSZ9897S Ethernet switch | `0x5f` | Conditional; no crate until schematic/netlist evidence proves I2C mode is fitted and strapped. |

The PCA9306 is a passive level translator, not an addressed device, and does
not receive a driver crate. The STTS22H address strap is fitted high through
R175. `TEMP_ADDR` also reaches `PS_MIO36`; board integration MUST leave that
line high-impedance and MUST NOT drive it low.

The shared PL bus has no admitted address collision. Its photographed
peripheral-side pull-up voltage may be incompatible with the PCM3168A input
high threshold; that is a board electrical risk and MUST remain visible in
bring-up reporting. Software MUST NOT claim to correct it.

## 3. Glossary

| Term | Meaning |
|---|---|
| **Leaf driver** | A `no_std` crate for exactly one device family, generic over an I2C implementation. |
| **Logical bus** | Stable board role (`PsI2c0`, `PsI2c1`, or `PlFrontPanelI2c`), independent of Linux adapter numbering. |
| **Backend** | A concrete implementation of `embedded_hal::i2c::I2c<SevenBitAddress>`. |
| **Bus handle** | A backend or shared-bus wrapper passed by value to a leaf driver and returned by `release`. |
| **Evidence expansion** | Retrieval of the full relevant datasheet/schematic passages before a register-level implementation pass. |
| **Lossless compression** | A short handoff containing stable decisions and unresolved obligations plus handles to full evidence, diffs, and check output. |
| **PUT** | The bounded plan-under-test supplied to the ClosedClaw executor. It contains implementation scope and acceptance gates, not meta-plan policy. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Board bus/address inventory | Memory Alpha notebook 57, artifacts 72–75 |
| STTS22H address-strap correction | Notebook 57, artifact 71 |
| PCM3168A reset/clock sequence and electrical risk | Notebook 57, artifact 80 plus source document 1262 |
| STTS22H register behavior | Notebook document 1264 (`STTS22H.pdf`) |
| VEML3235SL register behavior | Notebook document 1268 (`veml3235sl-vishay-80011-rev1.4.pdf`) |
| PTN3460 register behavior | Notebook document 1263 (`PTN3460.pdf`) |
| PCM3168A register behavior | Notebook document 1262 (`PCM3168A-SBAS452A.pdf`) |
| KSZ9897S conditional I2C behavior | Notebook document 1261 (`KSZ9897S-DS00002394C.pdf`) |
| RLVGL-compatible device-driver shape | `platform/src/ft5336.rs` and `platform/src/wm8994.rs` |
| Crate graph and dependency policy | Workspace `Cargo.toml` and the future KI2C phase manifests |
| Accepted transaction behavior | Per-crate host unit tests using a deterministic transaction recorder |
| Physical bring-up truth | Versioned bring-up log keyed by logical bus, board revision, and source evidence |

## 5. Frozen Decision — Rust I2C Contract

1. Every leaf driver MUST be `#![no_std]` and MUST deny missing public
   documentation, matching `rlvgl-platform` discipline.
2. The blocking v1 transport boundary is
   `embedded_hal::i2c::I2c<embedded_hal::i2c::SevenBitAddress>` from
   `embedded-hal` 1.0. Leaf crates MUST NOT depend on RLVGL, Linux, a Kria BSP,
   or a specific I2C controller.
3. Constructors MUST be side-effect free. Device communication occurs only in
   explicit methods such as `probe`, `read_*`, `configure`, or `reset`.
4. A driver that owns its bus handle MUST expose `release(self) -> I2C`. This
   keeps it compatible with existing RLVGL drivers and with shared-bus wrapper
   handles.
5. Transport errors MUST preserve the backend's concrete error. Semantic
   failures, such as a mismatched identity value, use a documented, allocation-
   free driver error that contains the transport error rather than erasing it.
6. Fixed board addresses are documented constants. A device family with legal
   alternate addresses MAY accept an address in its constructor, but the Kria
   integration selects only an evidence-admitted value.
7. No leaf driver may hard-code Linux `/dev/i2c-*` names, Kria PS/PL controller
   indices, GPIO numbers, clocks, or board reset lines.
8. Async I2C is deferred. A later additive feature MAY mirror the blocking API
   over `embedded-hal-async` after a real consumer is named.

## 6. Frozen Decision — Crate and Ownership Shape

The proposed workspace layout is one publishable leaf crate per confirmed
device plus one non-leaf integration crate:

```text
devices/
  rlvgl-device-stts22h/
  rlvgl-device-veml3235sl/
  rlvgl-device-ptn3460/
  rlvgl-device-pcm3168a/
  rlvgl-kria-i2c/
```

The exact directory names and publish policy remain a §12 ratification choice.
The separation itself is normative once this document is ratified:

1. Leaf crates MUST NOT know that three devices share the PL controller.
2. `rlvgl-kria-i2c` owns logical-bus mapping, shared-bus construction, board
   address selection, optional reset GPIO choreography, and backend selection.
3. Shared access MUST use an implementation that presents an ordinary
   `embedded-hal` 1.0 I2C handle to each driver. KI2C-01 will select and pin the
   shared-bus dependency after API/license verification; KI2C MUST NOT invent a
   second I2C trait.
4. A Linux backend SHOULD adapt a maintained kernel I2C implementation such as
   `linux-embedded-hal::I2cdev`. It MUST resolve device nodes from logical-bus
   configuration or device-tree identity, not assume adapter numbering.
5. A bare-metal or RTOS Kria backend MAY pass PS/PL controller handles directly
   or through the same sharing layer. The leaf crates remain unchanged.

## 7. Frozen Decision — Device Admission and Initial Depth

| Phase | Device | Minimum accepted surface |
|---|---|---|
| KI2C-01 | Shared test/support substrate | **Complete 2026-07-15.** Workspace/manifests, strict transaction recorder, logical-bus vocabulary, shared-bus adapter, and Linux backend compile checks. See `KI2C-01-SUPPORT-SUBSTRATE.md`. |
| KI2C-02 | STTS22H | **Complete 2026-07-15.** `WHOAMI` probe (`0x01 == 0xa0`), coherent signed temperature read from `0x06/0x07`, safe one-shot/low-ODR/freerun configuration, read-to-clear status, and exact alert-threshold encoding. See `KI2C-02-STTS22H.md`. |
| KI2C-03 | VEML3235SL | **Complete 2026-07-15.** Fixed-address/ID probe, typed shutdown and integration/analog/digital-gain configuration, raw ALS/white reads, and exact integer micro-lux conversion across the complete Rev 1.4 resolution matrix. See `KI2C-03-VEML3235SL.md`. |
| KI2C-04 | PTN3460 | **Complete 2026-07-15.** Configuration-magic health probe without a false identity claim, corrected `0x20` seven-bit address, and typed/reserved-safe LVDS electrical register `0x82`. EDID, flash, pin overrides, panel timing, and broader link configuration remain gated. See `KI2C-04-PTN3460.md`. |
| KI2C-05 | PCM3168A | **Complete 2026-07-15.** Readiness typestate requires explicit stable-rail, released-reset, and synchronized-clock attestation before I2C methods exist. Adds truthful reset-state health, common slave audio-format writes, and sampling-mode-preserving resynchronization. Audio transport and GPIO remain excluded. See `KI2C-05-PCM3168A.md`. |
| KI2C-06 | Kria integration | **Complete 2026-07-15.** One owned backend per controller, typed leaf factories over the separate PS/shared PL buses, caller-owned logical-to-physical mappings, Linux mapped opening, and allocation-free structured diagnostics. See `KI2C-06-KRIA-INTEGRATION.md`. |
| KI2C-07 | Hardware conformance | **Hardware-blocked 2026-07-15.** Safe probe order, required board/bitstream/mapping/electrical inputs, result vocabulary, evidence schema, and release gates are prepared. No physical result is claimed. See `KI2C-07-HARDWARE-CONFORMANCE.md`. |

EEPROM U6 remains blocked until its exact part number, capacity, page size,
address width, and write-cycle behavior are authoritative. KSZ9897S remains
blocked until board evidence confirms I2C mode and its `0x5f` strap; its
datasheet's 16-bit-register/8-bit-data protocol is not proof of board wiring.

## 8. Frozen Decision — Verification

Each leaf-driver pass MUST provide deterministic host tests for the exact I2C
transaction sequence, endianness, conversion boundaries, identity mismatch,
and injected backend errors. Tests MUST reject extra or reordered bus
transactions.

The minimum software gates for an accepted leaf crate are:

```text
cargo fmt --all -- --check
cargo test -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
cargo check -p <crate> --target thumbv7em-none-eabihf
cargo doc -p <crate> --no-deps
```

KI2C-01 MAY select a different installed `no_std` target when it demonstrates
an equivalent gate and records the reason. Integration crates add their host
or cross-Linux target without weakening leaf `no_std` checks. Hardware tests
MUST identify the board revision and logical bus and MUST distinguish a NACK,
identity mismatch, transaction failure, and electrically suspect bus.

## 9. Execution Discipline — Expand, Implement, Verify, Compress

The initiative is executed as a series of bounded PUTs, normally one leaf
crate's `src/lib.rs` behavior surface per PUT. The current ClosedClaw runner
has one candidate write path, so manifests, fixed tests, and other scaffold
are prepared and reviewed outside that candidate write surface. Multi-file
generation requires an explicit runner enhancement, not an oversized PUT.

For each device phase:

1. **Expand:** retrieve the source datasheet passages and current board
   evidence from notebook 57. Produce a register/transaction table with page
   provenance and list uncertainties. Do not code from a compressed summary.
2. **Prepare:** freeze a PUT brief, acceptance criteria, candidate path, fixed
   seed files, and deterministic checks. Keep executor/judge ladders, revision
   budgets, and escalation rules in the meta-plan only.
3. **Implement:** use the local Qwen coder executor for the bounded candidate.
4. **Verify:** run reactive Rust checks first. Then use the local Llama judge
   for tactical review against the PUT and evidence. Qwen 3.6 is an alternate
   judge, not a second simultaneous judge.
5. **Revise:** allow the bounded revision budget, escalating only according to
   ClosedClaw policy. Acceptance routes the change to human review; it does not
   commit or publish automatically.
6. **Compress:** record accepted paths/diff identity, check output, evidence
   handles, frozen decisions, unresolved obligations, and the next phase's
   dependencies. Store full evidence separately and retain its handles so
   compression is obligation-lossless.
7. **Reinitialize:** construct the next PUT from the ratified concepts doc and
   compressed handoff, then expand that device's full evidence.

## 10. Reconciliation With Existing RLVGL Primitives

| Existing primitive | KI2C relationship |
|---|---|
| `platform::Ft5336<I2C>` | Establishes the generic `I2c<SevenBitAddress>`, owned-handle, fixed-address, and `release` precedent. |
| `platform::Wm8994<I2C>` | Establishes a typed codec-control precedent and proves nontrivial register widths fit the same transport boundary. |
| Optional `embedded-hal` platform feature | Leaf device crates depend on `embedded-hal` directly; the Kria integration is composed by RLVGL consumers rather than hidden behind an unrelated display/input feature. |
| Linux display/input backends | Adjacent but not reusable as I2C transports. The KI2C Linux backend is a separate adapter. |
| RLVGL renderer/widget crates | No dependency in either direction is needed for leaf drivers. Applications translate sensor/codec state into UI state. |

## 11. Non-Goals

- Reconstructing missing proprietary JSA schematic/BOM content from inference.
- Treating an address ACK as sufficient device identity when a documented
  identity or health register exists.
- An EEPROM driver before U6 is identified.
- A KSZ9897S driver based only on a datasheet-supported mode.
- Implementing the PCA9306 as a software device.
- Audio streaming, DMA, clock synthesis, or codec data-plane handling.
- Correcting electrical voltage-level problems in software.
- A new RLVGL-specific I2C trait, allocator-backed APIs, or a mandatory async
  surface.
- Hard-coded Linux I2C adapter numbers.
- Automatic commit, publish, or hardware mutation on model-judge acceptance.

## 12. Ratification and Acceptance Checklist

The operator must accept or amend these choices before KI2C-01:

- [x] Confirm one publishable leaf crate per confirmed device and the proposed
      `devices/` naming/location.
- [x] Confirm that `rlvgl-kria-i2c` belongs in this workspace rather than a
      board-application repository.
- [x] Confirm blocking `embedded-hal` 1.0 as the v1 contract and async as
      deferred.
- [x] Confirm EEPROM and KSZ9897S remain gated on stronger board evidence.
- [x] Confirm PTN3460 and PCM3168A begin with board-required control-plane
      subsets rather than speculative full-datasheet coverage.
- [x] Confirm the local Qwen executor / Llama judge, one-candidate-path PUT,
      bounded-revision, human-review routing described in §9.
- [x] Confirm Memory Alpha notebook 57 is the evidence root and that every
      register behavior retains source-document/page provenance.

The initiative is complete only when:

- [x] All admitted leaf crates pass §8 gates and document their public APIs.
- [x] One shared PL bus can host VEML3235SL, PTN3460, and PCM3168A handles
      without mutable-aliasing or transaction-interleaving defects.
- [x] `PsI2c0`, `PsI2c1`, and `PlFrontPanelI2c` map without hard-coded Linux
      enumeration assumptions.
- [ ] Hardware smoke evidence exists for every admitted fitted device, or a
      named evidence-backed blocker remains explicit.
- [ ] The PCM3168A electrical-level risk and reset/clock prerequisites appear
      in bring-up output and documentation.
- [ ] RLVGL validation remains green and release versions are bumped where
      repository policy requires it.
- [ ] A retrospective records divergences, deferred items, and reusable
      lessons from the multi-pass local-model loop.

## 13. Files and Evidence Cited

- `platform/Cargo.toml` — `embedded-hal` 1.0 dependency precedent.
- `platform/src/ft5336.rs` — generic seven-bit I2C touch-driver precedent.
- `platform/src/wm8994.rs` — generic I2C codec-register precedent.
- Workspace `Cargo.toml` — crate membership and release surface.
- `CLAUDE.md` — typed hardware and spec-before-code discipline.
- Parent `docs/closedclaw/CLOSEDCLAW-13-ROLE-PARTITIONED-EXECUTOR.md` — local
  role-partitioned loop authority.
- Memory Alpha notebook 57 artifacts 71–75 and 80 — board topology,
  corrections, shared-bus map, and codec bring-up evidence.
- Memory Alpha notebook 57 documents 1261–1264 and 1268 — device datasheets.

## 14. Unblocks and Deferred Work

- **Unblocks after ratification:** KI2C-01 support substrate and the first
  bounded STTS22H PUT.
- **Deferred — Evidence-gated:** EEPROM U6 and KSZ9897S.
- **Deferred — Coupled:** exact shared-bus crate/version and exact Linux I2C
  adapter/version; resolve in KI2C-01 after compatibility and license checks.
- **Deferred — Safe:** async I2C, broader PTN3460 configuration, broader
  PCM3168A register coverage, and non-Kria address variants without consumers.
- **Operational prerequisite:** the current Windows host has Rust, Ollama, and
  the four local models, but lacks Docker and a Django-capable backend Python
  environment. Before running the stock `closedclaw_loop` management command,
  restore the supported container runtime or provision the repository's
  supported backend environment. Install the selected `no_std` Rust target
  before §8 cross-checks.

## 15. Change Log

- **2026-07-15 — KI2C-07 hardware-blocked handoff.** Recorded the exact board,
  bitstream, controller-mapping, electrical, and PCM readiness evidence needed
  for safe on-board conformance. Added a read-only typed probe order and
  versioned record schema without claiming hardware execution.
- **2026-07-15 — KI2C-06 complete.** Added the three-controller backend
  bundle, typed leaf factories over separate PS/shared PL buses, generic
  physical mappings, structured smoke diagnostics, and Linux mapped opening.
  Six integration/topology tests and all Rust gates passed; the local Llama
  judge returned `ACCEPT`. KI2C-07 software preparation is unblocked, while
  physical conformance remains explicitly hardware-gated.
- **2026-07-15 — KI2C-05 complete.** Expanded TI SBAS452A Rev. A and added
  the readiness-gated PCM3168A control-plane driver. Eight fixed tests and all
  Rust gates passed; the local Llama judge returned `ACCEPT`. KI2C-06 is
  unblocked.
- **2026-07-15 — KI2C-04 complete.** Expanded NXP Rev 4 and AN11128 Rev 1.9,
  corrected the PTN3460 address from eight-bit-byte notation `0x40` to
  seven-bit `0x20`, and added the bounded health/electrical leaf driver. Seven
  leaf tests, three amended Kria tests, and all Rust gates passed; the local
  Llama judge returned `ACCEPT`. KI2C-05 is unblocked.
- **2026-07-15 — KI2C-03 complete.** Added the `no_std` VEML3235SL leaf
  driver from Vishay Rev 1.4 evidence. Eight transaction/API tests and all
  scoped Rust gates passed; the local Llama judge returned `ACCEPT`. KI2C-04
  is unblocked.
- **2026-07-15 — KI2C-02 complete.** Added the `no_std` STTS22H leaf driver
  from DS12606 Rev 8 evidence. Eight transaction/API tests and all scoped Rust
  gates passed; the local Llama judge returned `ACCEPT`. KI2C-03 is unblocked.
- **2026-07-15 — KI2C-01 complete.** Added the strict reusable transaction
  recorder and Kria logical-topology/backend crate. Host tests, clippy,
  `thumbv7em-none-eabihf`, AArch64 Linux, and documentation gates passed; the
  local Llama judge returned `ACCEPT`. KI2C-02 is unblocked.
- **2026-07-15 — Ratified.** The operator accepted §12 without amendment.
  KI2C-01 is unblocked. No implementation behavior was included in the
  ratification change.
- **2026-07-15 — Drafted.** Recorded the corrected three-bus topology from
  Memory Alpha notebook 57, selected the existing RLVGL `embedded-hal` 1.0
  contract, proposed per-device crates plus a Kria integration crate, gated
  EEPROM/KSZ9897S on missing evidence, and defined the multi-pass
  expand/implement/verify/compress loop. Not ratified; KI2C-01 remains blocked.
