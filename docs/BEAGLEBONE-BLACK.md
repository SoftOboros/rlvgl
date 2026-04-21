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

## Direct LCDC via /dev/mem (Fallback / All-Prong Foundation)

If the kernel DRM driver doesn't produce `/dev/fb0`, or for the bare-metal
and FreeRTOS prongs, we program the LCDC registers directly. This is the
**same register sequence** used by all four prongs — validating it on Linux
via `/dev/mem` first gives us confidence for bare-metal.

The init sequence (from `bsp/lcdc.rs` and `bsp/am335x.rs`):

```
1. PRCM:  CM_PER_LCDC_CLKSTCTRL = 0x2 (SW_WKUP)
          CM_PER_LCDC_CLKCTRL = 0x2 (MODULEMODE_ENABLE)
          Poll IDLEST until 0x0

2. PINMUX: conf_lcd_data[0:23] = 0x08 (Mode 0, pull disabled, output)
           conf_lcd_vsync/hsync/pclk/ac_bias = 0x08

3. LCDC:  CLKC_ENABLE = 0x07 (DMA + Core)
          LCD_CTRL = 0x0201 (raster mode, clkdiv=2 → ~33 MHz)
          RASTER_CTRL = TFT | TFT24 | UNPACKED | PALMODE_DATA_ONLY
          RASTER_TIMING_0 = encode(HBP=46, HFP=210, HSW=20, PPL=800)
          RASTER_TIMING_1 = encode(VBP=23, VFP=22, VSW=10, LPP=480)
          RASTER_TIMING_2 = IPC (falling edge)
          LCDDMA_CTRL = burst 16, single FB
          LCDDMA_FB0_BASE = framebuffer physical address
          LCDDMA_FB0_CEILING = base + (800*480*4) - 4
          IRQENABLE_SET = EOF0 (bit 8)
          RASTER_CTRL |= LCDEN (bit 0) — START (must be last)
```

A Python script using `/dev/mem` and `mmap` can execute this sequence
from Linux userspace to prove pixels before the Rust binary is ready.
See `tools/lcdc-test.py` (to be written).

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
    └── patch-dtb-lcd.sh   # Patch DTB with panel + pinmux + HDMI disable
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
- [ ] **Pixels on screen** ← next: /dev/mem LCDC driver
- [ ] Touch input via I2C2

### Phase 3: Linux Prong

- [ ] /dev/mem LCDC init from userspace (Python proof, then Rust)
- [ ] Framebuffer mmap + pixel writes
- [ ] Cross-compile rlvgl-bbb and deploy
- [ ] DiscoController rendering on hardware
- [ ] Touch input (evdev or direct I2C)

### Phase 4: Bare-Metal + FreeRTOS

- [ ] U-Boot chainload bare-metal ELF
- [ ] LCDC register init (same sequence, no Linux)
- [ ] FreeRTOS task model (present/render/touch)
- [ ] DiscoController integration

### Phase 5: Zephyr

- [ ] Zephyr board support for AM335x LCDC
- [ ] Staticlib integration
- [ ] Touch via Zephyr input subsystem
