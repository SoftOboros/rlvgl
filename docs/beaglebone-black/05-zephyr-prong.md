<!--
05-zephyr-prong.md - BBB Phase 5 concepts chapter. Ratified shape per
CLAUDE.md "Spec-Before-Code Planning Discipline". Cited by subsequent
BBB-05a, BBB-05b, ... execution PRs as the authority for vocabulary,
frozen decisions, and acceptance gates.
-->

# BBB Phase 5 — Zephyr Prong Concepts

**Status:** Ratified 2026-05-11 (owner: Ira Abbott; see §15).
**Initiative:** `BBB` (BeagleBone Black + NHD-7.0CTP-CAPE-P four-prong port).
**Phase:** 5 — Zephyr.
**Companion docs:** [`README.md`](./README.md) (initiative index, informative),
[`bare-metal-bring-up.md`](./bare-metal-bring-up.md) (Phase 4 reference).

This chapter is the **normative** concepts gate for the Zephyr prong.
The §"Phase 5: Zephyr" checklist in the BBB README is informative; the
acceptance gates in §12 below are the binding closure conditions.

---

## §0. Authority policy

The Zephyr prong straddles three vocabulary owners. Each term in this
document inherits its definition from exactly one of them, and §3 names
which one.

| Domain | Authoritative source | Owns vocabulary for |
|--------|----------------------|---------------------|
| AM335x silicon (LCDC, PRCM, CTRLMOD, I2C, UART) | TI **SPRUH73Q** (AM335x TRM, rev Q) | Register names, bit positions, offsets, clock-tree nodes, pin-mux register addresses. |
| Zephyr OS subsystems | Zephyr `main` documentation pinned at the SDK revision (see §6 SDK pin) | `display-controller` driver model, `input` subsystem semantics, `SYS_INIT` priority bands, DT binding paths, `west` build verbs, `prj.conf` Kconfig symbols. |
| C-shell + staticlib Rust pattern | `docs/disco-zephyr-guide/` (DISCO Volume V) | The boundary between a thin C `main()` shell that owns Zephyr kernel interaction and a Rust staticlib that owns the render loop, widget tree, and framebuffer ownership. |
| rlvgl widget tree, controller, playit wire protocol | `rlvgl-app-disco-demo` (`examples/apps/disco-demo/`) + `rlvgl-playit` (`playit/`) | `DiscoController`, `DiscoCommand`, `DiscoEffect`, playit command syntax. Treated as **black-box reused contracts** — Phase 5 MUST NOT modify them. |
| Hand-written AM335x BSP (shared across prongs) | `examples/beaglebone-black/src/bsp/` | `bsp::lcdc`, `bsp::prcm`, `bsp::pinmux`, `bsp::am335x` register map. Phase 5 MAY add Zephyr-side wrappers but MUST NOT divergently re-define the LCDC raster sequence (see §10 row 1). |

Where Zephyr documentation and TRM SPRUH73Q appear to conflict (e.g.
the `display-controller` API requests a clock-frequency value while
the TRM specifies a clock divisor), TRM SPRUH73Q wins on the silicon
side and Zephyr wins on the API-shape side; §10 reconciles each such
case explicitly.

RFC 2119 / 8174 keywords (MUST, MUST NOT, SHOULD, MAY) carry their
RFC meanings when capitalised; lowercase use is ordinary English.

---

## §1. Purpose

Get the rlvgl `DiscoController` widget tree running on BeagleBone
Black + NHD-7.0CTP-CAPE-P **under Zephyr RTOS**, sharing the
LCDC register sequence and pin-mux constants already validated by
Phase 3 (Linux, ✅ shipped) and Phase 4 (bare-metal, ✅ shipped).

Phase 5 is the third dispatch model in the four-prong matrix. Its
purpose is **not** to ship new rlvgl features; it is to demonstrate
that the same `DiscoController` widget tree is portable across a
Linux userspace renderer, a `no_std` bare-metal loop, and a real
RTOS scheduler with driver-model abstractions — validating that the
rlvgl runtime contract is OS-independent on the AM335x platform.

Phase 5 closes the BBB initiative's prong-set coverage. Once Phase 5
clears its §12 acceptance gates, the BBB initiative may enter its
retrospective phase (see §15 / `BBB-RETROSPECTIVE.md`).

---

## §2. Problem statement

Phases 3 and 4 are shipped but they couple the rlvgl runtime to two
specific dispatch models that do not generalise to RTOS deployment:

- **Phase 3 (Linux)** runs `DiscoController` from a userspace process
  that mmaps `/dev/fb0` and pumps the render loop in a wall-clock-paced
  while-loop. Evidence: `examples/beaglebone-black/src/main.rs` `present_bbb_fbdev_16bpp_rect` and the
  README §"Main loop is wall-clock-paced" paragraph. The Linux kernel
  scheduler is between rlvgl and the panel; the LCDC register sequence
  is reached via the `tilcdc` + `panel-dpi` driver stack (overlay-driven
  bring-up).
