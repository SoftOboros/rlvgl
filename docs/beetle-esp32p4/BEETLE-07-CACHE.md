<!--
BEETLE-07-CACHE.md - Cache writeback (C2M direction) for PSRAM-backed
FBs. Implemented.
-->

**[← BEETLE-06](BEETLE-06-DPI-PANEL.md) · [Index](README.md) · [Next →](BEETLE-08-DEMO-INTEGRATION.md)**

# BEETLE-07 — Cache Writeback (CPU→Memory) for PSRAM Framebuffer

> **Implementation status:** Implemented.
> `dfr0550/cache.rs::writeback` is the canonical entry point. Awaits
> HIL validation under DSI DMA load.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "Cache" chapter | L1 D-cache + L2 cache topology, SYNC_* register layout, writeback semantics | `(TRM §Cache)` |
| `esp32p4 = 0.2` PAC | `CACHE.sync_*` register block | `(pac::CACHE.sync_*())` |
| `IDF components/hal/esp32p4/include/hal/cache_ll.h` | SYNC_MAP bit assignment, register-driven flow vs ROM helper | `(IDF cache_ll.h)` |
| `IDF components/esp_hw_support/include/esp_cache.h` | `esp_cache_msync` API + direction flags | `(IDF esp_cache.h)` |

## §1 Purpose

Make CPU writes to the PSRAM-backed framebuffer visible to the DSI
DMA by flushing dirty cache lines covering the modified range. The
DSI scanout reads from PSRAM physical addresses without participating
in cache coherency; without explicit writeback the DMA reads stale
memory.

## §2 Problem statement

The IDF reference is:

```c
esp_cache_msync(fb, fb_bytes, ESP_CACHE_MSYNC_FLAG_DIR_C2M);
```

The raw-PAC equivalent drives the CACHE peripheral's `SYNC_*`
registers directly (the IDF helper calls into the BootROM
`Cache_WriteBack_Addr`, but the underlying register interface is
the CACHE peripheral's `SYNC_{MAP,ADDR,SIZE,CTRL}` block — the ROM
helper just sets those same registers and spins on `SYNC_DONE`).

Direct register driving keeps the bring-up free of ROM-symbol
address bindings and is preferable per the PAC + TRM project posture.

Anchor: `dfr0550/cache.rs:46-86`.

## §3 Canonical glossary

- **Cache writeback C2M** — CPU-to-Memory direction: flush dirty
  cache lines covering `[ptr, ptr+len)` so a DMA peer reads current
  CPU-written contents. **As defined in BEETLE-00 §3; restated here
  for chapter accessibility.**
- **SYNC_MAP** — Bit mask selecting which cache layers participate
  in the sync. bits[0:3] = L1-ICache0..3, bit 4 = L1-DCache, bit 5
  = L2-Cache. **As defined in `IDF cache_ll.h`; used without
  modification.**
- **`SYNC_CTRL.WRITEBACK_ENA`** — Self-clearing trigger. Writing 1
  initiates writeback; hardware self-clears when done.
  **As defined in PAC + TRM; used without modification.**
- **`SYNC_CTRL.SYNC_DONE`** — Status bit; reads 1 once the operation
  finishes. **As above.**
- **Cache line** — 64 bytes on ESP32-P4 (L1 D and L2 are both 64 B).
  Operations are rounded out to 64-B boundaries. **As defined in
  TRM Cache chapter; used without modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| CACHE register block field names | `esp32p4` PAC |
| SYNC_MAP bit assignment | `IDF cache_ll.h` (PAC + TRM agree but IDF names the layers) |
| Cache line size 64 B | TRM (constant `CACHE_LINE_BYTES` in `dfr0550/cache.rs:35`) |
| Pre-sync wait + post-sync poll loop | This chapter §9 (mirrors IDF) |
| `writeback(ptr, len)` API | `dfr0550/cache.rs` (code is canonical) |
| Cache layers participating (L1-DCache + L2) | This chapter §9 INV-BEETLE-07-1 |

## §5 Authority relationship matrix

Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
No new external authorities.

## §6 Frozen enums

None. The chapter does not surface any user-facing enums (the
direction is fixed C2M; the SYNC_MAP is fixed L1-D + L2). A future
multi-direction API would introduce a `CacheSyncDir` enum
(**Standards Action** to add).

## §7 Frozen timing & topology

- **SYNC_MAP value:** `SYNC_MAP_L1_DCACHE | SYNC_MAP_L2_CACHE = 0x30`.
- **Alignment:** start rounded down to 64 B, end rounded up to 64 B.
- **Pre-call wait:** spin on `SYNC_CTRL.SYNC_DONE` to drain any
  prior sync. Required because back-to-back writebacks could
  otherwise race the in-flight operation.
