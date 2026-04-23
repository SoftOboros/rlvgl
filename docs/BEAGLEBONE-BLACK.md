# BeagleBone Black + NHD-7.0CTP-CAPE-P

Comprehensive guide for running rlvgl on the BeagleBone Black (AM3358) with
the Newhaven NHD-7.0CTP-CAPE-P 7" IPS capacitive touch display cape.

This document covers hardware setup, SD card preparation, display bring-up,
and the four-prong software architecture (Linux, bare-metal, FreeRTOS, Zephyr).
It will be split into tutorial chapters as each section matures.

---

## Hardware

### Bill of Materials

| Item | Part Number | Notes |
|------|-------------|-------|
| BeagleBone Black | Rev D (AM3358BZCZ100) | Cortex-A8 @ 1 GHz, 512 MB DDR3L |
| Display cape | NHD-7.0CTP-CAPE-P | 7" IPS, 800x480, cap touch, BBB cape form factor |
| Power supply | 5V 2A, 5.5x2.1mm barrel | **Required** — USB 500 mA is not enough for cape+backlight |
| USB cable | Mini-B to A | Data + serial console to host Mac/PC |
| microSD card | 4 GB+ (any speed class) | Primary boot device when cape is attached |

### Power Budget

The cape draws ~450 mA from the 5V rail (backlight: 180 mA @ 9.3V boosted
from 5V, panel logic: 55 mA, touch: 15 mA). The BBB itself draws ~300 mA.
**Total: ~750 mA — well over USB's 500 mA limit.**

When powered via USB only, the BBB's `VDD_5V` expansion header rail is not
supplied (TPS65217C PMIC limitation). The cape's backlight boost converter
needs this rail. **Always use the barrel jack when the cape is attached.**

Both barrel jack and USB can be connected simultaneously. The PMIC
automatically prefers DC power while USB provides data (serial console +
Ethernet gadget).

### Critical Hardware Constraint: LCD vs eMMC Pin Conflict

**LCD_DATA[0:7] share physical pins with eMMC (MMC1) data lines on the
BBB P8 expansion header.** Setting these pins to LCD Mode 0 kills eMMC
access, which kills the rootfs if booting from eMMC.

**When the NHD cape is attached and LCD pins are active, the BBB must
boot from the microSD card.** This is a known BBB hardware limitation
shared by all 4.3"/7" LCD capes. The bare-metal and FreeRTOS prongs
don't use eMMC rootfs so they can set LCD pin mux freely.

### Cape Identification

The NHD-7.0CTP-CAPE-P has an I2C EEPROM that identifies it to U-Boot as
`BB-BONE-NH7C-01`. U-Boot passes this as a kernel command line parameter:
`uboot_detected_capes=BB-BONE-NH7C-01`. No built-in U-Boot overlay exists
for this cape — display configuration is our responsibility.

### Panel Specifications (NHD-7.0-800480AF-ASXP)

| Parameter | Value |
|-----------|-------|
| Resolution | 800 x 480 |
| Interface | 24-bit parallel RGB |
| Driver IC | Sitronix ST7277 (DE mode recommended) |
| Pixel clock | 33.3 MHz typical |
| HBP / HFP / HSW | 46 / 210 / 20 DCLK cycles |
| VBP / VFP / VSW | 23 / 22 / 10 lines |
| Data clock edge | Falling (DCLK) |
| Backlight | 180 mA @ 9.3V (boost from 5V on cape) |
| Touch | FT5x06 capacitive, I2C @ 0x38 |

### AM3358 LCDC Register Summary

Base address: `0x4830_E000` (TRM section 13.5.1)

| Register | Offset | Key Fields |
|----------|--------|------------|
| LCD_CTRL | 0x04 | MODESEL (raster=1), CLKDIV [15:8] |
| RASTER_CTRL | 0x28 | LCDEN, LCDTFT, TFT24, TFT24_UNPACKED, PALMODE |
| RASTER_TIMING_0 | 0x2C | HBP [31:24], HFP [23:16], HSW [15:10], PPL [9:0] |
| RASTER_TIMING_1 | 0x30 | VBP [31:24], VFP [23:16], VSW [15:10], LPP [9:0] |
| RASTER_TIMING_2 | 0x34 | IPC (bit 11), IHS (12), IVS (13), LPP_B10 (26) |
| LCDDMA_CTRL | 0x40 | BURST_SIZE, FIFO_TH, FRAME_MODE |
| LCDDMA_FB0_BASE | 0x44 | Framebuffer physical start address |
| LCDDMA_FB0_CEILING | 0x48 | Framebuffer physical end address |
| IRQENABLE_SET | 0x60 | EOF0 (bit 8) for frame-complete interrupt |
| CLKC_ENABLE | 0x6C | DMA_CLK (bit 2), CORE_CLK (bit 0) |

PRCM clock enable: `CM_PER_LCDC_CLKCTRL` at `0x44E0_0018`, MODULEMODE=0x2.

Pin mux: CTRLMOD `conf_lcd_data0` at `0x44E1_08A0` through
`conf_lcd_ac_bias_en` at `0x44E1_090C`. Mode 0 = LCD function.

---

## SD Card Preparation

### Step 1: Download the Bookworm Image

The BBB needs Debian 12 (Bookworm) with kernel 6.12.x — this kernel has
`tilcdc` and `panel-simple` built-in (`=y`, not modules).

```bash
curl -L -o /tmp/bbb-bookworm.img.xz \
  "https://files.beagle.cc/file/beagleboard-public-2021/images/am335x-debian-12.13-base-v6.12-armhf-2026-03-17-4gb.img.xz"
```

### Step 2: Flash to microSD

