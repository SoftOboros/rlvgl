# BeagleBone Black Bare-Metal Bring-Up

Status: **Pixels on panel confirmed 2026-04-22** on a BeagleBone Black
Rev C with a Newhaven NHD-7.0CTP-CAPE-P (800×480 24-bit TFT + FT5x06
capacitive touch). This chapter documents the bare-metal prong end to
end: what U-Boot hands us, what we do before Rust runs, what the LCDC
initialisation recipe looks like, how the three non-obvious fixes were
found, and the SD-swap deploy flow we use on a Mac that has no USB
serial adapter and a flaky USB-gadget network.

This is a companion to [`docs/BEAGLEBONE-BLACK.md`](../BEAGLEBONE-BLACK.md).
That file still holds the project-level roadmap, cape EEPROM notes, and
the Linux-prong history (including five failed DT surgeries). This one
is exclusively the bare-metal story.

---

## 0. Why bare metal at all

The BBB port exists to demonstrate a single register-level LCDC recipe
that is reused across four prongs: Linux, bare-metal, FreeRTOS, Zephyr.
The Linux prong attempted to run that recipe from `/dev/mem` but was
blocked on two intractable issues on the Bookworm 6.12.76-bone50 image:

- **CTRLMOD pad config writes silently dropped from userspace.** Five
  wedges in a row trying to rewire LCDC pins via `/dev/mem`.
- **`fdtoverlay` applied to `am335x-boneblack-uboot.dtb` caused u-boot
  to wedge.** `fdtput` worked briefly but hit an unexplained pinctrl
  PIN2 conflict.

The bare-metal prong sidesteps both. The same LCDC register sequence
runs in SVC mode with the MMU off — no Linux kernel mediating access to
CTRLMOD, no device tree, no pinctrl-single fighting with u-boot
overlays. If pixels show up from bare-metal they will show up from the
other prongs once we thread the same calls through their respective
scheduling shells (FreeRTOS tasks / Zephyr threads / Linux
`/dev/mem`-backed display driver).

Bare-metal bring-up first is therefore not an alternative path — it is
the reference implementation that proves the register recipe against
silicon before we layer anything on top.

---

## 1. Hardware context

| Thing | Value |
|-------|-------|
| SoC | TI AM3358 (Cortex-A8, Sitara) |
| DDR | 512 MB DDR3L at `0x8000_0000..0xA000_0000` |
| Boot ROM | reads SD/eMMC MLO → u-boot.img → u-boot proper |
| LCDC base | `0x4830_E000` |
| Panel | NHD-7.0-800480AF-ASXP, 800×480, 33.3 MHz target pixel clock, DE mode |
| Cape I2C touch | FT5x06 on I2C2 (P9.19/P9.20), `0x38` |
| USR LEDs | GPIO1 bits 21..24 → USR0..USR3 |
| Debug UART | UART0 (`0x44E0_9000`) on J1 header, 115200 8N1 |

Notable hardware quirk: `LCD_DATA[0:7]` on the expansion header share
pins with the eMMC data bus. Consequence: when the cape is attached,
the board **must** boot from microSD (hold S2 during power-on). This is
baked into all deploy procedures here.

---

## 2. Boot sequence, start to pixels

```
┌───────────────┐   ┌──────────────┐   ┌──────────┐   ┌───────────┐
│  AM335x ROM   │→→ │ u-boot SPL   │→→ │  u-boot  │→→ │ our .bin  │
│   (mmcsd)     │   │ (DDR+PLL+PMUX│   │ (env +   │   │ _start +  │
│               │   │  + UART)     │   │  distro) │   │ rust_main │
└───────────────┘   └──────────────┘   └──────────┘   └───────────┘
                                             │
                                             └→ fatload 0x82000000 rlvgl-bbb-bare.bin
                                             └→ go 0x82000000
```

u-boot drops us at `0x82000000` in SVC mode with caches most likely ON
and its own 1:1 MMU map. u-boot does **not** call `cleanup_before_linux`
for `go`, so we cannot trust:

- D-cache is cold
- MMU is off
- Interrupts are masked
- SP is at any particular value
- Exception vectors point anywhere useful

Everything from here on must be established by our own code.

### 2.1 `_start` assembly preamble

`examples/beaglebone-black/src/bare_metal.rs` has a `core::arch::global_asm!`
block that produces `_start` at `0x82000000`. In order, it:

1. Writes `CPSR = 0xD3`: SVC mode, I and F masks set, ARM state.
2. Clears `SCTLR.M/C/I/Z` — MMU, D-cache, I-cache, branch prediction all
   off. Running with caches off costs us CPU perf but makes framebuffer
   coherency free: every store from Rust goes straight to DDR, every
   LCDC DMA read sees the current value.
3. `ICIALLU` (invalidate I-cache) and `TLBIALL`, then `dsb` + `isb`.
4. Points `VBAR` at our own `vector_table` so aborts and undefined
   instructions don't jump to ROM at `0x0` where they would be invisible.
   Each exception handler lights a distinctive LED pattern and spins:
   - data abort: USR0 + USR3
   - prefetch abort: USR0 + USR2
   - undefined instruction: USR1 + USR3
   - SVC / reserved / IRQ / FIQ: all four LEDs
5. `ldr sp, =__stack_top` — stack at `0x8400_0000`, grows down.
6. Zeroes the `.bss` section between `__bss_start` and `__bss_end`.
7. `bl rust_main`.

After this runs the CPU is in a defined state and Rust is safe to enter.

### 2.2 `rust_main` progression

```rust
bsp::wdt::disable();                      // WDT1 unlock sequence
bsp::prcm::enable_gpio1();                // PRCM module-clock enable + IDLEST poll
bsp::leds::configure();                   // USR0..USR3 as outputs, all low
bsp::leds::blink_stage(1);                // one USR0 blink
bsp::prcm::enable_lcdc();                 // pixel-clock mux + CLKCTRL enable
bsp::prcm::enable_i2c2();
bsp::leds::blink_stage(2);                // two USR0 blinks
bsp::pinmux::configure_lcd_pins();        // 24 LCD_DATA pads + 4 sync/pclk/AC_BIAS
bsp::pinmux::configure_i2c2_pins();
bsp::leds::blink_stage(3);
<fill framebuffer at 0x8400_0000>         // 800*480*4 bytes of pixel words
asm!("dsb sy");                           // drain CPU write buffer before DMA
bsp::leds::blink_stage(4);
<LCDC main-reset pulse>                   // write bit 3 of LCDC_CLKC_RESET
bsp::leds::blink_stage(5);
lcdc::init_raster(FB_BASE, FB_BYTES);     // the 11-step LCDC recipe
bsp::leds::blink_stage(6);
<Knight-Rider LED chase forever>
```

WDT disable runs **first**. U-Boot enables WDT1 with a ~60 s timeout
expecting Linux to kick it. If the disable doesn't run before the
timeout, the SoC resets and the user sees the panel flicker every
minute as u-boot reloads and reruns our binary. Symptom during bring-up
was "screen resets every so often" — diagnosis was immediate once WDT
was disabled and the flicker stopped.

---

## 3. The LCDC register recipe

`examples/beaglebone-black/src/bsp/lcdc.rs::init_raster` is shared
between all four prongs. It assumes PRCM clocks and pinmux are already
done (the callers handle that) and performs the following writes in this
order:

```
 0. LCDC_SYSCONFIG       = SMART_IDLE_WAKEUP | SMART_STANDBY_WAKEUP | AUTOIDLE
 1. LCDC_CLKC_ENABLE     = DMA | CORE
 2. LCD_CTRL             = MODESEL_RASTER | (clkdiv=5 << 8)
 3. RASTER_CTRL          = LCDTFT | TFT24 | TFT24_UNPACKED | PALMODE_DATA_ONLY
                           (LCDEN intentionally off for now)
 4. RASTER_TIMING_0      = encode(HBP=46, HFP=210, HSW=20, PPL=800)
 5. RASTER_TIMING_1      = encode(VBP=23, VFP=22, VSW=10, LPP=480)
 6. RASTER_TIMING_2      = IPC | IHS | IVS       (bits 22/21/20)
 7. LCDDMA_CTRL          = BURST_16 | FIFO_TH_8 | FRAME_MODE_SINGLE
 8. LCDDMA_FB0_BASE      = fb_pa
 9. LCDDMA_FB0_CEILING   = fb_pa + fb_size - 4
10. LCDC_IRQENABLE_SET   = EOF0
11. RASTER_CTRL |= LCDEN                         (must be last)
```