- **Phase 4 (bare-metal)** runs in SVC mode with MMU and caches off,
  no scheduler, no driver abstractions. Evidence:
  `examples/beaglebone-black/src/bare_metal.rs` (`_start` preamble,
  polled UART0, single cooperative loop) and
  `examples/beaglebone-black/src/bsp/lcdc.rs` (raw register sequence).
  The same register recipe lights pixels in ~10 s before warm-resetting
  back to Linux.

Neither model exercises:

1. The Zephyr **device-driver** abstraction layer (display-controller,
   input, gpio, i2c) which is the dominant deployment model for
   embedded RTOS products.
2. A real **preemptive scheduler** with ISR-to-thread synchronisation
   primitives (`k_sem`, `k_sleep`, `irq_connect_dynamic`).
3. Zephyr's **device-tree-driven** peripheral configuration, where
   pin mux and LCDC timings are specified in DT and a kernel driver
   programs the registers from those properties.
4. The **C-shell + Rust-staticlib** boundary pattern that the DISCO
   port has already validated (`docs/disco-zephyr-guide/02-c-shell-and-ffi.md`)
   and that is the recommended deployment shape for rlvgl-on-Zephyr.

Phase 5 covers all four. The same `DiscoController` widget tree
exercised on `/dev/fb0` (Phase 3) and on raw LCDC registers
(Phase 4) MUST drive panel pixels on Zephyr without changes to the
widget tree itself — only the dispatch layer changes.

A pre-existing scaffold lives at `examples/beaglebone-black/zephyr/`
(CMakeLists, prj.conf, board overlay, `src/main.c`) and at
`examples/beaglebone-black/src/zephyr_entry.rs` /
`zephyr_sync.rs`. Phase 5 makes these scaffolds boot. They MUST NOT
be treated as canonical until §12 (a) closes — current contents may
contain placeholder values (e.g. the touch-IRQ GPIO is marked
`TODO: confirm interrupt GPIO from cape schematic` in
`zephyr/boards/am335x_bone_black.overlay`).

---

## §3. Canonical glossary

Each term lists its definition source and adaptation status per
CLAUDE.md §"Definitions — reference vs. restatement".

### Hardware

- **`Cape`** — the Newhaven NHD-7.0CTP-CAPE-P populating the BBB
  P8/P9 headers. Defined in [`README.md`](./README.md) §"Bill of
  Materials" and §"Panel Specifications"; used without modification.
- **`LCDC`** — AM335x LCD Controller at MMIO base `0x4830_E000`.
  Defined by TRM SPRUH73Q §13; used without modification.
- **`Panel`** — the NHD-7.0-800480AF-ASXP TFT module driven by the
  cape. 800×480 active, 24-bit parallel RGB, falling-edge DCLK,
  active-low HSYNC/VSYNC, DE mode. Timings frozen in §5.1.
- **`FT5426G`** — Focaltech capacitive-touch controller on the cape,
  reachable at I2C address `0x38` on I2C2 (`0x4802_A000`). Currently
  **hardware-blocked** — see [`RMA-newhaven-2026-04-22.md`](../../examples/beaglebone-black/RMA-newhaven-2026-04-22.md).

### Software boundary (C shell ↔ Rust staticlib)

- **`BbbZephyrEntry`** — the Rust `extern "C" fn rlvgl_init(...)`
  symbol exported from the staticlib, called once by the C shell
  after Zephyr kernel init. As defined in
  `examples/beaglebone-black/src/zephyr_entry.rs:113` (current
  scaffold); used without modification at the FFI signature level.
  Owned by §6 staticlib boundary.
- **`zephyr/src/main.c`** — the C shell. Defined locally in
  `examples/beaglebone-black/zephyr/src/main.c`; adapted from the
  DISCO equivalent (`examples/stm32h747i-disco/zephyr/src/main.c`)
  with delta: AM335x IRQ numbers (LCDC EOF at IRQ 36 per TRM
  §6.3.1) instead of STM32 NVIC vectors; 800×480 ARGB8888 framebuffer
  pair statically allocated in DDR rather than `__attribute__((section(".sdram")))`.
- **`RlvglDisplayInfo`** — `#[repr(C)] struct` passed from C to Rust
  describing the framebuffer pair. As defined in
  `examples/beaglebone-black/src/zephyr_entry.rs:21`; used without
  modification. Mirrors the DISCO struct of the same name.
- **Staticlib boundary** — the link between the Rust `cdylib`/`staticlib`
  artifact (`libfreertos_example_bbb.a` produced from `lib.rs` /
  `zephyr_entry.rs`) and Zephyr's `west`-driven kernel link. The
  Rust side owns no boot code; Zephyr's linker script + `_start` own
  the boot path. Pattern owned by `docs/disco-zephyr-guide/01-build-and-link.md`;
  Phase 5 inherits the two-step build flow with `armv7a-none-eabihf`
  substituted for `thumbv7em-none-eabihf`.

### Zephyr-side abstractions

