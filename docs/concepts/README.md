# rlvgl-platform — Concepts (cross-cutting design lineage)

This directory holds cross-cutting **design concepts** for the
`rlvgl-platform` crate. It is the home of platform-discipline initiatives
that span multiple subsystems (DMA2D, LTDC, SAI, SDMMC, USB, …) and
that need a ratified vocabulary + frozen invariants before code lands.

It is *not* a port guide. Per-port bring-up narrative continues to live
under `docs/disco-platform-guide/`, `docs/beaglebone-black/`,
`docs/disco-zephyr-guide/`, etc. Those guides describe how a single
target boots and behaves; the docs in this directory describe contracts
that any target's code MUST satisfy.

## Why this directory exists

The CLAUDE.md "Spec-Before-Code Planning Discipline" section already
governs initiative families like DISCO-NN, BBB-NN, CREATOR-NN,
CHIPS-VENDOR-NN. Each of those families is *port-shaped*: a sequence
of chapters describing one tree's bring-up.

Some platform contracts are not port-shaped. The Register-Mashing
Discipline (CLAUDE.md §"Register-Mashing Discipline") is the canonical
example: typed framebuffer ownership, `InFlight<'dma, T>`, the three
address domains, `IsrChannel<T,N>`. It applies to every target. It
ratifies a contract on the platform crate itself, not a port narrative.