```bash
diskutil unmountDisk /dev/diskN          # Replace N with your SD card
xzcat /tmp/bbb-bookworm.img.xz | sudo dd of=/dev/rdiskN bs=4m
sync
```

### Step 3: Configure Credentials

After flashing, the SD card's boot partition mounts as `BOOT` on macOS.
Edit `sysconf.txt` to set the username, password, and SSH key:

```
user_name=debian
user_password=YourPasswordHere
user_authorized_key=ssh-rsa AAAA... you@host
```

This avoids the broken SSH forced-password-change flow on Bookworm.

### Step 4: Disable eMMC Boot (One-Time)

Boot the BBB **without** the cape from the SD card (hold S2 during power-on).
Once booted, SSH in and zero the eMMC SPL so the ROM bootloader falls
through to SD automatically:

```bash
# The SPL lives at raw sector 256 (offset 0x20000) on the eMMC
echo "$PASSWORD" | sudo -S dd if=/dev/zero of=/dev/mmcblk1 bs=512 seek=256 count=64
```

After this, the BBB boots from SD without holding S2. To restore eMMC
boot, reflash the eMMC with the BB imager tool.

### Step 5: Patch the DTB for LCD Output

The stock DTB wires the LCDC to the HDMI bridge (TDA19988). We need to:
1. Disable the TDA19988
2. Add a panel node with our timing
3. Add LCD pin mux
4. Wire the LCDC endpoint to the panel

**Important:** The correct DTB filename on Bookworm is
`am335x-boneblack-uboot.dtb` (not `am335x-boneblack.dtb`, which doesn't
exist in this image).

**Important:** The `bbbio-set-sysconf` service runs on first boot and
reinstalls DTBs from the kernel package. Patch the DTB **after** first
boot completes (the service shows `inactive dead`).

Use `fdtoverlay` to apply a compiled overlay to the binary DTB — this
avoids the broken phandle references that `dtc` decompile/recompile
introduces:

```bash
# On the BBB:
KVER=$(uname -r)
DTB=/boot/dtbs/$KVER/am335x-boneblack-uboot.dtb
cp $DTB ${DTB}.orig

# Disable HDMI bridge
fdtput -t s $DTB \
  /ocp/interconnect@44c00000/segment@200000/target-module@b000/i2c@0/tda19988@70 \
  status "disabled"

# Compile and apply the LCD overlay
dtc -I dts -O dtb -o /tmp/lcd.dtbo -@ /path/to/lcd-overlay.dts
fdtoverlay -i $DTB -o ${DTB}.new /tmp/lcd.dtbo
mv ${DTB}.new $DTB

# Add missing properties that panel-simple requires
fdtput -t i $DTB /panel connector-type 14    # DRM_MODE_CONNECTOR_DPI
fdtput -t i $DTB /panel bus-format 0x1013    # MEDIA_BUS_FMT_RGB888_1X24
```

The overlay DTS (`tools/patch-dtb-lcd.sh` automates this):

```dts
/dts-v1/;
/plugin/;

&{/} {
    panel {
        compatible = "innolux,at070tn92";  /* 800x480, timing-compatible */
        status = "okay";
        port {
            panel_in: endpoint {
                remote-endpoint = <&lcdc_0>;
            };
        };
    };
};

&lcdc {
    pinctrl-names = "default";
    pinctrl-0 = <&bb_lcd_pins>;
    port {
        lcdc_0: endpoint@0 {
            remote-endpoint = <&panel_in>;
        };
    };
};

&am33xx_pinmux {
    bb_lcd_pins: bb-lcd-pins {
        pinctrl-single,pins = <
            0x0a0 0x08 0x0a4 0x08 0x0a8 0x08 0x0ac 0x08  /* lcd_data0-3 */
            0x0b0 0x08 0x0b4 0x08 0x0b8 0x08 0x0bc 0x08  /* lcd_data4-7 */
            0x0c0 0x08 0x0c4 0x08 0x0c8 0x08 0x0cc 0x08  /* lcd_data8-11 */
            0x0d0 0x08 0x0d4 0x08 0x0d8 0x08 0x0dc 0x08  /* lcd_data12-15 */
            0x0e0 0x08 0x0e4 0x08 0x0e8 0x08 0x0ec 0x08  /* lcd_data16-19 */
            0x0f0 0x08 0x0f4 0x08 0x0f8 0x08 0x0fc 0x08  /* lcd_data20-23 */
            0x100 0x08 0x104 0x08 0x108 0x08 0x10c 0x08  /* vsync,hsync,pclk,ac_bias */
        >;
    };
};
```

### Step 6: Verify Display Bring-Up

After rebooting with the patched DTB and cape attached:

```bash
ls /dev/fb0                           # Framebuffer device should exist
cat /sys/class/drm/card0-DPI-1/status # Should say "connected"
cat /sys/class/drm/card0-DPI-1/modes  # Should say "800x480"
dmesg | grep tilcdc                   # Should show fb0 initialization
```

If the kernel DRM path doesn't cooperate (tilcdc endpoint wiring issues),
the fallback is direct LCDC register programming via `/dev/mem` — see
the "Direct LCDC via /dev/mem" section below.

### Step 7: Test Pixels

```bash
dd if=/dev/urandom of=/dev/fb0 bs=768000 count=1  # Random noise (RGB565)
```

You should see colorful static on the display.

---

## Direct LCDC via /dev/mem — the All-Prong Foundation

The Linux prong programs the LCDC registers directly through `/dev/mem`,
using the **same register sequence** that bare-metal, FreeRTOS, and
Zephyr will reuse. This is the primary Linux path (not a fallback). It
decouples pixels-on-panel from the kernel `tilcdc` driver so we can
validate the register recipe in a debuggable userspace harness before
running it on bare-metal where a bad write freezes the AXI bus.