- **`AM335xDisplayController`** — the Zephyr `display-controller`
  driver that programs LCDC raster timings from DT properties.
  **Owned by BBB-05a; does not exist in repo yet.** Phase 5
  produces this driver. Its register-write sequence MUST match
  `bsp::lcdc::init_raster` (see §10 row 1) — Phase 5 reuses the
  hand-written PAC (`examples/beaglebone-black/src/bsp/am335x.rs`)
  via FFI from Zephyr-driver C code, **OR** transcribes the writes
  into a Zephyr-driver-shaped C source; the choice is a BBB-05a
  decision deferred from this concepts doc. Either way the produced
  register state MUST be bit-identical.
- **`am335x_bone_black` board** — the Zephyr board definition.
  Upstream Zephyr ships a `beaglebone_black` board (verify path
  during BBB-05a: TBD against Zephyr SDK 0.16.x). Phase 5 either
  consumes the upstream board or adds an out-of-tree board layer.
  Decision deferred to BBB-05a; the choice MUST be recorded as a
  §15 change-log amendment to this doc.
- **`bb-nh7c` DT overlay** — the Phase 5 device-tree overlay that
  enables LCDC + panel + I2C2 + FT5336 on the chosen board. Current
  scaffold lives at `examples/beaglebone-black/zephyr/boards/am335x_bone_black.overlay`
  with placeholder `int-gpios` value. The cape's touch IRQ is
  wired to **`gpio3_19`** per the Phase 3 overlay
  (`examples/beaglebone-black/linux/BB-NHD7-CAPE.dts`); the Phase 5
  overlay MUST inherit this binding.

### Input pipeline

- **Zephyr input event** — `struct input_event` from
  `<zephyr/input/input.h>`, carrying `code` (e.g. `INPUT_ABS_X`,
  `INPUT_BTN_TOUCH`), `value`, `sync`, `type`, `dev`. Owned by
  Zephyr; used without modification.
- **`INPUT_CALLBACK_DEFINE`** — Zephyr macro that registers a
  global input-event sink. Used in the existing scaffold
  (`examples/beaglebone-black/zephyr/src/main.c:98`). Owned by
  Zephyr.
- **`take_touch()` / `take_keys()`** — Rust-side consumer of the
  atomic touch / key ring buffer. As defined in
  `examples/beaglebone-black/src/zephyr_entry.rs:95+`; used
  without modification. Mirrors the DISCO Zephyr pattern.

### Frozen enumerations referenced

- **BBB prong set** = `{linux, bare_metal, freertos, zephyr}`.
  **Standards Action.** Adding a value requires a §15 amendment
  to this doc and a §15 amendment to `README.md` §"The Four Prongs"
  table. (See §6 frozen decision (e).)
- **`PixelFmt`** — owned by `rlvgl_core`; Phase 5 consumes
  `PixelFmt::Argb8888` (Rust-side internal compose buffer) and
  presents as RGB565 16bpp or ARGB8888 24bpp to LCDC. Phase 5
  MUST NOT add a new variant.

---

## §4. Source-of-truth map

One owner per concept across the four prongs. Drift between rows
of this table is the dominant failure mode (per CLAUDE.md
§"Spec-Before-Code Planning Discipline"); every Phase 5 PR must
either fit an existing row or amend this table via §15.

| Concept | Linux (Phase 3) | Bare-metal (Phase 4) | FreeRTOS (Phase 4) | **Zephyr (Phase 5)** |
|---------|-----------------|----------------------|--------------------|----------------------|
| Clock / PRCM enable | kernel `tilcdc` driver | `bsp::prcm::enable_lcdc()` | `bsp::prcm::enable_lcdc()` | Zephyr clock-control driver (TBD: confirm AM335x PRCM binding in Zephyr SDK 0.16.x) — MUST end in CM_PER_LCDC_CLKCTRL=0x2, IDLEST=0x0. |
| Pin mux | kernel pinctrl + DT overlay `bb-lcd-pins` | `bsp::pinmux::configure_lcd_pins` | `bsp::pinmux::configure_lcd_pins` | DT `pinctrl-0` on the LCDC + I2C2 nodes; Phase 5 overlay supplies pad values bit-identical to the Phase 3 overlay table (§"Step 5: Patch the DTB for LCD Output"). |
| LCDC raster init | hand-written `bsp::lcdc::init_raster` invoked via `/dev/mem` (retired) → kernel `tilcdc` (current Phase 3) | `bsp::lcdc::init_raster` directly | `bsp::lcdc::init_raster` directly | `AM335xDisplayController` Zephyr driver — see §10 row 1 for the equivalence constraint. |
| Framebuffer DDR placement | `/dev/fb0` (kernel-allocated, RGB565) | static reserved region @ `0x8400_0000` (`memory.x`) | static reserved region @ `0x8400_0000` | Zephyr-allocated static framebuffer pair in DDR; current scaffold uses `static uint8_t fb_front[FB_SIZE] __attribute__((aligned(64)))` in `main.c` (ARGB8888, 1.5 MB each). Pixel format is a §6 frozen decision (b). |
| Touch (FT5x06 over I2C2) | kernel `edt-ft5x06` → evdev → `/dev/input/event1` → rlvgl evdev backend | `bsp::am335x::i2c2` polled (planned, hardware-blocked) | I2C4 interrupt pattern from DISCO ported to I2C2 (planned, hardware-blocked) | Zephyr `focaltech,ft5336` input driver → `INPUT_CALLBACK_DEFINE` → atomic ring buffer in `zephyr_entry.rs` → rlvgl event. Same C/Rust pattern as DISCO Volume V Chapter 4. |
| Playit transport | TCP loopback on `127.0.0.1:9999` via SSH-forward | UART0 (`bsp::uart0`) "playit-lite" subset, ~150 LoC | TBD (Phase 4 FreeRTOS in progress) | UART0 via Zephyr `uart_*` API + `INPUT_MODE_SYNCHRONOUS` parity; full `PlayitExecutor` once Rust heap is wired (`CONFIG_HEAP_MEM_POOL_SIZE` already 64 KiB in scaffold). |
| Logging / breadcrumbs | stdout / `/tmp/rlvgl.log` | `bsp::uart0` polled writes | TBD | Zephyr `LOG_*` macros via `CONFIG_UART_CONSOLE=y` (already in scaffold). |
| ISR registration | n/a (kernel-owned) | `_start` vector table | FreeRTOS port vectoring | `irq_connect_dynamic` from `main.c`; ISR body in Rust as `extern "C"`. LCDC EOF on **IRQ 36** (TRM §6.3.1). |
| Scheduler / loop | userspace while(1) | cooperative `loop {}` | FreeRTOS preemptive tasks | single Zephyr `main` thread running blocking render loop; touch/key callbacks are inline (`CONFIG_INPUT_MODE_SYNCHRONOUS=y`) — same posture as DISCO Volume V Chapter 5. |