The register-bit positions were verified against AM335x TRM SPRUH73Q
Table 13-26 — **not** against older cached values. A prior version of
`bsp/am335x.rs` had `TIMING2_IPC = 1 << 11`, which is inside the ACB
field. The actual IPC/IHS/IVS bits live at 22/21/20 and that fix
preceded this chapter.

Post-init sanity check (low nibbles read back on LEDs, see §5):

| Register | Address | Low-nibble readback | Meaning |
|----------|---------|---------------------|---------|
| RASTER_CTRL | `0x4830_E028` | `0x1` | LCDEN set |
| CONF_LCD_DATA0 | `0x44E1_08A0` | `0x8` | Mode 0 + slew fast |
| LCD_CTRL | `0x4830_E004` | `0x1` | MODESEL_RASTER set |
| LCDC_CLKC_ENABLE | `0x4830_E06C` | `0x5` | DMA + CORE |
| LCDC_STAT | `0x4830_E008` | `0x0` | no SYNC_LOST |
| LCDC_IRQSTATUS_RAW[7:4] | `0x4830_E058` | `0x0` | no FUF |
| LCDDMA_CTRL[7:4] | `0x4830_E040` | `0x4` | BURST_16 |
| LCDDMA_FB0_BASE[31:28] | `0x4830_E044` | `0x8` | top nibble of `0x8400_0000` |

All confirmed on hardware on 2026-04-22. If any of these diverges on a
future bring-up, it isolates exactly which write didn't take.

---

## 4. The three non-obvious fixes

Each of these three was necessary. Any one missing left the panel
showing backlit-white with no visible pixel data. Together they move
the panel to "black when FB=0, color bars when FB=pattern".

### 4.1 `LCDC_SYSCONFIG` must not be left at its reset default

The AM335x LCDC `SYSCONFIG` reset value is `0x0`, which encodes
`IDLEMODE = FORCE_IDLE` and `STANDBYMODE = FORCE_STANDBY`. The L4
interconnect respects those settings and parks LCDC whenever the CPU is
not actively poking it — which is essentially always during normal
streaming. DMA never gets its bus grants, the FIFO never fills, the
pixel output is garbage, and the panel sees no valid signal. Because
nothing the CPU does disturbs this — register writes still succeed,
LCDEN still reports set — it looks like everything is configured
correctly while nothing actually streams.

**Fix:** write `SMART_IDLE_WAKEUP | SMART_STANDBY_WAKEUP | AUTOIDLE` to
`SYSCONFIG` as the first step of `init_raster`:

```rust
SYSCONFIG_IDLEMODE_SMART_WAKEUP = 3 << 3;
SYSCONFIG_STANDBYMODE_SMART_WAKEUP = 3 << 5;
SYSCONFIG_AUTOIDLE = 1 << 0;
reg_write(LCDC_SYSCONFIG, /* all three */);
```

SMART_IDLE_WAKEUP lets the module assert its idle request only when
it's actually done with a frame and can wake up on its own when DMA
needs bus access. Without this, the bring-up looks like a ghost — every
readback says "fine", no IRQ fires, no pixels ship.

### 4.2 Pixel clock must be routed to DPLL_PER_M2

The AM335x chip default for `CM_CLKSEL_LCDC_PIXEL_CLK` (at
`0x44E0_0534`) is `0x0`, which selects DPLL_DISP_M2 as the LCDC pixel
clock source. On BBB, u-boot does **not** initialise DPLL_DISP — it
only needs DPLL_PER for MMC and DPLL_CORE for L3/L4. With DPLL_DISP in
bypass, the LCDC pixel clock is effectively dead: LCDC keeps requesting
pixels from DMA but its output runs at a meaningless frequency and the
panel can never lock.

**Fix:** before enabling the LCDC module clock, write `0x2` to
`CM_CLKSEL_LCDC_PIXEL_CLK` to route through DPLL_PER_M2 (which u-boot
does bring up at 192 MHz). With our `clkdiv = 5` in `LCD_CTRL[15:8]`,
the actual pixel clock is `192 MHz / (5 + 1) = 32 MHz`. Panel target is
33.3 MHz; 32 MHz is well within the NHD-7.0-800480AF-ASXP spec window
of 28–40 MHz.

```rust
// bsp/prcm.rs::enable_lcdc
reg_write(CM_CLKSEL_LCDC_PIXEL_CLK, 0x2);   // DPLL_PER_M2
reg_write(CM_PER_LCDC_CLKCTRL, MODULEMODE_ENABLE);
wait_idlest_bounded(CM_PER_LCDC_CLKCTRL);
```

