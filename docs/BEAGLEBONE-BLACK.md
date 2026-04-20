# BeagleBone Black + NHD-7.0CTP-CAPE-P: Plan of Plans

## Context

rlvgl is expanding from MCU-class targets (STM32H747I-DISCO, Beetle ESP32-C3/P4) to its first
**Cortex-A application processor** — the BeagleBone Black (AM3358, Cortex-A8 @ 1 GHz) paired
with the Newhaven NHD-7.0CTP-CAPE-P 7" IPS capacitive touch cape.

The BBB+cape matches the DISCO's 800x480 resolution but over parallel RGB (LCDC) instead of
MIPI DSI, on a fundamentally different processor class. Four delivery paths — Linux,
bare-metal, FreeRTOS, and Zephyr — mirror and extend the DISCO's multi-platform model,
proving rlvgl portability across bus architectures, CPU families, and runtime environments.

Each prong is a multi-phase effort with its own detailed planning cycle. This document is the
**unifying roadmap** that identifies shared vs. individual work, sequencing dependencies, and
the common app/platform contracts that keep all four prongs compatible with each other and
with existing targets.

---

## Hardware Profile

| Item | Spec |
|------|------|
| **Board** | BeagleBone Black (AM3358BZCZ100) |
| **CPU** | Cortex-A8 @ 1 GHz, ARMv7-A |
| **Memory** | 512 MB DDR3L, 4 GB eMMC, 128 KB SRAM |
| **Display cape** | NHD-7.0CTP-CAPE-P |
| **Panel** | NHD-7.0-800480AF-ASXP, 800x480, IPS |
| **Panel interface** | 24-bit parallel RGB, ST7277 driver IC, DE/SYNC modes |
| **Panel connector** | 40-pin 0.5mm FFC |
| **Touch** | Capacitive, FT5x06 family over I2C (same register set as FT5336 on DISCO) |
| **Display controller** | AM3358 LCDC — raster mode, built-in DMA, 512-word FIFO, up to 126 MHz pclk |
| **Backlight** | 180 mA @ 9.3V, driven by cape circuit |
| **HDMI** | Disabled when LCD cape active (shared LCDC + pin mux) |

---

## The Four Prongs

### Prong 1: Linux fbdev/DRM (stepping stone)
### Prong 2: Bare-metal (full control, LCDC driver foundation)
### Prong 3: FreeRTOS (preemptive tasks on bare-metal BSP)
### Prong 4: Zephyr RTOS (full RTOS ecosystem)

---

## Shared Work (All Four Prongs)

These pieces are built once and consumed by all four BBB entry points, and in most cases
by existing targets too. **Do this work first.**

### S1. DiscoController capability preset for BBB

**File:** `examples/apps/disco-demo/src/lib.rs`

Add `DiscoCapabilities::beaglebone_black()` preset:
```rust
pub fn beaglebone_black() -> Self {
    Self {
        audio: false,       // no on-board codec (HDMI audio disabled with cape)
        storage: true,      // eMMC + microSD
        diagnostics: true,
        effects: true,      // star crawl (CPU-rendered, no DMA2D)
        pointer: true,      // cap touch via FT5x06
        platform: "beaglebone-black",
    }
}
```

No widget tree changes needed — same 800x480 as DISCO. The controller is already
resolution-aware from `Screen`.

### S2. FT5x06 touch driver reuse

**Existing:** `platform/src/ft5336.rs` (84 lines)

The FT5336 on the DISCO and the FT5x06 on the NHD cape share the same register map
(I2C address 0x38, same TD_STATUS/Pn_XH/Pn_XL/Pn_YH/Pn_YL layout). The existing
`Ft5336` driver should work as-is for single-touch. Verify register compatibility
during bring-up; if minor differences exist, parameterize via a generic or feature flag
rather than forking.

### S3. Screen configuration

All three prongs use the same Screen:
```rust
Screen::new(800, 480, Rotation::Deg0, ColorFormat::Argb8888, 60)
```

The LCDC panel is landscape-native (no rotation needed, unlike the DISCO's portrait DSI
panel rotated to landscape). This simplifies the coordinate pipeline.

### S4. CpuBlitter for rendering

No hardware 2D accelerator on AM3358 (no DMA2D equivalent). All three prongs use
`CpuBlitter` exclusively. The `BlitterRenderer<CpuBlitter, N>` path is already
battle-tested on the simulator and UEFI backends.

