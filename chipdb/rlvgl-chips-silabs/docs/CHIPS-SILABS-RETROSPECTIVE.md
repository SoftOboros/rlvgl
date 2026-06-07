# CHIPS-SILABS Retrospective — divergences, refactor points, forward constraints

**Status:** Drafted 2026-05-15. Initiative-completion
retrospective for the CHIPS-SILABS initiative on rlvgl
`v0.2.0`. Captures the multi-slate trajectory that brought the
Silicon Labs vendor lane of the `rlvgl-creator` BSP generator
from concepts (CHIPS-SILABS-00) through a green compile-verify
gate and a runnable bsp_pac example crate on the SLSTK3701A.

This doc is a **historical artifact** with one normative
section (§6 forward constraints). Behaviour PRs reference
`CHIPS-SILABS-00-CONCEPTS.md` directly and its §15 change log;
this retrospective is the bridge between *what shipped* and
*what to do differently the next time a SILABS chip is added,
or the next time a pre-2021 vintage PAC is targeted by the
generator*.

Per CLAUDE.md "Spec-Before-Code Planning Discipline →
Initiative retrospective" the file is co-located with the
phase docs and follows the §1–§7 shape established by
`docs/concepts/DCB-RETROSPECTIVE.md`. Audience: future Codex /
Claude agents working on a structurally similar vendor lane.

## 1. Outcome snapshot

### Final architecture

`rlvgl-creator bsp from-yaml --vendor silabs` emits an 8-file
BSP set per board: six `.rs` files (`mod.rs`, `pac.rs`,
`clocks.rs`, `io_mux.rs`, `peripherals.rs`, `board.rs`) plus
two linker scripts (`memory.x` and the per-chip
`efm32_gg11.x`). The pac re-export is flattened at the
crate-root shape so consumer code uses `crate::bsp_generated::
pac::...` without any SKU sub-module nesting visible.

Templates target `efm32gg11b-pac 0.1.4` (Dec 2020 vintage).
The PAC is a pre-method-accessor svd2rust crate, so register
access is field-direct (`p.CMU.hfbusclken0.modify(|_, w|
w.gpio().set_bit())`) rather than method-style
(`p.CMU.hfbusclken0().modify(...)`). Peripherals are reached
via `pac::Peripherals::take().unwrap()` against the flattened
re-export.

Conformance is gated by:

- `cargo test -p rlvgl-chips-silabs` — chipdb adapter tests
  (SilabsIr → render IR roundtrip).
- `cargo test -p rlvgl --test bsp_silabs_slstk3701a_render
  --features creator,regression` — golden-snapshot text tests
  for the 8-file emission.
- `cargo test -p rlvgl --test bsp_silabs_slstk3701a_compile
  --features compile-verify -- --test-threads=1` — opt-in
  compile-verify; materialises a throwaway cargo project around
  the generated BSP and runs `cargo check --target
  thumbv7em-none-eabihf` against `efm32gg11b-pac 0.1.4`. Now
  green end-to-end.

The example crate `examples/slstk3701a/` is a detached
workspace (empty `[workspace]` stanza, matching the
disco-demo-states precedent). Its `bsp_pac` binary links, runs
on hardware, drives the PH10 LED0_R via `Px_DOUTTGL` atomic
XOR, and sends "hello" over USART4 VCOM at 115200-8-N-1.

13 slates shipped between 2026-05-12 and 2026-05-15:
CHIPS-SILABS-{00, 01, 01b, 01c, 02, 02b, 02c, 03, 04, 05,
05a, 06, 06a, 06b, 07}. The slate IDs are non-contiguous
because three amendments (-01c, -02b, -02c) landed mid-slate
in response to compile-verify findings.

### Deferred items (explicit)

1. **CHIPS-SILABS-06c** — rlvgl widget-tree integration on the
   SLSTK3701A's tiny 128×128 Sharp Memory LCD. Closed-with-deferral.
   The display panel exists on the dev kit but no Sharp Memory
   driver has been written for the rlvgl render path; resurfacing
   this requires adding a display driver layer first. Reopen
   trigger documented in CHIPS-SILABS-06 §11.

That is the only deferral remaining at initiative close.
CHIPS-SILABS-05 (linker emission), -06a (LED blink), and -06b
(USART4 VCOM hello) all shipped rather than deferring.