### Prerequisites (one-time, on the BBB)

1. **Reserve the framebuffer region.** The AM3358 has 512 MB of DDR3L
   at `[0x8000_0000, 0xA000_0000)`; we ship `mem=510M` on the kernel
   command line to keep the top 2 MB out of Linux's RAM pool so we can
   mmap it through `/dev/mem`:

   ```bash
   sudo bash examples/beaglebone-black/tools/reserve-fb.sh
   sudo reboot
   ```

   Verify after reboot:

   ```bash
   cat /proc/cmdline | tr ' ' '\n' | grep '^mem='
   ```

2. **Install the armv7 target on the host** (optional — you can also
   compile on the BBB directly):

   ```bash
   rustup target add armv7-unknown-linux-gnueabihf
   ```

### Run

```bash
# Build
cargo build --target armv7-unknown-linux-gnueabihf \
    -p rlvgl-example-bbb --features linux --release
scp target/armv7-unknown-linux-gnueabihf/release/rlvgl-bbb \
    debian@192.168.6.2:~

# On the BBB
sudo bash examples/beaglebone-black/tools/unbind-tilcdc.sh   # releases LCDC
sudo ./rlvgl-bbb                                             # writes pixels
```

### Register sequence (all four prongs)

Implemented in `examples/beaglebone-black/src/bsp/lcdc.rs` and shared
across Linux (`/dev/mem`), bare-metal, FreeRTOS, and Zephyr by way of
the `bsp::am335x` register map.

```
1. PRCM:  CM_PER_LCDC_CLKSTCTRL = 0x2 (SW_WKUP)
          CM_PER_LCDC_CLKCTRL = 0x2 (MODULEMODE_ENABLE)
          Poll IDLEST until 0x0

2. PINMUX: conf_lcd_data[0:23] = 0x08 (Mode 0, pull disabled, output)
           conf_lcd_vsync/hsync/pclk/ac_bias = 0x08

3. LCDC:  CLKC_ENABLE = 0x05 (DMA + Core)
          LCD_CTRL = 0x0501 (raster mode, clkdiv=5 → ~33 MHz)
          RASTER_CTRL = TFT | TFT24 | UNPACKED | PALMODE_DATA_ONLY
          RASTER_TIMING_0 = encode(HBP=46, HFP=210, HSW=20, PPL=800)
          RASTER_TIMING_1 = encode(VBP=23, VFP=22, VSW=10, LPP=480)
          RASTER_TIMING_2 = IPC|IHS|IVS (bits 22/21/20 — falling DCLK,
                            active-low HSYNC, active-low VSYNC)
          LCDDMA_CTRL = burst 16, FIFO threshold 8, single FB
          LCDDMA_FB0_BASE = 0x9FE0_0000  (reserved region)
          LCDDMA_FB0_CEILING = base + (800*480*4) - 4
          IRQENABLE_SET = EOF0 (bit 8)
          RASTER_CTRL |= LCDEN (bit 0) — START (must be last)
```

**Register-bit caveat:** RASTER_TIMING_2 bits 11/12/13 are **not**
IPC/IHS/IVS — those live at bits 22/21/20 per TRM Table 13-26. Earlier
versions of this code had the wrong positions; confirmed against the
AM335x TRM (SPRUH73Q) before shipping.

Expected live-register values after init (verify with `sudo devmem2
0x4830E028` etc.):

| Register | Offset | Expected |
|----------|--------|----------|
| LCD_CTRL | 0x04 | `0x0000_0501` |
| RASTER_CTRL (post-LCDEN) | 0x28 | `0x0620_0081` |
| RASTER_TIMING_0 | 0x2C | `0x2d0d_2c50` |
| RASTER_TIMING_1 | 0x30 | `0x1716_25df` |
| RASTER_TIMING_2 | 0x34 | `0x0070_0000` |
| LCDDMA_CTRL | 0x40 | `0x0000_0040` |
| LCDDMA_FB0_BASE | 0x44 | `0x9FE0_0000` |
| LCDDMA_FB0_CEILING | 0x48 | `0x9FF7_6FFC` |
| CLKC_ENABLE | 0x6C | `0x0000_0005` |

A Python script using `/dev/mem` and `mmap` can execute this sequence
from Linux userspace to prove pixels before the Rust binary is ready.
See `tools/lcdc-test.py` (to be written).

---

## Phase 4: Bare-Metal Chainload

The bare-metal prong runs the **exact same register sequence** from a
`no_std` ELF that U-Boot chainloads — no Linux, no `tilcdc`, no DTB
surgery. This is the fastest smoke-test path: if pixels light here, the
`bsp::lcdc::init_raster` sequence is correct by construction, and any
remaining Linux-prong blockers are kernel-side (pinctrl, framebuffer
coherency, LCDC bound by tilcdc).

### Framebuffer & layout

The bare-metal build lives at `0x82000000` (standard AM335x
`loadaddr`). DDR layout after `go 0x82000000`:

| Region                   | Purpose                              |
|--------------------------|--------------------------------------|
| `0x80000000..0x82000000` | U-Boot SPL + scratch (32 MB)         |
| `0x82000000..0x83000000` | `.text/.rodata/.data/.bss` (16 MB)   |
| `0x83000000..0x84000000` | Stack, grows down from `__stack_top` |
| `0x84000000..0x84200000` | Framebuffer (800×480×4 = 1.5 MB)     |
| `0x84200000..0x9F800000` | Free                                 |
| `0x9F800000..0xA0000000` | U-Boot relocated image + heap        |

See `examples/beaglebone-black/memory.x` for the linker script.

### Build + flat .bin