Star crawl effect needs a CPU-only path (currently gated on `feature = "dma2d"`).
Factor the composition logic to work with CpuBlitter — this benefits all non-DMA2D
targets.

### S5. Example crate scaffold

**New:** `examples/beaglebone-black/`

```
examples/beaglebone-black/
├── Cargo.toml            # workspace member, feature-gated entry points
├── src/
│   ├── main.rs           # Linux entry (feature = "linux")
│   ├── bare_metal.rs     # Bare-metal entry (feature = "bare_metal")
│   ├── freertos_entry.rs # FreeRTOS preemptive task entry (feature = "freertos")
│   ├── zephyr_entry.rs   # Zephyr staticlib entry (feature = "zephyr")
│   ├── lib.rs            # Shared re-exports, panic handler, allocator
│   ├── bsp/              # Board-specific code (shared across non-Linux prongs)
│   │   ├── mod.rs
│   │   ├── lcdc.rs       # LCDC timing constants + register driver
│   │   └── pins.rs       # Pin assignments (cape header → AM3358 pad)
│   └── bsp_generated/    # BSP generator output (bare-metal/FreeRTOS)
├── freertos/             # FreeRTOS C glue (port layer, heap, ISR trampolines)
├── zephyr/               # Zephyr app shell (C main, DTS overlay, prj.conf)
├── assets/               # Shared splash/icons (symlink or copy from disco)
└── README.md
```

**Cargo.toml features:**
```toml
[features]
linux = ["std"]                    # Linux userspace (armv7-unknown-linux-gnueabihf)
bare_metal = ["rlvgl-chips-ti"]    # no_std bare-metal (armv7a-none-eabi)
freertos = ["bare_metal"]          # FreeRTOS tasks on bare-metal BSP
zephyr = []                        # Zephyr staticlib (crate-type = ["staticlib"])
splash = []
```

### S6. LCDC timing constants (shared BSP)

Panel timing from NHD-7.0-800480AF-ASXP datasheet, consumed by all three prongs
(Linux DTS overlay, Zephyr DTS, bare-metal LCDC register setup):

```
Resolution:  800 x 480
Pixel clock: ~33.3 MHz (typ)
HBP: 46,  HFP: 210, HSW: 20  (horizontal back porch, front porch, sync width)
VBP: 23,  VFP: 22,  VSW: 10  (vertical)
DE mode recommended (ST7277)
Data clocked on DCLK falling edge
```

These values go in `bsp/lcdc.rs` as constants and are also used to generate the
device tree snippets for Linux and Zephyr.

### S7. Chipdb YAML for AM3358

**New:** `chipdb/rlvgl-chips-ti/db/chips/am3358.yaml`

Minimum viable content for display bring-up:
- Memory map: L4_WKUP, L4_PER, OCMC, DDR (0x80000000)
- PRCM clock gates: LCDC, I2C1/I2C2, GPIO0-3, UART0
- CTRLMOD pad mux: LCD_DATA[0:23], LCD_VSYNC, LCD_HSYNC, LCD_PCLK, LCD_AC_BIAS_EN
- LCDC peripheral: base 0x4830E000, IRQ 36
- I2C instances: I2C0 (0x44E0B000), I2C1 (0x4802A000), I2C2 (0x4819C000)

**New:** `chipdb/rlvgl-chips-ti/db/boards/beaglebone_black_nhd7.yaml`

Pin assignments mapping cape header signals to AM3358 pads:
- LCD_DATA[0:23] on expansion header P8 (pins 45-27, specific pad mapping)
- LCD_VSYNC, LCD_HSYNC, LCD_PCLK, LCD_AC_BIAS_EN
- I2C2_SDA/SCL for touch controller (P9.19/P9.20)
- Backlight enable GPIO
- Console: UART0 (P9.21/P9.22)

This YAML is consumed by bare-metal BSP generation and serves as documentation
for the Linux/Zephyr device tree overlays.

---

## Prong 1: Linux fbdev/DRM

**Target:** `armv7-unknown-linux-gnueabihf` (std, runs on Debian/Ubuntu on BBB)

**Value:** Fastest path to pixels on panel. Validates display + touch hardware.
Proves rlvgl as a lightweight embedded Linux GUI alternative.

