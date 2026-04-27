# Newhaven RMA — NHD-7.0CTP-CAPE-P touch sensor non-functional

**Filing date:** 2026-04-22
**Contact:** Ira Abbott &lt;abbott.ira.r@gmail.com&gt;
**Purchase channel:** Digi-Key Electronics (unit shipped by Newhaven)
**Affected part number:** NHD-7.0CTP-CAPE-P
**Cape EEPROM identifier (u-boot reported):** `BB-BONE-NH7C-01`
**Host board:** BeagleBone Black Rev C, Debian 12 Bookworm, kernel 6.12.76-bone50
**Host driver:** mainline Linux `edt_ft5x06` via device-tree overlay, I²C bus 2, address 0x38
**S1 DIP-switch:** left at factory default (cape detected successfully by u-boot — S1 is not the issue)

---

## Failure summary

The **display side of the cape works perfectly** — 800×480 24-bit RGB comes up through the TI LCDC driver, backlight is at full brightness, framebuffer console is visible, and a user-space Rust GUI renders a splash + widget tree at 57 fps with no issues. All 28 LCD signals are wired and driving correctly.

The **touch side** is dead. The FT5426 touch controller is alive on I²C (all IDs and config registers respond), and its internal scan engine is actively running (scan-cycle counter ticks as expected), but `TD_STATUS` stays at **0x00 under any amount of firm sustained finger pressure**. Every byte in the touch-data register window (0x00–0x1F) is bit-for-bit identical untouched vs. held.

**The FT5426 silicon is fine. The touch sensor glass (or its FPC path to the chip) is not producing any capacitance change when touched.** This is consistent with a defective sensor assembly or a touch-panel FPC connector that doesn't make reliable electrical contact (can look seated yet still be open). All flex cables were visually inspected on the affected unit and appear normal, so the defect is not externally obvious.

---

## Live-hardware evidence

The following probe was taken on the affected cape, with the kernel `edt_ft5x06` driver unbound to release the I²C device for direct `i2cget`/`i2cset` access.

### Chip identity (all correct per FT5426 datasheet)

| Register | Name | Expected | Observed |
|----------|------|----------|----------|
| `0x00`   | WORKMODE           | 0x00 (run)              | **0x00** ✓ |
| `0xA3`   | CHIP_ID            | 0x54 (FT5426)            | **0x54** ✓ |
| `0xA5`   | POWER_MODE         | 0 = Active, 1 = Monitor  | **0x01** (Monitor — normal default) |
| `0xA6`   | FW_VER             | non-zero                 | **0x14** ✓ |
| `0xA8`   | VENDOR_ID          | Newhaven tag             | **0x79** ✓ (matches `(79)` appended to the kernel input-device name) |

### Scan-engine liveness vs. touch reporting

The following two counters were sampled every ~50 ms for 5 seconds while a finger was held on the panel with firm pressure.

| Register | Name | Behaviour |
|----------|------|-----------|
| `0x91`   | FLOW_WORK_CNT  | **Increments ~12 counts per 50 ms consistently** — silicon-level proof that the scan engine is running its internal work cycle. |
| `0x8F`   | INT_CNT        | **Frozen at 0x29 for the full 5 seconds** — zero interrupt events raised. |
| `0x02`   | TD_STATUS      | **0x00 for every single sample**, untouched or firmly held. |

Additionally, the full register block 0x00–0x1F (DEVICE_MODE, TD_STATUS, and all six touch-point coordinate slots) reads **bit-for-bit identical** before and during a firm 4-second press:

```
00: 00 00 00 42 fe 00 98 00 00 ff ff ff ff ff ff ff
10: ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
```

The `42 fe 00 98` at 0x03–0x06 is stale power-up state, not updated touch data (it matches both the untouched and held snapshots).

### INT line (P9_27 / gpio3_19) never asserts

