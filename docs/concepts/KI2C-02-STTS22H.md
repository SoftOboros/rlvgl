# KI2C-02 — STTS22H Driver As-Built Handoff

**Status:** Complete 2026-07-15. Accepted by deterministic gates and the local
Llama tactical judge. KI2C-03 is unblocked.

## 0. Authority Policy

This phase implements the STTS22H surface admitted by KI2C-00. Register
behavior is sourced from STMicroelectronics DS12606 Rev 8 (March 2026), the
current official revision. Memory Alpha notebook 57 document 1264 remains the
project evidence handle; its MCP endpoint returned HTTP 502 throughout this
pass, so the official primary source was expanded directly and notebook
artifact backfill was deferred until the KI2C-06 compression pass.

## 1. Purpose

Provide a publishable, allocation-free `no_std` STTS22H crate compatible with
RLVGL's `embedded-hal` 1.0 I2C boundary. Cover identity, safe operating-mode
configuration, one-shot triggering, coherent temperature reads, status, and
both alert thresholds without hiding board timing or inventing reset behavior.

## 2. Outcome

`devices/rlvgl-device-stts22h` is a workspace crate at version `0.2.5`. It owns
an arbitrary `I2c<SevenBitAddress>` handle, defaults to the Kria `0x38`
address, preserves backend error types, and returns the handle through
`release`.

## 3. Glossary

| Term | Meaning |
|---|---|
| Coherent read | Low-byte-first two-byte read with BDU and auto-increment forced by successful configuration. |
| Power-down transition | CTRL write with `LOW_ODR_START`, `FREERUN`, and `ONE_SHOT` clear before any mode/ODR selection. |
| Centi-degree | Signed hundredth of a degree Celsius, the sensor's native 100 LSB/°C scale. |
| Exact threshold | A value representable by the sensor's `(register - 63) * 0.64 °C` decoder. |

## 4. Expanded Evidence Map

| Behavior | DS12606 Rev 8 source |
|---|---|
| Address options | Page 4, Table 2; Kria uses ADDR tied to VDD → `0x38` seven-bit |
| Boot timing | Page 6, Table 4; caller waits at least 12 ms |
| SMBus/I2C register read | Pages 8–12; register-pointer write plus repeated-start read |
| Auto-increment | Page 10; CTRL `IF_ADD_INC` enables consecutive addresses |
| Register map | Page 14, Table 12 |
| CTRL fields and AVG/ODR | Page 15, §7.4 and Table 13 |
| STATUS read-to-clear flags | Page 15, §7.5 |
| Signed temperature conversion | Page 17, §8; two's-complement word / 100 LSB/°C |
| Threshold range/step | Page 18, §9 and Table 14 |
| ALERT open-drain/clear behavior | Page 19, §10 |
| Mode transition rule | Page 20, §11 and Table 15 |

