<!--
BEETLE-01-PSRAM.md - PSRAM init chapter (octal HEX @ 200 MHz). Stub
shell. v0/v1 inherit bootloader-managed PSRAM; v2 makes this raw-PAC.
-->

**[← BEETLE-00](BEETLE-00-CONCEPTS.md) · [Index](README.md) · [Next →](BEETLE-02-LDO.md)**

# BEETLE-01 — PSRAM Init (Octal HEX @ 200 MHz)

> **Implementation status:** Stub. `dfr0550/psram.rs::init()` returns
> `None`. v0 and v1 inherit bootloader-managed PSRAM via the
> `CONFIG_IDF_EXPERIMENTAL_FEATURES` / `CONFIG_SPIRAM_*` flags.
> Full raw-PAC bring-up (BEETLE-01a) is the v2 conformance gate.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "SPI Memory Controller" + "Cache" chapters | MSPI register layout, PSRAM access timing, cache MMU window | `(TRM §<chapter>)` |
| `esp32p4 = 0.2` PAC | `SPI0` / `SPI1` / `CACHE` peripheral register blocks | `(pac::PERIPH.reg().field())` |
| IDF `components/esp_psram/` | Bootloader-managed init reference | `(IDF esp_psram/...)` |
| APS6408L datasheet (Adesto/Renesas 64 Mb OPI PSRAM) | MR0 latency / drive strength / octal-HEX enable | `(APS6408L p.NN)` |

The APS6408L datasheet citation is best-effort — IDF treats the part
as a black box once the right command sequence has been issued. If
the on-board PSRAM revision differs (different latency tables), this
chapter MUST add a §15 amendment naming the new part number.

## §1 Purpose

Replace the bootloader-managed PSRAM init with a raw-PAC sequence so
the binary is end-to-end PAC + TRM with no IDF-bootloader dependency.
This is a **v2 goal**; v0 and v1 ship without this chapter
implemented.

The PSRAM bandwidth is load-bearing: the DSI DMA requires
~78 MB/s sustained, the silent 20 MHz default gives only ~40 MB/s,
and the bridge desyncs to white. So the chapter is "high
priority for v2" but "not blocking for first light" — the bootloader
covers the bandwidth requirement when its sdkconfig is right.

## §2 Problem statement

In raw-PAC bring-up we need to issue the APS6408L (or equivalent)
octal-HEX init sequence ourselves before anything touches
PSRAM-backed framebuffers. The IDF equivalent is the implicit
`bootloader_init_spiram` driven by sdkconfig:

```
CONFIG_IDF_EXPERIMENTAL_FEATURES=y    # required to unlock SPIRAM_SPEED_200M
CONFIG_SPIRAM=y
CONFIG_SPIRAM_MODE_HEX=y
CONFIG_SPIRAM_SPEED_200M=y
```

The raw-PAC equivalent has four sub-phases:

1. **MSPI clock up to 200 MHz** via PLL_F480M / 240.
2. **APS6408L sequence**: Reset (RSTEN/RST), MR0 write (latency,
   drive strength), enable octal-HEX mode (CMR / variable latency).
3. **Cache MMU window**: map PSRAM into CPU address space (typically
   `0x4810_0000` data / `0x4ff0_0000` MMU window on P4) and configure
   write-back + cache line size (64 B).
4. **Slab export**: expose a `&'static mut [u8]` for the framebuffer
   allocator.

Anchor: `dfr0550/psram.rs:40` (`init() -> Option<(*mut u8, usize)>`,
currently stub).

## §3 Canonical glossary

*TBD — populate when BEETLE-01a implementation begins. Initial
candidates: `MspiBus`, `PsramSlab`, `OctalHexMode`, `MR0`, `CacheWindow`.*

## §4 Source-of-truth map

*TBD — populate alongside §3.*

## §5 Authority relationship matrix

*Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
APS6408L row added when this chapter ratifies.*

## §6 Frozen enums

*TBD — likely an `MspiSpeed` enum (`Hz20Mhz / Hz200Mhz`) for the v0 /
v2 split, but pending implementation. Registration policy: **Standards
Action** for any enum that crosses the platform-vs-application
boundary.*

## §7 Frozen timing & topology

*TBD. Will pin: MSPI clock divider, APS6408L MR0 value, cache MMU
window addresses, cache line size, slab size.*

## §8 (reserved)

## §9 Frozen invariants

*TBD. Likely: INV-BEETLE-01-1 (sustained ≥78 MB/s),
INV-BEETLE-01-2 (cache write-back policy aligns with
INV-BEETLE-00-3), INV-BEETLE-01-3 (slab base alignment ≥64 B).*

## §10 Reconciliation vs adjacent repo primitives

The v0/v1 path keeps the IDF bootloader in the flash image; the
linker script `bsp_generated/esp32_p4.x` and `memory.x` therefore do
**not** define a PSRAM region today. Adding raw-PAC PSRAM init will
require either (a) extending those linker scripts to include a PSRAM
region with the right MMU-window address, or (b) keeping PSRAM
runtime-only (slab API) and not exposing it through the linker.
Decision deferred to BEETLE-01a.

The chipdb yaml at `chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml`
declares the module's PSRAM size; the generator currently does not
emit init code. Whether to push raw-PAC PSRAM init upstream into
`rlvgl-chips-esp` or keep it in `dfr0550/psram.rs` is a BEETLE-01a
question.

## §11 Non-goals

- Sub-200 MHz fallback (no use case yet).
- Quad SPI PSRAM (the DFR1172 carries octal HEX PSRAM only).
- ECC PSRAM variants.

## §12 Acceptance checklist

*A conforming BEETLE-01a (v2 deployment) implementation MUST:*

- [ ] (a) Initialise MSPI at 200 MHz against the APS6408L (or noted
      equivalent), without bootloader help.
- [ ] (b) Configure the cache MMU window for PSRAM access with
      write-back enabled and 64 B cache line size.
- [ ] (c) Export a slab API yielding ≥1 152 000 bytes of PSRAM
      (FB_BYTES) at a 64-B-aligned base.
- [ ] (d) Sustain ≥78 MB/s read bandwidth from PSRAM under DSI DMA
      load, verified by absence of bridge desync over a 5-minute
      continuous re-fill run.

*Until BEETLE-01a lands, the chapter's acceptance reduces to: the
bootloader's PSRAM init is observably running (FB writes don't fault),
the sdkconfig flags listed in §2 are present, and the bridge does not
desync.*

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/psram.rs`
- ESP32-P4 TRM "SPI Memory Controller", "Cache" chapters
- `~/esp/esp-idf/components/esp_psram/`
- APS6408L datasheet (TBD: confirm exact part number and
  ingest into memalpha when BEETLE-01a starts)

## §14 Unblocks

- v2 conformance per [README §Conformance targets](README.md#conformance-targets).
- Drops the IDF-bootloader dependency from the bare-metal binary.
- Enables custom PSRAM use cases beyond framebuffer (audio capture
  buffers, etc.) once future families need them.

## §15 Change log

- **2026-05-28** (initial shell) — Authored as part of the BEETLE
  family setup. No implementation yet; v0/v1 inherit bootloader
  init. §3, §4, §6, §7, §9 marked TBD. Real ratification waits for
  BEETLE-01a when implementation starts.

---

**[← BEETLE-00](BEETLE-00-CONCEPTS.md)** · **[Index](README.md)** · **Next →** [BEETLE-02 — DPHY LDO](BEETLE-02-LDO.md)