---

## §5. Frozen decisions — panel & LCDC timing

(a) **LCDC raster timing constants are frozen at TRM SPRUH73Q
values for the NHD-7.0-800480AF-ASXP panel.** Phase 5 MUST NOT
introduce a divergent timing table; the Zephyr `display-timings`
DT node values MUST match the Phase 3 overlay
`examples/beaglebone-black/linux/BB-NHD7-CAPE.dts` and the
Phase 4 constants in `examples/beaglebone-black/src/bsp/lcdc.rs`:

| Field | Value | Source |
|-------|-------|--------|
| clock-frequency | 33 300 000 Hz | README §"Panel Specifications" |
| hactive | 800 | as above |
| vactive | 480 | as above |
| hfront-porch | 210 | as above |
| hback-porch | 46 | as above |
| hsync-len | 20 | as above |
| vfront-porch | 22 | as above |
| vback-porch | 23 | as above |
| vsync-len | 10 | as above |
| hsync-active | 0 (active-low) | as above |
| vsync-active | 0 (active-low) | as above |
| de-active | 1 | as above |
| pixelclk-active | 0 (DCLK falling edge) | as above |

(b) **`RASTER_TIMING_2` bit positions for IPC / IHS / IVS are
22 / 21 / 20 respectively.** This is an **invariant** verified
against TRM SPRUH73Q Table 13-26 and pinned by the memory note
"AM335x LCDC bit positions verified" (`feedback_am335x_lcdc_verified`).
Earlier code shipped with the wrong positions (11 / 12 / 13) and
produced a non-functional panel. **Standards Action** — changing
these positions requires a §15 amendment with a corroborating TRM
citation.

(c) **Framebuffer placement under Zephyr** is statically allocated
in BSS by the C shell (current scaffold:
`static uint8_t fb_front[FB_SIZE] __attribute__((aligned(64)))`).
The Zephyr linker script places `.bss` in DDR — the bare-metal
`0x8400_0000` reservation does **not** apply; Zephyr owns DDR
allocation. Phase 5 MUST verify that the framebuffer's physical
address satisfies LCDC DMA word-alignment requirements (TRM
§13.5.3); the `aligned(64)` attribute is necessary but not
sufficient if Zephyr places `.bss` non-contiguously.