The INT line is stuck low with our configured pull-down for the entire observation window. The FT5426 is not producing the high→low edge that normally signals new touch data to the host. This is consistent with the chip having no data to report (TD_STATUS=0, so nothing to interrupt about), not with a driver misconfiguration.

---

## Expected good behaviour

A healthy unit, probed the same way, should show:

- `FLOW_WORK_CNT (0x91)` ticking — same as this unit, confirming scan engine alive.
- `INT_CNT (0x8F)` **incrementing** on each finger down / up — this unit: frozen.
- `TD_STATUS (0x02)` reading **1–5** for the number of concurrent touches, 0 when untouched.
- Touch-point registers 0x03–0x2A populating with live X/Y coordinates when pressed.

None of those behaviours are observable on this cape.

---

## Reproduction steps

On any BBB running a 6.12 kernel with the edt_ft5x06 driver bound to a NHD-7.0CTP-CAPE-P at I²C2 0x38:

```bash
# 1. Release the kernel driver so i2c-tools can access the chip.
sudo sh -c 'echo 2-0038 > /sys/bus/i2c/drivers/edt_ft5x06/unbind'

# 2. Confirm chip identity — must match the Expected column above.
for r in 0x00 0x02 0xA3 0xA5 0xA6 0xA8; do
    printf "  %s = %s\n" "$r" "$(sudo i2cget -y 2 0x38 $r b)"
done

# 3. Force Active mode so the chip runs at full 100 fps scan rate.
sudo i2cset -y 2 0x38 0xA5 0x00 b

# 4. Watch the scan-cycle counter (must tick) vs TD_STATUS (must remain 0 on
#    the defective unit even under sustained firm press).
for i in $(seq 1 50); do
    F=$(sudo i2cget -y 2 0x38 0x91 b)
    T=$(sudo i2cget -y 2 0x38 0x02 b)
    I=$(sudo i2cget -y 2 0x38 0x8F b)
    echo "FLOW=$F  TD=$T  INT_CNT=$I"
    sleep 0.1
done
```

On the affected cape the output is an increasing `FLOW` column, a `TD=0x00` column with no exceptions, and an `INT_CNT` column frozen at its boot value. On a healthy cape the last two columns respond to every touch.

---

## Software stack — ruled out as cause

To pre-empt the common "is your driver right?" line of questioning, here is what was tried and what the evidence says:

- **Mainline `edt_ft5x06` driver** binds and registers `/dev/input/event1`; I²C reads succeed; chip IDs come back correctly. Ruled out as the blocker.
- **Newhaven's own FT5426 Linux driver** (GitHub: `NewhavenDisplay/FT5X26-Focaltech-Drivers`) was reviewed — its init sequence contains no register writes that would change the post-POR scan-engine behaviour beyond what the FocalTech datasheet specifies: *"After resetting, FT5X26 shall enter the Active mode"* with auto-scanning at 100 fps. No host-side firmware load is required.
- **Manual register pokes** were attempted: WORKMODE=0x00, POWER_MODE=0x00 (force Active), software reset via DEVICE_MODE bit 7. The chip's mode/config registers accept writes and read back correctly, but no combination causes TD_STATUS to become non-zero or INT_CNT to tick on touch.
- **DIP switch S1** is the cape EEPROM I²C-address selector (0x54..0x57 per the NHD-7.0CTP-CAPE user guide). Since the cape is already being detected by u-boot as `BB-BONE-NH7C-01`, S1 is set correctly. This is not a configuration issue.
- **Flex cables** were visually inspected on the unit and appear correctly seated.

The `FLOW_WORK_CNT`-increments-while-`TD_STATUS`-stays-zero signature rules out every software-side class of cause. The chip would report touches if any capacitance change were being measured on its sense lines.

---

## Requested resolution

Replacement of the cape under warranty. We are happy to ship the failing unit back for inspection at Newhaven's engineering team if it would help root-cause the defect on your assembly line — the live-register signature above is distinctive and should be straightforward to reproduce.