```bash
# One-time setup on the host
rustup target add armv7a-none-eabihf
# and any `arm-none-eabi-*` GNU toolchain (e.g. brew install gcc-arm-embedded)

# Build ELF + emit flat .bin
bash examples/beaglebone-black/tools/build-bare.sh
```

The script emits:

```
target/armv7a-none-eabihf/release/rlvgl-bbb-bare        # ELF
target/armv7a-none-eabihf/release/rlvgl-bbb-bare.bin    # raw binary (~1.2 KB)
```

Release `.bin` is tiny because the bare-metal path has no std, no
allocator, no `DiscoController`, and no lvgl widget tree — it's just
"fill FB with dark blue, program LCDC, wait on EOF". This is deliberate
for v1: we want to prove the register sequence end-to-end with the
smallest possible code.

### U-Boot chainload procedure

Attach USB-serial to the BBB J1 header (3.3 V FTDI cable):

| J1 pin | Signal | Wire to |
|--------|--------|---------|
| 1      | GND    | FTDI GND |
| 4      | RX     | FTDI TX  |
| 5      | TX     | FTDI RX  |

Open serial at 115200 8N1 (`screen /dev/cu.usbserial-* 115200` on
macOS, or `tools/serial.sh` equivalent). Power-cycle the BBB with the
`rlvgl-bbb-bare.bin` on the SD FAT partition (alongside the Bookworm
`boot` files — Bookworm leaves FAT partition 1 readable via `fatload
mmc 0:1`).

At the U-Boot prompt (press any key during the 1-second boot delay):

```
U-Boot# fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin
  reading rlvgl-bbb-bare.bin
  1229 bytes read in 5 ms (240 KiB/s)
U-Boot# go 0x82000000
  ## Starting application at 0x82000000 ...
```

Expected serial output (from the `bsp::uart0` breadcrumb driver):

```
=== rlvgl-bbb-bare ===
stage 1: enable peripheral clocks
stage 2: configure LCD pin mux
stage 3: fill framebuffer at 0x84000000 (0x00177000 bytes)
stage 4: init LCDC raster
stage 5: LCDEN set; entering main loop
eof 0x00000040
eof 0x00000080
...
```

Expected visual: solid dark-blue screen (`0xFF_00_40_80` = ARGB opaque
dark blue). If byte order comes out wrong (e.g. dark red instead of
dark blue), that's a TFT24_UNPACKED byte-lane confirmation — fix by
adjusting the pixel constant rather than the LCDC init, since the init
must match the Linux prong for reuse.

### What succeeds here proves, transitively

- `bsp::prcm::enable_lcdc/enable_i2c2/enable_gpio1` is correct.
- `bsp::pinmux::configure_lcd_pins` drives the pads to Mode 0
  (writes to `CONF_LCD_*` succeed because bare-metal is ring 0 —
  the write-lock observed from Linux `/dev/mem` is a CP15/MMU or
  Linux-side quirk, not a hardware block).
- `bsp::lcdc::init_raster` programs RASTER_CTRL/TIMING_0/1/2,
  LCDDMA_CTRL, LCDDMA_FB0_BASE/CEILING, CLKC_ENABLE, and LCDEN in
  the correct order with the correct bit fields.
- LCDC DMAs from `0x84000000` (bare-metal) are equivalent to DMAs
  from `0x9FE0_0000` (Linux `/dev/mem` reserved region) — only the
  framebuffer address changes between prongs.

---

## Software Architecture

### The Four Prongs

| Prong | Feature | Target | Display Driver | Status |
|-------|---------|--------|---------------|--------|
| Linux | `linux` | armv7-unknown-linux-gnueabihf | `/dev/mem` LCDC or `/dev/fb0` | SD card boots, SSH works |
| Bare-metal | `bare_metal` | armv7a-none-eabihf | Direct LCDC registers | Scaffold compiles |
| FreeRTOS | `freertos` | armv7a-none-eabihf | Same + preemptive tasks | Scaffold compiles |
| Zephyr | `zephyr` | Zephyr west build | Zephyr display API | Scaffold compiles |

All four share the same LCDC register definitions (`bsp/am335x.rs`),
timing constants (`bsp/lcdc.rs`), and the same `DiscoController` widget
tree from `rlvgl-app-disco-demo`.

### Crate Structure

```
examples/beaglebone-black/
├── Cargo.toml             # features: linux, bare_metal, freertos, zephyr
├── build.rs               # linker script + FreeRTOS C compilation
├── memory.x               # DDR @ 0x80000000 (bare-metal)
├── src/
│   ├── main.rs            # Linux entry (DiscoController + fbdev/devmem)
│   ├── bare_metal.rs      # no_std entry (PRCM → pinmux → LCDC → loop)
│   ├── freertos_entry.rs  # present/render/touch tasks
│   ├── freertos_sync.rs   # Semaphore FFI wrappers
│   ├── zephyr_entry.rs    # rlvgl_init() + input callbacks
│   ├── zephyr_sync.rs     # k_sem FFI
│   └── bsp/
│       ├── am335x.rs      # Hand-written PAC (PRCM, CTRLMOD, LCDC, I2C, GPIO)
│       ├── lcdc.rs        # Panel timing + raster init
│       ├── prcm.rs        # Clock enable helpers
│       └── pinmux.rs      # LCD + I2C2 pin configuration
├── freertos/              # FreeRTOS C glue (config, shims, stubs)
├── zephyr/                # Zephyr app shell (CMake, prj.conf, main.c, DTS)
├── linux/                 # Device tree overlay source
└── tools/
    ├── setup-bbb.sh       # First-time eMMC password + SSH key setup
    ├── prepare-sd.sh      # Flash Bookworm image + configure sysconf.txt
    ├── patch-dtb-lcd.sh   # Patch DTB with panel + pinmux + HDMI disable
    ├── reserve-fb.sh      # Append mem=510M to uEnv.txt for fb reservation
    └── unbind-tilcdc.sh   # Release LCDC from kernel driver before rlvgl-bbb
```