(d) **Pixel format: ARGB8888 (32 bpp) in the Rust compose buffer;
LCDC outputs TFT24_UNPACKED.** This is the bare-metal Phase 4
posture. Phase 5 SHOULD adopt the same so the LCDC raster-init
sequence remains bit-identical between prongs. (Phase 3 Linux
ships at 16 bpp RGB565 to dodge the eMMC pin conflict via
`tilcdc` mode-set; that path is not reachable from Zephyr,
so the cape's full 24-bit width is available.) See §11 (c) for
the deferred 32 bpp non-goal.

(e) **Frozen enumerations registration policy.** The BBB prong
set `{linux, bare_metal, freertos, zephyr}` is **Standards Action**
per CLAUDE.md §"Frozen enumerations — registration policy". The
Phase 5 acceptance of the `zephyr` variant by this doc is the
ratification event for the fourth prong.

---

## §6. Frozen decisions — Zephyr SDK and build

(a) **Zephyr SDK version pinned at 0.16.x.** Per memory note
`project_zephyr_sdk_pin` (DISCO precedent): SDK 0.17 has picolibc
API drift, SDK 1.0 refuses pre-1.0 build artifacts. Phase 5
inherits the pin. Touching it requires a §15 amendment and a
fresh DISCO retest first. **Standards Action.**

(b) **`armv7a-none-eabihf` Rust target.** Bare-metal already uses
this; Phase 5 inherits. The staticlib invocation is the existing
scaffold pattern (`examples/beaglebone-black/zephyr/CMakeLists.txt`
references `target/armv7a-none-eabihf/debug/librlvgl_example_bbb.a`).

(c) **Feature flag: `zephyr`** on `rlvgl-example-bbb`. Mutually
exclusive with `linux`, `bare_metal`, `freertos`. Already declared
in `Cargo.toml`. The Zephyr-side Rust entry points are gated
behind this feature; Phase 5 MUST keep the feature exclusivity.

(d) **Two-step build flow.** Inherited from DISCO Volume V Chapter 1:

```
1. cargo build --target armv7a-none-eabihf -p rlvgl-example-bbb \
       --features zephyr --lib
2. west build -b <board> examples/beaglebone-black/zephyr
```

(e) **DT bindings — provisional.** The Phase 5 overlay extends
the chosen Zephyr board with:

- a `panel` node bound to `newhaven,nhd-7.0-800480af` (TBD: verify
  this compatible exists in Zephyr 0.16.x; otherwise Phase 5 lands
  an out-of-tree binding under
  `examples/beaglebone-black/zephyr/dts/bindings/`).
- the `display-timings` block per §5 (a).
- I2C2 enabled with `focaltech,ft5336@38`, `int-gpios = <&gpio3 19 GPIO_ACTIVE_LOW>`
  (inheriting the Phase 3 wiring).
- A `pinctrl-0` reference for LCD_DATA[0..23] + VSYNC/HSYNC/PCLK/AC_BIAS
  carrying the same pad values as the Phase 3 overlay
  (`bb-lcd-pins` node).

Any DT path or compatible string marked "TBD" above is a **BBB-05a
decision deferred from this doc** and MUST be resolved with a §15
amendment before BBB-05b lands.

---

## §7. Frozen decisions — C-shell / Rust-staticlib boundary

(a) **C owns:** Zephyr kernel init, `SYS_INIT` hooks, ISR
registration via `irq_connect_dynamic`, input subsystem callback
dispatch, FFI shims for `k_sem` / `k_sleep`, framebuffer
allocation. (Mirrors DISCO Volume V Chapter 2.)

(b) **Rust owns:** the render loop, the `DiscoController` instance,
widget-tree dispatch, blitter / rasteriser state, framebuffer
ownership *after* C hands the pointers over, touch/key event
edge detection, playit dispatcher, star-crawl pipeline.

(c) **The FFI signature `extern "C" fn rlvgl_init(eof_sem: *mut k_sem,
info: *const RlvglDisplayInfo) -> !`** is the single entry from C
to Rust. Already declared in the scaffold; semantics:

  - Takes the LCDC EOF semaphore + display info struct.
  - Never returns. The `loop { k_sleep(K_FOREVER); }` in C's
    `main()` is unreachable in nominal operation.

(d) **C → Rust callbacks** (called from Zephyr threads / ISRs):

  - `rlvgl_touch_event(*const TouchEventC)` — called from C input
    callback on `evt->sync`. Atomic ring-buffer write; no
    allocation, no blocking.
  - `rlvgl_key_event(code: u16, pressed: u8)` — called from C
    input callback for non-touch events.
  - `rlvgl_lcdc_eof_isr()` — called from C ISR wrapper. Clears
    LCDC EOF flag; signals the EOF semaphore (C side does the
    `k_sem_give`).

(e) **The C shell MUST NOT call rlvgl widget-tree code directly.**
All `DiscoController` interaction is via the C → Rust callbacks
above. Phase 5 PRs that thread widget code into C are out of
scope and require a §15 amendment.

---

## §8. Frozen decisions — input pipeline (touch + joystick)

(a) **FT5x06 driver: Zephyr `focaltech,ft5336` input driver,
polling mode.** `CONFIG_INPUT_FT5336=y`, `CONFIG_INPUT_FT5336_INTERRUPT=n`,
`CONFIG_INPUT_FT5336_PERIOD=10` (10 ms poll, ~100 Hz). Already in
the scaffold `prj.conf`. This matches the DISCO posture
(interrupt path was unreliable there too); the BBB cape's IRQ
wiring is still TBD against schematic and can be enabled via §15
amendment once verified.

(b) **`CONFIG_INPUT_MODE_SYNCHRONOUS=y`.** Same rationale as DISCO
Volume V Chapter 4: the render loop's `k_sleep(33ms)` would otherwise
fill the input queue and drop events.

(c) **No `SYS_INIT` early-reset hook required on BBB.** Rationale:
the FT5x06 on the cape has its own reset circuit driven by the
3.3 V cape rail; there is no shared reset line with a separately
disabled driver (unlike DISCO's PG3 shared between NT35510 and
FT5336). Phase 5 MAY add a `SYS_INIT` hook later if hardware
investigation reveals one is needed; doing so is a §15 amendment.

(d) **Touch-controller hardware is RMA-blocked.** Per memory note
`project_bbb_touch_hardware_blocker` and
[`RMA-newhaven-2026-04-22.md`](../../examples/beaglebone-black/RMA-newhaven-2026-04-22.md):
the FT5426G on the current cape unit responds on I²C with valid
IDs but `TD_STATUS` stays at 0 under sustained press. Phase 5
acceptance gates that depend on real touch events are
**gated, not abandoned** — see §12 (c).

(e) **Joystick / button events** route through the same input
callback. The BBB has no on-board user-direction buttons exposed
through the cape; Phase 5 ships with only the BBB's user button
(GPIO1_27 / P8_25) bound, if at all. The keyboard surface is
optional for Phase 5 closure.

---

## §9. Frozen decisions — render loop, ISR, transport

(a) **Single render thread** running the `DiscoController` cooperative
loop. The Zephyr main thread enters `rlvgl_init()` and stays there.
No additional Zephyr threads are required for the §12 acceptance
gates. Phase 5 MAY add helper threads in follow-up work but the
ratified design is single-threaded.

(b) **LCDC EOF semaphore (`eof_sem`)** is given by the ISR wrapper
in C and taken in the Rust render loop to pace frames against the
panel cadence. Frame budget under Phase 5 nominal: 1 / FRAME_HZ ≈
17.5 ms (compare `lcdc.rs` `FRAME_HZ` constant; ~57 Hz from the
33.3 MHz pixel clock + 1056 × 553 total timing).

(c) **Playit over UART0**: same wire protocol as Phase 3 and
Phase 4 (`playit/README.md`). Phase 5 uses Zephyr's interrupt-driven
UART API (`CONFIG_UART_INTERRUPT_DRIVEN=y`, already in scaffold).
Full `PlayitExecutor` requires a Rust heap; Phase 5 SHOULD enable
`linked_list_allocator` against `CONFIG_HEAP_MEM_POOL_SIZE` (64 KiB
in scaffold) but MAY ship with playit-lite (as bare-metal does)
for §12 (d) closure.

---

## §10. Reconciliation decisions

How Phase 5 fits with adjacent repo primitives. Each row is a
load-bearing invariant — a Phase 5 PR that breaks one of these
MUST file a §15 amendment first.

(a) **`AM335xDisplayController` Zephyr driver MUST produce the
same LCDC register-write sequence as `bsp::lcdc::init_raster`.**
Authority: `examples/beaglebone-black/src/bsp/lcdc.rs` — that
function (and the constants in the same file) defines the canonical
Phase 4 sequence. Expected post-init register values are pinned
in the README §"Direct LCDC via /dev/mem — Register sequence (all
four prongs)" table. Phase 5's BBB-05a PR MUST include a test or
diagnostic dump confirming the Zephyr driver leaves LCDC in
the same state.