### Known residual risks

- **PAC pinned to `efm32gg11b-pac 0.1.4`.** This is a Dec 2020
  vintage crate. The SKU sub-module gating, field-style register
  access, and absence of `Px_DOUTSET` / `Px_DOUTCLR` per-port
  registers are all artifacts of that PAC era. If the crate is
  ever republished to modern svd2rust shape (method-style
  accessors, crate-root `Peripherals`, decomposed set/clear
  registers) the templates will need a coordinated amendment —
  the divergence is structural, not cosmetic.
- **Reference-Manual vs. PAC divergence is silent at the spec
  surface.** The EFM32GG11B Reference Manual and the
  `efm32gg11b-pac 0.1.4` crate disagree on where the GPIO
  clock-gate lives (`CMU.HFPERCLKEN0` vs `CMU.HFBUSCLKEN0`); both
  documents are internally consistent. Only the compile-verify
  gate exposes the disagreement. Any future SILABS chip yaml
  that is written RM-first without a compile-verify pass will
  inherit the same class of bug.
- **No hardware bring-up gate beyond LED + USART hello.** The
  `bsp_pac` example demonstrates clocks, GPIO output, and a
  single USART TX path, but does not exercise interrupts, DMA,
  the LCD, or the on-board flash. A regression that breaks any
  of those surfaces will not be caught by the existing gates.

## 2. Divergence log

Capturing where reality diverged from the CHIPS-SILABS-00
spec. Each entry: **Assumption** (what the spec / template
said) → **Symptom** (observable build failure) → **Root cause**
(mechanistic) → **Detection gap** (why earlier gates didn't
catch it).

### 2.1 SKU sub-module gating

- **Assumption.** Templates emitted `pac::Peripherals::steal()`
  and `pac::CMU::ptr()`-style paths assuming `Peripherals` and
  the per-peripheral structs live at the PAC crate root, the
  way every Espressif PAC is shaped.
- **Symptom.** `cargo test --features compile-verify` produced
  5 × `error[E0433]: failed to resolve: could not find
  'Peripherals' in 'efm32gg11b_pac'` plus matching errors for
  `CMU`, `GPIO`, `USART4`, `PRS`.
- **Root cause.** `efm32gg11b-pac 0.1.4` gates the entire
  peripheral set under a chip-specific sub-module
  `efm32gg11b820::*`. The crate root only re-exports a tiny
  prelude. This convention is common in pre-2021 svd2rust
  output for vendors with multiple-SKU SVDs.
- **Detection gap.** Slate-6 templates were authored against
  the Espressif precedent (`esp32c3 0.31` exposes `Peripherals`
  at crate root). No mechanism in slate-1 through slate-5
  attempted a build against the real PAC; the render-snapshot
  gate is text-only.

### 2.2 Field-style vs. method-style register access

- **Assumption.** Templates emitted method-style accessors:
  `p.CMU.hfperclken0().modify(|_, w| w.gpio().set_bit())`,
  matching the post-2021 svd2rust convention used by the
  Espressif PACs.
- **Symptom.** After 2.1 was fixed, `cargo check` produced
  ~102 errors of the shape `error[E0599]: no method named
  'hfperclken0' found for struct 'CMU'`; one per register
  access site across `clocks.rs`, `io_mux.rs`, and
  `peripherals.rs`.
- **Root cause.** `efm32gg11b-pac 0.1.4` is pre-method-accessor
  svd2rust. Each register is exposed as a **field** of the
  peripheral struct, not as a method. The correct shape is
  `p.CMU.hfperclken0.modify(|_, w| w.gpio().set_bit())` —
  drop the parens.
- **Detection gap.** Same as 2.1; no compile-verify gate was
  running yet. Once the gate ran, the error count was high
  enough (102 → 11 after slate -02b) that the slate boundary
  itself acted as the detector.

### 2.3 GPIO clock-gate routing (RM vs. PAC disagreement)

- **Assumption.** Chip yaml `system_gates.gpio.clk_en_reg`
  routed the GPIO peripheral clock through
  `cmu.hfperclken0.gpio`, per the EFM32GG11B Reference Manual
  §10 "CMU — Clock Management Unit".