### L1. New platform backend: `LinuxFbdevDisplay`

**File:** `platform/src/linux_fbdev.rs` (new, feature-gated on `linux_fbdev`)

Implements `DisplayDriver`:
- Open `/dev/fb0`, query `FBIOGET_VSCREENINFO` / `FBIOGET_FSCREENINFO`
- mmap framebuffer
- `screen()` → Screen from queried resolution
- `flush(area, colors)` → write ARGB8888 pixels to mmap'd buffer
  (with format conversion if panel is RGB565)
- Optional: `FBIOPAN_DISPLAY` for double-buffering

### L2. New input backend: `LinuxEvdevInput`

**File:** `platform/src/linux_evdev.rs` (new, feature-gated on `linux_fbdev`)

Implements `InputDevice`:
- Open `/dev/input/eventN` (auto-detect touch device via `EVIOCGBIT`)
- Parse `input_event` structs (EV_ABS for touch, EV_KEY for buttons)
- Map ABS_MT_POSITION_X/Y → `Event::PointerDown/Up/Move`
- Multi-touch: ABS_MT_SLOT + ABS_MT_TRACKING_ID → `Event::Touch`

### L3. Linux entry point

**File:** `examples/beaglebone-black/src/main.rs`

Standard `fn main()`:
1. Open fbdev display
2. Open evdev input
3. Create `DiscoController::new(screen, DiscoCapabilities::beaglebone_black())`
4. Main loop: poll input → dispatch events → tick → render → flush
5. Signal handling for clean shutdown

### L4. Device tree overlay

**File:** `examples/beaglebone-black/linux/BB-NHD7-CAPE.dts`

- Enable LCDC with NHD panel timing
- Disable HDMI (nxp,tda998x)
- Configure I2C2 for FT5x06 touch
- Pin mux for LCD_DATA[0:23] + sync signals
- Backlight GPIO

### L5. Build + run instructions

Cross-compile on macOS/Linux host:
```bash
cross build --target armv7-unknown-linux-gnueabihf \
  -p rlvgl-example-bbb --features linux --release
```
scp to BBB, load DT overlay, run.

---

## Prong 2: Bare-Metal

**Target:** `armv7a-none-eabi` (no_std, no OS)

**Value:** Full register-level control. Extends BSP generator to TI/Sitara vendor.
Exercises the LCDC from Rust with zero abstraction overhead. The LCDC driver and
BSP code written here is reused directly by the FreeRTOS prong.

### B1. TI BSP generator pipeline

**New files in `src/bin/creator/bsp/ti/`:**
```
ti/
├── mod.rs          # module index
├── ir.rs           # TiChip, TiBoard, TiIr structs
├── load.rs         # YAML → IR parsing, chip+board merge
├── render.rs       # IR → Jinja template rendering
└── templates/
    ├── mod.rs.jinja
    ├── pac.rs.jinja         # init() entry: clocks → pinmux → peripherals
    ├── clocks.rs.jinja      # PRCM CM_PER/CM_WKUP MODULEMODE enables
    ├── pinmux.rs.jinja      # CTRLMOD conf_* pad configuration
    ├── peripherals.rs.jinja # LCDC, I2C, UART init sequences
    └── board.rs.jinja       # Pin constants, clock frequencies
```

Wire `"ti" | "sitara"` into CLI vendor matching in `src/bin/creator/cli.rs`.

### B2. AM3358 LCDC driver (bare-metal Rust)

**File:** `examples/beaglebone-black/src/bsp/lcdc.rs` (hand-written, not generated)

The LCDC is the critical peripheral. Bare-metal init sequence:
1. PRCM: Enable LCDC module clock (CM_PER_LCDC_CLKSTCTRL)
2. CTRLMOD: Mux LCD_DATA[0:23] + sync pins to Mode 0
3. LCDC: Configure raster controller
   - Set RASTER_CTRL for active matrix, 24-bit TFT
   - Program timing registers (RASTER_TIMING_0/1/2) from S6 constants
   - Set framebuffer base address in DMA (LCDDMA_FB0_BASE/CEILING)
   - Enable raster, enable DMA
4. Backlight GPIO enable

### B3. DDR3L and boot