(b) **Pin-mux register writes MUST match the Phase 3 overlay
`bb-lcd-pins` table.** Phase 3 ships pad values via DT overlay
applied by u-boot; Phase 5 ships pad values via Zephyr DT applied
by the pinctrl driver. The **values** at CTRLMOD pads
`conf_lcd_data0..23` + `conf_lcd_vsync/hsync/pclk/ac_bias` MUST
all be `0x08` (Mode 0, pull disabled, output). Sources:
`examples/beaglebone-black/linux/BB-NHD7-CAPE.dts` and
`examples/beaglebone-black/src/bsp/pinmux.rs`.

(c) **Touch input flow differs from Phase 3 but produces the
same `rlvgl::Event::Pointer` stream.** Phase 3 path:
`edt-ft5x06` (Linux kernel) → evdev → `/dev/input/event1` →
`LinuxEvdevInput` → rlvgl event. Phase 5 path:
`focaltech,ft5336` (Zephyr) → `INPUT_CALLBACK_DEFINE` →
atomic ring buffer in `zephyr_entry.rs` → rlvgl event. The
rlvgl-side event payload (landscape coords, pressed flag,
timestamp) is bit-identical; the `DiscoController` cannot
distinguish the two.

(d) **DDR layout differs from Phase 4 but framebuffer alignment
is preserved.** Phase 4 reserves `0x8400_0000 .. 0x8420_0000`
(1.5 MiB framebuffer) via `memory.x`. Phase 5 lets Zephyr
allocate from `.bss` in DDR; the C shell's
`__attribute__((aligned(64)))` MUST be retained to satisfy LCDC
DMA word-alignment. Cross-prong framebuffer-address comparison
is not meaningful (each prong owns its own DDR map); only the
register-write *sequence* is invariant.