- **Symptom.** After 2.1 and 2.2 were fixed, `cargo check`
  still produced `error[E0599]: no method/field named 'gpio'
  found for struct 'HFPERCLKEN0'`.
- **Root cause.** The RM and the PAC disagree. The PAC routes
  GPIO through `CMU.HFBUSCLKEN0` bit 5 (the bus clock domain),
  not `HFPERCLKEN0` (the peripheral clock domain). Both
  documents are internally consistent; the disagreement is
  vendor-side. The PAC's field naming is load-bearing because
  it is what the generated code must type-check against.
- **Detection gap.** A yaml authored from the RM alone will
  reproduce this bug for every future SILABS chip. The
  compile-verify gate is the only mechanism that surfaces it.
  An RM-cross-check pass at chip-yaml authoring time would
  have caught it, but no such gate exists.

### 2.4 IO_MUX MODEH absolute pin index

- **Assumption.** The `io_mux.rs.jinja` template's MODEH
  (pins 8-15) branch emitted relative field names —
  `mode{N-8}` — on the theory that MODEH "starts at mode0
  again the same way MODEL does".
- **Symptom.** `error[E0599]: no method named 'mode0' found
  for struct 'MODEH'` for pin assignments in the 8-15 range.
- **Root cause.** `efm32gg11b-pac 0.1.4` uses **absolute**
  pin-index field names in both MODEL and MODEH: MODEL has
  `mode0..mode7`, MODEH has `mode8..mode15`. The two registers
  do not restart numbering. This is unusual — many
  vendor PACs split into two registers with relative naming
  — but the PAC is canonical here.
- **Detection gap.** Same as 2.3. Template was authored from
  the RM register-set description, which describes MODEL/MODEH
  as a pair indexed 0-7 each; the PAC encodes the *aggregate*
  16-pin namespace.

### 2.5 Px_DOUTSET / Px_DOUTCLR absence

- **Assumption.** GPIO output toggle would use the pair
  `Px_DOUTSET` (atomic set) and `Px_DOUTCLR` (atomic clear),
  as is conventional on most modern Cortex-M GPIO PACs.
- **Symptom.** During CHIPS-SILABS-06a LED-blink scaffolding,
  the PAC exposed only `Px_DOUT` and `Px_DOUTTGL` per port;
  no `Px_DOUTSET` / `Px_DOUTCLR` accessors. Using `Px_DOUT`
  with read-modify-write would have raced any sibling-port
  bit toggled by interrupt handlers or future peripherals.
- **Root cause.** EFM32GG11B GPIO is a "DOUT + toggle"
  architecture, not a "DOUT + set/clear" architecture. The PAC
  faithfully reflects the SVD, which faithfully reflects
  silicon.
- **Detection gap.** Not a generator bug per se — a templating
  decision needed to be made. The detection gap is at the
  abstraction-layer choice: no "atomic GPIO toggle" abstraction
  exists in the slate-6 example scaffold, so the consumer crate
  has to encode the choice manually. See §3.3.

### 2.6 pac.rs double-nesting

- **Assumption.** The slate-6 `pac.rs.jinja` template emitted
  `pub use efm32gg11b_pac::efm32gg11b820 as pac;` and consumer
  code reached peripherals via `crate::bsp_generated::pac::
  pac::Peripherals`.
- **Symptom.** Consumer ergonomics: `pac::pac::Peripherals` is
  awkward; touching any consumer-side import after a regen
  required reasoning about which `pac::` was which.
- **Root cause.** The re-export name (`pac`) shadowed itself
  through the nested module. Two siblings of the same name
  collapsed onto one path.
- **Detection gap.** Compile-verify gate is happy — the code
  type-checks fine. This is a usability divergence surfaced
  during slate-13 (-07) cross-vendor consumer cleanup, not a
  build-failure divergence.

## 3. Refactor points

Decision inflection nodes — where the initiative changed
direction. Each as **Trigger → Alternatives → Selection
rationale → Cost of switch**.

### 3.1 SKU flatten — per-call-site vs. re-export amendment