This is the hardest part. Options:
- **U-Boot handoff:** Use U-Boot SPL to init DDR3L + clocks, then chainload
  bare-metal Rust ELF. U-Boot already does this for Linux. Pragmatic.
- **Full bare-metal boot:** Write DDR3L EMIF controller init in Rust. Very hard,
  fragile, rarely done outside TI's own ROM/SPL code.

**Recommendation:** U-Boot SPL handoff for initial bring-up. Full boot is a
stretch goal — the LCDC/touch/rendering work is the same either way.

### B4. Bare-metal entry point

**File:** `examples/beaglebone-black/src/bare_metal.rs`

- `#[no_mangle] pub extern "C" fn main() -> !` (called from U-Boot or reset vector)
- Set up MMU + caches (Cortex-A8 specific)
- Init heap in DDR
- Init LCDC, I2C, GPIO via generated BSP + hand-written LCDC driver
- Create DiscoController
- Main loop: poll touch → dispatch → tick → render → flush to LCDC framebuffer

### B5. Linker script + flash/load

- `memory.x` for AM3358 memory layout (DDR at 0x80000000, SRAM at 0x402F0400)
- `Makefile` targets: `build-bbb`, `flash-bbb` (via JTAG or TFTP from U-Boot)

---

## Prong 3: FreeRTOS

**Target:** `armv7a-none-eabi` (no_std, FreeRTOS scheduler)

**Value:** Preemptive task scheduling on the bare-metal BSP. Proven pattern from
DISCO FreeRTOS (present/render/touch as separate tasks with semaphore-gated frame
sync). Lighter than Zephyr — just adds a scheduler on top of the same LCDC driver
and I2C touch code from Prong 2. Particularly valuable on the BBB since CPU-only
rendering (no DMA2D) benefits from isolating the render pipeline in a lower-priority
task that can be preempted by touch and present.

### F1. FreeRTOS port layer for Cortex-A8

**Dir:** `examples/beaglebone-black/freertos/`

FreeRTOS has an official Cortex-A port (`portable/GCC/ARM_CA8_AM335x/`
in the FreeRTOS-Kernel repo) that uses:
- SVC/IRQ mode switching for context save/restore
- Timer interrupt (DMTIMER) for tick
- GIC (or direct INTC on AM335x) for interrupt routing

Integrate as C sources linked into the Rust staticlib, same pattern as
`examples/stm32h747i-disco/freertos/`.

### F2. FreeRTOS task model

Same architecture as DISCO FreeRTOS (`freertos_entry.rs`):

| Task | Priority | Stack | Blocks on |
|------|----------|-------|-----------|
| present | 3 | 2 KB | LCDC end-of-frame IRQ sem |
| render | 1 | 8 KB | render-request sem |
| touch | 2 | 1 KB | periodic timer (120 Hz) |

The LCDC end-of-frame interrupt replaces the DISCO's DSI ERIF as the
present-gate signal. Touch reads FT5x06 via I2C2 (same `Ft5336` driver).

### F3. FreeRTOS entry point

**File:** `examples/beaglebone-black/src/freertos_entry.rs`

Same pattern as DISCO:
- Init BSP (LCDC, I2C, GPIO) using bare-metal code from Prong 2
- Create FreeRTOS tasks with `xTaskCreate`
- Create semaphores for frame sync (`erif_sem`, `render_sem`, `dma_done_sem`)
- Call `vTaskStartScheduler()`
- DiscoController owned by render task, commands drained each frame

### F4. ISR routing

- LCDC end-of-frame → `xSemaphoreGiveFromISR(erif_sem)`
- Touch timer → `xSemaphoreGiveFromISR(touch_sem)`
- NVIC/INTC priorities set so ISRs don't call FreeRTOS API above
  `configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY`

### F5. Build instructions

```bash
RUSTFLAGS="-C target-cpu=cortex-a8" \
cargo build \
  --target armv7a-none-eabi \
  -p rlvgl-example-bbb \
  --features freertos
```

---

## Prong 4: Zephyr RTOS

**Target:** Zephyr application for `am335x_bone_black` board

**Value:** Full RTOS ecosystem with device tree, driver model, and input subsystem.
Proven staticlib integration pattern from DISCO. Tests rlvgl Zephyr on Cortex-A
(vs Cortex-M on DISCO). Most unknowns of any prong due to Zephyr's Cortex-A8
maturity (MMU, cache management, LCDC driver availability).

