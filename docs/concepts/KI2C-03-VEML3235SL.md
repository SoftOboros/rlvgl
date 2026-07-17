# KI2C-03 — VEML3235SL Driver As-Built Handoff

**Status:** Complete 2026-07-15. Accepted by deterministic gates and the local
Llama tactical judge. KI2C-04 is unblocked.

## 0. Authority Policy

This phase implements only the VEML3235SL surface admitted by KI2C-00.
Register behavior is sourced from Vishay document 80011 Rev 1.4
(28-Nov-2024). Memory Alpha notebook 57 document 1268 remains the project
evidence handle; its MCP endpoint returned HTTP 502 throughout this pass, so
the official primary source was expanded directly and notebook artifact
backfill was deferred until the KI2C-06 compression pass.

## 1. Purpose

Provide a publishable, allocation-free `no_std` VEML3235SL crate compatible
with RLVGL's `embedded-hal` 1.0 I2C boundary. Cover identity, legal command
configuration, both raw light channels, and exact lux conversion without
hiding conversion delays or admitting obsolete fields from older datasheet
revisions.

## 2. Outcome

`devices/rlvgl-device-veml3235sl` is a workspace crate at version `0.2.5`.
It owns an arbitrary `I2c<SevenBitAddress>` handle, always uses the fixed
`0x10` address, preserves backend error types, and returns the handle through
`release`.

## 3. Glossary

| Term | Meaning |
|---|---|
| ALS | Photopic ambient-light channel used for lux conversion. |
| White | Broad-spectrum 16-bit channel exposed as raw data only. |
| IT | ALS/white integration time: 50, 100, 200, 400, or 800 ms. |
| DG | Additional digital gain: x1 or x2. |
| Micro-lux | One millionth of a lux; the integer unit used to preserve every table value exactly. |

## 4. Expanded Evidence Map

| Behavior | Vishay Rev 1.4 source |
|---|---|
| Fixed address and two 16-bit channels | Page 7, command-register introduction |
| I2C frequency ceiling | Pages 2–3, basic and timing characteristics; 400 kHz |
| Word write/read byte order | Pages 4 and 6, word-command timing and command protocol |
| Command register fields | Page 7, Table 1 |
| White, ALS, and ID registers | Page 7, Table 1 |
| Little-endian data access | Page 8, data-access section and Table 2 |
| Lux example and complete resolution matrices | Page 8, DG x1 and DG x2 tables |