(e) **LCD vs eMMC pin conflict applies identically.** Phase 5
inherits the hardware constraint from the README §"Critical
Hardware Constraint": with the cape installed and LCD_DATA[0:7]
in Mode 0, eMMC is unreachable. Zephyr deployments MUST boot
from SD when the cape is attached. This is **not** software-
recoverable; it is the same pad-sharing rule that gates
Phases 3 and 4.

(f) **The handwritten BSP (`examples/beaglebone-black/src/bsp/`)
remains the single source of truth for register field constants.**
If the Zephyr driver path is implemented in C (per §3 deferred
decision), the C driver MAY duplicate constants but MUST cite
the Rust source as authoritative; if a discrepancy ships, the
Rust constants win and the C driver is the bug.

---

## §11. Non-goals

(a) **No Zephyr-side audio.** The NHD cape exposes no audio
codec. The DISCO's `audio` feature has no analog on BBB +
this cape. If a future cape variant adds audio, the gate
re-opens via §15 amendment.

(b) **No SD storage via Zephyr.** Phase 5 boots from SD via
u-boot but does not expose SD as a filesystem to the rlvgl
runtime. (Phase 3 reads splash assets from the SD's ext4
rootfs via Linux; Phase 5 builds splash into the staticlib
as a `static` byte slice. The DISCO Zephyr port's filesystem
plumbing in `main.c` (`rlvgl_readdir`) does not apply.)

(c) **No 32 bpp XRGB8888 path on the Linux prong's userspace
side.** This non-goal already exists in Phase 3 (README
roadmap); Phase 5 leaves it untouched.

(d) **No Zephyr-side modification to `rlvgl-app-disco-demo` or
the playit wire protocol.** Phase 5 consumes these as black
boxes; the §3 authority policy is explicit.

(e) **No upstream contribution of the AM335x display-controller
driver.** Phase 5 ships an in-tree driver under
`examples/beaglebone-black/zephyr/` (or out-of-tree board layer).
Upstream Zephyr contribution is a follow-up effort outside
this initiative's scope.

(f) **No DISCO retest as part of Phase 5 closure.** The DISCO
Volume V acceptance gates are independent. Phase 5 PRs MUST
NOT touch DISCO code paths; if they do, the gate boundary is
ill-defined and the PR is out of scope.

**Candidate for promotion-back-to-in-scope:** items (b) and (e).
(b) becomes load-bearing if the Phase 5 demo grows beyond the
built-in splash + desktop; the DISCO Zephyr port already needed
`CONFIG_FAT_FILESYSTEM_ELM=y` for its SD asset path. (e) becomes
load-bearing if a second AM335x-class board adopts the driver.

---

## §12. Acceptance checklist

A conforming Phase 5 deployment MUST satisfy gates (a), (b), (d),
(e) below. Gate (c) is **hardware-blocked** pending RMA
resolution; it MUST be satisfied for "full" Phase 5 closure but
SHALL NOT block §15 ratification of the remaining gates.

The §"Initiative retrospective" of `BBB-RETROSPECTIVE.md` is
expected to surface divergence-log entries from the categories
foreshadowed in parentheses; Phase 5 PRs should keep §13 evidence
paths up to date as those categories surface.

- [ ] **(a) — Software-only.** Zephyr boots on BBB hardware with
      cape attached and produces panel pixels via the Zephyr
      `display-controller` driver. The first-pixel test is a
      solid colour fill (e.g. dark blue, matching the bare-metal
      Phase 4 colour-bars step). Verifies §10 (a), §10 (b),
      §10 (e). *(Expected retro divergence category: Zephyr
      driver subsystem semantics vs raw register sequence — does
      the Zephyr `display-controller` API surface accept the
      Phase 4 register-write order, or does it impose its own
      ordering?)*
- [ ] **(b) — Software-only.** The `DiscoController` widget tree
      renders end-to-end via the Zephyr display path: splash,
      desktop launcher, settings wing, info wing. Verifies §7
      (b), §9 (a). *(Expected retro divergence category:
      staticlib FFI boundary — does the C → Rust handoff hand
      pointer types that Rust can soundly own for the program's
      lifetime?)*
- [ ] **(c) — Hardware-blocked.** FT5x06 touch events reach
      `DiscoController` through Zephyr `INPUT_CALLBACK_DEFINE`.
      **Gated on the cape RMA returning a responsive FT5426G;
      see [`RMA-newhaven-2026-04-22.md`](../../examples/beaglebone-black/RMA-newhaven-2026-04-22.md).**
      No software change is expected to be required when the
      replacement cape ships — the Zephyr `focaltech,ft5336`
      driver attaches transparently per §8 (a). This gate
      remains open in §15 ratification metadata until the
      RMA closes. *(Expected retro divergence category:
      hardware dependencies that block software ratification
      and the policy for distinguishing them from software bugs.)*
- [ ] **(d) — Software-only.** Playit transport works over
      UART0 — at minimum the `?` / `T<x>,<y>` / `QE:<tag>`
      subset. May be playit-lite per §9 (c). Verifies §4
      "Playit transport" row. *(Expected retro divergence
      category: Zephyr UART API ergonomics vs the bare-metal
      polled-UART driver.)*
