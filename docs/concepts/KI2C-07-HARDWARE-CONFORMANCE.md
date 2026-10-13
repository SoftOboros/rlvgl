# KI2C-07 — Kria I2C Hardware Conformance Handoff

**Status:** Owner-ratified scope amendment 2026-10-06 (§15 0.1.0). The physical
identity/typed-health profile remains hardware-blocked since 2026-07-15 until
the §5 prerequisites are supplied. Software gates remain separate from physical
qualification; no board, temperature or wiring result is claimed.

## 0. Authority Policy

KI2C-00 remains normative for admitted topology. KI2C-02 through KI2C-05 own
device behavior, and KI2C-06 owns bus composition and diagnostics. This phase
may record only direct observations from an identified board and deployed
software/hardware image. Mock transactions, address inference, cross-compiled
binaries, and an I2C ACK without the typed probe semantics are not conformance
evidence.

The owner-ratified §6 profile requires strict read-only identity and existing
reporting. PTN3460 and PCM3168A retain typed health semantics. Identity/health
success MUST NOT be represented as temperature validation or board qualification.
Temperature validation and matched prototype wiring remain gates for specific
CCPS functions that depend on them, not generic RLVGL publication prerequisites.

## 1. Purpose

Define the minimum safe, reproducible identity/typed-health reporting procedure
on an identified Kria board, preserving electrical risk, board readiness and
failure provenance. Software release readiness does not imply completion of
this physical profile or temperature-dependent CCPS validation.

## 2. Current Software Baseline

The accepted software baseline contains six crates at version `0.2.5`:

- `rlvgl-i2c-test-support`;
- `rlvgl-kria-i2c`;
- `rlvgl-device-stts22h`;
- `rlvgl-device-veml3235sl`;
- `rlvgl-device-ptn3460`; and
- `rlvgl-device-pcm3168a`.

Forty-two deterministic tests pass across those crates. Formatting, host
Clippy with warnings denied, `thumbv7em-none-eabihf` no-std checks, AArch64
Linux-feature Clippy, public documentation, and diff checks also pass. These
are software-release inputs, not physical evidence.

A focused host compatibility check with the RLVGL core, widgets, platform,
UI, and API crates passes. The full 41-package Windows-host workspace check is
currently blocked by unchanged Linux evdev/fbdev modules that are enabled by
workspace feature unification and import Unix-only APIs. The check reached and
compiled the KI2C crates before that unrelated failure.

## 3. Conformance Vocabulary

| Result | Meaning |
|---|---|
| `pass` | The typed probe/read completed on the identified board and all prerequisite/electrical checks for that endpoint are satisfied. |
| `bus-error` | The backend preserved a transport failure; record the complete error and logical bus. |
| `semantic-failure` | Transport completed, but a leaf identity, reserved-state, or configuration-validity check failed. |
| `prerequisite-blocked` | The operation was intentionally not attempted because a required rail/reset/clock/GPIO/electrical fact was unavailable or unsafe. |
| `not-admitted` | Device behavior is excluded by the evidence policy, including EEPROM U6 and KSZ9897S. |

## 4. Safety Invariants

1. `TEMP_ADDR`/`PS_MIO36` MUST remain high-impedance before STTS22H access.
2. The caller MUST resolve physical controllers to `PsI2c0`, `PsI2c1`, and
   `PlFrontPanelI2c`; adapter numbers MUST NOT be inferred from this repository.
3. A broad address scan SHOULD NOT be used as the primary test. Access only
   admitted addresses with the typed drivers because generic probing can have
   device-specific side effects.
4. PCM3168A bus access MUST NOT occur until stable rails, released RST, and
   synchronized SCKI/BCK/LRCK have been observed and attested.
5. A successful PCM3168A transaction MUST NOT override an unresolved PL
   pull-up-voltage or input-threshold risk.
6. Smoke probing is read-only. Configuration writes require a separate named
   test, expected values, muting/settling ownership where relevant, and operator
   authorization.

## 5. Inputs Required to Resume

- Board model, revision, serial or asset identifier, and operator.
- RLVGL commit plus application commit.
- Bitstream identity/hash and device-tree identity/hash.
- Operating system/kernel or bare-metal BSP version.
- Caller-resolved physical identifiers for all three logical buses.
- Confirmation that `TEMP_ADDR` is high-impedance.
- PL peripheral-side pull-up voltage measurement and applicable input-threshold
  assessment, especially for PCM3168A.
- Observable PCM3168A rail, RST, SCKI, BCK, and LRCK readiness evidence.
- Permission to open the controllers and perform the exact read-only probes.

Without these inputs, the correct phase state is `hardware-blocked`.

## 6. Required Probe Order

1. Record the §5 inputs before opening any controller.
2. Build a caller-owned `PhysicalBusMap` and open it as one `KriaI2cBuses`
   bundle. Attribute an open failure to the exact `LogicalBus`.
3. On PS I2C1, run `probe_stts22h`; on success, record the typed identity
   result only. Do not configure the device or require a temperature read.
4. On the shared PL bus, run `probe_veml3235sl`; on success, record the typed
   identity result only. Do not configure the device or require raw ALS/white
   capture for this identity-only profile.
5. On the same PL bundle, run `probe_ptn3460` and record both the raw
   configuration magic and `configuration_valid` result. Do not call this an
   identity test.
6. If and only if all PCM prerequisites are evidenced, create `HardwareReady`,
   convert the lent driver to `Ready`, run `probe_pcm3168a`, and record the
   decoded reset state. Do not call this an identity test.