Primary source: [VEML3235SL document 80011 Rev 1.4](https://www.vishay.com/docs/80011/veml3235sl.pdf).

## 5. Implemented Register Contract

| Register | Address | Driver behavior |
|---|---:|---|
| Command | `0x00` | Full three-byte write: register, low byte, high byte. |
| White | `0x04` | `read_white_raw`; low byte first. |
| ALS | `0x05` | `read_als_raw`; low byte first. |
| ID | `0x09` | `probe` requires low part-number byte `0x35`; reserved high byte is ignored. |

The reserved `0x02` register is not exposed. Rev 1.4 defines command low bits
3:1 as reserved zero, so the driver deliberately has no force or trigger API
from older register descriptions.

## 6. Implemented Configuration Contract

`Config` admits exactly five integration times, analog gains x1/x2/x4, digital
gains x1/x2, and a paired enabled/shutdown state. Its serializer:

- writes integration time to low bits 6:4;
- sets low `SD` and high `SD0` together only when disabled;
- writes DG to high bit 5 and analog gain to high bits 4:3;
- keeps every reserved-zero field clear; and
- always writes required high reserved bit zero as one.

`Config::default()` is the enabled 100 ms, x1, DG x1 setting. `configure`
invalidates local state before every bus write and caches the new value only
after the complete command succeeds.

## 7. Data and Conversion Types

Raw white and ALS reads are permitted without configuration and while shut
down; the hardware may return reset or stale data in those states.

`read_illuminance` requires a successfully written active configuration and
performs no transaction on `NotConfigured` or `Shutdown`. The caller owns the
integration-time delay needed for a fresh sample.

`Illuminance` carries the original `u16` ALS count and an exact `u64`
micro-lux result. The 30-point Rev 1.4 resolution matrix is represented without
floating point. At the least-sensitive setting, full scale is
`17_867_462_400` micro-lux, which requires the 64-bit result.

## 8. Verification Evidence

The following gates passed:

```text
cargo fmt -p rlvgl-device-veml3235sl -- --check
cargo test -p rlvgl-device-veml3235sl
cargo clippy -p rlvgl-device-veml3235sl --all-targets -- -D warnings
cargo check -p rlvgl-device-veml3235sl --target thumbv7em-none-eabihf
cargo doc -p rlvgl-device-veml3235sl --no-deps
git diff --check
```

Eight fixed host tests cover ownership and fixed address, identity success/
mismatch/bus failure, exact active and shutdown command bytes, state
invalidation after a failed write, both raw register transactions and byte
order, illuminance state guards, all 30 resolution combinations, the datasheet
1480-count example, and full-scale overflow safety.

## 9. Model-Loop Record

The local `qwen2.5-coder:7b` executor was limited to `src/lib.rs`. Its initial
candidate mis-shifted command fields, set the wrong shutdown bit, divided lux
by an invented factor, skipped bus-error mapping, and broke the copied-config
contract. The first bounded revision introduced async operations on the
blocking trait, omitted the command register from writes, swapped gain fields,
and still lacked required APIs. The final bounded revision switched to an
unrelated TSL2561 and the removed `embedded_hal::blocking` module. The fixed
tests were never relaxed.

The primary executor escalation replaced only `src/lib.rs`. All §8 gates then
passed. The local `llama3.1:8b` tactical judge returned strict
`{"verdict":"ACCEPT"}` with no findings.

## 10. Reconciliation

Rev 1.4 marks low command bits 3:1 reserved zero. Older material described
force/trigger behavior in that area; the current authoritative revision wins,
and no such API is admitted.

The datasheet's ID label renders the part number as 3235 while its documented
low byte is binary `0011_0101`. The probe checks the unambiguous byte value
`0x35` and ignores the reserved high byte.

## 11. Non-Goals

- Delay ownership or automatic sleeping between configuration and sampling.
- Floating-point lux output or lens/transmittance compensation.
- Interpreting the white channel as lux.
- Fields absent from the Rev 1.4 command table.
- Async I2C or Linux-specific behavior.
- Hard-coding PL I2C controller/device-node enumeration.

## 12. Acceptance Checklist

- [x] Publishable `no_std` embedded-hal 1.0 leaf crate.
- [x] Side-effect-free constructor and owned-handle release.
- [x] Fixed `0x10` address and low-byte identity probe.
- [x] Only legal integration, analog-gain, and digital-gain encodings.
- [x] Paired active/shutdown state with reserved-bit discipline.
- [x] Little-endian raw ALS and white channel reads.
- [x] Exact integer micro-lux conversion for all 30 table entries.
- [x] Configuration failure and inactive-state safety.
- [x] Host, lint, cross-target, docs, and diff gates pass.
- [x] Independent Llama verdict `ACCEPT`.

## 13. Artifact Hashes

SHA-256 at acceptance:

```text
5398D9F1A7B4DC061D15A07C5DC1E6DD6375F575F9F63688C41B50B32977A8E8  devices/rlvgl-device-veml3235sl/src/lib.rs
42C0383F989D917369A8B85586372B2B3771FA9C46EFCB5F89C36E9128F08E7B  devices/rlvgl-device-veml3235sl/tests/driver.rs
```

## 14. Unblocks and Deferred Work

- **Unblocked:** KI2C-04 PTN3460 evidence expansion and driver PUT.
- **Deferred to KI2C-06/07:** shared-PL-bus integration, physical `0x10`
  probe, conversion-delay scheduling, and optical/lens calibration evidence.
- **Memory Alpha:** as-built artifact 90 is embedded in notebook 57; the
  earlier HTTP 502 backfill obligation is closed.

## 15. Change Log

- **2026-07-15 — Completed.** Expanded Vishay document 80011 Rev 1.4,
  implemented and tested the bounded driver after local-Qwen exhaustion,
  received local-Llama `ACCEPT`, and recorded full source/check/hash/deferred-
  work handles.