- **Trigger.** Slate 6 / -02: 5 × E0433 from §2.1.
- **Alternatives.**
  - (a) Update every template call site to use the full
    `pac::efm32gg11b820::Peripherals` path.
  - (b) Update `pac.rs.jinja` once to re-export the entire
    SKU sub-module at the crate-root shape:
    `pub use efm32gg11b_pac::efm32gg11b820::*;`
  - (c) Move the SKU name into chip yaml so the template is
    SKU-aware.
- **Selection rationale.** (b). Single line of template
  change, no per-call-site edits, no chip-yaml schema growth.
  Re-export semantics are well understood; the same pattern
  works for any future pre-2021 PAC that uses SKU sub-module
  gating.
- **Cost of switch.** Trivial — one template line plus the
  golden-snapshot regen. The follow-up cleanup
  (CHIPS-SILABS-07) flattened the re-export itself to remove
  the `pac::pac::` double-nesting from §2.6.

### 3.2 RM vs. PAC — "PAC wins" judgment

- **Trigger.** Slate 8 / -01c: §2.3 GPIO clock-gate
  disagreement, plus -02c §2.4 MODEH disagreement.
- **Alternatives.**
  - (a) "RM wins" — keep the chip yaml RM-faithful, vendor a
    field-rename layer in the generator that converts
    `hfperclken0.gpio` → `hfbusclken0.gpio` at emit time.
  - (b) "PAC wins" — amend the chip yaml to match the PAC
    field names, document the RM divergence inline.
- **Selection rationale.** (b). The load-bearing gate is
  compile-verify against the real PAC. Adding a generator-side
  field-rename layer would push the divergence underground and
  make future readers second-guess both the yaml and the
  emitted code. Documenting the divergence in the yaml comment
  and in CHIPS-SILABS-00 §15 keeps the trail visible.
- **Cost of switch.** Two chip-yaml edits and two
  golden-snapshot regens. No template churn. Cumulative cost
  at initiative close: ~30 lines of yaml + comment.

### 3.3 GPIO toggle — RMW vs. DOUTTGL atomic XOR

- **Trigger.** Slate 11 / -06a: LED-blink consumer code needed
  to drive PH10 high then low. PAC offered only `Px_DOUT` (RMW)
  and `Px_DOUTTGL` (atomic XOR); see §2.5.
- **Alternatives.**
  - (a) Read-modify-write `Px_DOUT` from the consumer crate,
    accepting the sibling-bit race window for v0.
  - (b) Write `Px_DOUTTGL` with a one-bit mask, getting atomic
    XOR semantics for free.
- **Selection rationale.** (b). The example crate is a
  reference for future consumers; using DOUTTGL teaches the
  correct pattern for SILABS GPIO and avoids encoding a
  known-racy idiom into the canonical example. Cost of (a)
  would compound: every future consumer would copy the racy
  pattern.
- **Cost of switch.** Negligible — `Px_DOUTTGL.write(|w|
  unsafe { w.bits(1 << 10) })` is the same line-count as the
  RMW equivalent.

## 4. Mitigation patterns

Abstract the fixes into reusable units. Encoded as
preconditions, invariants, and template-authoring guidance.

### 4.1 Pre-2021 PAC structural inspection

- **When.** A new vendor lane is added to `rlvgl-creator`
  and the target PAC was published before ~2021.
- **Apply.** Before authoring `pac.rs.jinja`, open the PAC
  crate's `src/lib.rs` and check three things: (a) is
  `Peripherals` at the crate root, or gated behind a SKU
  sub-module? (b) are peripheral registers accessed via
  methods or fields? (c) are atomic set/clear registers
  decomposed (`*SET`, `*CLR`) or aggregated (`*TGL`, `*INV`)?
  Encode all three answers in CHIPS-`<VENDOR>`-00 §0 (authority
  policy) before writing templates.
- **Rationale.** §2.1, §2.2, §2.5 are all instances of the
  same class of bug: structural assumptions inherited from a
  more modern PAC. Catching them in the concepts doc avoids
  re-paying the cost in compile-verify churn.

### 4.2 RM vs. PAC disagreement protocol

- **When.** Chip yaml authoring touches a register that the
  RM names but the PAC implements.
- **Apply.** The PAC field name is canonical. If the RM and
  PAC disagree, document the divergence (a) in the chip yaml
  as an inline comment naming both the RM section and the PAC
  field, and (b) in CHIPS-`<VENDOR>`-00 §15. Do not add a
  generator-side rename layer; keep the divergence visible.