Primary source: [STTS22H DS12606 Rev 8](https://www.st.com/resource/en/datasheet/stts22h.pdf).

## 5. Implemented Register Contract

| Register | Address | Driver behavior |
|---|---:|---|
| WHOAMI | `0x01` | `probe` requires `0xa0`. |
| TEMP_H_LIMIT | `0x02` | `set_high_threshold`. |
| TEMP_L_LIMIT | `0x03` | `set_low_threshold`. |
| CTRL | `0x04` | Full safe `Config`, mode transition, and one-shot trigger. |
| STATUS | `0x05` | `read_status`; exposes low/high/busy bits and documents clearing. |
| TEMP_L_OUT / TEMP_H_OUT | `0x06` / `0x07` | One coherent little-endian `i16` read. |

No software-reset method exists because Rev 8 admits no reset register.

## 6. Implemented Configuration Contract

`Config` supports:

- one-shot with 8/4/2/1-sample averaging;
- low ODR at 1 Hz with averaging;
- freerun at 25/50/100/200 Hz, coupled to the documented AVG encoding;
- enabled-by-default SMBus timeout with an explicit override.

Every configuration sets BDU and `IF_ADD_INC`. `configure` invalidates local
state first, writes the power-down form, writes the final mode only when
different, and records state only after all writes succeed. `start_one_shot`
is legal only in configured one-shot mode.

## 7. Temperature, Status, and Threshold Types

`Temperature` carries the native signed centi-degree `i16`. Configuration is
required before `read_temperature`, preventing an incoherent read when the
hardware CTRL state is unknown.

`Status` exposes one-shot busy, over-high, and under-low. Its read-to-clear
semantics and ALERT deassert/reassert behavior are documented at the method.

`Threshold` separates `disabled()` (register zero) from exact active values.
`from_centi_celsius` accepts `-3968..=12288` only at 64-centi-degree steps and
returns the rejected value in `ThresholdError` otherwise.

## 8. Verification Evidence

The following gates passed:

```text
cargo fmt -p rlvgl-device-stts22h -- --check
cargo test -p rlvgl-device-stts22h
cargo clippy -p rlvgl-device-stts22h --all-targets -- -D warnings
cargo check -p rlvgl-device-stts22h --target thumbv7em-none-eabihf
cargo doc -p rlvgl-device-stts22h --no-deps
git diff --check
```

Eight fixed host tests cover side-effect-free ownership, identity success/
mismatch/bus failure, safe freerun transition, one-shot mode enforcement,
positive and negative signed readings, read-to-clear status bits, exact
threshold boundaries/rejection/writes, and configuration invalidation after a
failed bus operation.

## 9. Model-Loop Record

The local `qwen2.5-coder:7b` executor was limited to `src/lib.rs`. Its initial
candidate used the removed `embedded_hal::blocking` API. Two bounded revisions
continued to use invalid 1.0 generics and then added incorrect CTRL encodings,
private acceptance APIs, recursive configuration, wrong rate vocabulary, and
`unimplemented!()` hardware paths. The fixed tests were never relaxed.

The primary executor escalation replaced only `src/lib.rs`. All §8 gates then
passed. The local `llama3.1:8b` tactical judge returned `ACCEPT` with no
actionable findings.

## 10. Reconciliation

The Rev 8 register table lists CTRL reset value `0x00`, while the I2C prose says
`IF_ADD_INC` is enabled by default. The driver does not choose between those
conflicting claims. It requires explicit configuration before the only
multi-byte data read and forces both auto-increment and BDU.

The board's `TEMP_ADDR`/`PS_MIO36` high-impedance obligation remains in the
Kria integration layer; a transport-agnostic leaf driver cannot enforce GPIO
direction.

## 11. Non-Goals

- Waiting or delay ownership; callers enforce the 12 ms power-on wait.
- SMBus ARA transaction support.
- ALERT GPIO input handling.
- A software reset not present in the authoritative register map.
- Async I2C or Linux-specific behavior.
- Hard-coding PS I2C controller/device-node enumeration.

## 12. Acceptance Checklist

- [x] Publishable `no_std` embedded-hal 1.0 leaf crate.
- [x] Side-effect-free constructors and owned-handle release.
- [x] Typed legal addresses with Kria `0x38` default.
- [x] WHOAMI identity and backend error preservation.
- [x] Safe one-shot, low-ODR, and freerun configuration.
- [x] Coherent positive/negative centi-degree temperature reads.
- [x] Read-to-clear status semantics.
- [x] Exact high/low threshold encoding and rejection.
- [x] Host, lint, cross-target, docs, and diff gates pass.
- [x] Independent Llama verdict `ACCEPT`.

## 13. Artifact Hashes

SHA-256 at acceptance:

```text
43F12EFD5855F88E0A92B79F59547237770EA4624A415069D5125EB14582F8A3  devices/rlvgl-device-stts22h/src/lib.rs
AC85D005637CB07C64900FC4CA338AD35A0CD85E20BCB1DAF813F5512A6AD45B  devices/rlvgl-device-stts22h/tests/driver.rs
```

## 14. Unblocks and Deferred Work

- **Unblocked:** KI2C-03 VEML3235SL evidence expansion and driver PUT.
- **Deferred to KI2C-06/07:** 12 ms board delay, `TEMP_ADDR` high-impedance
  enforcement, physical `0x38` probe, and ALERT-pin smoke behavior.
- **Memory Alpha:** as-built artifact 89 is embedded in notebook 57; the
  earlier HTTP 502 backfill obligation is closed.

## 15. Change Log

- **2026-07-15 — Completed.** Expanded DS12606 Rev 8, implemented and tested
  the bounded driver after local-Qwen exhaustion, received local-Llama
  `ACCEPT`, and recorded full source/check/hash/deferred-work handles.
