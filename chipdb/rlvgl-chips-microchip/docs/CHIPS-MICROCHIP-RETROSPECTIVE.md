# CHIPS-MICROCHIP Retrospective — divergences, refactor points, forward constraints

**Status:** Drafted 2026-05-15. Initiative-completion retrospective for
the CHIPS-MICROCHIP initiative on rlvgl `v0.2.0`. Co-located with
`CHIPS-MICROCHIP-00-CONCEPTS.md`, `CHIPS-MICROCHIP-05-LINKER.md`, and
`CHIPS-MICROCHIP-06-EXAMPLE.md` per the CLAUDE.md "Spec-Before-Code
Planning Discipline → Initiative retrospective" convention.

Retrospective in the agile sense: surfaces what diverged from plan,
what gates worked / didn't work, what patterns to carry forward, and
what preconditions future MICROCHIP and adjacent CHIPS-* initiatives
must satisfy. Audience is future Codex / Claude agents working a
structurally similar single-vendor BSP-generator initiative.

This doc is a **historical artifact** with one normative section (§6
forward constraints). Behaviour PRs reference the canonical concepts
doc + §15 change log directly, never the retrospective.

## 1. Outcome snapshot

### Final architecture

`chipdb/rlvgl-chips-microchip/` houses the chip and board inventory
for the Microchip vendor family. Phase 4.8 of the pre-publish gate
exercises:

- Chipdb crate tests (`cargo test -p rlvgl-chips-microchip`).
- 8-file BSP emission (6 `.rs` + `memory.x` + `atsamd51j19a.x`) for
  the Adafruit Feather M4 Express, snapshotted by
  `tests/bsp_microchip_render.rs`.
- Opt-in compile-verify (`tests/bsp_microchip_compile.rs` under
  `--features compile-verify`) running `cargo check --target
  thumbv7em-none-eabihf` against `atsamd51j19a 0.7.1`.
- Standalone example crate at `examples/feather-m4-express/` that
  links the slate-9 generator output and slate-9 linker scripts,
  shipping PA23 LED blink and SERCOM5 USART hello-world.

The snapshot is FREE of `MISMATCH` fallback comments after slate 13
`-01b` (commit `84d530a`). Both PB22/PB23 PMUX `fn_c` rows and PA04 /
PA06 ADC0 `fn_b` rows are real PMUX writes, not chipdb-generator
escapes.

### Deferred items (explicit)

1. **CHIPS-MICROCHIP-06c** — rlvgl widget tree on the Feather M4
   Express. Coupled to the assumption that the Feather M4 has a
   built-in display, which it does not. Closed-with-deferral; reopens
   if the example grows an external display companion module.

### Known residual risks

- **PAC version pin.** `atsamd51j19a 0.7.1` is the latest available
  on crates.io as of initiative close; it was published in December
  2020 and predates the modern svd2rust method-accessor convention.
  Templates were amended in slate 6 `-02` (`fc88383`) to match its
  field-style register access. Any future republish of `atsamd51j19a`
  with a method-accessor surface (`p.MCLK.apbamask()`) would require
  a counter-amendment to the templates; the compile-verify gate is
  the canonical detector.
- **PB22/PB23 yaml-transcription pattern.** The slate-4 snapshot
  shipped `MISMATCH` fallback comments for these pads because the
  chip yaml `io_mux.fn_c:` column had been transcribed asymmetrically
  (PB23 inherited PB22's `SERCOM1_PAD2` instead of getting its real
  `SERCOM1_PAD3`). Adjacent-pad carry-over is the dominant
  yaml-transcription failure mode and recurred (in a more benign form)
  with PA04 / PA06 ADC0 `fn_b`.

## 2. Divergence log