- [ ] **(e) — Software-only.** The staticlib build is reproducible
      from a single `west build` invocation (after the Rust
      `cargo build` step). The CMakeLists.txt MUST resolve
      `librlvgl_example_bbb.a` without manual path edits.
      Verifies §6 (d). *(Expected retro divergence category:
      SDK version pinning + two-step build coordination —
      does the Zephyr SDK 0.16.x toolchain agree with the
      `armv7a-none-eabihf` Rust target's ABI?)*

A second conformance level — "full Phase 5" — additionally
requires gate (c). Phase 5 may be declared complete at the
first level for retrospective purposes (per CLAUDE.md
§"Initiative retrospective" — "closed-with-deferral" is a
valid completion state).

---

## §13. Files cited

- `/Users/iraabbott/rlvgl/CLAUDE.md` — Spec-Before-Code Planning
  Discipline, Initiative retrospective section.
- `/Users/iraabbott/rlvgl/docs/beaglebone-black/README.md` —
  initiative index, panel timings, LCDC register table,
  four-prong matrix.
- `/Users/iraabbott/rlvgl/docs/beaglebone-black/bare-metal-bring-up.md`
  — Phase 4 reference chapter.
- `/Users/iraabbott/rlvgl/docs/disco-zephyr-guide/README.md` and
  `01-build-and-link.md` through `07-adapted-cmd-deep-dive.md` —
  authority for the C-shell + Rust-staticlib pattern.
- `/Users/iraabbott/rlvgl/docs/concepts/DCB-RETROSPECTIVE.md` —
  retrospective reference shape (per CLAUDE.md).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/bsp/lcdc.rs`
  — canonical LCDC raster-init sequence (§10 row 1 authority).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/bsp/pinmux.rs`
  — canonical pad-config values (§10 row 2 authority).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/bsp/prcm.rs`
  — canonical PRCM enable sequence.
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/bsp/am335x.rs`
  — hand-written PAC (register map authority).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/zephyr_entry.rs`
  — Rust-side staticlib entry (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/src/zephyr_sync.rs`
  — Rust-side `k_sem` FFI shims (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/zephyr/CMakeLists.txt`
  — Zephyr build glue (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/zephyr/prj.conf`
  — Kconfig surface (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/zephyr/src/main.c`
  — C shell (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/zephyr/boards/am335x_bone_black.overlay`
  — DT overlay (existing scaffold).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/linux/BB-NHD7-CAPE.dts`
  — Phase 3 DT overlay (§10 row 2 cross-reference authority).
- `/Users/iraabbott/rlvgl/examples/beaglebone-black/RMA-newhaven-2026-04-22.md`
  — FT5426G hardware blocker (gates §12 (c)).
- TI **SPRUH73Q** (AM335x and AMIC110 Sitara Processors TRM) —
  §§6.3.1 (interrupt assignments), 13.5 (LCDC register
  definitions), 13.5.3 (DMA alignment), Table 13-26
  (RASTER_TIMING_2 fields). Citation authority for §0 row 1.
- Zephyr documentation at SDK 0.16.x — `display-controller`
  driver model, `input` subsystem, `SYS_INIT` priority bands,
  `west` build flow. Citation authority for §0 row 2.

---

## §14. Unblocks

Closing Phase 5 gates (a) (b) (d) (e) reaches **3 of 4 prongs
at full software conformance** (Linux ✅, bare-metal ✅, Zephyr ✅;
FreeRTOS still in progress). Closing gate (c) when the cape RMA
returns reaches **full prong-set coverage** for the BBB
initiative — the conformance event that authorises a
`BBB-RETROSPECTIVE.md` first-draft.

Phase 5 also unblocks future AM335x-class boards consuming the
Zephyr driver (per §11 (e) non-goal); the in-tree
`AM335xDisplayController` becomes a reference for any out-of-tree
adopter that pulls it.

---

## §15. Change log

| Date | Status | Notes |
|------|--------|-------|
| 2026-05-11 | Ratified (owner: Ira Abbott) | Doc *shape* ratified. `BBB-05[a-z]:` PRs MAY now cite §-numbers as frozen authority. Open BBB-05a decisions (1) upstream `am335x_bone_black` vs out-of-tree board layer, (2) C-transcribed vs Rust-via-FFI `AM335xDisplayController` driver, (3) FT5x06 IRQ GPIO confirmation, (4) upstream `newhaven,nhd-7.0-800480af` panel-binding confirmation — each lands as a dated row below the BBB-05a sub-letter analysis. Hardware-blocked acceptance gate (c) (FT5x06 touch through Zephyr input subsystem) remains gated on the cape RMA per `examples/beaglebone-black/RMA-newhaven-2026-04-22.md`. |
| 2026-05-11 | DRAFT — awaiting ratification | Initial draft. Author: Ira Abbott. Pending: (1) Zephyr board layer decision; (2) display-controller C-vs-Rust decision; (3) FT5x06 IRQ GPIO confirmation; (4) panel-binding confirmation. |