If you need exactly 33.3 MHz — for instance to match a panel that's
less tolerant — configure DPLL_DISP explicitly and select mode `0x0`.
We haven't needed to for this panel.

### 4.3 Do NOT write `CM_PER_LCDC_CLKSTCTRL = 0x2`

An earlier version of `prcm::enable_lcdc` wrote `0x2` (SW_WKUP) to
`CM_PER_LCDC_CLKSTCTRL` before the `CLKCTRL` enable. This came from
one of the AM335x reference manuals' sample sequences. TI's StarterWare
`LCDCClocksEnable` **does not** do this — it only writes MODULEMODE,
polls IDLEST, then polls CLKSTCTRL for CLKACTIVITY bits to come up.

On this u-boot state, writing SW_WKUP forced the clock domain into a
state the IDLEST poll never returned from, hanging the bring-up between
stage 1 and stage 2 (one USR0 blink, then silence).

**Fix:** remove the CLKSTCTRL write entirely. Let the domain stay in
HW_AUTO (u-boot's default). Writing MODULEMODE=ENABLE is enough to kick
the domain out of idle and transition IDLEST through TRANS to FUNC.
Also, bound every IDLEST poll to ~2M iterations so that a genuinely
dead clock source never locks the bring-up permanently — we advance
and surface the problem through the LED state display rather than
hanging.

---

## 5. Debug techniques used

Without a serial cable, every diagnostic had to flow through the four
USR LEDs. Several visual encodings proved their weight:

- **Blink-count stage indicator** (`leds::blink_stage(n)`). USR0 blinks
  `n` times at ~2 Hz, then pauses. Called at each major bring-up
  checkpoint (1 through 6). The last complete burst the user sees
  identifies exactly which stage executed last. This was how we
  isolated the SW_WKUP CLKSTCTRL hang.
- **Exception-vector LED patterns** (installed at VBAR). Data abort,
  prefetch abort, undefined instruction, and catch-all each light a
  distinctive LED pattern and spin. Without these, any fault would
  silently loop forever and look indistinguishable from a normal code
  hang. With them we can rule out "we faulted" as a cause.
- **Nibble-on-LEDs register readback** (`leds::show_nibble(v)`). USR0
  lights for bit 0, USR1 for bit 1, etc. Cycling through four register
  views — RASTER_CTRL, CONF_LCD_DATA0, LCD_CTRL, CLKC_ENABLE — we
  confirmed all four config-register writes took effect on hardware,
  forcing us to look past "init returned" for the white-screen cause.
- **All-4-LEDs "new-binary signature"** (`leds::all_on_mark()`). After
  stage 6 blinks, we hold all four USR LEDs for 2 s before any main
  loop starts. This exists so the user can tell "I am running the
  build I just flashed" from "the SD didn't actually get my new
  `.bin`". Saved at least one debugging round where the observed
  behaviour looked identical to the previous build.
- **Post-init status-register cycle.** A second 4-frame rotation shows
  LCDC_STAT (to catch SYNC_LOST), IRQSTATUS_RAW (to catch FUF), and
  the top nibble of LCDDMA_FB0_BASE (to confirm `0x8400_0000` landed in
  the register). All read `0` or the expected value on the successful
  bring-up.

The WDT disable came out of a user observation — "the screen resets
every so often" — which was direct evidence of an SoC reset rather
than a render glitch. Without the user's eye on the physical board that
would have taken a lot longer to find.

---

## 6. Memory layout

```
 DDR (512 MB @ 0x8000_0000..0xA000_0000)
 ─────────────────────────────────────────────
 0x8000_0000 ─┬── U-Boot SPL scratch + loadargs
              │
 0x8200_0000 ─┼── rlvgl-bbb-bare .text + .rodata
              │   + .data + .bss  (~2 KB release build,
              │   16 MB reserved for headroom in memory.x)
              │
 0x8300_0000 ─┼── stack, grows down from __stack_top
              │   (16 MB window, never more than a few KB used
              │   in practice because the binary has no dynamic
              │   allocation and no deep recursion)
              │
 0x8400_0000 ─┼── framebuffer (800×480×4 = 1,536,000 B)
              │   ends at 0x8417_6FFF
              │
 0x8418_0000 ─┼── free DDR (for future double-buffer etc.)
              │
 0x9F80_0000 ─┼── U-Boot relocated image + heap
 0xA000_0000 ─┘
```

`memory.x` covers only the `0x8200_0000..0x8300_0000` region (code +
.data + .bss). `__stack_top` is a hand-written constant at
`0x8400_0000`; the framebuffer address `FB_BASE = 0x8400_0000` in
`bare_metal.rs` matches by convention — not by any linker relationship.
If someone moves either, the other must move with it.

---

## 7. Build and deploy workflow

### 7.1 Build

```bash
bash examples/beaglebone-black/tools/build-bare.sh
```

Runs:
```
RUSTFLAGS="" cargo build --target armv7a-none-eabihf \
    -p rlvgl-example-bbb --bin rlvgl-bbb-bare \
    --no-default-features --features bare_metal --release
arm-none-eabi-objcopy -O binary <ELF> rlvgl-bbb-bare.bin
```

The `.bin` for the current colour-bar test pattern is about 1.8 KB —
small enough to `fatload` in milliseconds.

### 7.2 Deploy (no USB serial, no USB-gadget network)

Neither the FTDI J1 serial path nor the USB-gadget 192.168.6.2 path was
available during this bring-up (no cable on the former; persistent
enumeration failures on the latter). The working path is SD-swap + two
scripts:

```bash
# 1. Put rlvgl-bbb-bare.bin + boot.scr on the FAT partition
bash examples/beaglebone-black/tools/deploy-bare-sd.sh

# 2. Put the uenvcmd override on the ext4 partition via debugfs
bash examples/beaglebone-black/tools/deploy-bare-sd-ext4.sh
```

The ext4 script is the critical one: BBB Bookworm u-boot reads
`/boot/uEnv.txt` from the rootfs (partition 3), **not** from the FAT
partition. Any `uEnv.txt` on `/Volumes/BOOT` is ignored during
distro_boot. The script uses `/opt/homebrew/opt/e2fsprogs/sbin/debugfs`
(no macFUSE, no kext) in a single `-w` session:

```
rm /boot/uEnv.txt
write <tempfile> /boot/uEnv.txt
```

where `<tempfile>` contains:

```
uenvcmd=echo "rlvgl override"; led usr0 on; led usr1 on; led usr2 on; led usr3 on; fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin; go 0x82000000
```

The `led usrN on` commands give u-boot a "signature" — all four USR
LEDs light simultaneously for a moment before the `fatload` runs. If
the user ever power-cycles and does **not** see that 4-LED flash, the
override is not being read and u-boot is falling through to a normal
Linux boot. This was how we diagnosed the initial "wrong uEnv.txt
location" issue.

### 7.3 Revert

To go back to a normal Linux boot:

```
sudo /opt/homebrew/opt/e2fsprogs/sbin/debugfs -w /dev/disk12s3 <<EOF
rm /boot/uEnv.txt
EOF
```

or, if a `.rlvgl-bak` still exists, `mv` it back. The FAT-side files
(`rlvgl-bbb-bare.bin`, `boot.scr`, FAT `uEnv.txt`) are harmless to leave
— u-boot ignores the FAT `uEnv.txt` as discussed above, and the `.bin`
just sits unused.

---

## 8. File map

| Path | Purpose |
|------|---------|
| `examples/beaglebone-black/src/bare_metal.rs` | `_start` asm + `rust_main` (stage indicator + FB fill + LCDC init + main loop) |
| `examples/beaglebone-black/src/bsp/am335x.rs` | Register-base + bit-field constants for CM_PER, CM_DPLL, CTRLMOD, LCDC, I2C2, GPIO1, WDT1 |
| `examples/beaglebone-black/src/bsp/prcm.rs` | `enable_lcdc / enable_i2c2 / enable_gpio1` with bounded IDLEST polls |
| `examples/beaglebone-black/src/bsp/pinmux.rs` | 24 LCD data pads + 4 sync/pclk/AC_BIAS + I2C2 SDA/SCL |
| `examples/beaglebone-black/src/bsp/lcdc.rs` | `init_raster(fb_pa, fb_size)` — the 12-step recipe shared across prongs |
| `examples/beaglebone-black/src/bsp/wdt.rs` | WDT1 disable (two-write unlock to WSPR) |
| `examples/beaglebone-black/src/bsp/leds.rs` | USR0..USR3 helpers: configure / set_level / set_one / blink_stage / show_nibble / all_on_mark |
| `examples/beaglebone-black/src/bsp/uart0.rs` | Polled TX helpers (currently unused in bare-metal since no serial hardware is connected) |
| `examples/beaglebone-black/memory.x` | Linker script: code at `0x8200_0000`, stack at `0x8400_0000` |
| `examples/beaglebone-black/tools/build-bare.sh` | cargo build + objcopy to flat `.bin` |
| `examples/beaglebone-black/tools/deploy-bare-sd.sh` | FAT-partition copy (`.bin` + `boot.scr` + FAT-level `uEnv.txt`) |
| `examples/beaglebone-black/tools/deploy-bare-sd-ext4.sh` | ext4 `/boot/uEnv.txt` override via debugfs |
| `examples/beaglebone-black/tools/boot-bare.cmd` / `.scr` | mkimage-blessed u-boot script (fallback for distros that scan for `boot.scr`) |

---

## 9. Lessons and antipatterns

- **Do not trust reset defaults for power-management-related registers
  on a module-by-module basis.** `LCDC_SYSCONFIG = 0` is a perfectly
  valid hardware state that also makes the module completely useless
  for streaming. Every module with a SYSCONFIG register deserves an
  explicit, documented configuration even if you're copying the value
  verbatim from a reference driver.
- **Do not trust "the TRM sample sequence" over working driver code.**
  The CLKSTCTRL SW_WKUP write was in a TRM example. TI StarterWare
  didn't do it. Linux didn't do it. We shouldn't have either.
- **Every polling loop in bring-up code should be bounded.** An
  unbounded `while (IDLEST) {}` turns a clock-routing bug into a
  completely invisible CPU hang that can't be distinguished from any
  other loop. Bounded polls degrade gracefully — the bring-up advances,
  the symptom shows up downstream, and the LED state display can still
  tell us what went wrong.
- **When you can't print, light LEDs. When you can't light LEDs, blink
  one LED.** The four USR LEDs carried an order of magnitude more
  diagnostic information than we initially believed possible. Binary
  encoding was hard to read under camera clipping; blink-count was
  trivial. Position encoding (which LED is lit) was clearer than
  pattern encoding (combination of LEDs). Different encodings for
  different failure modes stacked well — stage indicator for "where"
  plus exception-vector pattern for "what kind of fault" plus register
  nibble for "what value".
- **When the hardware appears to work but doesn't stream pixels, check
  its idle state settings before doubting your init order.** Our entire
  `init_raster` order was already correct on the first attempt. The
  panel only lit up when we told the interconnect to keep LCDC awake.
- **Keep the framebuffer address in Rust, not in the linker script.**
  Any fixed-address peripheral buffer that may move between builds,
  panel resolutions, or prongs should be a `const` in `.rs`, not a
  symbol in `memory.x`. That way `memory.x` stays a concise description
  of where the code lives, and the framebuffer can be argued about in
  plain Rust.

---

## 10. What's next on the bare-metal prong

Now that pixels are confirmed the next steps are straightforward:

1. **DiscoController on bare-metal.** Link the existing
   `rlvgl-app-disco-demo` widget tree + `CpuBlitter` +
   `BlitterRenderer` into the bare-metal main loop. Needs either a tiny
   bump allocator for the widget tree or the pre-allocated alloc-free
   variant.
2. **Touch via I2C2.** `bsp::i2c2` + `bsp::ft5x06` exist as stubs; we
   need polled I2C reads of FT5x06 touch coordinate registers and the
   same gesture dispatch path the other prongs use.
3. **Backlight PWM.** Cape backlight comes up on the 5 V rail and
   stays on at full brightness. For the Disco demo we want
   `DiscoCommand::SetBacklight(level)` to do actual PWM; that means
   wiring one of the GPIO1 pins through a timer in PWM mode.
4. **Enable caches with proper mappings.** Running with D-cache off
   gives us coherent DMA for free but costs maybe 2–3× on CPU-bound
   paths (blitter, text render). A minimal MMU setup that marks the
   framebuffer region as non-cacheable while letting code + stack run
   cached would recover most of that.
5. **FreeRTOS layer on the same init.** Once the DiscoController runs
   in bare-metal cooperative mode, swap the forever loop for
   `freertos_entry::start` with present / render / touch tasks. The
   bsp layer doesn't change.

All five can happen independently; the register recipe beneath them is
now frozen.