---

## Lessons Learned (Hardware Bring-Up Log)

### What Worked

- **Cape detection:** U-Boot reads the cape EEPROM and identifies
  `BB-BONE-NH7C-01` automatically.
- **Panel-simple driver:** Probes successfully with `innolux,at070tn92`
  compatible string when the panel DT node is present.
- **fdtoverlay:** Reliable way to add nodes to binary DTB without
  breaking phandle cross-references (unlike dtc decompile/recompile).
- **fdtput:** Adds/modifies individual properties in binary DTB.
- **Backlight:** Cape's backlight circuit works immediately with 5V barrel
  jack power — no GPIO enable needed.
- **Pin mux via /dev/mem:** Python script can read and write CTRLMOD pad
  registers directly. Mode 0 confirmed to set LCD function.

### What Didn't Work

- **U-Boot DT overlays:** Overlays with `port`/`endpoint` nodes crash
  U-Boot on this Bookworm image. The board hangs during overlay
  application with no serial output. Symptom: 4 solid blue LEDs, no
  heartbeat, no USB enumeration.

- **dtc round-trip:** Decompiling a DTB to DTS, editing, and recompiling
  breaks phandle references. The resulting DTB has missing or incorrect
  cross-references. Symptom: LCDC shows `status = "disabled"` in the
  live DT even though the file looks correct.

- **eMMC boot with LCD pin mux:** Setting LCD_DATA[0:7] to Mode 0
  kills eMMC access because these pins are shared with MMC1 data bus.
  The board appears to brick (no USB, no serial, no network). Recovery
  requires booting from microSD.

- **Panel-simple connector_type:** Kernel 6.12.76-bone50 requires
  `connector-type = <14>` (DRM_MODE_CONNECTOR_DPI) in the panel DT node.
  Without it, panel-simple logs "Specify missing connector_type" and
  tilcdc fails to create a DRM card.