### Z1. Zephyr project scaffold

**Dir:** `examples/beaglebone-black/zephyr/`

```
zephyr/
├── CMakeLists.txt      # staticlib linkage
├── prj.conf            # CONFIG_DISPLAY, CONFIG_INPUT, CONFIG_FT5336
├── boards/
│   └── am335x_bone_black.overlay   # LCDC + cape DTS overlay
├── src/
│   └── main.c          # k_thread_create → rlvgl_init()
└── Kconfig             # optional board-specific configs
```

### Z2. LCDC display driver (Zephyr)

Zephyr's AM335x support may not include an LCDC display driver. If missing:
- Write `drivers/display/display_am335x_lcdc.c` (or contribute upstream)
- Implements Zephyr `display_driver_api` (write, read, get_capabilities, blanking)
- Configure LCDC raster mode with panel timing from DTS
- Framebuffer in DDR

If exists but incomplete, extend for 24-bit RGB + DE mode.

### Z3. Zephyr staticlib entry

**File:** `examples/beaglebone-black/src/zephyr_entry.rs`

Same pattern as DISCO Zephyr:
- `extern "C" fn rlvgl_init(display_info: *const DisplayInfo)`
- Create DiscoController with BBB capabilities
- Spawn render/present threads via Zephyr `k_thread` wrappers
- Touch input via Zephyr input subsystem callback

### Z4. Touch via Zephyr input subsystem

Zephyr has FT5336 driver (`drivers/input/input_ft5336.c`). The FT5x06 on the cape
is register-compatible. Configure via DTS:
```dts
&i2c2 {
    ft5x06@38 {
        compatible = "focaltech,ft5336";
        reg = <0x38>;
        int-gpios = <&gpio1 PIN GPIO_ACTIVE_LOW>;
    };
};
```

### Z5. Build instructions

```bash
west build -b am335x_bone_black examples/beaglebone-black/zephyr
west flash
```

---

## Dependency Graph

```
                    ┌─────────────────────────────────┐
                    │  S1-S7: Shared Work              │
                    │  (capabilities, screen, touch,   │
                    │   blitter, scaffold, chipdb)     │
                    └──────────┬──────────────────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
┌─────────────────┐   ┌──────────────────┐   ┌──────────────┐
│ Prong 1: Linux  │   │ Prong 2:         │   │ Prong 4:     │
│ L1-L5           │   │ Bare-metal       │   │ Zephyr       │
│                 │   │ B1-B5            │   │ Z1-Z5        │
│ Fastest to      │   │                  │   │              │
│ screen; no BSP  │   │ LCDC driver +    │   │ Needs LCDC   │
│ gen needed      │   │ chipdb + BSP gen │   │ Zephyr driver│
└────────┬────────┘   └────────┬─────────┘   └──────────────┘
         │                     │
         │              ┌──────┴──────┐
         │              ▼             │
         │     ┌──────────────────┐   │
         │     │ Prong 3:         │   │
         │     │ FreeRTOS         │   │
         │     │ F1-F5            │   │
         │     │                  │   │
         │     │ Adds preemptive  │   │
         │     │ tasks on top of  │   │
         │     │ bare-metal BSP   │   │
         │     └──────────────────┘   │
         │                            │
         ▼                            ▼
    ┌──────────────────────────────────────────────┐
    │  Same DiscoController + widget tree           │
    │  Same CpuBlitter + BlitterRenderer            │
    │  Same FT5x06 touch (reuse ft5336.rs)          │
    │  Same 800x480 Screen, same app as DISCO/sim   │
    └──────────────────────────────────────────────┘
```

## Sequencing

| Phase | Work | Depends On | Unlocks |
|-------|------|-----------|---------|
| **Phase 0** | S1-S4: Capabilities, Screen, touch reuse audit, CpuBlitter star crawl | Nothing | All prongs |
| **Phase 1** | S5-S6: Crate scaffold + LCDC timing constants | Phase 0 | All prongs |
| **Phase 2a** | L1-L5: Linux fbdev backend + entry point | Phase 1 | Hardware validation |
| **Phase 2b** | S7: AM3358 chipdb YAML | Phase 1 | Prongs 2-4 |
| **Phase 3** | B1-B5: BSP generator + bare-metal LCDC + U-Boot boot | Phase 2a (validates HW), Phase 2b | Bare-metal demo, unlocks FreeRTOS |
| **Phase 4** | F1-F5: FreeRTOS port + preemptive task model | Phase 3 (reuses BSP) | FreeRTOS demo |
| **Phase 5** | Z1-Z5: Zephyr project + LCDC driver | Phase 2a, Phase 2b | Zephyr demo |