`docs/concepts/` is the home for that second class of initiative. Each
family inside this directory follows the §0–§15 phase-document shape
established in CLAUDE.md and ratified in DAA-00 (the
disco-analyzer subrepo's first concepts doc).

## Active initiatives

- **DCB** — *DMA Cacheable Buffers*. RAII typestate for DMA buffers
  in cacheable RAM. Extends the existing `InFlight<'dma, T>` ownership
  rule (Register-Mashing Discipline rule #3) with automatic D-cache
  clean / invalidate at the typestate transitions, so application code
  cannot forget cache maintenance and cannot misorder it. First user is
  the SAI1 line-in/line-out path on the disco-analyzer subrepo. Future
  users: DMA2D destination buffers, SDMMC R/W buffers, USB endpoint
  buffers, LTDC scanout (or MPU non-cacheable carve-out, per DCB-00
  §10).

  - [DCB-00-CONCEPTS.md](DCB-00-CONCEPTS.md) — foundational
    vocabulary, frozen typestate, invariants, source-of-truth map.
    **Ratified 2026-05-02 (§15); DCB-01 unblocked.**
  - [DCB-RETROSPECTIVE.md](DCB-RETROSPECTIVE.md) — initiative-
    completion retrospective (2026-05-03). Captures
    divergences against the original DCB-00 spec, refactor
    inflection points, portable mitigation patterns,
    deferred-work reclassification (Safe / Coupled /
    Abandoned), and forward constraints for future
    register-mashing-discipline-scoped initiatives.
    Provenance hooks link each entry to commit hashes, doc
    sections, and datasheet references for outcome → issue
    → fix → evidence traversal. Convention documented in
    `CLAUDE.md` "Spec-Before-Code Planning Discipline →
    Initiative retrospective".
  - [DCB-02-A.md](DCB-02-A.md) — sub-letter analysis surfaced
    during DCB-02 TX retrofit. Proposed Option A
    (`DeviceActiveDoubleBuf<DIR>` typestate) for DMA double-buffer-
    mode consumers (SAI1 RX, SAI4 PDM RX, future SDMMC / USB
    streaming).
    **Resolved 2026-05-02 — Option A ratified into DCB-00
    §3/§5/§6/§10/§15. DCB-01b + DCB-02-R unblocked.**
  - [DCB-04-A.md](DCB-04-A.md) — sub-letter analysis surfaced
    during DCB-02c BASELINE shrink. The remaining `raw_dcache`
    entry is the LTDC scanout pre-clean in `freertos_entry.rs`;
    DCB-00 §10 deferred the typestate-vs-MPU decision to
    DCB-04. Proposed Option A (`DeviceLtdcScan<T, N>` typestate)
    over Option B (MPU non-cacheable carve-out) on bench-
    behaviour-preservation and portability grounds.
    **Resolved 2026-05-02 — Option A ratified into DCB-00
    §3/§5/§6/§10/§14/§15. DCB-01c + DCB-04 unblocked.**
  - [DCB-01b-A.md](DCB-01b-A.md) — sub-letter analysis on
    the cache-op placement for the `Read` direction of
    `HalfGuard` / `BankGuard`. The DCB-01b shipped pattern
    cleans at *guard entry*; the pre-DCB SAI1 TX pattern
    cleans *after* CPU writes. Steady-state effect: audio is
    correct but ~10.67 ms (2 half-periods) late vs the
    pre-DCB shape — a latency regression rather than a "bees"
    repeat. Proposed Option A (move clean to `release` for
    `Read`; keep entry for `Write`); requires a small
    DCB-01b API change (`release` gains `&mut DcaCacheCtx`)
    and mechanical updates to the two disco-analyzer
    consumer sites.
    **Resolved 2026-05-03 — Option A ratified into DCB-00
    §5/§6/§15. DCB-01d unblocked.**
  - [DCB-02b-A.md](DCB-02b-A.md) — sub-letter analysis on
    the residual raw-pointer write path in
    `audio_player::PollResult::NeedRefill`. DCB-02b made the
    cache op type-system-tracked but kept the legacy raw
    `*mut u8` for PCM byte writes. Proposed Option A
    (callback-based `poll_refill<F>` replacing `poll()` +
    `refill_done(pcm)`; closure scope = bank-guard scope;
    no self-referential token complexity); single in-tree
    consumer (disco bare-metal binary) updates mechanically.
    **Resolved 2026-05-03 — Option A ratified into DCB-00
    §10/§15. DCB-02b-A2 unblocked.**
  - [DCB-04-B.md](DCB-04-B.md) — sub-letter analysis on the
    full `LtdcScan<u8, FB_BYTES>` typestate refactor for
    `freertos_entry.rs`'s FRONT_FB atomic-swap pattern (and
    the bare-metal `Scanout::swap` parallel). DCB-04 cleared
    the `raw_dcache` BASELINE entry via trait-dispatch
    (`DcaCacheCtx::cache.clean + barrier`); the §10 row
    prescribed pushing the FB ownership through the
    `LtdcScan` typestate. The atomic-swap pattern doesn't
    fit `&'static mut DcaBuf`'s exclusive-borrow rule
    without rearchitecting render/present task ownership.
    Proposed Option C (close-with-deferral, mirroring
    DCB-03-A and DCB-02c-A): the `LtdcScan` typestate is
    preserved in-tree (DCB-01c) for future ports to adopt;
    reopen DCB-04-B-2 with a named first user.
    **Resolved 2026-05-03 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers. DCB
    initiative reaches natural software-side completion.**
  - [DCB-02c-A.md](DCB-02c-A.md) — sub-letter analysis on the
    §10-prescribed `DcaBuf` push through the
    `rlvgl_core::fs::BlockDevice` trait surface. DCB-02c
    routed the SCB cache calls through `DcaCacheCtx`
    (clearing the `raw_dcache` BASELINE) but kept the
    caller-supplied `&mut [u8]` trait shape. The §10 row
    prescribed pushing `DcaBuf<u8, BLOCK_BYTES>` into the
    trait. Proposed Option C (close-with-deferral analogous
    to DCB-03-A): no current consumer needs the typestate at
    the trait surface; the embedded-sdmmc adapter is
    third-party so can't take DcaBuf anyway; future
    non-Write-Through consumers route through `DcaCacheCtx`
    or reopen DCB-02c-B with a named first user.
    **Resolved 2026-05-03 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers; the
    initiative's optional-cleanup track ends here.**
  - [DCB-03-A.md](DCB-03-A.md) — sub-letter analysis on the
    DMA2D destination retrofit. Documented that DMA2D
    destinations on the disco target all live in MPU-Write-
    Through SDRAM, so the §10-prescribed cache discipline
    emits no-op runtime ops; the §10 row's value is forward-
    looking design hygiene rather than a fix for an observed
    bug. Proposed Option C (close DCB-03 as deferred; future
    non-Write-Through consumers route through `DcaCacheCtx`
    per the DCB-02c / DCB-04 pattern, or reopen DCB-03-B with
    a named first user).
    **Resolved 2026-05-02 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers; the
    initiative's BASELINE-shrink track ends at DCB-04.**

- **DPR** — *Disco Platform Runtime*. Reusable STM32H747I-DISCO
  board runtime support so the disco demo and disco analyzer can both
  consume the same RLVGL-owned platform surface instead of copying
  demo-local bring-up code. Scope includes board runtime construction,
  display frame scheduling, DSI/LTDC MMIO ownership, warm-reset safe
  stop, and profile-owned telemetry. The disco demo remains the first
  hardware validation app; the disco analyzer is the second-app proof
  that the API is not demo-shaped.

  - [DPR-00-CONCEPTS.md](DPR-00-CONCEPTS.md) — foundational
    vocabulary, scan/profile decisions, invariants, and phase plan.
    **Drafted 2026-05-19; not ratified.** DPR-01 remains blocked until
    DPR-00 §12 is accepted or amended.

- **KI2C** — *Kria I2C Device and Backend*. Evidence-backed `no_std` Rust
  crates for the STTS22H, VEML3235SL, PTN3460, and PCM3168A, plus a Kria
  integration layer that maps the separate PS buses and shared PL bus onto
  RLVGL's existing `embedded-hal` 1.0 contract. EEPROM and KSZ9897S admission
  stays gated on missing board evidence.

  - [KI2C-00-CONCEPTS.md](KI2C-00-CONCEPTS.md) — corrected bus/address
    inventory, crate/backend boundary, per-device phase plan, deterministic
    Rust gates, and the local-model expand/verify/compress discipline.
    **Ratified 2026-07-15.** KI2C-01 is unblocked.
  - [KI2C-01-SUPPORT-SUBSTRATE.md](KI2C-01-SUPPORT-SUBSTRATE.md) — strict
    transaction recorder, corrected Kria topology constants, single-threaded
    shared-bus adapter, optional Linux backend, model-loop evidence, and
    cross-target gates. **Complete 2026-07-15; Llama verdict `ACCEPT`.**
    KI2C-02 is unblocked.
  - [KI2C-02-STTS22H.md](KI2C-02-STTS22H.md) — Rev 8 register evidence,
    explicit configuration contract, coherent signed temperature reads,
    one-shot/continuous modes, read-to-clear alerts, exact thresholds, model
    loop record, and Rust gates. **Complete 2026-07-15; Llama verdict
    `ACCEPT`.** KI2C-03 is unblocked.
  - [KI2C-03-VEML3235SL.md](KI2C-03-VEML3235SL.md) — Rev 1.4 register
    evidence, fixed-address identity, typed integration/analog/digital gain,
    coherent raw channels, exact integer micro-lux conversion, model-loop
    record, and Rust gates. **Complete 2026-07-15; Llama verdict `ACCEPT`.**
    KI2C-04 is unblocked.
  - [KI2C-04-PTN3460.md](KI2C-04-PTN3460.md) — normalized seven-bit
    address correction, configuration-magic health probe, typed LVDS
    electrical register, reserved-encoding rejection, model-loop record, and
    Rust gates. **Complete 2026-07-15; Llama verdict `ACCEPT`.** KI2C-05 is
    unblocked.
  - [KI2C-05-PCM3168A.md](KI2C-05-PCM3168A.md) — readiness typestate,
    truthful reset-state health probe, common slave audio formats,
    sampling-mode-preserving resynchronization, model-loop record, and Rust
    gates. **Complete 2026-07-15; Llama verdict `ACCEPT`.** KI2C-06 is
    unblocked.
  - [KI2C-06-KRIA-INTEGRATION.md](KI2C-06-KRIA-INTEGRATION.md) — explicit
    three-controller ownership, typed leaf factories over the separate PS and
    shared PL buses, caller-owned physical mappings, Linux mapped opening,
    structured smoke diagnostics, model-loop record, and Rust gates.
    **Complete 2026-07-15; Llama verdict `ACCEPT`.** KI2C-07 software
    preparation is unblocked; physical conformance remains hardware-gated.
  - [KI2C-07-HARDWARE-CONFORMANCE.md](KI2C-07-HARDWARE-CONFORMANCE.md) —
    safe read-only probe order, required board/bitstream/mapping/electrical
    inputs, result vocabulary, conformance-record schema, and release gates.
    **Hardware-blocked 2026-07-15; no physical success is claimed.**

- **SCTD** — *SCXML Tutorial Demo*. Target-neutral state-chart demo app
  for the Alex Z SCXML tutorial examples vendored through scjson. The
  first planned machines are Dining Philosophers and the Skoda Bolero
  media-player example, selected from a right-edge icon strip matching
  the disco demo's position.

  - [SCTD-00-CONCEPTS.md](SCTD-00-CONCEPTS.md) — admission set, iState
    MCP generation boundary, selector/icon policy, target portability,
    and acceptance gates. **Ratified 2026-06-19.** Foundational frozen
    decisions §5–§9.
  - **SCTD-01** (no separate concepts doc — executed under SCTD-00 §14).
    **Landed 2026-06-20** (`c2f7d39`): the target-neutral
    `examples/apps/sctd-demo` app with Dining Philosophers (faithful) +
    Media Player (normalized) live over iState-generated machine crates;
    host/sim/`thumbv7em`/UEFI build gates green. FreeRTOS-on-STM32 mount
    deferred as a ratified exception (SCTD-00 §15).
  - [SCTD-02-FIREBEETLE-P4-INTERACTIVE.md](SCTD-02-FIREBEETLE-P4-INTERACTIVE.md)
    — FireBeetle 2 ESP32-P4 + DFR0550-V2 ESP-IDF interactive mount:
    Interactive Dining Philosophers (arrive/depart/panic/reset lifecycle
    inputs), on-screen touch controls + host pause/speed, and the
    compose-then-flip / decoupled-logical-tick cadence (INV-SCTD02-1/-2).
    **Ratified 2026-06-20** (amends SCTD-00 §5/§7 — see SCTD-00 §15).
    SCTD-02 execution unblocked.
  - [SCTD-03-SETUP-AND-AUTO-MODE.md](SCTD-03-SETUP-AND-AUTO-MODE.md)
    — Setup screen, selector recomposition to `[Setup, DP, MP]`, DP Auto
    mode, Media Player source / Auto-Ready configuration, and touch-oriented
    footer text. **Ratified 2026-06-21.**
  - [SCTD-04-RATATUI-RLVGL-INTEGRATION.md](SCTD-04-RATATUI-RLVGL-INTEGRATION.md)
    — shipped two-way Ratatui ↔ rlvgl bridge: an upstream-shaped
    `ratatui-rlvgl` backend, an additional full-screen hybrid Dining
    Philosophers popup launched from the unchanged native DP screen (rlvgl
    rounded chrome/buttons around Ratatui-rendered table content), host
    simulator capture, and a flashable STM32H747I-DISCO bare-metal “Rust all
    the way down” proof. **Ratified and implemented 2026-07-12.** The
    [Ratatui tutorial](../ratatui-tutorial/README.md) is the implementation
    walkthrough.

### Wave-2 UI/runtime initiatives (2026-06-11)

Single-phase, ticket-driven initiatives on the core/widgets/ui crates
(not platform-scoped, but this directory is the established concepts
ledger; each is a §0–§15 doc with its behaviour change landing as
`<INIT>-01`):

- **ANIM** — tick-driven tween/animation system in `rlvgl-core`
  (`Tween` + `Animations` registry, deterministic, no wall clock).
  - [ANIM-00-CONCEPTS.md](ANIM-00-CONCEPTS.md) — **Ratified
    2026-06-11**; ANIM-01 landed same day.
- **REND** — parent-bounds child clipping (`ClipRenderer`) + generic
  `ScrollView`. Critical path: a downstream consumer blocks a delivery
  phase on its 0.2.x publish.
  - [REND-00-CONCEPTS.md](REND-00-CONCEPTS.md) — **Ratified
    2026-06-11**; REND-01 landed same day.
- **INPUT** — `DragRecognizer` gesture middleware in `rlvgl-platform`
  with click-vs-drag suppression (chaining + `TapRecognizer::cancel`).
  - [INPUT-00-CONCEPTS.md](INPUT-00-CONCEPTS.md) — **Ratified
    2026-06-11**; INPUT-01 landed same day.
- **WID** — editable text `Input`/`Textarea` in `rlvgl-ui` (edit
  buffer, caret, `set_active` key routing, `Key::Backspace`
  vocabulary).
  - [WID-00-CONCEPTS.md](WID-00-CONCEPTS.md) — **Ratified
    2026-06-11**; WID-01 landed same day.

### Multi-wave UI/runtime initiatives

- **MPY** — MicroPython-directed stage-and-actors runtime: language-neutral
  object identity, actor descriptors and creation, requested layout with
  rlvgl-performed geometry, queued callback cues, same-core proof, and CM7/CM4
  transport.
  - [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md) — **Draft 2026-08-09.** Defines
    the director/stage/actor/cue vocabulary, semantic LVGL introspection parity
    levels, ten initiative invariants, and the MPY-01 through MPY-09 dependency
    map. Awaiting owner ratification before MPY-01 begins.
- **LPAR** — LVGL parity backlog across runtime substrate, style,
  draw, layout, widget families, conformance fixtures, examples,
  documentation, and release tracking.
  - [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md) — **Ratified
    2026-06-12.** Breaks the parity backlog into waves 0-6 and phases
    LPAR-01 through LPAR-16, with dependency and conflict analysis.
  - [LPAR-01-BASELINE.md](LPAR-01-BASELINE.md) — **Ratified
    2026-06-12.** Pins `LVGL 9.4.0-dev @ 5a89ce8a`, defines source
    baseline/config assumptions, naming policy, conformance levels,
    runtime matrix, widget matrix, and Wave 0 conflict resolutions.
  - [LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md) —
    **Ratified 2026-06-12; implementation landed same day.** Wave 1 object substrate:
    compatibility-first plan for object metadata, base flags/states,
    sibling order, hit testing, and deletion lifecycle.
  - [LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md) —
    **Ratified 2026-06-12; implementation landed same day.** Wave 1
    invalidation and display runtime: logical dirty rects, dirty-source
    collection, present plans, target-buffer dirty retention, display
    flush compatibility, and overflow-to-full-frame fallback
    (`core::invalidation` + `platform::present`).
  - [LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md) —
    **Ratified 2026-06-12; core implementation landed same day.** Wave 1
    event, focus, and input runtime: two-tier event vocabulary and
    growth policy, trickle/bubble propagation on `ObjectNode`,
    tree-resident focus groups, and tick-driven long-press/repeat
    timing (`core::object` dispatch + `core::focus` +
    `platform::gesture::LongPressRecognizer`). Input-device adapters
    pending.
  - [LPAR-05-SCROLL-RUNTIME.md](LPAR-05-SCROLL-RUNTIME.md) —
    **Ratified 2026-06-12; implementation landed same day**
    (`core::scroll` + `PointerDevice::with_scroll`; scrollbar pixel
    rendering deferred to consuming widgets). Wave 1 scroll runtime:
    `SCROLLABLE` flag
    finalization, scroll `ObjectEvent` codes
    (`ScrollBegin`/`Scroll`/`ScrollEnd`/`ScrollThrow`), drag→scroll
    composition with inherited click/long-press suppression, tick-driven
    throw/momentum via the ANIM `Tween` substrate, snapping, nested-scroll
    chaining, scrollbar overlay, and additive `ScrollView` reconciliation.
  - [LPAR-06-TIMERS-OBJECT-ANIM.md](LPAR-06-TIMERS-OBJECT-ANIM.md) —
    **Ratified 2026-06-12.** Wave 1 timers and object animations:
    tick-counted `Timers`/`TimerId`, tree-resident object-bound
    animations (`ObjectNode::bind_anim` + `ObjectAnims` walker) with
    detach-cancellation by construction, the LPAR-07 transition seam,
    and deprecate-in-place reconciliation of the legacy wall-clock
    `core::animation` animators (keeping the shared `Easing`/`LoopMode`
    math). Completes the Wave 1 runtime substrate.

### Wave 2 — style / draw / layout substrate

- **LPAR** (continued — Wave 2)
  - [LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md) —
    **Ratified 2026-06-12; implementation landed same day**
    (`core::style_cascade` + `core::theme` `LparTheme`; widget draw-path
    wiring rides with LPAR-11+). Wave 2 style and theme substrate:
    `(Part, ObjectStates)` selector cascade above the unchanged
    `core::style::Style` property bag, tree-resident `StyleState` on the
    node, top-down property inheritance, `bind_anim`-driven style
    transitions (LPAR-06 seam), `LparTheme` default-theme chaining, and
    deprecate-in-place reconciliation of the overlapping `ui::style` /
    `ui::theme` surfaces.
  - [LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md) —
    **Ratified 2026-06-12.** Wave 2 text/draw/image/mask substrate:
    defaulted `Renderer` capability methods (no implementer breaks), a
    `FontMetrics` trait unifying the bitmap/packed/fontdue backends,
    glyph-extent-aware text clipping (resolving the REND-00 §5.4
    limitation via a new path — REND-00 amended), LTR wrapping with a
    named RTL boundary, alpha masks / gradients / shadows over the raster
    `CoverageSink`, `ImageDescriptor`/cache/recolor/transform, a
    software-reference + hardware-tolerance rule, and a new resolved
    `TextStyle` (the frozen `core::style::Style` stays untouched).
  - [LPAR-09-ASSET-FILESYSTEM.md](LPAR-09-ASSET-FILESYSTEM.md) —
    **Ratified 2026-06-12.** Wave 2 asset and filesystem sources:
    extends the existing `core::fs` `AssetSource`/`AssetManager` (zero
    consumers, safe), a typed `AssetPath` source model (Embedded / FATFS /
    Simulator / Memory) replacing LVGL drive letters, an opaque
    `AssetHandle` registry token bridging to LPAR-08's
    `ImageData::Asset` variant (LPAR-08 amended), source-dispatched decode
    via the existing plugins, and a bounded LRU cache.
  - [LPAR-10-LAYOUT.md](LPAR-10-LAYOUT.md) — **Ratified 2026-06-12.**
    Wave 2 layout substrate: object-managed bounds via a tree-resident
    `LayoutState` slot + `effective_bounds()`/translation draw + an
    additive `Widget::set_bounds` default-no-op (the static `ui::layout`
    helpers stay unchanged), a `Dimension` sizing model (Px/Pct/Content +
    min/max), flex and grid engines, a deterministic pre-draw layout pass
    with old∪new invalidation, `SizeChanged`/`LayoutChanged` events
    (LPAR-04 amended), and padding/gap via the cascade (frozen `Style`
    untouched).

### Wave 3 — primitive and control widgets

- **LPAR** (continued — Wave 3)
  - [LPAR-11-PRIMITIVE-WIDGETS.md](LPAR-11-PRIMITIVE-WIDGETS.md) —
    **Ratified 2026-06-13.** Primitive widget wave:
    additive `Arc`, `Bar`, `Led`, `Line`, `Spinner`, and `Scale` modules
    in `rlvgl-widgets`, preserving `ProgressBar` and audio-meter
    `Scale`/`LedBargraph` surfaces while consuming the settled style,
    draw, text, layout, and animation substrate.
  - [LPAR-12-CONTROL-WIDGETS.md](LPAR-12-CONTROL-WIDGETS.md) —
    **Ratified 2026-06-13; implementation landed same day.** Control
    widget wave: additive `ButtonMatrix`, `ImageButton`, and `Spinbox`
    modules over the LPAR-04 event/focus, LPAR-08 text/image draw,
    LPAR-09 asset, LPAR-10 layout, and LPAR-11 primitive-widget
    substrate. Key navigation via imperative helper methods (app wires
    `ObjectEvent::Key`); no new `Renderer`/`Style`/`Event` surface;
    widgets own their data (no borrowed-map hazards).

### Wave 4 — selection / navigation / data widgets

- **LPAR** (continued — Wave 4)
  - [LPAR-13-SELECTION-NAV-WIDGETS.md](LPAR-13-SELECTION-NAV-WIDGETS.md) —
    **Ratified 2026-06-13; implementation landed same day.**
    Selection/navigation wave: additive `Dropdown`, `Keyboard`, `Menu`,
    `Roller`, `Tabview`, `Tileview`, and `Window` modules reusing
    `List`/`ButtonMatrix`/scroll/snap, coexisting with (not renaming) the
    adjacent `ui::Drawer`/`Modal`/`EventWindow`. Roller snap reuses the
    public `core::scroll::snap_offset_to_points` helper (one snap
    mechanism); overlay/text-binding/`ValueChanged` deferred.
  - [LPAR-14-DATA-RICH-WIDGETS.md](LPAR-14-DATA-RICH-WIDGETS.md) —
    **Ratified 2026-06-13; implementation landed same day.**
    Data/rich-content wave: additive
    `widgets::textarea::Textarea` (reusing the WID-01 `EditCore`, promoted
    to `core` and re-exported from `ui` to avoid a crate cycle), `Chart`,
    `Table`, `Span`, `Calendar`, and `MessageBox` (reusing `ButtonMatrix`),
    coexisting with `ui::Input`/`Textarea`/`Modal`/`Alert`. Span/Table/
    Textarea wrapping reuses LPAR-08 `core::font` measurement (no fork); the
    LPAR-13 Keyboard→text binding is resolved via `apply_key_output`.

### Wave 5 — canvas / media / property / observer

- **LPAR** (continued — Wave 5)
  - [LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md](LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md) —
    **Ratified 2026-06-13; LPAR-Core + ArcLabel landed same day.**
    LPAR-Core: a `Canvas` widget (owns a lightweight local `PixelBuffer`;
    the `core::plugins::canvas` plugin coexists unchanged but is NOT
    wrapped — it is feature-gated and pulls `embedded-graphics`, see
    LPAR-15 §5.C), tick-driven `AnimImage` (Spinner-pattern frame phase),
    `core::property` (identity-free `Queryable`), and `core::observer`
    (`Subject<T>` value-binding, orthogonal to the LPAR-04 event system —
    the §9 ownership conflict resolved). LPAR-Optional (feature-gated):
    `ArcLabel` (landed); `Lottie`/`DashLottie`/`Texture3d` deferred
    (external-renderer deps).

### Wave 6 — conformance / examples / docs / release

- **LPAR** (continued — Wave 6)
  - [LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md](LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md) —
    **Ratified 2026-06-14; validation cleanup in progress.** The capstone
    conformance phase every prior phase feeds. Defines one fixture-contract
    shape (four kinds: determinism, geometry, pixel-golden, behavioral/trace;
    software-reference oracle; LPAR-08 §5.H tolerance) and a per-phase fixture
    ledger (§6). LPAR-05/06/08/10/11/12/13/14/15 are Landed; LPAR-09 is
    substrate-complete with the FATFS-over-`SimBlockDevice` prong deferred to
    `FatfsAssetSource` + `rlvgl-fs-sim`. Separates *implemented* from
    *conformance-complete* (§5.A), owns no-std/feature gates (§7), the
    simulator parity example (§8), the `cargo doc` gate (§9), release
    readiness (§10), and the initiative retrospective
    ([LPAR-RETROSPECTIVE.md](LPAR-RETROSPECTIVE.md)).

### FONT — font selection & anti-aliased widget text

Builds on the LPAR-08 text substrate. The glyph pipeline
(`draw_text_shaped` → coverage → `blend_row`) was already wired and
`Label`/~all widgets already render real 1-bit coverage; FONT closes the
remaining gaps — font *selection* and anti-aliasing.

- [FONT-00-CONCEPTS.md](FONT-00-CONCEPTS.md) — **Ratified 2026-06-14.**
  Freezes the `WidgetFont`/`set_font` per-widget selection model (default
  `FONT_6X10`; no global registry in v1 — deferred-Coupled on theming),
  AA-by-font-choice (feed a `PackedFont` for 8-bit AA; `FONT_6X10` stays the
  1-bit default), the `ArcLabel` coverage migration (the lone legacy
  `draw_text` widget), rotated-renderer glyph throughput
  (rotate-bitmap-then-blit, mirroring `Dma2dOverlayCtx`), and the AA
  conformance fixture (assert partial-alpha through a real
  `blend_row`-overriding renderer). Phases FONT-01..04 in §12.
  **FONT-01..04 all complete 2026-06-15 (§12.A–§12.D boxed);** see the §15
  change log for the per-phase landing record.
- [FONT-05-FONT-REGISTRY.md](FONT-05-FONT-REGISTRY.md) — **Ratified + complete
  2026-06-15.** Reopens the FONT initiative's deferred `FontId → handle`
  registry now that the LPAR-07 theming owner exists. Adds an immutable
  borrow-backed `FontRegistry<'a>`, a defaulted `Widget::widget_font_mut` font
  sink, and an `apply_font_registry` pass over `resolve_tree_with_text` that
  resolves each node's cascade `font_id` and feeds the mapped handle into the
  widget's `WidgetFont` slot — bridging cascade/theme/locale font identity to
  the FONT-00 selection model. Cascade-overrides-else-preserve precedence;
  default-`font_id` trees render identically.
  - [FONT-RETROSPECTIVE.md](FONT-RETROSPECTIVE.md) — initiative-completion
    retrospective (2026-06-15). Captures the stale-premise divergence (the
    "Label migration" was already done before the initiative began), the
    synthetic-`PackedFont`-vs-DejaVu fixture refactor, the `ClipRenderer`
    interception limitation on the rotated glyph fast path, deferred-work
    reclassification, and forward constraints for the deferred `FontId`
    theming-registry work.

### RATATUI — curated Unicode, AA text, and modifier fidelity for `ratatui-rlvgl`

Extends the SCTD-04 `ratatui-rlvgl` backend with the capabilities that doc
explicitly bypassed or deferred: box-drawing/block/arrow/status glyphs
(today collapsed to ASCII `+`/`-`/`|`/`?`), anti-aliased text via the
FONT-00 `PackedFont`/`WidgetFont` model (which postdates SCTD-04), real
bold/italic font variants (replacing a pixel-offset hack and a no-op), and
a blink-fidelity decision reconciled against SCTD-04 §7's redraw-on-change
invariant.

- [RATATUI-00-CONCEPTS.md](RATATUI-00-CONCEPTS.md) — **Ratified and
  implemented 2026-07-17.** Curated 375-codepoint repertoire (box
  drawing, block elements, full arrow block, status symbols; Braille
  excluded this pass) packed into four crate-local `PackedFont` variants;
  real bold/italic variant selection replacing the old pixel-offset hack;
  a tick-driven `RatatuiSurface::advance_blink_phase()` with its
  companion SCTD-04 §7 amendment; a `Bitmap6x10` opt-out reproducing the
  pre-RATATUI-00 behavior byte-for-byte. Landed on `SoftOboros/ratatui`
  `dev/ratatui-rlvgl-backend` @ `adc75755`.
  - [RATATUI-RETROSPECTIVE.md](RATATUI-RETROSPECTIVE.md) — initiative-
    completion retrospective (2026-07-18). Covers the single-day
    draft-to-landed arc: the crate-local asset-placement correction, a
    worktree/`isolation` dispatch near-miss, the 12×20→14×21 cell-geometry
    divergence, uneven `.notdef` glyph coverage across font-style variants,
    and forward constraints — most notably that the SCTD demo's hero-popup
    pixel layout (tuned against the old 12×20 grid) has not yet been
    re-verified against the new default geometry.

(Future concepts initiatives — for example: cross-core IPC primitives,
non-cacheable MPU region management, SDMMC ownership lifecycle — land
as additional families here when they cross the ~3-phase / ~3-subsystem
threshold.)

## Conformance

A conforming `rlvgl-platform` consumer MUST satisfy the acceptance
gates of every active initiative whose surface it touches. For DCB
specifically: any new DMA buffer added to a cacheable RAM region (D1
SRAM, D2 SRAM, AXI SRAM) MUST go through the DCB typestate API; manual
`clean_dcache_by_*` / `invalidate_dcache_by_*` calls in new code are a
discipline violation unless explicitly carved out per DCB-00 §11.

Existing call sites (`audio_player.rs`, `stm32h747i_disco_sd.rs`,
`sd_emmc_adapter.rs`) are grandfathered until DCB-02 / DCB-03 retrofits
land — see DCB-00 §10.

## Vocabulary discipline

Per CLAUDE.md normative-keyword convention: **MUST**, **MUST NOT**,
**SHALL**, **SHOULD**, **SHOULD NOT**, **MAY**, **RECOMMENDED** in
docs under this directory follow RFC 2119 / RFC 8174. Plain narrative
without capitalised keywords is informative.

## Sub-letter doc convention

Per the established pattern (DAA-01-A, DAA-01-B, …): a `<INIT>-NN-X`
doc is a tradeoff analysis surfaced during phase NN that needs its own
ratified resolution before phase NN proceeds. Sub-letter docs are
scoped to one decision, transient (resolution folds into the parent
phase doc's §15), and do not introduce new frozen invariants of their
own.

## Execution discipline

Once a concepts doc here is ratified (dated §15 entry), execution PRs
cite the phase as `<INIT>NN[a-z]:` in the commit subject (e.g.
`DCB01a:`, `DCB02:`). Touching a frozen typestate value or invariant
requires a §15 amendment **first**, in a separate PR. No behaviour PR
rides on an unamended invariant.