- **Wrong DTB filename:** The Bookworm image uses
  `am335x-boneblack-uboot.dtb`, not `am335x-boneblack.dtb` (which
  doesn't exist). Patching the wrong file wastes a reboot cycle.

- **bbbio-set-sysconf overwrites DTBs:** On first boot, this service
  processes `sysconf.txt` and reinstalls DTBs from the kernel package.
  Any DTB patches applied before first boot are lost.

- **Debian 13 (Trixie) kernel:** The Trixie SD image (6.19-bone) has
  `panel-simple` as a module with an empty device table. The
  `innolux,at070tn92` compatible string is not in the module's
  `MODULE_DEVICE_TABLE`, so the panel never auto-matches. Stick with
  Bookworm (6.12) where panel-simple is built-in with the full table.

### Recovery Procedure

When the BBB is bricked (no USB, no network, solid LEDs):

1. Power off (unplug everything)
2. Insert the rescue microSD (Bookworm or Trixie image)
3. Hold S2 (boot button near SD slot) while applying power
4. Boot from SD — SSH at 192.168.6.2 (Bookworm) or 192.168.7.2 (Trixie)
5. Mount eMMC: `sudo mount /dev/mmcblk1p3 /mnt`
6. Fix uEnv.txt: comment out broken overlay/DTB lines
7. Unmount and reboot without SD

---

## Roadmap

### Phase 0: Shared Work ✅

- [x] DiscoCapabilities::beaglebone_black() preset
- [x] FT5x06 touch driver compatibility confirmed
- [x] Screen configuration (800x480, Deg0, landscape-native)
- [x] CpuBlitter rendering path

### Phase 1: Scaffold ✅

- [x] Example crate with Linux, bare-metal, FreeRTOS, Zephyr features
- [x] LCDC timing constants
- [x] AM3358 register map (hand-written PAC)
- [x] PRCM, pin mux, LCDC driver modules
- [x] Linker script, build.rs, FreeRTOS config
- [x] Zephyr CMakeLists, prj.conf, main.c, DTS overlay

### Phase 2: Hardware Bring-Up (In Progress)

- [x] SD card preparation (Bookworm 6.12 image)
- [x] First boot, SSH access, password configuration
- [x] eMMC boot disabled (SD is default)
- [x] Cape detected (BB-BONE-NH7C-01 EEPROM)
- [x] DTB patched (panel node, HDMI disabled, pin mux)
- [x] Panel-simple driver probes panel node
- [x] Backlight confirmed (visible glow)
- [x] **Pixels on screen** (through kernel tilcdc + overlay; 16bpp RGB565 fbdev)
- [ ] Touch input via I2C2 — **hardware-blocked**, cape RMA in flight.
      FT5426G responds on I2C with correct IDs but its scan engine
      reports zero touches under sustained firm press. Full diagnostic
      and RMA packet in [`docs/NewhavenRMA.md`](NewhavenRMA.md).

### Phase 3: Linux Prong

- [x] /dev/mem LCDC init from userspace (Rust; retired — superseded by
      the kernel tilcdc + overlay path below)
- [x] Framebuffer mmap + pixel writes (retired — `/dev/fb0` via tilcdc)
- [x] AM335x register layout verified against TRM SPRUH73Q
      (RASTER_TIMING_2 bits fixed: IPC/IHS/IVS at 22/21/20)
- [x] Overlay-driven tilcdc + panel-dpi path (replaces the legacy
      patch-dtb-lcd.sh fdtoverlay flow)
- [x] fbcon unbound via boot-time systemd oneshot so the framebuffer
      console's blinking cursor doesn't paint over rlvgl's renders
- [x] Cross-compile `rlvgl-bbb` (armv7-unknown-linux-gnueabihf) and
      deploy to hardware
- [x] DiscoController rendering on hardware (splash + desktop + widget
      tree all live at 57 Hz; 16bpp RGB565)
- [x] Playit TCP driver (stand-in for the dead touch sensor) —
      SSH-forwarded loopback TCP, accepts the full playit wire protocol
- [x] Star-crawl overlay with splash crop + text body + logo column
      (STM32-style 720×480 viewport; horizontal jumbo starfield)
- [ ] Touch via kernel edt-ft5x06 → evdev — DT binding in place and the
      driver binds; blocked on the cape's touch sensor (see RMA).
      The moment a responsive cape is back, events flow without any
      further software change.
- [ ] Direct I2C2 register-based touch (follow-up, unifies with bare-metal)
- [ ] DT split: 24-bit-from-SD vs 16-bit-from-eMMC variant (deferred)
- [ ] 32bpp XRGB8888 fbdev mode via DRM client-side mode set (deferred;
      needs to not conflict with eMMC pin sharing)
- [ ] R/B swap in `LinuxFbdevDisplay::flush` 16bpp path (cosmetic)
- [ ] Dirty-rect redraw to lighten CPU (full-frame blit at 57 Hz is fine
      on AM3358 for now)

#### Overlay-driven bring-up (current Linux path)

The tilcdc kernel driver with `panel-dpi` is the primary Linux path. We
ship our own overlay (not the upstream NH7C, which still carries
`//FIXME - LCD doesn't init...` on the LCDC endpoint wiring in the
maintained `v6.12.x-Beagle` branch). The flow is:

```
examples/beaglebone-black/linux/BB-NHD7-CAPE.dts        source overlay
examples/beaglebone-black/tools/build-overlay.sh        dtc -@ compile
examples/beaglebone-black/tools/deploy-overlay-sd-ext4.sh  SD install
```

Build:

```bash
bash examples/beaglebone-black/tools/build-overlay.sh
# → target/bbb-overlays/BB-NHD7-CAPE.dtbo
```

The script runs `cc -E` on the DTS (inlining `IRQ_TYPE_*` bindings if
the host doesn't have `dt-bindings` headers installed) and then `dtc -@
-I dts -O dtb` to emit a `.dtbo` with `__symbols__` and `__fixups__`
nodes — without `-@` the u-boot overlay applier can't resolve
`&lcdc`/`&am33xx_pinmux`/`&i2c2` against the live base DTB, which is
how earlier attempts silently did nothing.

Deploy (power off the BBB, pull the SD, insert into the Mac):

```bash
bash examples/beaglebone-black/tools/deploy-overlay-sd-ext4.sh
# optional: pass /dev/diskNs3 explicitly
```

The script probes the SD's ext4 rootfs with debugfs, reads
`/boot/dtbs/` to find the kernel version directory, writes the
`.dtbo` into `/boot/dtbs/<kver>/overlays/BB-NHD7-CAPE.dtbo`, and
rewrites `/boot/uEnv.txt` with:

| Key                               | Value | Why |
|-----------------------------------|-------|-----|
| `enable_uboot_overlays`           | `1`   | Turns on u-boot's overlay applier |
| `disable_uboot_overlay_video`     | `1`   | Suppresses the HDMI bridge overlay that conflicts with `bb-lcd-pins` |
| `uboot_overlay_addr0`             | `/boot/dtbs/<kver>/overlays/BB-NHD7-CAPE.dtbo` | Applies our overlay via the **user-custom slot** |
| `disable_uboot_overlay_addr4..7`  | `1`   | Suppresses cape-EEPROM auto-load (stock NH7C overlay is incomplete and the matching dtbo isn't shipped anyway) |

On Robert C. Nelson's BBB u-boot, `uboot_overlay_addr{0..3}` are the
USER custom-overlay slots and `addr{4..7}` are reserved for
cape-EEPROM-auto-detected overlays. Putting our `.dtbo` in `addr0`
was the key fix for the original "overlay file on disk, none of its
content in the live DT" symptom — using `addr4` made the user-slot
mechanism ignore us while the cape-slot mechanism looked for a file
that wasn't there.

The stock `/boot/uEnv.txt` is preserved as
`/boot/uEnv.txt.pre-rlvgl` on the first run; subsequent runs don't
clobber the backup.

Revert (from the Mac, SD reinserted):

```bash
sudo /opt/homebrew/opt/e2fsprogs/sbin/debugfs -w /dev/diskNs3 <<EOF
rm /boot/uEnv.txt
mv /boot/uEnv.txt.pre-rlvgl /boot/uEnv.txt
EOF
```

#### Framebuffer-console (fbcon) cursor — must be disabled

The Linux framebuffer console (`fbcon`) stays bound to `/dev/fb0`
alongside any userspace renderer and paints its **blinking 8×16
cursor character cell** on top of the rendered frame every ~500 ms.
On a rlvgl-driven panel that reads as a *flashing underline* at a fixed
screen position (character cell 18, 9 = pixel `(144, 144)` on the
default 100×30 VGA grid), appearing to track content whenever a
widget's body row happens to sit near that pixel. The cursor wins
whichever frame fbcon paints in, then our next render wipes it — hence
the flash.

The fix is to unbind `fbcon` from the framebuffer device; this leaves
the serial console and SSH untouched. A oneshot systemd unit does it
at boot:

```
examples/beaglebone-black/tools/rlvgl-unbind-fbcon.service  unit source
examples/beaglebone-black/tools/install-fbcon-unbind-sd.sh  SD installer
```

Install once per fresh SD (Mac-side, SD in reader, ext4 via debugfs):

```bash
bash examples/beaglebone-black/tools/install-fbcon-unbind-sd.sh
# or: bash tools/install-fbcon-unbind-sd.sh /dev/disk12s3
```

The installer writes the unit to `/etc/systemd/system/` and creates the
`multi-user.target.wants/` symlink using debugfs's `symlink` command so
systemd enables it at next boot. Verify on the booted BBB:

```bash
ssh debian@192.168.6.2 'cat /sys/class/vtconsole/vtcon1/bind'  # should be 0
ssh debian@192.168.6.2 'systemctl is-active rlvgl-unbind-fbcon'  # active
```

Diagnostic probe (if the flash comes back): sample `/dev/fb0` at ~100
ms intervals and diff per-row — a healthy board shows zero row churn
between consecutive static-UI frames. When fbcon is bound, you'll see
exactly 16 contiguous rows at the cursor's Y position light up every
~500 ms, 8 pixels wide. This was the probe used to diagnose it on
2026-04-22; full details in
[`feedback_bbb_fbcon_cursor.md`](../.claude/projects/-Users-iraabbott-rlvgl/memory/feedback_bbb_fbcon_cursor.md)
(kept outside the public tree).

**Deprecated path:** `tools/patch-dtb-lcd.sh` used `fdtoverlay` against
`am335x-boneblack-uboot.dtb` to bake the patch into the base DTB. It
worked sporadically and blew up u-boot on any port/endpoint fragment
because the Bookworm 6.12 base DTB isn't compiled with `-@` (no
`__symbols__` node), so cross-reference fixups silently unresolved.
The overlay path above is a strict improvement — overlays are applied
by u-boot before the kernel's DTB passes to it, and `__symbols__` live
in our `.dtbo`, not the base DTB. The script and its sibling
`unbind-tilcdc.sh` / `reserve-fb.sh` / `bbb-revert-*` were removed from
`tools/` as of 2026-04-22.

#### rlvgl-bbb Linux userspace app

The Linux-prong binary lives at `examples/beaglebone-black/src/main.rs`
and renders through `rlvgl_platform::LinuxFbdevDisplay` (mmap of
`/dev/fb0`) + `LinuxEvdevInput` (`/dev/input/event1` for touch once the
cape is responsive).

Cargo features (all default-on):

| Feature      | What it enables |
|--------------|-----------------|
| `linux`      | `LinuxFbdevDisplay` + evdev input path |
| `splash`     | RLE splash decoded once into the framebuffer at boot, via `rlvgl_decomp::decode_argb_into_rotated(Rot90Ccw)` (portrait asset → landscape FB). Per-board orientation is the decompress-step option discussed in [`project_splash_orientation_model.md`](../.claude/projects/-Users-iraabbott-rlvgl/memory/project_splash_orientation_model.md). |
| `desktop`    | DiscoController desktop widget tree layered over the splash (root is transparent; widgets paint on top). |
| `playit`     | Optional `rlvgl-playit` TCP listener (see below). Requires `RLVGL_PLAYIT_PORT` env var at runtime to bind. |
| `star_crawl` | Star-Wars-style scrolling crawl overlay (see below). |

Build + deploy:

```bash
# Cross-compile (requires the messense armv7-unknown-linux-gnueabihf
# toolchain and rustup's matching target). The repo ships a
# .cargo/config.toml that pins the linker.
RUSTFLAGS="" cargo build \
    --target armv7-unknown-linux-gnueabihf \
    -p rlvgl-example-bbb --bin rlvgl-bbb \
    --features linux,splash,desktop,playit,star_crawl --release

# Push to the board + relaunch.
scp target/armv7-unknown-linux-gnueabihf/release/rlvgl-bbb \
    debian@192.168.6.2:/home/debian/rlvgl-bbb

ssh debian@192.168.6.2 "echo \$BBB_SUDO_PASS | sudo -S bash -c '
  pkill -x rlvgl-bbb 2>/dev/null
  RLVGL_FB=/dev/fb0 RLVGL_INPUT=/dev/input/event1 RLVGL_PLAYIT_PORT=9999 \\
    setsid /home/debian/rlvgl-bbb > /tmp/rlvgl.log 2>&1 < /dev/null &'"
```

Expected log lines on startup:

```
rlvgl-bbb: fb=/dev/fb0 input=/dev/input/event1
fbdev: 800x480 @ 16bpp, stride=1600, mmap=750K
evdev: opened /dev/input/event1
rlvgl-bbb: screen 800x480 @ 57Hz
rlvgl-bbb: playit listening on loopback 127.0.0.1:9999 (SSH-forward to drive)
rlvgl-bbb: splash 480×800 → 800×480 via Rot90Ccw
```

Main loop is **wall-clock-paced**: elapsed time accumulates into a
virtual-tick budget at `frame_time = 1/57 s`, and each full frame_time
consumed emits one logical tick (`controller.tick()`, gesture
recognisers, and — when active — one `Event::Tick` into the star
crawl). On a slow frame the next frame emits multiple logical ticks,
so the crawl's apparent scroll speed and the gesture recogniser
timeouts stay stable under CPU-bound stalls.

#### Playit-driven input (stand-in for the RMA'd touch)

While the FT5426G is out for RMA, all user-facing input is injected
via the playit wire protocol over an SSH-forwarded loopback TCP socket.
The app binds `127.0.0.1:<RLVGL_PLAYIT_PORT>` at startup; the Mac
reaches it with `ssh -L ...`.

One-shot command:

```bash
ssh debian@192.168.6.2 'printf "?\n" | nc -q 1 127.0.0.1 9999'
# → STAT:<tick_count>,<present_count>
```

Interactive session:

```bash
ssh -L 9999:127.0.0.1:9999 -N debian@192.168.6.2 &
nc 127.0.0.1 9999
T400,240          # tap centre of panel
T@disco.main.info:0,0  # tap Info launcher by widget tag
QE:disco.info.star_crawl   # EXISTS:1
QB:disco.main.settings     # BOUNDS:x,y,w,h
?                 # STAT:2051,2051
```

Command | Effect
--------|-------
`T<x>,<y>` | tap (`PressRelease`) at landscape coords
`T@<tag>:<dx>,<dy>` | tap inside a tagged widget with relative offset
`PD<x>,<y>` / `PM<x>,<y>` / `PU<x>,<y>` | raw pointer down/move/up
`MT<n>:<id>,<s>,<x>,<y>;…` | multi-touch frame
`KD:<key>` / `KU:<key>` | key down / up
`QE:<tag>` / `QB:<tag>` / `QC:<tag>` | exists / bounds / child-count query
`D<x>,<y>,<w>,<h>[,<frames>]` | framebuffer pixel dump
`RS` / `RE` / `RD` | event recorder start / stop+dump / dump
`?` | status (tick + present counters, serial queue state)

Events dispatched via playit are also fed back into the
`DiscoController` so its side-effect queue
(`SetBacklight`, `StartEffect`, `ShowStatus`, …) fires the same way it
would for real hardware touches.

#### Star-crawl overlay

Activated by tapping the `disco.info.star_crawl` hotspot in the Info
wing (`T@disco.main.info:0,0` to open the wing, then
`T@disco.info.star_crawl:0,0`). The controller emits
`DiscoCommand::StartEffect(DiscoEffect::StarCrawl)` and the main loop
builds a fresh `CrawlWindow<StarCrawl<'static>>` via
`crawl_buffers::build_star_crawl_window(width, height, frame_hz)`.

The BBB crawl is sized to match the STM32 DISCO visual style:

- **Viewport:** 720 × 480 (leaves the right-side icon strip exposed).
- **Jumbo orientation:** `JumboOrientation::Horizontal` — starfield
  drifts right-to-left while the text column crawls upward.
- **Pre-rendered text column:** 600 × 6400 A8 surface laid out as:
  1. 120 px top margin
  2. 384 × 384 splash crop (cropped from the portrait splash asset,
     luminance-inverted into A8, rotated 90° CW into the column)
  3. 40 px gap
  4. 119 lines of `README_CRAWL` rendered by
     `pre_render_text(&CRAWL_FONT, CRAWL_LINES, 36, …)` at
     24 px DejaVu Sans with 36 px (1.5×) line spacing
  5. 40 px gap
  6. `softoboros-letter-logo.raw` blitted as A8 alpha
- **Scanline scratch:** 800 × 4 bytes.

Total activation cost: ~6.8 MiB RSS (jumbo background ~3 MiB, A8 text
~3.7 MiB, scanline 3.2 KiB), all leaked to satisfy the engine's
`'static` lifetime contract. Acceptable on 512 MiB AM3358 for an
interactive demo; each re-activation leaks a fresh set so successive
toggles are O(N) memory. Re-activations drop the previous window
first so the old engine stops ticking.

The crawl paint runs **after** the widget tree draw each frame, so
dashboard widgets stay visible under a semi-transparent crawl during
the ramp-in. `Event::Tick` is delivered once per logical tick (not
once per frame), keeping scroll speed stable under CPU-bound frames.

Verification probe — grab 4 framebuffer snapshots ~150 ms apart and
diff per-row; a running crawl shows ~90% of rows changing across each
delta (scroll motion) and ~52% of columns (twinkle + edge drift). A
static UI shows zero rows changing. Used 2026-04-22 to confirm the
crawl is actually animating end to end.

#### Hardware blocker: FT5426G touch RMA

Touch is non-functional on the current unit. The FT5426G responds to
I²C probes with correct IDs (`CIPHER=0x54`, `FW_VER=0x14`,
`VENDOR_ID=0x79`) and its internal scan engine ticks
(`FLOW_WORK_CNT 0x91` advances ~12 counts / 50 ms), but `TD_STATUS`
(`0x02`) stays at `0x00` under sustained firm press and `INT_CNT`
(`0x8F`) is frozen. Every register in the touch-data window `0x00–0x1F`
reads bit-for-bit identical between untouched and held states — the
chip is running but seeing no capacitance change on its sense lines.

Full diagnostic + submittable RMA packet: [`docs/NewhavenRMA.md`](NewhavenRMA.md).

Until a replacement arrives, all input is driven via the playit
path described above. The overlay's `edt-ft5x06` binding + IRQ-on-
`gpio3_19` pinmux is already correct for the FT5426 family (per
Newhaven's own Linux driver header), so touch will attach transparently
without any software change once a responsive cape is installed.

### Phase 4: Bare-Metal + FreeRTOS

- [x] `_start` preamble: SVC mode, MMU/caches off, SP set, `.bss` zeroed
- [x] Polled UART0 breadcrumb driver (`bsp::uart0`, 0x44E0_9000)
- [x] `memory.x` relocated to `loadaddr` 0x82000000; framebuffer past stack
- [x] `tools/build-bare.sh` builds ELF + flat `.bin` (~1.2 KB release)
- [ ] U-Boot chainload + pixels on panel (first hardware smoke test)
- [ ] FreeRTOS task model (present/render/touch)
- [ ] DiscoController integration (needs heap; deferred until smoke passes)

### Phase 5: Zephyr

- [ ] Zephyr board support for AM335x LCDC
- [ ] Staticlib integration
- [ ] Touch via Zephyr input subsystem
