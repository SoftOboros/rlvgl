<!--
docs/TODO-FATFS-ASSETS.md - TODO – FATFS-backed Asset Load for rlvgl (optional core feature).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# TODO – FATFS-backed Asset Load for rlvgl (optional core feature)

> **Epic:** Add optional filesystem-based asset loading to rlvgl using a portable FAT implementation. Core exposes a small, stable `AssetSource` API; platform crates provide block-device drivers (SD card on H747I‑DISCO) or a simulator stub. When disabled, core still supports baked-in assets.

---

## Goals & Non‑Goals

- **Goals**
  - Optional `` core feature enabling FATFS-backed assets.
  - Platform glue via a `BlockDevice` trait implemented by each target (SD on DISCO; file-backed image on simulator).
  - Zero `std` in core; `std` only in simulator backend.
  - Read-only v0 (mount, list, open, read). Write/flush are future.
  - Safe DMA & D‑Cache handling on H7 for SDMMC.
- **Non‑Goals (v0)**
  - No journaling or exotic filesystems.
  - No dynamic partitioning tools.

---

## Features & Crate Layout

| ✓   | Description                                     | Dependencies           | Notes                               |
| --- | ----------------------------------------------- | ---------------------- | ----------------------------------- |
| [x] | Add `fs` feature to `rlvgl/core`                | `alloc`                | All FS code behind feature flag     |
| [x] | FS traits (`BlockDevice`, `FsError`) in core     | —                     | Moved from standalone crate        |
| [x] | New crate: `rlvgl-fs-sim` (std)                 | `fatfs`, `std`         | Simulator: file-backed block device |
| [x] | Platform module: `platform/stm32h747i_disco_sd` | HAL + DMA              | SDMMC + DMA + cache maintenance     |

> **FAT impl choice:** Prefer the Rust `fatfs` crate in `no_std` mode for consistent API across targets. `embedded-sdmmc` is an alternative; keep the abstraction thin so either can slot in later.

---

## Public API (Core‑facing)

**In **``

```rust
/// 512-byte logical sectors recommended; expose actual size via `block_size()`.
pub trait BlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError>;
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError>; // v1: may be stubbed for RO
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn flush(&mut self) -> Result<(), FsError>;
}

/// Filesystem handle (FAT volume) constructed over a BlockDevice.
pub struct FatVolume<'a, B: BlockDevice> { /* ... */ }

pub trait AssetSource {
    /// Open an asset by logical path, e.g., "fonts/regular.bin".
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, FsError>;
    fn exists(&self, path: &str) -> bool;
    fn list(&self, dir: &str) -> Result<AssetIter, FsError>;
}

pub trait AssetRead {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, FsError>;
    fn len(&self) -> usize;
    fn seek(&mut self, pos: u64) -> Result<u64, FsError>;
}
```

**In **``** (behind **``**)**

```rust
pub struct AssetManager<S: AssetSource> { /* ... */ }
impl<S: AssetSource> AssetManager<S> {
    pub fn load_font(&self, path: &str) -> Result<Font, AssetError>;
    pub fn load_image(&self, path: &str) -> Result<Image, AssetError>;
    // generic helper
    pub fn open(&self, path: &str) -> Result<Box<dyn AssetRead + '_>, AssetError>;
}
```

---

## Simulator (std) – Single File Disk Image

| ✓   | Description                   | Dependencies        | Notes                                               |
| --- | ----------------------------- | ------------------- | --------------------------------------------------- |
| [x] | Implement `SimBlockDevice`    | `std::fs::File`     | One big **disk image** file, pre-sized (e.g., 32MB) |
| [x] | Optional memory-map for speed | `memmap2` (feature) | Fallback to pread/pwrite if unavailable             |
| [x] | Tool: create/populate image   | Rust CLI            | `mkfatimg --size 32M --from ./assets/`              |
| [ ] | Mount & smoke test            | rlvgl sim           | Read a PNG/font, render a label                     |

**Rationale:** Keep FAT logic intact by letting FATFS manage the on-disk layout. The simulator just provides sector reads/writes into a single host file.

---

## STM32H747I‑DISCO SD Card Driver (SDMMC + DMA)