- **Trigger:** `SYNC_CTRL.WRITEBACK_ENA = 1` via `modify` (preserves
  other bits in the register).
- **Post-call wait:** spin on `SYNC_CTRL.SYNC_DONE` until 1. Hardware
  self-clears `WRITEBACK_ENA` when done.

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-07-1 — Both L1 D-cache and L2 cache participate

Every writeback MUST set `SYNC_MAP = L1_DCACHE | L2_CACHE = 0x30`.
Setting only L1-D leaves dirty L2 lines untouched if the original
write hit a write-back miss; setting only L2 leaves the L1-D dirty
copies in place.

**Registration policy:** **Standards Action**.

### INV-BEETLE-07-2 — 64-B alignment

The start address MUST be rounded down to a 64-B boundary; the
range end MUST be rounded up. The CACHE peripheral does not silently
extend; an unaligned range leaves the partially-covered cache lines
at each end un-synced.

**Registration policy:** **Standards Action**.

### INV-BEETLE-07-3 — Drain prior sync before submitting new

Before writing `SYNC_ADDR / SYNC_SIZE / SYNC_MAP / SYNC_CTRL`, the
caller MUST wait for `SYNC_CTRL.SYNC_DONE == 1`. Submitting a new
sync mid-operation may corrupt the address/size of the in-flight
sync.

**Registration policy:** **Standards Action**.

### INV-BEETLE-07-4 — Mutually exclusive sync enables

`SYNC_CTRL` carries four enable bits (invalidate, clean, writeback,
writeback-invalidate). The four are mutually exclusive per the TRM
— only one MUST be set per submission. This chapter's
`writeback()` sets only `WRITEBACK_ENA`.

A future API extending this to invalidate / clean directions MUST
preserve this exclusivity.

**Registration policy:** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

This chapter does not modify the PAC-side `CACHE` register block,
the cache controller hardware, or the IDF reference. The
`writeback(ptr, len)` API is a thin shim — its main contribution is
the in-tree pattern (PAC + TRM, no ROM symbols) and the alignment
guarantee.

Consumers in this initiative:
- BEETLE-06 calls `writeback` at the initial FB paint (§9
  INV-BEETLE-06-7).
- BEETLE-08 calls `writeback` after every continuous re-fill
  iteration (§9 INV-BEETLE-00-3).

## §11 Non-goals

- Invalidate direction (M2C: DMA → CPU). The DSI DMA only reads from
  PSRAM; the CPU never reads back the FB. Not needed for this
  initiative. Future audio capture or DSI-input use cases will need
  M2C; that's a separate chapter / family.
- Clean direction (write-back without invalidate). Same: not needed.
- Writeback-invalidate (combined). Not needed.
- Per-layer SYNC_MAP customization. The fixed `L1_DCACHE | L2_CACHE`
  is the only correct mask for DMA-visible writes.

## §12 Acceptance checklist

A conforming `cache::writeback(ptr, len)` MUST:

- [ ] (a) Set `SYNC_MAP = 0x30` (L1-DCache + L2-Cache).
- [ ] (b) Round `[ptr, ptr+len)` to 64-B-aligned boundaries.
- [ ] (c) Wait for `SYNC_CTRL.SYNC_DONE` before submitting.
- [ ] (d) Set only `SYNC_CTRL.WRITEBACK_ENA` (no other sync bits).
- [ ] (e) Wait for `SYNC_CTRL.SYNC_DONE` after submitting.
- [ ] (f) Short-circuit on `len == 0` (already implemented).
- [ ] (g) **HIL verification:** confirm color cycles in BEETLE-06
      acceptance gate (g) do not exhibit the "colors briefly
      visible then fade" failure mode documented in
      `project_dfr1237_dfr0550v2.md`.

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/cache.rs:1-86`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/cache_ll.h`
- `~/esp/esp-idf/components/esp_hw_support/include/esp_cache.h`
- ESP32-P4 TRM "Cache" chapter

## §14 Unblocks

- BEETLE-06 initial paint correctness.
- BEETLE-08 continuous re-fill correctness.
- Future audio-capture or DSI-input chapters can extend with M2C
  direction.

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  `dfr0550/cache.rs::writeback` from commit `36a56cd`. Invariants
  1-4 first ratification. Awaits HIL validation under DSI DMA load
  (acceptance gate (g)) — pending BEETLE-06 implementation.

---

**[← BEETLE-06](BEETLE-06-DPI-PANEL.md)** · **[Index](README.md)** · **Next →** [BEETLE-08 — Demo Integration](BEETLE-08-DEMO-INTEGRATION.md)