Linux goes first because it validates the hardware with minimal software investment.
Bare-metal follows because the LCDC register driver and BSP code it produces are
reused directly by FreeRTOS. FreeRTOS adds preemptive task scheduling on top of the
same BSP — a lighter step than Zephyr. Zephyr is last because its Cortex-A8 support
has the most unknowns (MMU gaps, uncertain LCDC driver availability).

---

## What Stays Compatible With Other Targets

| Component | BBB (all 4) | DISCO | ESP32 | Simulator |
|-----------|-------------|-------|-------|-----------|
| DiscoController | beaglebone_black() | stm32h747i_disco() | N/A (too small) | simulator() |
| Widget tree | Same | Same | Different (128x64) | Same |
| Blitter | CpuBlitter | Dma2dBlitter | CpuBlitter | WgpuBlitter |
| Touch driver | FT5x06 (= FT5336) | FT5336 | N/A | Mouse |
| Screen | 800x480 Deg0 | 800x480 Deg0* | 128x64 | 800x480 |
| App crate | disco-demo | disco-demo | N/A | demo or disco-demo |
| FreeRTOS | Cortex-A8 port | Cortex-M7 port | N/A | N/A |

*DISCO is physically portrait, rotated to landscape in software.

### Cross-platform parity (DISCO vs BBB)

The DISCO supports bare-metal, FreeRTOS, and Zephyr. The BBB adds Linux and
targets all four. The same `DiscoController` + command queue pattern is used
everywhere. Platform differences are isolated to:

- **Display driver:** LTDC+DSI (DISCO) vs LCDC parallel RGB (BBB) vs fbdev (Linux)
- **Touch transport:** I2C4 (DISCO) vs I2C2 (BBB) vs evdev (Linux)
- **Frame sync:** DSI ERIF (DISCO) vs LCDC EOF IRQ (BBB bare-metal/FreeRTOS)
- **Scheduler:** cortex-m-rt (DISCO bare-metal) vs Cortex-A8 reset (BBB bare-metal)
- **Task model:** SVC/PendSV (DISCO FreeRTOS) vs SVC/IRQ mode (BBB FreeRTOS)

---

## Documentation

This plan of plans is maintained as a durable reference in the repo:

**File:** `docs/BEAGLEBONE-BLACK.md`

This mirrors how the DISCO has `docs/STM32H747I-DISCO.md` plus per-platform guide
directories (`docs/disco-freertos-guide/`, `docs/disco-zephyr-guide/`, etc.). As each
prong matures, it will get its own guide directory:

- `docs/bbb-linux-guide/` — Linux fbdev bring-up, DT overlay, cross-compile
- `docs/bbb-bare-metal-guide/` — BSP generation, U-Boot handoff, JTAG debug
- `docs/bbb-freertos-guide/` — FreeRTOS Cortex-A8 port, task model, frame sync
- `docs/bbb-zephyr-guide/` — Zephyr west build, LCDC driver, touch config

The initial commit of this plan creates `docs/BEAGLEBONE-BLACK.md` as a copy of
the plan of plans. It evolves as work progresses.

---

## Verification Strategy

Each prong has its own verification, but the **cross-prong check** is:

1. **Visual parity:** Same widget tree renders identically on BBB Linux, BBB bare-metal,
   BBB FreeRTOS, BBB Zephyr, DISCO, and desktop simulator. Use `playit` framebuffer
   dump (`D` command) or screenshot comparison.

2. **Touch parity:** Tap on settings icon → wing opens on all platforms. Touch
   coordinates map correctly (no rotation confusion since BBB is landscape-native).

3. **App compatibility:** `DiscoController` compiles and links for all targets without
   `#[cfg]` divergence. Capability differences are runtime, not compile-time.

4. **Regression:** Existing targets (DISCO bare-metal, FreeRTOS, Zephyr, simulator)
   continue to pass pre-publish validation after shared code changes (S1-S4).
