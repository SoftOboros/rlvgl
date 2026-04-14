<!--
docs/ZEPHYR.md - Zephyr RTOS integration with rlvgl.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Zephyr Integration

Guide to building, flashing, and running rlvgl on Zephyr RTOS, focused on
the STM32H747I-DISCO board with the MB1166-A09 LCD shield.

---

## Contents

1. [Prerequisites](#prerequisites)
2. [Zephyr SDK install](#zephyr-sdk-install)
3. [Environment variables](#environment-variables)
4. [Common environment issues](#common-environment-issues)
5. [Build the rlvgl Zephyr image (video mode)](#build-video-mode)
6. [Flash and run](#flash-and-run)
7. [Architecture: how Zephyr links the Rust staticlib](#architecture)
8. [STM32H747I-DISCO standard case (video mode)](#stm32h747i-disco-video-mode)
9. [Adapted command mode (preliminary, DMA2D pipeline)](#adapted-command-mode)
10. [Troubleshooting](#troubleshooting)

---

## Prerequisites

- **macOS / Linux** host (instructions below cover macOS arm64; Linux is
  the same flow with different SDK URLs)
- **Zephyr workspace** at `~/zephyrproject`. If you don't have it yet:
  ```bash
  pip3 install --user west
  cd ~ && west init zephyrproject && cd zephyrproject && west update
  ```
  See the [Zephyr getting-started guide](https://docs.zephyrproject.org/)
  for full instructions.
- **`probe-rs`** for flashing via ST-Link. `cargo install probe-rs --locked`.
- **CMake**, **ninja**, **python3** in `PATH`. On macOS:
  `brew install cmake ninja python3`.

The rlvgl Cargo build will use whatever ARM toolchain Rust ships for
`thumbv7em-none-eabihf` — no extra configuration needed for the Rust
staticlib. The Zephyr image build needs the SDK (next section).

---

## Zephyr SDK install

The repo includes `make zephyr-sdk-install` which downloads the
**Zephyr SDK 0.16.8** (matched to Zephyr 4.1.x) plus the toolchains
needed for the chips rlvgl currently targets:

```bash
make zephyr-sdk-install
```

This installs to `~/zephyr-sdk-0.16.8/` and registers the SDK with
CMake's user package registry (`~/.cmake/packages/Zephyr-sdk`). The
toolchains it includes by default are:

- `arm-zephyr-eabi` — Cortex-M / Cortex-R (STM32H7, etc.)
- `aarch64-zephyr-elf` — ARM 64-bit
- `xtensa-espressif_esp32_zephyr-elf`
- `xtensa-espressif_esp32s2_zephyr-elf`
- `xtensa-espressif_esp32s3_zephyr-elf`

To install other toolchains, override `ZEPHYR_SDK_TCS`:

```bash
make zephyr-sdk-install \
  ZEPHYR_SDK_TCS="arm-zephyr-eabi riscv64-zephyr-elf"
```

To pin a different SDK version:

```bash
make zephyr-sdk-install ZEPHYR_SDK_VERSION=0.17.4
```

> **Why 0.16.8 and not the latest?**
> Zephyr 4.1.x ships picolibc bindings matched to the **0.16.x** SDK.
> SDK 0.17 changed the lock signature (`void(struct __lock **)` vs
> `void(void **)`) and breaks compilation. SDK 1.0 explicitly refuses
> any pre-1.0 `find_package` request and is also incompatible. If you
> upgrade Zephyr to 4.2+, switch the SDK accordingly.

Verify the install:

```bash
make zephyr-disco-sdk-check
```

---

## Environment variables

The Makefile sets sensible defaults; override on the command line as
needed.

| Variable | Default | Purpose |
|---|---|---|
| `ZEPHYR_BASE` | `~/zephyrproject` | Path to `west init` workspace |
| `ZEPHYR_BUILD` | `~/zephyrproject/build` | west build output dir |
| `ZEPHYR_BOARD` | `stm32h747i_disco/stm32h747xx/m7` | Board target |
| `ZEPHYR_SHIELD` | `st_b_lcd40_dsi1_mb1166_a09` | Display shield |
| `ZEPHYR_SDK_INSTALL_DIR` | `~/zephyr-sdk-0.16.8` | SDK location |
| `ZEPHYR_SDK_VERSION` | `0.16.8` | For `zephyr-sdk-install` |
| `ZEPHYR_SDK_HOST` | `macos-aarch64` | SDK host triple |
| `ZEPHYR_TOOLCHAIN_VARIANT` | `zephyr` | Or `gnuarmemb` for stm32cube GCC |
| `GNUARMEMB_TOOLCHAIN_PATH` | stm32cube bundle | Used when variant=gnuarmemb |

Run `make zephyr-disco-help` for the live values.

You do **not** need to source any Zephyr environment script. The
Makefile passes the right env via `env VAR=value west build ...`.

---

## Common environment issues

### `direnv: unloading` / build env disappears

If you have direnv loaded for your `~/rlvgl` checkout, `cd
~/zephyrproject` will *unload* those env vars. The Makefile compensates
by passing all the necessary variables via `env` directly to `west`,
so you can run `make zephyr-disco` from `~/rlvgl/` without worrying.

If you invoke `west` manually, set the env yourself:

```bash
env ZEPHYR_TOOLCHAIN_VARIANT=zephyr \
    ZEPHYR_SDK_INSTALL_DIR=~/zephyr-sdk-0.16.8 \
    west build ...
```

### `Could not find a package configuration file provided by "Zephyr-sdk"`

Either the SDK isn't installed, or it's not registered with CMake.
Re-run:

```bash
make zephyr-sdk-install
```

Or, if the SDK is already installed, just re-register:

```bash
cd ~/zephyr-sdk-0.16.8 && ./setup.sh -c
```

### `picolibc-hooks.h:13` error: `conflicting types for '__lock___libc_recursive_mutex'`

You're using SDK 0.17+ with Zephyr 4.1.x. Downgrade:

```bash
rm -rf ~/zephyr-sdk-0.17.* ~/.cmake/packages/Zephyr-sdk/*
make zephyr-sdk-install
```

### probe-rs `A timeout occurred during an operation`

The CM7 may be in a state where the AHB-AP debug port is busy
(continuous DSI/DMA traffic). Workarounds:

- `probe-rs reset --connect-under-reset` — halts before it gets busy
- `probe-rs download --connect-under-reset` — same, for re-flashing
- Brief power cycle of the board

### Serial monitor shows nothing after a fresh flash

USART1 init takes a few hundred ms after reset. Wait a couple seconds
before starting the monitor, and use a long timeout (`--timeout 10`).

---

## Build video mode

Stock Zephyr DSI driver in continuous video scan mode. Everything
except DMA2D M2M (memory-to-memory) blits works.

```bash
make zephyr-disco
```

Under the hood this runs:

1. `cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco --lib
   --features cm7,splash,desktop,dma2d,zephyr`
2. `cd ~/zephyrproject && west build ... -DSHIELD=st_b_lcd40_dsi1_mb1166_a09`

The west build links the Rust staticlib at
`target/thumbv7em-none-eabihf/debug/librlvgl_example_disco.a` into the
Zephyr image.

Expected output (truncated):

```
Memory region         Used Size  Region Size  %age Used
           FLASH:      230664 B         1 MB     22.00%
             RAM:      147744 B       512 KB     28.18%
          SDRAM2:       3000 KB        32 MB      9.16%
```

`SDRAM2: 3000 KB` is Zephyr's LTDC framebuffer (800×480 × 4 bytes × 2
buffers = 3 MB). If you see `0 B`, the LTDC driver isn't binding —
check that the shield is set correctly.

---

## Flash and run

```bash
make zephyr-disco-flash
```

This runs `probe-rs download --chip STM32H747XIHx ~/zephyrproject/build/zephyr/zephyr.elf`
followed by `probe-rs reset`.

For serial output:

```bash
python3 ~/rlvgl/tools/serial_monitor.py --timeout 10
```

You should see:

```
*** Booting Zephyr OS build v4.1.0 ***
rlvgl-zephyr: starting
rlvgl-zephyr: display 800x480 fmt=8
rlvgl-zephyr: blanking off
rlvgl-zephyr: fb_front=0xd0000000 fb_back=0xd0177000 fb_len=1536000
rlvgl-zephyr: SD mounted at /SD:
rlvgl-zephyr: calling rlvgl_init
```

The display should show the splash image, then the rlvgl widget tree.

---

## Architecture

rlvgl on Zephyr uses a **two-language layered build**:

```
┌──────────────────────────────────────────────────────────────┐
│  Zephyr application (C)                                      │
│  examples/stm32h747i-disco/zephyr/src/main.c                 │
│                                                              │
│  - Defines K_SEM kernel objects (erif_sem, dma2d_done_sem)   │
│  - Registers IRQ handlers (DMA2D=90, DSI=123)                │
│  - Initializes display + SD via Zephyr drivers               │
│  - Calls rlvgl_init() in Rust                                │
└────────────────────────────┬─────────────────────────────────┘
                             │ FFI (extern "C")
┌────────────────────────────▼─────────────────────────────────┐
│  Rust staticlib (rlvgl_example_disco)                        │
│  examples/stm32h747i-disco/src/lib.rs                        │
│                                                              │
│  - rlvgl_init() — render loop, widget tree, splash decode    │
│  - rlvgl_dsi_isr / rlvgl_dma2d_isr — IRQ delegation          │
│  - rlvgl_touch_event / rlvgl_key_event — input intake        │
│  - rlvgl_readdir — filesystem callback                       │
│  - ZephyrFrameSync — implements FrameSync/Dma2dSync traits   │
│    backed by k_sem instead of raw atomics                    │
│                                                              │
│  Shared with the bare-metal binary:                          │
│    - rlvgl_app_disco_demo (the entire application)           │
│    - rlvgl_widgets, rlvgl_core, rlvgl_platform, rlvgl_ui     │
│    - star_crawl, file_browser_panel, event_overlay           │
└──────────────────────────────────────────────────────────────┘
```

### What's shared vs platform-specific

**Shared** (compiled identically for both bare-metal and Zephyr):
- All widgets, the application controller, the file browser panel
- The render loop's logic (call `controller.tick()`, render to back
  buffer, call present)
- `FrameSync` / `Dma2dSync` / `ScopeProbe` traits in `rlvgl-platform`
- `dsi_cmd_mode` and `display_init` modules (used by Zephyr ACM path)

**Platform-specific**:
- *Bare-metal*: owns all PAC peripherals via `cortex-m-rt` entry,
  HAL-driven clock setup, inline DSI register init.
- *Zephyr*: hands clocks/SDRAM/DSI/LTDC/I2C/SD/Filesystem to Zephyr
  drivers; Rust borrows what it needs via `Peripherals::steal()` for
  DMA2D and (in ACM mode) the DSI host.

### Cargo features

| Feature | What it does |
|---|---|
| `zephyr` | Build as `staticlib`, exclude `cortex-m-rt::entry` |
| `cm7` | STM32H747 Cortex-M7 PAC, stm32h7xx-hal |
| `splash` | Include the splash bitmap |
| `desktop` | Restore-pristine-each-frame rendering model |
| `dma2d` | DMA2D-accelerated blits |
| `adapted_cmd` | Optional: full Rust DSI init in adapted command mode |

Default Zephyr build features: `cm7,splash,desktop,dma2d,zephyr`.

---

## STM32H747I-DISCO video mode

The standard, working integration. Zephyr does the heavy lifting:

1. **Boot**: Zephyr's STM32 SoC init configures clocks (PLL1/PLL2/PLL3),
   FMC SDRAM, MPU regions (SDRAM2 marked `MPU_RAM_NOCACHE`), and
   peripheral clocks.
2. **Display init**: Zephyr's `dsi_stm32` driver brings DSI up in
   **video mode** (continuous scan), runs the NT35510 panel init via
   DCS commands, and configures LTDC for 800×480 landscape (the panel
   is 480×800 portrait but the shield's `rotation = <90>` property
   tells the panel to rotate via MADCTL).
3. **C `main()`** calls `display_get_framebuffer()` to grab the FB
   address allocated by the LTDC driver in SDRAM2, then calls
   `rlvgl_init()` with that address.
4. **Rust** decodes the splash into both buffers (with a 90° CW rotation
   to match the LTDC's landscape scan order), builds the widget tree,
   and enters its render loop. Each frame:
   - Restore the pristine splash background to the back buffer
   - Walk the widget tree, drawing into the back buffer (CPU or DMA2D)
   - Call `display_write()` to swap front/back

Because the LTDC scans continuously in video mode, **DMA2D M2M
transfers deadlock** under sustained load — the LTDC monopolizes the
SDRAM AXI bus, and DMA2D's read-side AXI requests never complete.
DMA2D R2M (register-to-memory) fills work fine because they only
write. CPU-driven rendering also works because the Cortex-M7 has its
own AXI initiator (INI2) and doesn't compete with LTDC (INI6) the
same way DMA2D (INI5) does.

This mode is fine for any UI that doesn't need DMA2D blits or blends.

---

## Adapted command mode

> **Status: preliminary.** Builds, boots, panel lights up, but display
> still shows incorrect content as of v0.2.0. See the
> "Outstanding issues" section below.

Adapted command mode (DSIM=1 in `DSI_WCFGR`) gives the host explicit
control over each scan: LTDC scans only when `DSI_WCR.LTDCEN` is
pulsed, and the wrapper auto-clears LTDCEN + sets ERIF when the scan
completes. The ERIF ISR can then run DMA2D with exclusive SDRAM access
before pulsing LTDCEN again for the next frame.

This is how the bare-metal path operates; for Zephyr it requires
**bypassing Zephyr's video-mode DSI driver** (it can't do command mode)
and bringing DSI + LTDC up in adapted command mode from Rust directly.

### Architecture (ACM)

The ACM path is gated behind the `adapted_cmd` Cargo feature, plus a
DTS overlay that disables Zephyr's display nodes:

```
examples/stm32h747i-disco/zephyr/adapted_cmd.overlay
```

Disables:
- `&zephyr_mipi_dsi` (DSI host)
- `&nt35510` (panel)
- `&zephyr_lcd_controller` (LTDC)
- `chosen { zephyr,display }` (display chosen reference)

Zephyr still provides:
- Clocks (`&pll3` configured via shield overlay → 27.5 MHz pixel clock)
- SDRAM (`&sdram2`, MPU NOCACHE)
- GPIO bank power (selectively — see below)
- I2C, SDMMC, FatFS, kernel, console

Rust does:
- Enable LTDC + DSI + DMA2D + GPIOG + GPIOJ peripheral clocks
  (Zephyr won't, since the display nodes are disabled)
- Force PLL3ON if Zephyr left it off
- Full DSI bring-up sequence (RM0399 §34.14 steps 2–14):
  - Regulator enable + wait RRS
  - DSI PLL config (HSE/5 × 100 / 1 = 500 Mbps/lane)
  - D-PHY init (2 lanes)
  - Lane timings, flow control, video mode timings
  - Adapted command mode config (WCFGR, CMCR, LCCR, WIER)
  - Panel reset (PG3 toggle) + full NT35510 DCS init
- LTDC config (timing, layer, GCR.LTDCEN)
- Backlight on (PJ12 high)

The Rust code is in `platform/src/display_init.rs` and shares the
adapted-cmd register helpers (`configure_adapted_cmd_mode`, `present`,
`handle_erif_isr`, etc.) in `platform/src/dsi_cmd_mode.rs` with the
bare-metal path.

### Build and flash

```bash
make zephyr-disco-acm           # build with adapted_cmd feature + overlay
make zephyr-disco-flash         # flash via probe-rs
```

### Diagnostics

The C `main()` writes breadcrumbs to SRAM3 (`0x38000200`/`0x38000204`)
at known points so you can verify init progress without needing serial
output:

```bash
probe-rs read --chip STM32H747XIHx b32 0x38000200 2
# Expected: b0070010 b1a10013
#   0xB0070010 = C main reached "about to call rlvgl_init"
#   0xB1A10013 = Rust display_init::init_full_adapted_cmd returned TRUE
#   0xDEAD_D51A at 0x38000204 = init failed (some step returned false)
```

Inspect runtime state:

```bash
# DSI wrapper config
probe-rs read --chip STM32H747XIHx b32 0x50000400 4   # WCFGR/WCR/WIER/WISR
# DSI host errors
probe-rs read --chip STM32H747XIHx b32 0x500000bc 2   # ISR0/ISR1
# LTDC global timing + layer
probe-rs read --chip STM32H747XIHx b32 0x50001008 4   # SSCR/BPCR/AWCR/TWCR
probe-rs read --chip STM32H747XIHx b32 0x50001018 1   # GCR
probe-rs read --chip STM32H747XIHx b32 0x50001084 1   # L1CR
probe-rs read --chip STM32H747XIHx b32 0x500010ac 1   # L1CFBAR
# Framebuffer content
probe-rs read --chip STM32H747XIHx b32 0xd0000000 8
```

If probe-rs times out reading peripheral registers while the firmware
is running, use `--connect-under-reset` (this halts the chip first;
all registers will read 0 because nothing has run).

### Outstanding issues (v0.2.0)

The ACM path **builds and boots** but the display does not yet show
the splash correctly. Confirmed working:

- Boot reaches `rlvgl_init` in Rust
- `display_init::init_full_adapted_cmd()` returns success
- DSI host has no errors (`ISR0/ISR1 = 0`)
- LTDC config sticks (after a workaround re-write in `zephyr_entry.rs`
  — see comment about Zephyr SYS_INIT clearing LTDC regs)
- Framebuffer has decoded splash data at both `0xD0000000` and
  `0xD0800000`
- `WCFGR` shows DSIM=1 (adapted command mode), AR=0 (manual refresh)
- Backlight enables

Not yet working:

- Display shows random / corrupted content rather than the splash
- Likely root cause: LTDC scan triggered by `WCR.LTDCEN` pulse may not
  be reaching the panel correctly. Suspects: panel column/page
  addressing, DSI WMS packet sizing, or PLL3 frequency mismatch
  between `display_init` math (27.5 MHz) and the actual configured
  `pll3_r_ck`.

Next-session investigation plan is in `project_zephyr_dma2d_bus.md`.

---

## Troubleshooting

### "Display not ready" at boot

Means `device_is_ready(g_disp)` returned false. Either:
- The shield isn't being included in the build (`-DSHIELD=...` missing
  or wrong name)
- The panel driver init failed (check serial for `nt35510` errors)
- For ACM mode: this is expected — the C code skips `device_is_ready`
  when `DT_HAS_CHOSEN(zephyr_display)` is unset.

### `SDRAM2: 0 B` in build output (video mode)

Zephyr's LTDC driver should allocate ~3 MB. If 0 B, the driver isn't
binding. Check:
- The `&sdram2` node has `zephyr,memory-attr =
  <(DT_MEM_ARM(ATTR_MPU_RAM_NOCACHE))>` (set by shield overlay)
- The `&zephyr_lcd_controller` is `status = "okay"` and references
  `ext-sdram = <&sdram2>`
- For ACM mode: `0 B` is expected — Rust manages the FB at fixed
  addresses (`0xD0000000` / `0xD0800000`).

### "rlvgl-zephyr: SD mount failed (-2)"

`-2` is `-ENOENT` — no SD card detected. Insert one and reset, or
ignore (file browser will be empty but the rest of the demo runs).

### Touch input swapped X/Y or inverted

Zephyr's FT5336 driver reports raw panel coordinates. The disco shield
is mounted in landscape rotation, so the C input callback in `main.c`
forwards raw coordinates to Rust which transforms them at the dispatch
site. If touch is mis-mapped, edit the per-axis mapping in
`zephyr_entry.rs::take_touch()`.

### "FATAL ERROR: command exited with status 1"

That's just west propagating CMake's exit code. Scroll up for the
actual error.

### `Could not find a package configuration file provided by "Zephyr-sdk"`

See [Common environment issues](#common-environment-issues).

---

## Related documentation

- `docs/STM32H747I-DISCO.md` — board-specific hardware notes (pinmap,
  shield wiring, peripheral selection)
- `docs/STM32H747I-DISCO-BRINGUP.md` — bare-metal bring-up history and
  DSI / LTDC sequencing notes
- `docs/MAKE.md` — full Makefile target reference
- `examples/stm32h747i-disco/zephyr/CMakeLists.txt` — Zephyr application
  CMake, shows how the Rust staticlib is consumed
- `platform/src/dsi_cmd_mode.rs` — shared adapted command mode register
  configuration used by both bare-metal and Zephyr ACM
- `platform/src/display_init.rs` — full DSI + LTDC init sequence
  (raw-register, no PAC dependency) used by Zephyr ACM
