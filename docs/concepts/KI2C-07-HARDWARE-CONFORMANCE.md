# KI2C-07 — Kria I2C Hardware Conformance Handoff

**Status:** Hardware-blocked 2026-07-15. The software crates and host/cross
gates are complete, but no physical-board result is claimed. Execution resumes
only when the §5 prerequisites are supplied.

## 0. Authority Policy

KI2C-00 remains normative for admitted topology. KI2C-02 through KI2C-05 own
device behavior, and KI2C-06 owns bus composition and diagnostics. This phase
may record only direct observations from an identified board and deployed
software/hardware image. Mock transactions, address inference, cross-compiled
binaries, and an I2C ACK without the typed probe semantics are not conformance
evidence.

## 1. Purpose

Define the minimum safe, reproducible on-board procedure needed to close the
KI2C initiative without erasing electrical risk, board readiness, or failure
provenance.

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
3. On PS I2C1, run `probe_stts22h`; on success, record one coherent signed
   temperature read without changing its configuration.
4. On the shared PL bus, run `probe_veml3235sl`; on success, record raw ALS and
   white values only if a known active configuration already exists or a
   separately authorized configuration step is performed.
5. On the same PL bundle, run `probe_ptn3460` and record both the raw
   configuration magic and `configuration_valid` result. Do not call this an
   identity test.
6. If and only if all PCM prerequisites are evidenced, create `HardwareReady`,
   convert the lent driver to `Ready`, run `probe_pcm3168a`, and record the
   decoded reset state. Do not call this an identity test.
7. Release all three backends and record whether every expected operation was
   attempted, skipped, or failed.

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

## 8. Release Gates

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

Resume KI2C-07 when the operator supplies the §5 hardware facts and access to
the identified Kria environment. Until then, KI2C-01 through KI2C-06 are ready
for human review, but the initiative as a whole remains hardware-blocked.

## 12. Change Log

- **2026-07-15 — Blocked handoff created.** Recorded the cumulative software
  baseline, safe read-only probe order, evidence schema, release gates, and
  precise physical prerequisites. No board operations or conformance claims
  were made.
