<!--
docs/TODO-FATFS-ASSETS.md - TODO – FATFS-backed Asset Load for rlvgl (optional core feature).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# TODO – FATFS-backed Asset Load for rlvgl (optional core feature)

> **Epic:** Add optional filesystem-based asset loading to rlvgl using a portable FAT implementation. Core exposes a small, stable `AssetSource` API; platform crates provide block-device drivers (SD card on H747I-DISCO) or a simulator stub. When disabled, core still supports baked-in assets.

---

## Goals & Non-Goals

- **Goals**
  - Optional core feature enabling FATFS-backed assets.
  - Platform glue via a `BlockDevice` trait implemented by each target (SD on DISCO; file-backed image on simulator).
  - Zero `std` in core; `std` only in simulator backend.
  - Read-only v0 (mount, list, open, read). Write/flush are future.
  - Safe DMA & D-Cache handling on H7 for SDMMC.
- **Non-Goals (v0)**
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

## Public API (Core-facing)

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

## STM32H747I-DISCO SD Card Driver (SDMMC + DMA)