- **Rationale.** §2.3 and §2.4 prove that RM-first authoring
  reproduces this class of bug deterministically. The
  compile-verify gate is the only mechanism that catches it,
  and only after the fact. Making the divergence visible at
  the yaml site is the cheapest forward-fix.

### 4.3 Atomic XOR over RMW for "DOUT-plus-toggle" GPIO

- **When.** The target PAC exposes `Px_DOUTTGL` /
  equivalent atomic-XOR register but does not expose discrete
  `Px_DOUTSET` / `Px_DOUTCLR`.
- **Apply.** Example-crate templates SHOULD prefer the atomic
  XOR register. RMW on `Px_DOUT` is permitted, but MUST be
  justified inline with a comment naming the sibling bits that
  do not collide (typically because the port is exclusively
  owned by the example).
- **Rationale.** §2.5 + §3.3. Atomic XOR with a one-bit mask
  is the same code size as the RMW pattern, has no race
  surface, and teaches the correct idiom to future consumers
  copying the example.

### 4.4 Re-export the SKU sub-module at the crate-root shape

- **When.** The target PAC gates `Peripherals` and per-
  peripheral structs behind a SKU sub-module.
- **Apply.** `pac.rs.jinja` emits a single-line re-export:
  `pub use <pac_crate>::<sku_module>::*;`. Consumer paths
  use `crate::bsp_generated::pac::Peripherals` regardless of
  the underlying PAC's nesting.
- **Rationale.** §2.1 + §3.1. One line of template change
  insulates every downstream call site from the PAC's
  structural choice. The same pattern is reusable for any
  future vendor lane that targets a pre-2021 multi-SKU PAC.

## 5. Deferred work reclassification

Don't leave deferred items as a flat list. Classify by
coupling to core invariants.

### Safe (orthogonal, no impact on core invariants)

None. All slate-1 through slate-13 in-scope work either shipped
or is reclassified under §Coupled below. CHIPS-SILABS-05
(linker emission), -06a (LED blink), and -06b (USART4 hello)
all shipped rather than deferring.

### Coupled (named assumption attached)

- **CHIPS-SILABS-06c — rlvgl widget tree on the SLSTK3701A
  LCD.** Coupled to **"SLSTK3701A's 128×128 Sharp Memory LCD
  has a driver in the rlvgl render stack."** The display
  panel exists on the dev kit, the SPI interface is wired,
  but no driver layer has been written. Resurrecting this
  deferral requires (a) writing the Sharp Memory LCD driver
  trait implementation in the rlvgl crate, (b) wiring it
  through `examples/slstk3701a/`'s `board::init()`, and (c)
  porting the splash-desktop disco-demo path to a 128×128
  monochrome render target. The coupling is **structural** —
  no amount of generator work substitutes for the missing
  driver.

### Abandoned (resurrection-prevention notes)

None. No slate of work was explicitly killed during the
CHIPS-SILABS initiative. Future agents who feel like adding a
generator-side RM→PAC field-rename layer (see §3.2) should
read §4.2 first and confirm the divergence is not better
served by yaml + comment.

## 6. Forward constraints

This is the only normative section in the retrospective.
Future planning docs treat these as binding rules.

- **Future SILABS BSP work MUST verify SKU sub-module gating
  in the pinned PAC crate before assuming crate-root
  `Peripherals`.** Specifically, inspect the PAC's
  `src/lib.rs` for `pub mod <sku>;` patterns and either
  flatten via `pub use <pac>::<sku>::*;` in `pac.rs.jinja`
  (per §4.4) or thread the SKU name through chip yaml.
- **Future SILABS chipdb yaml additions MUST cross-check
  register-field names against actual PAC field names. RM
  section names alone are insufficient.** When the RM and PAC
  disagree, the PAC name is canonical; document the divergence
  inline in the yaml AND in CHIPS-SILABS-00 §15. Do not add a
  generator-side rename layer (see §4.2).
- **Future SILABS GPIO toggle code SHOULD use `Px_DOUTTGL`
  atomic XOR. RMW on `Px_DOUT` is permitted but MUST be
  justified inline with a "no sibling-bit conflict" comment
  naming each occupied pin on the port** (see §4.3).
