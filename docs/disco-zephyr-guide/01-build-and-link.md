<!--
01-build-and-link.md - Volume V Chapter 1: Two-step build, prj.conf,
DTS overlays.
-->

**[<- Prev](README.md) . [Index](README.md) . [Next ->](02-c-shell-and-ffi.md)**

# Chapter 1 — Build & Link

## Volume II reference

Vol II built a single Rust binary targeting `thumbv7em-none-eabihf`
with `cortex-m-rt` providing the vector table and linker script.
Zephyr replaces that: the linker script, startup code, and vector
table come from the Zephyr kernel. Rust compiles as a **staticlib**
that Zephyr's CMake links into the final ELF.

## What this chapter covers

The two-step build flow, critical `prj.conf` flags, and the DTS
overlays that switch between video mode and adapted command mode.

## The Zephyr delta

Bare-metal and FreeRTOS share a linker script and `cortex-m-rt`
entry. Zephyr owns the boot sequence: `_start` -> kernel init ->
`SYS_INIT` hooks -> `main()`. Rust code enters at `rlvgl_init()`,
called from C `main()`.

## Walkthrough

### 1. Two-step build

```bash
# Step 1: Build Rust staticlib
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco --lib \
  --features cm7,dma2d,splash,desktop,zephyr

# Step 2: Build Zephyr image (links the staticlib)
cd examples/stm32h747i-disco/zephyr
west build -b stm32h747i_disco/stm32h747xx/m7 \
  -p always \
  -- -DSHIELD=st_b_lcd40_dsi1_mb1166_a09
```

The `Makefile` wraps this as `make zephyr-disco`.

### 2. Critical prj.conf flags

| Config | Value | Why |
|--------|-------|-----|
| `CONFIG_MAIN_STACK_SIZE` | 16384 | Render loop + StarCrawl scanline + RotatedRenderer |
| `CONFIG_HEAP_MEM_POOL_SIZE` | 65536 | Zephyr kernel heap (separate from Rust 64 KB heap) |
| `CONFIG_STM32_LTDC_FB_NUM` | 2 | Double-buffering for ping-pong framebuffers |
| `CONFIG_INPUT_MODE_SYNCHRONOUS` | y | Input callbacks inline — avoids dropped events |
| `CONFIG_INPUT_FT5336_PERIOD` | 10 | Touch poll every 10 ms (~100 Hz) |
| `CONFIG_DYNAMIC_INTERRUPTS` | y | `irq_connect_dynamic()` for DMA2D + DSI ISRs |
| `CONFIG_FAT_FILESYSTEM_ELM` | y | FatFS for SD card file browser |

### 3. Video mode (default)

Zephyr's DSI driver (`dsi_stm32`) + NT35510 panel driver +
LTDC driver bring up the display in continuous landscape
(800x480) video mode. No DTS overlay needed.

### 4. Adapted command mode overlay

`adapted_cmd.overlay` disables Zephyr's display drivers:

```dts
&zephyr_mipi_dsi { status = "disabled"; };
&nt35510 { status = "disabled"; };
&zephyr_lcd_controller { status = "disabled"; };
/ { chosen { /delete-property/ zephyr,display; }; };
```

Build with:
```bash
west build ... -- \
  -DSHIELD=st_b_lcd40_dsi1_mb1166_a09 \
  -DEXTRA_DTC_OVERLAY_FILE=adapted_cmd.overlay
```

Rust then initializes DSI + LTDC from scratch via
`display_init::init_full_adapted_cmd()`.

### 5. Feature gating

```toml
[features]
zephyr = []
adapted_cmd = []
```

Both are local to the disco example crate. `zephyr` gates
Zephyr-specific code paths. `adapted_cmd` selects between
video mode (landscape, Zephyr drivers) and adapted command mode
(portrait, Rust raw-register init).

## Verify

```bash
make zephyr-disco       # video mode
make zephyr-disco-flash # flash via probe-rs
```

Serial output should show the SRAM3 breadcrumbs and the boot
banner from `main.c`.

## Going deeper

- [`docs/ZEPHYR.md`](../ZEPHYR.md) — SDK install, environment,
  troubleshooting.
- [`examples/stm32h747i-disco/zephyr/prj.conf`](../../examples/stm32h747i-disco/zephyr/prj.conf)
  — full Kconfig.

---

**[<- Prev](README.md) . [Index](README.md) . [Next ->](02-c-shell-and-ffi.md)**