Capturing where reality diverged from plan. Each entry follows:
**Assumption** (what the spec or generator-as-shipped said) →
**Symptom** (observable failure) → **Root cause** (mechanistic) →
**Detection gap** (why automated gates didn't catch it earlier).

### 2.1 PB22/PB23 fn_c transcription carry-over

- **Assumption.** Chip yaml `io_mux.fn_c:` rows for adjacent PORT B
  pads were independently transcribed from the SAMD51 datasheet's
  pin-MUX table; the slate-4 snapshot of `bsp_microchip_render`
  reflects accurate `fn_c` assignments for SERCOM1.
- **Symptom.** `bsp_microchip_render` snapshot showed two
  `// MISMATCH: ...` fallback comments in `io_mux.rs` for the two
  pads, surfacing in slate 4 (`ecb1436` /  `ec44d16` — the CHIPS-MICROCHIP-03
  Feather M4 Express snapshot test).
- **Root cause.** PB23's `fn_c:` slot had been transcribed as
  `SERCOM1_PAD2` — a row-down carry-over from PB22 — when the real
  SAMD51 pin-MUX assigns `SERCOM1_PAD3` to PB23. The
  chipdb-generator escape path emits a MISMATCH comment when the
  named signal doesn't match the consumer's requested function, so
  the bug surfaced as fallback text rather than wrong-PMUX writes.
- **Detection gap.** The snapshot test detects *changes* against the
  golden; the MISMATCH fallback was committed verbatim as the
  initial golden, deferring the real fix. Render-only gates cannot
  distinguish "intentional escape" from "transcription bug". Fixed
  in slate 6 `-01a` (`eab0581`).

### 2.2 Method-style register access against pre-method-accessor PAC

- **Assumption.** `atsamd51j19a` templates would emit modern
  svd2rust method-accessor calls — `p.MCLK.apbamask().modify(...)`,
  `p.GCLK.pchctrl(n).write(...)`, `p.PORT.group(g).pmux(h)` — matching
  the convention used by recent Espressif and Silicon Labs PACs.
- **Symptom.** Slate 6 `-04` (compile-verify gate, `4506ef7`)
  surfaced ~130 `cargo check` errors at first run against
  `atsamd51j19a 0.7.1` on `thumbv7em-none-eabihf`.
- **Root cause.** `atsamd51j19a 0.7.1` was published December 2020,
  before the svd2rust method-accessor migration. It exposes
  peripheral register blocks as direct fields: `p.MCLK.apbamask`,
  `p.GCLK.pchctrl[N]`, `p.PORT.group0.pmux[h]`, with `.modify(|_,
  w| w.field().bits(...))` chained directly on the field. Methods
  like `apbamask()` simply do not exist on this PAC vintage.
- **Detection gap.** The slate-4 render snapshot is a text diff; it
  cannot catch an ABI mismatch with a real PAC. The compile-verify
  gate (CHIPS-MICROCHIP-04) was specifically designed for this class
  of error, and it did catch it — but only after `--features
  compile-verify` was made opt-in and run manually in slate 6. Fixed
  in slate 6 `-02` (`fc88383`).

### 2.3 `w.gen_()` vs. `w.gen()` writer-field escape inconsistency

- **Assumption.** Adjacent writer-method escapes are consistent: if
  `apbXmask`'s `sercom1_` / `adc0_` writer methods carry the
  trailing `_` to escape the bare token, then `gen` (used for
  GCLK_PCHCTRL.GEN field selection) would behave the same way.
- **Symptom.** Bonus E0599 errors during slate 6 `-02` template
  iteration on the field selector for `pchctrl[N].gen()`.
- **Root cause.** `gen` is not a reserved Rust keyword in editions
  earlier than Rust 2024. `atsamd51j19a 0.7.1` is built with
  edition-default 2015 / 2018, so svd2rust did not apply the
  raw-identifier escape. The writer method is plain `gen()`. The
  `_` escape on `sercom1_` / `adc0_` is unrelated; svd2rust escapes
  those because they collide with type names, not keywords.
- **Detection gap.** Template authors guessed the escape rule by
  inspecting one peripheral's writer surface and extrapolating.
  The PAC's actual surface is authoritative; `cargo doc` on the PAC
  would have surfaced the inconsistency, but the templates were
  drafted from a small sample. Caught and fixed in the same
  slate 6 `-02` commit.

### 2.4 SERCOM USART `DATA.data()` writer typed `u32`, not `u16`

- **Assumption.** SAMD51's SERCOM_USART_INT.DATA register is 9 bits
  wide (8 data bits + parity), so the writer field would accept
  `u16` (or `u8` zero-extended). Slate 12 `-06b` template emitted
  the load as `.data(byte as u16)`.
- **Symptom.** E0308 at compile-verify with
  `expected u32, found u16` on the `.data(...)` call site.
- **Root cause.** `atsamd51j19a 0.7.1` SERCOM USART DATA writer
  field is typed `u32` on this svd2rust vintage — the PAC widens
  small register fields to the surrounding u32 word for writer
  ergonomics. Datasheet width is informational; PAC ABI is
  authoritative.
- **Detection gap.** Template author defaulted to the datasheet
  field width. Caught in slate 12 `-06b` (`dbeba8c`); fix was a
  one-character template edit (`u16` → `u32`).

### 2.5 PA04 / PA06 fn_b missing ADC0 entries

- **Assumption.** SAMD51 PORT A pads PA04 and PA06 do not need
  ADC0 entries in chip yaml `io_mux.fn_b:` for the Feather M4
  Express, because the Feather doesn't expose them as analog
  inputs in the v0 example.
- **Symptom.** Two further `MISMATCH` fallback comments persisted
  in the `bsp_microchip_render` snapshot golden after slate 6
  `-01a` resolved PB22/PB23. Originally captured as Coupled at
  slate 6.
- **Root cause.** Chip yaml `io_mux.fn_b:` column for PA04 and
  PA06 was empty where the SAMD51 datasheet assigns `ADC0_AIN4`
  and `ADC0_AIN6` respectively. The chipdb-generator escape path
  emitted MISMATCH fallbacks for the same reason as §2.1.
- **Detection gap.** Same as §2.1 — snapshot tests cannot
  distinguish intentional escape from missing data. Reclassified
  Safe and fixed in slate 13 `-01b` (`84d530a`) once the broader
  asymmetric-fn-column pattern was understood.

### 2.6 Generator `pac.rs.jinja` double-nest re-export

- **Assumption.** The generator's `pac.rs.jinja` template wraps
  the PAC crate behind a `pub use atsamd51j19a as pac;`
  re-export so consumers can `use crate::bsp::pac;` without
  knowing the upstream crate name.
- **Symptom.** Consumer-site references to peripheral types
  resolved through `crate::bsp::pac::pac::PERIPHERAL` — a
  doubled `pac::pac::` path — surfaced in slate 13 `-07`
  (`3b94d13`).
- **Root cause.** The template emitted both a module-shaped
  `pub mod pac { pub use atsamd51j19a::*; }` and a flat
  `pub use atsamd51j19a as pac;` block, producing a
  resolvable-but-confusing double-nest. The TI and SILABS
  templates had already converged on the flat re-export shape
  earlier in slate 13.
- **Detection gap.** Compile-verify accepts the double-nest
  (it resolves), so the gate stayed green. The shape
  divergence only surfaced when the consumer-path flatten
  sweep audited all three vendors side-by-side. Fixed in
  slate 13 `-07` (`3b94d13`).

## 3. Refactor points

Decision inflection nodes where the initiative changed direction.

### 3.1 Slate 6 `-01a` + `-02` commit split

- **Trigger.** Slate 4 (`ecb1436` / `ec44d16`) had committed the
  golden snapshot with `MISMATCH` fallback comments verbatim for
  PB22 / PB23 / PA04 / PA06. Slate 6 simultaneously needed to fix
  the PB22/PB23 yaml AND switch templates to field-style register
  access. A combined commit would have produced an enormous
  snapshot diff entangling two unrelated concerns.
- **Alternatives.** (a) Single commit, hope reviewers can untangle
  the snapshot diff. (b) Split into two commits with a
  temporary stash of templates-and-snapshots between them.
  (c) Defer one concern entirely to a later slate.
- **Selection rationale.** Option (b). Each commit's snapshot diff
  needed to be minimal so future audits could attribute snapshot
  changes to a single root cause. The PA04/PA06 MISMATCH residuals
  were deliberately left in the golden after `-01a` and explicitly
  documented as Coupled in the §15 change log, deferring them to
  slate 13 `-01b` once the asymmetric-fn-column pattern was
  understood.
- **Cost of switch.** One additional commit (`eab0581` before
  `fc88383`); zero loss in CI signal because each commit ran
  pre-publish independently.

### 3.2 Slate 9 `-05` linker emission shape with empty body

- **Trigger.** CHIPS-MICROCHIP-00 §11 specified an
  `atsamd51j19a.x` per-chip linker include as a parallel to the
  TI `cc1352_r.x` and SILABS `efm32_gg11.x` slots, but the
  `atsamd51j19a` PAC's `build.rs` already provides a `device.x`
  fragment that `cortex-m-rt`'s `link.x` INCLUDEs unconditionally.
- **Alternatives.** (a) Skip the per-chip include slot entirely
  for MICROCHIP — diverge from TI / SILABS shape. (b) Emit a
  per-chip include that re-INCLUDEs `device.x` — duplicates the
  PAC's build-script contract. (c) Emit a per-chip include with
  an intentionally empty body, documenting that the runtime
  content arrives via the PAC build script.
- **Selection rationale.** Option (c). Establishing the slot now
  means future amendments (custom interrupt vectors, board-level
  symbol overrides) target a real file path that consumers
  already reference. Skipping the slot would force a
  shape-divergence amendment if the slot ever became needed.
- **Cost of switch.** Documented explicitly in §10.2 of
  `CHIPS-MICROCHIP-05-LINKER.md` to prevent future agents from
  re-deriving the question. Commit pair `bb209ea` (ratify) +
  `65d16ed` (emit).

## 4. Mitigation patterns

Abstract the fixes into reusable units. Each pattern is a "When X +
Y, apply Z" rule intended to short-circuit future re-discovery.

1. **Yaml transcription carry-over audit.** When chip yaml
   transcription appears asymmetric across consecutive pads
   (PB22 / PB23, PA04 / PA06, etc.), audit for adjacent-row
   carry-over bugs before trusting the snapshot golden.
   Transcription typos cluster in adjacent rows because authors
   copy-paste-and-edit pad rows in batches.
2. **Pre-2024 PAC keyword escape audit.** When PAC writer field
   names look unfamiliar relative to recent PACs (e.g. `w.gen_()`
   where a 2024-edition PAC would emit `w.r#gen()`), check the
   PAC's Rust edition. Pre-2024 svd2rust does not escape modern
   reserved keywords, so identifier shapes change at the
   edition boundary. `cargo doc` on the PAC is the canonical
   resolution.
3. **PAC writer field width audit.** When DATA-style registers
   are sized larger than the bit-width the chip nominally
   supports (SAMD51 SERCOM USART DATA is 9 bits but the writer
   takes `u32`), check the PAC's actual writer field type
   before trusting the datasheet width. Datasheet width is
   informational; PAC ABI is authoritative.
4. **Empty-body linker slot.** When emitting a linker fragment
   slot whose runtime content is provided by the PAC's build
   script (e.g. atsamd51's `device.x` via `cortex-m-rt`), an
   empty body is acceptable. Document the layering explicitly
   in §10 of the linker concepts doc so future amendments
   target the slot rather than re-deriving whether it should
   exist.
5. **Multi-vendor consumer-path audit.** When two or more
   sibling vendor generators converge on a flat re-export shape,
   sweep the remaining vendor for the older nested shape in
   the same slate. Compile-verify cannot catch double-nest
   re-exports because they resolve; only shape audits do.

## 5. Deferred work reclassification

Classifying deferred items rather than leaving them as a flat list.

### Safe (orthogonal, no impact on core invariants)

- **CHIPS-MICROCHIP-05 linker emission** — shipped (slate 9,
  `bb209ea` + `65d16ed`).
- **CHIPS-MICROCHIP-06a PA23 LED blink** — shipped (slate 11,
  `a32c874`).
- **CHIPS-MICROCHIP-06b SERCOM5 UART hello-world** — shipped
  (slate 12, `dbeba8c`).
- **PA04 / PA06 ADC0 fn_b MISMATCH residual** — was tracked as
  Coupled at slate 6 `-01a` pending the broader
  asymmetric-fn-column-pattern analysis; reclassified Safe and
  shipped in slate 13 `-01b` (`84d530a`).

### Coupled (affects assumptions; reopen requires named context)

- **CHIPS-MICROCHIP-06c rlvgl widget tree.** Coupled to the
  assumption that the Feather M4 Express has a built-in display.
  It does not — the Feather M4 is a 4 KB-flash microcontroller
  board with an LED and a USB-serial interface, no on-board
  display. Reopens if the example grows an external display
  companion module (SSD1306 over SERCOM I2C, ILI9341 over SPI,
  etc.) — that's a new chapter, not a deferred slice of `-06`.

### Abandoned

- None. No MICROCHIP-scoped phases were killed during the
  initiative.

## 6. Forward constraints

Normative section. Future MICROCHIP and adjacent CHIPS-* work
treats these as binding rules.

1. **Yaml-row transcription cross-check.** Future MICROCHIP chip
   yaml additions MUST cross-check adjacent-pad `fn_a` / `fn_b`
   / `fn_c` columns for transcription carry-over before
   committing. If two adjacent rows share a `fn_X:` value, the
   author MUST verify against the SAMD51 (or equivalent) pin-MUX
   table that the duplication is real and not a copy-paste
   carry-over.
2. **PAC version bump re-runs compile-verify.** Future MICROCHIP
   PAC version bumps (e.g. a re-published `atsamd51j19a` with
   modern method-accessor surface) MUST re-run the
   `bsp_microchip_compile` gate end-to-end before treating any
   template ABI assumption as stable. December-2020-era and
   modern svd2rust differ at multiple shapes (method vs. field
   accessors, keyword escapes, writer-field widths).
3. **SERCOM mode-union convention.** Future SERCOM peripheral
   integrations MUST verify the `usart_int()` / `i2c_master()`
   / `spi_master()` mode-union method-accessor convention at
   consumer sites. These are mode-union *methods* on the SERCOM
   register block, NOT direct fields — getting this wrong
   yields E0599 "no field" errors at compile-verify, not
   wrong-runtime-behaviour. The MICROCHIP template baseline
   established by slate 12 `-06b` (`dbeba8c`) is the
   reference.
4. **Snapshot MISMATCH as deferred bug, not intentional shape.**
   Any future `MISMATCH` fallback comment in a chipdb-rendered
   snapshot MUST be tracked as a Coupled deferred item with a
   named root-cause hypothesis in §15 of the relevant chapter.
   Committing a snapshot with a MISMATCH and forgetting it is
   the failure mode that produced §2.1 and §2.5.

## 7. Provenance hooks

Each divergence and refactor point linked to authoritative
artifacts (commit SHA, sub-letter doc, §15 amendment).

### Divergence-log provenance

- **§2.1 PB22/PB23 fn_c carry-over.** Surfaced by slate 4
  (`ecb1436` / `ec44d16` — CHIPS-MICROCHIP-03 snapshot tests).
  Fixed by `eab0581` CHIPS-MICROCHIP-01a (PB22/PB23 PMUX fn_c).
  Documented in `CHIPS-MICROCHIP-00-CONCEPTS.md` §15 change log
  entry for 2026-05-13.
- **§2.2 Method-style register access vs. pre-method PAC.**
  Surfaced by `4506ef7` CHIPS-MICROCHIP-04 (Feather M4 Express
  compile-verify test). Fixed by `fc88383` CHIPS-MICROCHIP-02
  (field-style register access for `atsamd51j19a 0.7.1`).
- **§2.3 `w.gen_()` vs. `w.gen()` escape inconsistency.**
  Bonus discovery during the slate 6 `-02` iteration; same
  commit (`fc88383`) carries the fix.
- **§2.4 SERCOM USART `DATA.data()` u32 vs. u16.** Surfaced
  and fixed in `dbeba8c` CHIPS-MICROCHIP-06b (send hello over
  Feather SERCOM USART).
- **§2.5 PA04 / PA06 fn_b missing ADC0 entries.** Surfaced
  alongside §2.1 in slate 4; deferred-Coupled through slate
  6 `-01a`; fixed in `84d530a` CHIPS-MICROCHIP-01b
  (ADC0_AIN4 / ADC0_AIN6 in `ATSAMD51J19A` fn_b column).
- **§2.6 `pac.rs.jinja` double-nest.** Surfaced and fixed in
  `3b94d13` CHIPS-MICROCHIP-07 (flatten pac.rs re-export for
  clean consumer path) during the slate-13 cross-vendor
  consumer-path flatten.

### Refactor-point provenance

- **§3.1 Slate 6 `-01a` + `-02` split.** Commit pair
  `eab0581` (CHIPS-MICROCHIP-01a, PB22/PB23 PMUX fn_c) then
  `fc88383` (CHIPS-MICROCHIP-02, field-style register access).
  Slate-6 cross-vendor summary in `9908af1` (CHIPS-*-05 slate
  6 amendments).
- **§3.2 Slate 9 `-05` empty-body linker slot.** Ratification
  doc `bb209ea` (CHIPS-MICROCHIP-05, ratify linker emission
  chapter); emission `65d16ed` (CHIPS-MICROCHIP-05a, emit
  atsamd51j19a.x linker include). Layering documented in
  `CHIPS-MICROCHIP-05-LINKER.md` §10.2.

### Example-crate provenance

- `a53d7ec` CHIPS-MICROCHIP-06 (ratify example crate chapter).
- `8170ee9` CHIPS-MICROCHIP-06a (scaffold Feather M4 Express
  bsp_pac example crate).
- `a32c874` CHIPS-MICROCHIP-06a (drive PA23 LED in busy-wait
  toggle loop).
- `dbeba8c` CHIPS-MICROCHIP-06b (send hello over Feather
  SERCOM USART).

### External evidence

- `atsamd51j19a 0.7.1` on crates.io (published December 2020,
  pre-method-accessor svd2rust era).
- SAMD51 datasheet pin-MUX table for PORT B pads PB22 / PB23
  (`SERCOM1_PAD2` / `SERCOM1_PAD3` on `fn_c`) and PORT A pads
  PA04 / PA06 (`ADC0_AIN4` / `ADC0_AIN6` on `fn_b`).

## 8. Change log

- **2026-05-15** — Initial draft. Captures the CHIPS-MICROCHIP
  initiative from CHIPS-MICROCHIP-00 ratification through slate
  13 `-01b` / `-07` cleanups. All thirteen named phases either
  shipped or closed-with-deferral; `bsp_microchip_compile`
  compile-verify gate green; snapshot free of `MISMATCH`
  fallback comments. Single Coupled deferral
  (CHIPS-MICROCHIP-06c rlvgl widget tree) tied to the
  no-on-board-display assumption.
