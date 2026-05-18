# CHIPS-TI Retrospective — divergences, refactor points, forward constraints

**Status:** Drafted 2026-05-15. Initiative-completion retrospective
for the CHIPS-TI initiative on rlvgl `v0.2.0`. Not a chronicle and
not a celebration — a delta against the original
[`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md) ratification,
organised for future chipdb-vendor initiatives (CHIPS-NXP,
CHIPS-RENESAS-02, CHIPS-NRF-02, future MSPM0 / AM335x revival) to
consume.

Retrospective in the agile sense: surfaces what diverged from
plan, what gates worked / didn't work, what patterns to carry
forward, and what preconditions future chipdb-vendor initiatives
must satisfy. The initiative-retrospective convention is
documented in `CLAUDE.md` "Spec-Before-Code Planning Discipline →
Initiative retrospective"; one retrospective per multi-phase
initiative, co-located with the phase docs at
`<initiative-dir>/<INIT>-RETROSPECTIVE.md`.

This doc is a **historical artifact** with one normative section
(§6 forward constraints). Behaviour PRs reference
[`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md),
[`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md), and
[`CHIPS-TI-06-EXAMPLE.md`](CHIPS-TI-06-EXAMPLE.md) directly;
this retrospective is the bridge between *what we shipped* and
*what to do differently next time*.

## 1. Outcome snapshot

### Final architecture

`chipdb/rlvgl-chips-ti/` ships a structurally complete vendor-BSP
pipeline. The 8-file emission set
([`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md) §1) per board:

```text
mod.rs           (host crate-shaped module index)
pac.rs           (PAC re-export — flat after CHIPS-TI-07)
clocks.rs        (PRCM ungate + reset-release)
io_mux.rs        (IOC per-pin PORT_CFG writes)
peripherals.rs   (UART0 console + I2C0 master real init)
board.rs         (XTAL_HZ / APB_HZ constants)
memory.x         (FLASH + SRAM regions)
cc1352_r.x       (CCFG section directive at last-Flash-sector offset)
```