7. Release all three backends and record whether every expected operation was
attempted, skipped, or failed.

This profile MUST NOT perform configuration writes. Use the existing §7 record
and §3 vocabulary to name each attempted operation and retain exact errors or
skip causes. Record temperature capture as unattempted for this profile; identity
success MUST NOT fill a temperature-validation result. The existing reporting
shape is retained without adopting a new schema or version. Temperature-dependent
CCPS functions remain gated by their own validation and matched rig requirements.

PS I2C0 ownership is verified by mapping/opening only. EEPROM U6 receives no
protocol transaction until its exact part and write contract are admitted.

## 7. Minimum Conformance Record

```text
run_id:
timestamp_utc:
operator:
board_model:
board_revision:
board_asset_id:
rlvgl_commit:
application_commit:
bitstream_sha256:
device_tree_sha256:
runtime_version:
logical_bus_mapping:
  PsI2c0:
  PsI2c1:
  PlFrontPanelI2c:
temp_addr_high_impedance:
pl_pullup_voltage_v:
pcm_input_threshold_assessment:
pcm_rails_stable:
pcm_reset_released:
pcm_clocks_synchronous:
results:
  stts22h_0x38:
  veml3235sl_0x10:
  ptn3460_0x20:
  pcm3168a_0x44:
unattempted_operations:
notes:
```

Each device result records the logical bus, physical identifier, operation,
typed value when successful, exact leaf/backend error when unsuccessful, and
one vocabulary result from §3.

## 8. Software Release and Physical Profile Gates

The completed software gates below are generic RLVGL release inputs. The
unchecked board/probe/electrical/record items govern this physical identity and
typed-health profile. They MUST NOT be interpreted as missing temperature or
CCPS prototype-wiring prerequisites for generic RLVGL publication. Identity
success alone does not satisfy those physical qualification items.

- [x] All admitted software crates pass their deterministic and cross-target
      gates.
- [x] Public APIs and as-built handoffs are documented.
- [x] KI2C-01 through KI2C-06 are embedded in Memory Alpha notebook 57, and
      this conformance runbook is embedded as artifact 93.
- [ ] An identified board satisfies every §5 prerequisite.
- [ ] Read-only typed probes execute according to §6.
- [ ] Electrical and PCM readiness evidence is attached to the run.
- [ ] Every failure or skip retains its cause and logical endpoint.
- [ ] The completed conformance record is reviewed and persisted.

## 9. Model-Loop Disposition

No Qwen executor or Llama judge pass is started for physical conformance while
§5 is missing. A model cannot generate the required observations, and a
simulated candidate would weaken the evidence boundary. Once the hardware
inputs exist, the bounded plan-under-test may cover a concrete bring-up binary
and one identified conformance record; the judge must review both code and
captured evidence.

## 10. Deferred and Excluded Work

- EEPROM U6 protocol access until exact part/capacity/address-width/page/write
  timing evidence exists.
- KSZ9897S access until board wiring and I2C-mode strap evidence exists.
- PTN3460 EDID/flash/link configuration and PCM3168A broader controls until a
  board requirement and safe register contract are expanded.
- Async, interrupt-safe, multicore, or executor-specific bus sharing until a
  concrete consumer selects the synchronization model.

## 11. Resume Condition

Resume this physical KI2C-07 identity/typed-health profile when the operator
supplies the §5 hardware facts and access to the identified Kria environment.
Until then, its physical qualification remains hardware-blocked. That status
does not make temperature validation or CCPS prototype wiring a generic RLVGL
release blocker. Affected CCPS functions still require their own actual
temperature/wiring validation before they can be qualified.

## 12. Change Log

- **2026-07-15 — Blocked handoff created.** Recorded the cumulative software
  baseline, safe read-only probe order, evidence schema, release gates, and
  precise physical prerequisites. No board operations or conformance claims
  were made.

## 15. Change Log

The legacy §12 record above remains historical input. This current-SBC bridge
governs new amendments without rewriting that record. The dedicated
[KI2C errata log](KI2C-ERRATA.md) is activated before this scope amendment.

### 0.1.0 — 2026-10-06 — amended

**Author:** Ira Abbott; recorded by Codex
**Change kind:** scope
**Touches:** §0, §1, §6, §8, §11
**Summary:** Owner accepts strict read-only identity/typed-health reporting for RLVGL, retaining temperature and prototype wiring qualification only for affected CCPS dependencies and physical claims.

#### Rationale

The former STTS22H smoke requirement combined a configuration-dependent
temperature read with a prohibition on configuration writes. Owner decision 4
selects identity-only probing and reporting as they stand. Adding writes or an
observed-configuration adoption API would expand the accepted operation and
driver contracts, so neither is selected. Identity or typed-health success
does not establish temperature accuracy, electrical readiness or board
qualification; all physical prerequisites and existing reporting remain.

Owner decision 6 confirms that CCPS prototype wiring divergences are deliberate
ease-of-build adaptations with matching firmware/hardware, including ESP32
routing revisions. They are not generic RLVGL release blockers. Future firmware
revisions may retire old rigs; this amendment does not choose a pin map,
qualify any current rig or authorize hardware changes. Affected CCPS functions
retain their actual temperature/wiring validation dependencies. See
KI2C ERRATA-001 and the [owner decision receipt](../releases/v0.2.7-embedded-owner-decision.md).