| ✓   | Description                    | Dependencies | Notes                                                                     |
| --- | ------------------------------ | ------------ | ------------------------------------------------------------------------- |
| [x] | Pin/clock config in CubeMX     | CubeMX       | SDMMC1 4-bit wide, proper GPIO AF                                         |
| [x] | `DCache` strategy              | cortex-m     | Use non-cacheable region for DMA buffers or clean/invalidate around xfers |
| [x] | Implement `DiscoSdBlockDevice` | HAL + LL     | Init card, read\_multi, write\_multi, block size=512                      |
| [ ] | DMA double-buffering           | HAL DMA      | Optimize sequential reads                                                 |
| [ ] | Mount FAT volume               | `fatfs`      | Provide `TimeProvider` (if required)                                      |
| [ ] | Long runner test               | on-device    | Stream read assets for minutes; check CRC/errors                          |

**Cache coherency:** H7’s D‑Cache requires explicit maintenance around DMA buffers. Encapsulate this in the driver so upper layers stay safe.

---

## Asset Layout Convention (v0)

```
/
  assets/
    fonts/   *.bin, *.fnt
    images/  *.raw, *.png (if decoder present)
    ui/      layout/*.json (optional)
```

- Paths are relative to the FAT root or a configured mount point.
- Keep small, contiguous files to minimize random I/O on SD.

---

## Integration Points

| ✓   | Description                      | Dependencies        | Notes                                                      |
| --- | -------------------------------- | ------------------- | ---------------------------------------------------------- |
| [x] | `core` feature gate `fs`         | cargo               | Must not pull in `std`                                     |
| [ ] | `platform` selects a BlockDevice | target BSP          | DISCO uses `DiscoSdBlockDevice`; sim uses `SimBlockDevice` |
| [ ] | Image decoder selection          | feature flags       | `png`, `jpeg`, `qoi`, etc. (optional)                      |
| [ ] | Font loader hook                 | `fontdue` or custom | From file → font cache                                     |

---

## Tests

| ✓   | Description                        | Dependencies | Notes                                    |
| --- | ---------------------------------- | ------------ | ---------------------------------------- |
| [x] | Unit: path open/list/exists        | `fatfs`      | Fake device with in-memory image         |
| [x] | Unit: partial read/seek            | —            | Verify correct offsets/lengths           |
| [ ] | Prop: random file sizes            | proptest     | Ensure no over/underruns                 |
| [ ] | Sim integration: load font & image | rlvgl sim    | Render text+bitmap; save PNG             |
| [ ] | DISCO integration: SD mount        | on-device    | OLED/LCD shows PASS/FAIL and asset names |

---

## CLI & Developer Tools

| ✓   | Description                   | Dependencies       | Notes                               |
| --- | ----------------------------- | ------------------ | ----------------------------------- |
| [x] | `mkfatimg` CLI                | `fatfs`, `walkdir` | Create disk image from a folder     |
| [x] | `fatcat` CLI                  | —                  | List/print files inside image       |
| [ ] | CI step: build/populate image | GitHub Actions     | Attach artifact for simulator tests |

---

## Error Model

- `FsError` enums: `Io`, `NoSuchFile`, `InvalidPath`, `MountFailed`, `Device`, `NotSupported`.
- Map device errors cleanly; never `panic!` in driver paths.
- Return deterministic errors for missing decoders vs missing files.

---

## Risks & Mitigations

| Risk                           | Mitigation                                                        |
| ------------------------------ | ----------------------------------------------------------------- |
| SDMMC + D‑Cache bugs           | Centralize cache ops in driver; add asserts & debug counters      |
| FAT fragmentation hurts perf   | Encourage asset packing; sequential reads; optional defrag tool   |
| `std` leakage into core        | FS API behind `fs` feature in core; simulator implementation isolated |
| Time source for FAT timestamps | Provide dummy time or feature flag to disable timestamps          |

---

## Exit Criteria (v0)

- Simulator: mounts disk image, loads a font and bitmap, renders a demo.
- DISCO: mounts SD card, lists `/assets`, renders text and one image.
- Core builds without `fs`; with `fs` feature it links using either backend.
- CI artifacts include a reproducible disk image used by sim tests.