Four active vendor templates under
`src/bin/creator/bsp/ti/templates/`. `TiIr` adapter parses chip +
board YAML and feeds MiniJinja. Snapshot regression
([`tests/bsp_ti_cc1352r_render.rs`](../../../tests/bsp_ti_cc1352r_render.rs))
asserts 8-file emission. Compile-verify gate
([`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs))
materialises a throwaway cargo project around the emitted BSP and
runs `cargo check --target thumbv7em-none-eabihf` against
`cc13x2_26x2_pac 0.10.3`; **PASSES end-to-end as of CHIPS-TI-01e
(2026-05-13)**.

Example crate `examples/launchxl-cc1352r1/` consumes the slate-9
generator output and demonstrates an LED blink on DIO_6 (LED_RED)
plus `"hello\r\n"` over UART0 (XDS110 VCOM bridge). Detached from
workspace via empty `[workspace]` stanza; checked via
`--manifest-path`.

13 slates landed between 2026-05-12 and 2026-05-15: ratification
(-00), chip-YAML population (-01a), template port (-01b), snapshot
(-01c), compile-verify (-01d), structural amendments (-01b
lowercase + -01e three structural), `TiIr` adapter (-02), linker
ratification (-05), CCFG comment fix (-05a), example crate
ratification + scaffold + LED + UART (-06 / -06a / -06b), and
generator pac.rs flatten (-07).

### Deferred items (explicit)

1. **CHIPS-TI-06c** — rlvgl widget tree integration on the
   LAUNCHXL example crate. Closed-with-deferral. The LAUNCHXL-
   CC1352R1 carries **no on-board display** (no LCD, no OLED, no
   external panel header); the slate-7 PAC is also pre-SPI/I2C-DMA
   so a software-bit-banged off-board display path would be the
   only option. Reopen requires adding an off-board display module
   (e.g. SSD1306 over I2C, or ST7789 over SPI bit-bang) and
   constitutes a separate `examples/`-side scoping exercise.
   Reopen trigger documented in
   [`CHIPS-TI-06-EXAMPLE.md`](CHIPS-TI-06-EXAMPLE.md) §11 / §14.
2. **CHIPS-TI-02-and-later additional chips within §5.5** —
   CC13x0, CC26x0, CC26x2, CC32xx. Closed-with-deferral. Shape
   ratified by CHIPS-TI-01; reopen requires per-chip YAML
   population, snapshot test, and compile-verify gate addition.
   Reopen trigger named in
   [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md) §14.

### Known residual risks

- **`cc13x2_26x2_pac 0.10.3` is the pinned compile-verify
  target.** The slate-7 `-01b` (lowercase) and `-01e` (three
  structural) amendments were calibrated specifically against PAC
  0.10.3's pre-uppercase-accessor svd2rust-era output. The SVD
  source (`seanmlyons22/ti-lprf-pacs`) is community-maintained,
  not vendor-blessed; an upstream re-publish at PAC 0.11 that
  regenerates against modern svd2rust (uppercase peripherals +
  method accessors + indexed `iocfg(n)`) would invalidate the
  amendments in lockstep. Mitigation: PAC pin in
  `examples/launchxl-cc1352r1/Cargo.toml` is exact-equality
  (`= "0.10.3"`), not a `^0.10` range. Future agents touching the
  TI templates MUST first audit whether the PAC pin still matches
  the emitted accessor shape.
- **Chip yaml carries latent PAC-version-specific data.** The
  CHIPS-TI-01e amendment introduced a `clk_en_variant` chip-yaml
  field that names PAC enum variants directly (`Uart0`, `Uart1`,
  `Ssi0`, etc.). This binds the chip yaml to one PAC vintage's
  enum naming. Adding a sibling chip whose PAC encodes
  clock-gates differently requires per-chip data, not template
  rework.
- **`launchxl_cc1352r1` is the sole compile-verified board.**
  The §5.5 SimpleLink Cortex-M4F member set
  `{CC13x0, CC13x2, CC26x0, CC26x2, CC32xx}` ratified five
  families. Only CC1352R1F3RGZ (CC13x2) is exercised end-to-end.
  Sibling chips would surface their own PAC-vintage / register-
  shape divergences on first compile-verify attempt.

## 2. Divergence log

Capturing where reality diverged from the original
[`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
ratification. Each entry as **Assumption** (what the spec said)
→ **Symptom** (observable failure) → **Root cause**
(mechanistic) → **Detection gap** (why automated gates didn't
catch it).

### 2.1. Templates assumed modern svd2rust uppercase peripheral access

- **Assumption.**
  [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md) §3 and
  §5 froze "Peripheral instance access style" as
  `p.PERIPHERAL_NAME.register_name()` — uppercase peripheral
  field on the `p` instance, method accessor on the register.
  Inherited verbatim from the Espressif precedent.
- **Symptom.** First-pass `bsp_ti_cc1352r_compile` run (slate
  CHIPS-TI-01d) surfaced ~40+ `error[E0609]: no field 'PRCM' on
  type 'Peripherals'` / `'IOC'` / `'GPIO'` / `'UART0'` errors
  against `cc13x2_26x2_pac 0.10.3`.
- **Root cause.** `cc13x2_26x2_pac 0.10.3` was generated by a
  pre-uppercase-accessor svd2rust release. Peripheral fields are
  lowercase (`p.prcm`, `p.ioc`, `p.gpio`, `p.uart0`); the
  `Peripherals` *type* stays capitalised. The Espressif PAC
  ecosystem at version 0.31 is generated by a newer svd2rust and
  uses uppercase. The spec ratified one ecosystem's convention as
  cross-vendor without auditing the per-vendor PAC vintage.
- **Detection gap.** Snapshot tests (CHIPS-TI-01c) check
  generated *text* only — uppercase emission renders, snapshots
  match, no failure surfaced. The compile-verify gate (CHIPS-TI-
  01d) was the first layer that exercised actual PAC
  type-checking. Fixed via CHIPS-TI-01b template amendment
  (slate 7) lowercasing peripheral field access in
  `clocks.rs.jinja` / `io_mux.rs.jinja` / `peripherals.rs.jinja`;
  the `Peripherals` type stays capitalised because PAC type
  exports are version-stable.

### 2.2. Templates assumed `iocfg(n)` indexer API

- **Assumption.** `io_mux.rs.jinja` (slate 6 -01b initial port)
  emitted `p.ioc.iocfg({{ pin.dio }})` — indexer call form.
  Inherited from the Espressif `io_mux.gpio({{ pin.gpio }})`
  pattern.
- **Symptom.** Post-lowercase amendment (slate 7 -01b),
  compile-verify dropped from 40+ casing errors to 18 remaining
  errors. Top of the remaining list:
  `error[E0599]: no function or associated item named 'iocfg'
  found for struct 'Ioc'`.
- **Root cause.** `cc13x2_26x2_pac::ioc::Ioc` exposes 32
  per-DIO methods (`iocfg0()`, `iocfg1()`, ..., `iocfg31()`),
  with no `iocfg(n)` indexer. This matches the pre-indexer-API
  svd2rust output era the PAC was generated under. Modern
  svd2rust would emit the indexer; PAC 0.10 vintage does not.
- **Detection gap.** Same as §2.1 — snapshot only checks text;
  compile-verify is the first gate that exercises PAC method
  resolution. Fixed in slate 7 -01e fix 1: `io_mux.rs.jinja`
  emits `p.ioc.iocfg{{ pin.dio }}()` (concatenated method name).

### 2.3. Templates assumed `clk_en` is a single bit

- **Assumption.** `clocks.rs.jinja` (slate 6 -01b initial port)
  emitted `w.{{ field }}().set_bit()` for every PRCM clock-gate
  write. Inherited from the Espressif pattern where every
  clock-enable is a single bit.
- **Symptom.** After lowercase + iocfg fixes, compile-verify
  still failed:
  `error[E0599]: no method named 'set_bit' found for struct
  'cc13x2_26x2_pac::prcm::uartclkgr::CLK_EN_W<...>'`.
- **Root cause.** `cc13x2_26x2_pac::prcm::uartclkgr::CLK_EN_W`
  is a **2-bit enum FieldWriter** with variants
  `ClkEn::Uart0` / `ClkEn::Uart1`, not a `BitWriter`. Same
  pattern for `ssiclkgr.clk_en` (`Ssi0`/`Ssi1`) and
  `gptclkgr.clk_en` (`Gpt0`..`Gpt3`). The CC13x2 PRCM packs
  multi-instance clock-gates into a single 2-bit field per
  peripheral class; the Espressif single-bit-per-peripheral
  model doesn't fit.
- **Detection gap.** Snapshot doesn't model PAC method
  resolution. The spec did not require a per-chip PAC API
  audit before template port. Fixed in slate 7 -01e fix 2:
  `clocks.rs.jinja` now branches on a new optional chip-yaml
  field `clk_en_variant`. When present (e.g.
  `clk_en_variant: uart0`) the template emits
  `w.clk_en().uart0()`; when absent it falls back to
  `.set_bit()` for `BitWriter` fields. `TiPrcmGate` in
  `src/bin/creator/bsp/ti/ir.rs` gained the corresponding
  `Option<String>` field.

### 2.4. Chip yaml named PRCM reset fields generically

- **Assumption.** Initial CC1352R.yaml `prcm:` block (slate 6
  -01b) named reset-register fields with generic peripheral
  names: `resetuart.uart`, `reseti2c.i2c`. Template wrote
  `w.{{ rst_field }}().set_bit()` against those names.
- **Symptom.** Compile-verify failure:
  `error[E0609]: no field 'uart' on type
  'cc13x2_26x2_pac::prcm::resetuart::W'`.
- **Root cause.** `cc13x2_26x2_pac::prcm::resetuart::W`
  exposes per-instance bits as `.uart0()` / `.uart1()`, not
  the generic `.uart()`. The PAC follows TI SWCU185 UART
  *instance* numbering (UART0, UART1) in its field names,
  not the *peripheral class* name. Generic class names are
  authored from RM section headings (`§14.1 UART`); PAC
  field names come from SVD register-field descriptions
  which name the actual bit position.
- **Detection gap.** No automated gate cross-checks chip
  yaml field names against PAC field names. Fixed in slate 7
  -01e fix 3: chip yaml `resetuart.rst_field` corrected to
  `uart0`; `reseti2c.rst_field` to `i2c0`.

### 2.5. Chip yaml `resetaudio` / `resetsec` were latent typos

- **Assumption.** Initial CC1352R.yaml `prcm:` block named two
  reset registers `resetaudio` and `resetsec`. The names looked
  plausible against TI SWCU185 §14 register summaries.
- **Symptom.** None at compile-verify time — neither register
  was touched by the `launchxl_cc1352r1` board's pin / console
  / I2C config, so neither emitted in `clocks.rs`. The bugs
  were latent.
- **Root cause.** `cc13x2_26x2_pac::prcm` has no `resetaudio`
  or `resetsec` registers. The actual names are `reseti2s` (I2S
  audio is reset via the I2S-specific register, not an
  "audio" alias) and `resetsecdma` (the SecDMA / crypto / TRNG /
  PKA cluster shares a single reset register). The chip yaml
  was authored from a misread of the RM register summary —
  "audio" and "sec" were the TRM's prose chapter names, not
  register identifiers.
- **Detection gap.** Bonus discovery during the slate-7 -01e
  amendment pass. The active fix for §2.4 (`resetuart` /
  `reseti2c` field renames) prompted a sweep of the entire
  `prcm:` block; the sweep caught `resetaudio` / `resetsec` by
  cross-checking every register name against the PAC's
  `prcm::*` module names. Had the §2.4 fix been scoped to only
  the two registers it directly touched, these two would have
  remained latent until a sibling board enabled SSI / I2S /
  crypto / DMA. Fixed in the same slate 7 -01e commit, but
  worth a separate divergence entry because the **fix scope**
  was the bug-finding mechanism — not the symptom-driven
  triage.

### 2.6. CCFG yaml comment conflated structure size with sector size

- **Assumption.** CC1352R.yaml linker-section comment
  asserted the CCFG occupies "4 KB" of Flash. 4 KB is the
  CC13x2 Flash sector size (SWCU185G §11.1).
- **Symptom.** No emitted-value change at any layer.
  Generator emission, snapshots, compile-verify all pass.
  Cosmetic only.
- **Root cause.** Misreading of SWCU185G §11.1 Table 11-1.
  The CCFG **structure** is 88 bytes (`ccfg_length: 0x58`,
  per the emitted value). It resides within the last 4 KB
  Flash sector but does not span it; placing other data in
  the same sector below the CCFG offset is valid. The
  emitted `ccfg_length` value was correct; only the prose
  comment was wrong.
- **Detection gap.** Comments are not type-checked. The
  divergence was caught during a documentation re-read pass
  in slate 13 (CHIPS-TI-05a). Spec-discipline-significant
  because the BBB / DCB precedent is "every authoritative
  reference cite must be verifiable against the named TRM
  section"; a misread comment in a `db/chips/*.yaml` file
  is structurally identical to a misread comment in a
  `docs/*-NN-*.md` file.

### 2.7. Generator `pac.rs.jinja` emitted double-nested re-export

- **Assumption.** `pac.rs.jinja` (initial slate 2 emission)
  used `pub use cc13x2_26x2_pac as pac;` — alias-style
  re-export. The alias was intended to give consumer code a
  short module name regardless of the PAC crate's actual name.
- **Symptom.** Consumer code in
  `examples/launchxl-cc1352r1/src/bsp_pac_main.rs` (slate 11
  CHIPS-TI-06a) needed `bsp_generated::launchxl_cc1352r1::pac
  ::pac::Peripherals` — **double `pac::`** segment. The first
  `pac` is the BSP's `pac.rs` module; the second is the alias
  inside that module pointing at the PAC crate.
- **Root cause.** The alias-style re-export creates a nested
  namespace: `bsp::pac` is the module, `bsp::pac::pac` is the
  re-aliased crate, and `Peripherals` lives at the crate root.
  A glob re-export (`pub use cc13x2_26x2_pac::*;`) would have
  hoisted `Peripherals` into the BSP's `pac` module directly,
  giving a single-segment consumer path.
- **Detection gap.** Snapshot tests check the *text* of
  `pac.rs`; they do not exercise the path consumers would
  walk through it. Slate 11 (CHIPS-TI-06a) initially worked
  around this by importing `cc13x2_26x2_pac` directly at the
  binary scope, bypassing the BSP's `pac.rs`. Slate 13
  (CHIPS-TI-07) fixed the template emission and unwound the
  workaround in the example crate.

## 3. Refactor points

Decision inflection nodes where the initiative changed
direction. Each entry: **Trigger** (what forced the pivot) →
**Alternatives** (what was considered) → **Selection**
(constraint-driven rationale) → **Cost of switch** (what was
paid).

### 3.1. Slate 7 -01e divergence handling — data-side vs. template-conditional

- **Trigger.** §2.3 surfaced that `clk_en` is a 2-bit enum
  FieldWriter on some PRCM registers and a single-bit
  `BitWriter` on others within the same PAC. Both shapes need
  template support.
- **Alternatives.** (A) Per-template conditional: hard-code
  the enum-vs-bit matrix in `clocks.rs.jinja` keyed on
  register name. (B) Chip-yaml extension: introduce
  `clk_en_variant` as an optional chip-yaml field; emit the
  enum variant call when present, fall back to `.set_bit()`
  when absent. (C) Per-chip template fork: generate one
  `clocks.rs.jinja` per chip family.
- **Selection.** Option B. The enum-vs-bit matrix is genuine
  *data* — it varies per PAC vintage (slate-7 PAC 0.10
  packs instances, a hypothetical PAC 0.11 might unpack
  them) and per chip member (CC2640R may differ from
  CC1352R). Encoding the matrix in chip yaml lets sibling
  chips carry their own variant set without template churn.
- **Cost.** New optional chip-yaml field (`clk_en_variant`)
  added; corresponding `Option<String>` field on
  `TiPrcmGate` in `src/bin/creator/bsp/ti/ir.rs`; one
  per-row update in CC1352R.yaml's `prcm:` block (`uart0` /
  `i2c0` / etc. now carry their enum variant name).
  Snapshot test re-blessed; `clocks.rs` snapshot diff is
  the only behavioural change.

### 3.2. Slate 11 main.rs PAC import location — workaround vs. fix

- **Trigger.** §2.7 surfaced that the slate-2 `pac.rs.jinja`
  alias-style re-export forced consumer code through a
  double-segment path (`bsp::pac::pac::Peripherals`).
- **Alternatives.** (A) Import the PAC crate directly at
  binary scope: `use cc13x2_26x2_pac as pac;`. (B) Fix the
  generator: switch to `pub use cc13x2_26x2_pac::*;`. (C)
  Add a re-export layer in the BSP's `mod.rs`.
- **Selection.** Initially (slate 11 -06a) chose Option A
  as a workaround — the slate's scope was "ship LED blink",
  not "fix generator". Slate 13 (-07) unwound the
  workaround in favour of Option B. Option C was rejected
  because it would have duplicated the BSP's module
  surface and confused consumers about which `pac` is
  authoritative.
- **Cost.** Two slate-touch sequence: -06a accepted
  technical debt to unblock LED blink; -07 amortised the
  debt by fixing the generator template. The interim
  workaround was load-bearing (LED blink shipped), but it
  left a comment in -06a's §15 entry that needed
  unwinding once -07 landed. Future similar choices
  SHOULD prefer fixing the generator first when the
  workaround would leave a documented debt entry.

## 4. Mitigation patterns (portable)

Abstracted from the divergences and refactor points. Reusable
units for future chipdb-vendor initiatives.

### 4.1. "When a chipdb-amendment slate touches register N, audit all chip yaml entries referencing the same register block"

**When**: a structural amendment slate (renaming a field,
correcting a field, changing the access shape of a register)
fixes one specific register but the chip yaml contains
sibling registers in the same block authored from the same
source.

**Apply**: extend the slate's scope to a full sweep of the
register block. Cross-check every register name in the
chip yaml against the PAC's module surface. Cross-check
every field name against the PAC's writer surface.

**Encode as**: the slate 7 -01e amendment, scoped to fix
the `resetuart` / `reseti2c` field-name divergence (§2.4),
found two latent bugs `resetaudio` / `resetsec` (§2.5) by
adopting this pattern. Future structural-amendment slates
in any CHIPS-VENDOR-NN initiative SHOULD adopt the
"block-sweep" rule explicitly in their slate scope.

### 4.2. "Prefer `pub use <crate>::*;` over `pub use <crate> as <alias>;` in re-export modules"

**When**: a generator emits a `pac.rs` (or similar) module
that re-exports the entire surface of an upstream PAC
crate.

**Apply**: glob re-export (`pub use <crate>::*;`) hoists
the crate's items into the wrapping module's namespace at
a single segment. Alias re-export
(`pub use <crate> as <alias>;`) creates a nested namespace
that double-segments consumer paths.

**Encode as**: the slate 13 -07 generator fix unwound the
slate-2 alias-style emission. Future chipdb-vendor
templates emitting a `pac.rs` (Microchip, Silicon Labs,
NXP, etc.) MUST follow the glob-re-export pattern. Cross-
vendor consistency in consumer paths is a deliberate goal;
double-segment paths surface as ergonomic friction in
example crates.

### 4.3. "When chip PAC is pre-method-accessor svd2rust era, template should emit field-style access"

**When**: a chipdb-vendor template is being ported from
the Espressif precedent (modern svd2rust, uppercase +
method accessors) to a PAC crate generated by an older
svd2rust version.

**Apply**: PAC vintage audit BEFORE template port. Read
`docs.rs` for the PAC at the pinned version and verify:
(a) peripheral field-access casing (uppercase vs.
lowercase); (b) register accessor shape (method `.reg()`
vs. field `.reg`); (c) per-instance method names
(`.iocfg0()` vs. indexer `.iocfg(n)`); (d) FieldWriter
shape (single-bit `BitWriter` vs. enum FieldWriter with
variants). Emit accordingly.

**Encode as**: this pattern is the union of the slate 7
-01b lowercase fix and the slate 6 / slate 12
CHIPS-MICROCHIP-02 field-style port. Both addressed the
same root cause — newer-svd2rust assumptions baked into
templates ported against older-svd2rust PACs. Future
CHIPS-VENDOR-01b template-port slates MUST land a PAC
audit in the same slate or block on one.

### 4.4. "Snapshot-only is insufficient; compile-verify is the first real gate"

**When**: a chipdb-vendor template port is judged "done"
on the basis of snapshot tests passing.

**Apply**: treat snapshot tests as text-level only. They
catch *intended* output regressions, not *type-correct*
output. The compile-verify gate is the first layer that
exercises actual PAC type-checking, method resolution,
field-access shape, and FieldWriter enum variants. Block
acceptance on compile-verify, not on snapshot-only.

**Encode as**: every divergence in §2.1 / §2.2 / §2.3 /
§2.4 above shipped through snapshot-passing emission and
surfaced first at compile-verify. Pre-publish bullets for
new chipdb-vendor compile-verify tests SHOULD start
commented out (expected-fail) and uncomment only once the
gate passes end-to-end. Snapshot-pass alone is not
publication-grade.

## 5. Deferred work reclassification

Per the framework: **Safe** (orthogonal, no impact on core
invariants), **Coupled** (affects assumptions; must be
revisited with context), **Abandoned** (explicitly killed).

### 5.1. Coupled: CHIPS-TI-06c — rlvgl widget tree integration

- **Coupled to**: "vendor has a display." LAUNCHXL-CC1352R1
  has no on-board display (no LCD, no OLED, no external
  panel header per TI SWRU527). Resurfacing this deferral
  requires adding an off-board display module (SSD1306 over
  I2C, ST7789 over SPI bit-bang, etc.) to the example
  crate's bill of materials.
- **Revisit context**: a SimpleLink Cortex-M4F LaunchPad
  with on-board display ships, OR an off-board display
  module is named in the example crate's hardware list with
  driver wiring documented, OR the example crate target
  rotates to a different LaunchPad SKU. The current target
  (LAUNCHXL-CC1352R1) is not a display-capable platform on
  its own.
- **Reopen ID**: CHIPS-TI-06c.

### 5.2. Coupled: CHIPS-TI-02 and later — additional §5.5 chips

- **Coupled to**: per-chip PAC vintage. Each sibling chip
  (CC13x0 via `cc26x0`, CC26x0 via `cc26x0`, CC26x2 via
  `cc26x2`, CC32xx via `cc3200` or successor) has its own
  PAC crate with its own svd2rust vintage. The slate 7
  -01b / -01e amendments are calibrated against
  `cc13x2_26x2_pac 0.10.3` specifically; sibling chips
  surface their own field-access / register-shape
  divergences on first compile-verify attempt.
- **Revisit context**: a CHIPS-TI-NN slate proposes
  bringing up a new §5.5 member. The slate MUST repeat the
  PAC vintage audit (per §4.3 mitigation pattern), repeat
  the snapshot + compile-verify dual gate, and document
  any new chip-yaml extensions in CHIPS-TI-00 §15.
- **Reopen ID**: CHIPS-TI-02 onward (numbering ratified by
  CHIPS-TI-00 §14).

### 5.3. Abandoned: AM335x chipdb-driven generation

- **Killed in**:
  [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
  §10.1, ratified 2026-05-11.
- **Why abandoned**: AM335x is Cortex-A8, has no
  svd2rust-shaped PAC, and the hand-written prong under
  `examples/beaglebone-black/src/bsp/` already owns
  AM335x bring-up vocabulary. Unifying chipdb-driven and
  hand-written paths would constitute a separate
  cross-cutting initiative.
- **Resurrection prevention**: §10.1 explicitly forbids
  any future `CHIPS-TI-NN` PR from emitting AM335x code,
  and any future `BBB-NN` PR from consuming TI
  chipdb-generated output. Resurrection requires a new
  initiative under
  CLAUDE.md §"Spec-Before-Code Planning Discipline" with
  its own §0 / §5 / §10 / §15 cycle. The placeholder
  `db/chips/AM335x.yaml` exists only to satisfy the
  `am335x_chip_is_present` smoke test and MUST NOT grow.

### 5.4. Abandoned: MSPM0 / MSP430 / C2000 / TDA / Cortex-R5F

- **Killed in**:
  [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
  §11, ratified 2026-05-11.
- **Why abandoned**: each has its own resurrection-
  prevention rationale captured in §11 (MSPM0 PAC
  maturity, MSP430 ISA, C2000 rustc support, TDA
  heterogeneous SoC, Cortex-R5F lockstep). None block
  CHIPS-TI initiative completion.
- **Resurrection prevention**: §11 names the gating
  condition per item. Future agents proposing any of these
  MUST cite the relevant §11 row and document the gating
  condition's resolution before proposing a new phase.

## 6. Forward constraints

Preconditions for the next chipdb-vendor initiative (CHIPS-
NXP, CHIPS-RENESAS-02, future CHIPS-TI sibling-chip slates,
or any structurally similar generator port). Treat these as
binding rules during planning, not aspirational guidelines.

### 6.1. PAC accessor shape MUST be verified on docs.rs before template port

Future TI BSP work MUST verify PAC accessor shape (uppercase
vs. lowercase peripheral fields, method vs. field register
access, per-instance methods vs. indexer, single-bit vs. enum
FieldWriter) by reading `docs.rs` for the pinned PAC version
**before** porting templates from a sibling vendor. The
CHIPS-TI-01b initial port assumed the Espressif precedent
applied verbatim; the cost was four post-port amendment slates
(-01b lowercase + -01e three structural). A 30-minute PAC
audit during the initial port slate would have collapsed those
four slates into one.

### 6.2. Chip yaml additions MUST cross-check register field names against actual PAC field names

Future TI chipdb yaml additions (sibling chips within §5.5,
or future SimpleLink families) MUST cross-check every
register name against the PAC's `<crate>::<peripheral>::*`
module surface and every field name against the PAC's
writer-method surface (`<reg>::W::<field>()`). Authoring
from TRM section headings produces names that look
plausible but do not type-check (§2.4 / §2.5). The
`bsp_ti_<chip>_compile` gate catches *used* registers; it
does not catch *unused* latent typos (§2.5 was caught by
manual audit, not by the gate).

### 6.3. Generator template changes MUST be validated through both snapshot AND compile-verify

Future generator template changes MUST be validated through
both the snapshot test AND the compile-verify gate;
snapshot-only is insufficient. Every divergence in §2.1
through §2.4 above shipped through snapshot-passing emission
and surfaced first at compile-verify. Pre-publish bullets
for new chipdb-vendor compile-verify tests SHOULD start
commented out (expected-fail) and uncomment only once the
gate passes end-to-end. A snapshot diff alone is not
publication-grade evidence that a template change is
correct.

### 6.4. PAC pin MUST be exact-equality when template amendments are PAC-vintage-specific

When template amendments are calibrated against a specific
PAC vintage's accessor shape (the slate 7 -01b lowercase +
-01e three structural amendments against
`cc13x2_26x2_pac 0.10.3`), the consumer `Cargo.toml` PAC
pin MUST be exact-equality (`= "0.10.3"`), not a range
(`^0.10`). An upstream re-publish at 0.11 with modern
svd2rust output would invalidate the amendments in
lockstep; range-pin would silently break the example
crate on `cargo update`.

### 6.5. Re-export modules MUST use glob form, not alias form

Future chipdb-vendor templates emitting a `pac.rs` (or
similar) re-export module MUST use `pub use <crate>::*;`
(glob form). Alias form (`pub use <crate> as <name>;`)
creates a nested namespace that double-segments consumer
paths (§2.7). This applies across CHIPS-MICROCHIP /
CHIPS-SILABS / CHIPS-NXP and any future vendor — the
slate 13 -07 generator fix unwound the alias form in TI;
sibling vendors received the same fix in the same slate
to enforce cross-vendor consistency.

### 6.6. Structural-amendment slates MUST sweep the affected register block

When a structural-amendment slate touches register N in
chip yaml, the slate's scope MUST extend to the full
register block N belongs to. Sweep every register name
against the PAC's module surface; sweep every field name
against the writer surface. The slate 7 -01e amendment
caught two latent bugs (`resetaudio` / `resetsec`, §2.5)
by adopting this rule by accident; future amendments MUST
adopt it deliberately. Symptom-scoped triage misses
latent companions in the same block.

## 7. Provenance hooks

Linking each divergence and refactor point to the
authoritative artifacts so future agents can traverse:
**outcome → issue → fix → underlying evidence**.

### 7.1. Divergence-to-fix traversal

| Divergence (§2) | Slate | Fix commit | Spec amendment |
|---|---|---|---|
| 2.1 Uppercase peripheral access | CHIPS-TI-01b | `51c5296` | CHIPS-TI-00 §15 2026-05-13 entry |
| 2.2 `iocfg(n)` indexer assumption | CHIPS-TI-01e fix 1 | `51523a1` | CHIPS-TI-00 §15 2026-05-13 entry |
| 2.3 `clk_en` single-bit assumption | CHIPS-TI-01e fix 2 | `51523a1` | CHIPS-TI-00 §15 2026-05-13 entry |
| 2.4 Generic reset-field names | CHIPS-TI-01e fix 3 | `51523a1` | CHIPS-TI-00 §15 2026-05-13 entry |
| 2.5 Latent `resetaudio` / `resetsec` | CHIPS-TI-01e bonus | `51523a1` | CHIPS-TI-00 §15 2026-05-13 entry |
| 2.6 CCFG byte-count comment | CHIPS-TI-05a | `c5ae4d0` | CHIPS-TI-00 §15 2026-05-15 entry |
| 2.7 `pac.rs` double-nest re-export | CHIPS-TI-07 | `59a2779` | CHIPS-TI-00 §15 2026-05-15 entry |

### 7.2. Refactor-point traversal

| Refactor (§3) | Slate range | Anchor commits |
|---|---|---|
| 3.1 Data-side `clk_en_variant` | CHIPS-TI-01e | `51523a1` (template + ir.rs + chip yaml) |
| 3.2 Slate-11 workaround → slate-13 fix | CHIPS-TI-06a → -07 | `07f7930` (workaround in main.rs) → `59a2779` (generator fix) |

### 7.3. Phase-completion anchors

| Slate | Description | Commit |
|---|---|---|
| CHIPS-TI-01b | Lowercase peripheral templates | `51c5296` |
| CHIPS-TI-01e | Three structural amendments | `51523a1` |
| CHIPS-TI-05 | Linker emission chapter ratified | `efa6dcf` |
| CHIPS-TI-06 + -06a (scaffold) | Example crate ratified + scaffolded | `7bf8592` + `1bf6379` |
| CHIPS-TI-06a (LED) | DIO_6 LED_RED blink shipped | `07f7930` |
| CHIPS-TI-06b | UART0 hello-world over XDS110 VCOM | `1e3967c` |
| CHIPS-TI-07 | Generator `pac.rs` re-export flatten | `59a2779` |
| CHIPS-TI-05a | CCFG byte-count comment fix | `c5ae4d0` |

### 7.4. External evidence anchors

- **`cc13x2_26x2_pac 0.10.3` API surface** (drove §2.1 /
  §2.2 / §2.3 / §2.4 / §2.5): crates.io
  `cc13x2_26x2_pac` 0.10.3, BSD-3-Clause, SVD source
  `cc13x2_26x2.svd` from `seanmlyons22/ti-lprf-pacs`.
  Accessor shape readable on docs.rs at `prcm::`,
  `ioc::Ioc`, `prcm::uartclkgr::CLK_EN_W`, etc.
- **CCFG structure size + sector size**: TI SWCU185G
  §11.1 "Customer Configuration Area (CCFG)" Table 11-1
  (88-byte CCFG structure) and §6 (4 KB Flash erase
  sector). Drove §2.6 comment correction.
- **LAUNCHXL-CC1352R1 absence of display**: TI SWRU527
  "CC1352R1 LaunchPad Development Kit Hardware User's
  Guide". §2.3 / §2.4 / §2.5 / §2.6 enumerate the
  on-board peripherals (XDS110, LEDs, buttons, I2C); no
  display module is listed. Drove §5.1 deferral.

### 7.5. Memory-system traversal

Future Claude / Codex agents working on a CHIPS-TI sibling
chip slate or a structurally similar chipdb-vendor
initiative should traverse:

- [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
  §15 (canonical change log with all amendments).
- [`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md) for
  linker-emission contract.
- [`CHIPS-TI-06-EXAMPLE.md`](CHIPS-TI-06-EXAMPLE.md) for
  example-crate contract.
- This retrospective for the divergences-and-mitigations
  corpus.

The traversal pattern is: **start at CHIPS-TI-00, drill
into §15 for the canonical decisions, drill into this
retrospective for the failure-mode analysis, drill into
slate commit messages for the per-slate option-space
exploration**. Behaviour PRs reference CHIPS-TI-00 /
-05 / -06 sections only.

## 8. Change log

- **2026-05-15 — Drafted.** Initiative-completion
  retrospective for the CHIPS-TI initiative on rlvgl
  `v0.2.0`. Captures divergences (§2), refactor points
  (§3), portable mitigation patterns (§4), deferred-work
  reclassification (§5), and forward constraints (§6) for
  use by future chipdb-vendor initiatives. Provenance
  hooks (§7) link each entry to the authoritative
  artifact (commit hash, doc section, TRM reference).