- **Compile-verify against the pinned PAC MUST be in the
  acceptance gate for every new SILABS board added through
  the generator.** Render-snapshot text gates alone do not
  catch §2.1 / §2.2 / §2.3 / §2.4. The cost of running
  compile-verify is bounded (cached target dir under
  `$TMPDIR/rlvgl-bsp-...-compile-verify-target`); skipping it
  trades a one-time test cost for an unbounded compile-failure
  cost downstream.

## 7. Provenance hooks

Each divergence and refactor point traces to its commit /
amendment / external reference. Future agents traverse
outcome → issue → fix in one hop.

### Divergence → fix → commit

- §2.1 (SKU sub-module gating) → §3.1 (re-export amendment) →
  `11075e6 CHIPS-SILABS-02: SKU-flatten pac.rs template for
  efm32gg11b-pac 0.1.4`.
- §2.2 (field-style register access) → field-direct template
  amendment → `10cdd84 CHIPS-SILABS-02b: field-style register
  access for efm32gg11b-pac 0.1.4`.
- §2.3 (GPIO clock-gate routing, RM vs. PAC) → §3.2 ("PAC
  wins") → `b003c42 CHIPS-SILABS-01c: route GPIO clock-gate
  through hfbusclken0 in EFM32GG11 yaml`.
- §2.4 (MODEH absolute pin index) → io_mux template amendment
  → `7cb8d76 CHIPS-SILABS-02c: io_mux MODEH branch uses
  absolute pin index`.
- §2.5 (Px_DOUTSET / Px_DOUTCLR absence) → §3.3 (atomic XOR
  example) → `76c9748 CHIPS-SILABS-06a: drive PH10 (LED0_R)
  in busy-wait toggle loop`.
- §2.6 (pac.rs double-nesting) → re-export flatten →
  `2ae1930 CHIPS-SILABS-07: flatten pac.rs re-export for
  clean consumer path`.

### Slate-level provenance

- **Linker emission.** `2a38c31 CHIPS-SILABS-05: ratify linker
  emission chapter` + `37eef42 CHIPS-SILABS-05a: emit memory.x
  and efm32gg11b.x linker scripts`.
- **Example crate scaffold.** `fe93a7c CHIPS-SILABS-06:
  ratify example crate chapter` + `b25af55 CHIPS-SILABS-06a:
  scaffold SLSTK3701A bsp_pac example crate`.
- **Hardware bring-up demonstrations.** `76c9748
  CHIPS-SILABS-06a: drive PH10 (LED0_R) in busy-wait toggle
  loop` + `8c8c2bc CHIPS-SILABS-06b: send hello over USART4
  VCOM`.
- **Consumer-path cleanup.** `2ae1930 CHIPS-SILABS-07: flatten
  pac.rs re-export for clean consumer path`.

### External references

- **PAC pin.** `efm32gg11b-pac 0.1.4` on crates.io, published
  Dec 2020. Source SVD: Silicon Labs `EFM32GG11B820F2048GL192`
  device file.
- **Reference Manual.** EFM32GG11B Reference Manual (Silicon
  Labs document `efm32gg11-rm.pdf`), Rev. 1.4 — §10 CMU,
  §32 GPIO. Cited by chip yaml comments at the
  `cmu.hfbusclken0.gpio` route (§2.3) and the MODEH absolute
  pin index (§2.4).
- **Board.** Silicon Labs SLSTK3701A Starter Kit
  (`EFM32GG11B Giant Gecko 11 Starter Kit`). LED0_R is wired
  to PH10; VCOM USART4 routes through USART4_TX on PH4.

## 8. Change log

- **2026-05-15** — Initial retrospective drafted at natural
  initiative completion. All 13 slates (CHIPS-SILABS-{00, 01,
  01b, 01c, 02, 02b, 02c, 03, 04, 05, 05a, 06, 06a, 06b, 07})
  shipped or closed with deferral; compile-verify gate green
  against `efm32gg11b-pac 0.1.4`; `examples/slstk3701a/
  bsp_pac` blinks LED0_R and emits "hello" over USART4 VCOM
  on hardware. Only CHIPS-SILABS-06c (rlvgl widget tree on the
  SLSTK3701A LCD) remains coupled-deferred pending a Sharp
  Memory LCD driver.
