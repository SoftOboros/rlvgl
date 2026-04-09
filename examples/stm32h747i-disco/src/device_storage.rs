// SPDX-License-Identifier: MIT
//! Real device storage browser for QSPI flash and SD card.
//!
//! Implements [`StorageBrowser`] via `embedded-sdmmc` for both QSPI flash
//! (FAT16 over the first 1 MB) and SD card.

extern crate alloc;

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use rlvgl::ui::file_browser::{EntryKind, FileEntry, StorageBrowser};

// ── QSPI block device for embedded-sdmmc ──────────────────────────────────

/// FAT filesystem partition size on QSPI flash (1 MB).
#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
const QSPI_FAT_SIZE: u32 = 1024 * 1024;

/// Wraps `Mt25tlFlash` as an `embedded_sdmmc::BlockDevice`.
///
/// Uses `RefCell` interior mutability (same pattern as `SdMmcBlockDev`)
/// because the trait takes `&self`.
#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
pub struct QspiBlockDev {
    flash: Rc<RefCell<rlvgl::platform::Mt25tlFlash>>,
}

#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
#[derive(Debug, Clone, Copy)]
pub enum QspiError {
    Flash,
}

#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
impl QspiBlockDev {
    pub fn new(flash: Rc<RefCell<rlvgl::platform::Mt25tlFlash>>) -> Self {
        Self { flash }
    }
}

#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
impl embedded_sdmmc::blockdevice::BlockDevice for QspiBlockDev {
    type Error = QspiError;

    fn read(
        &self,
        blocks: &mut [embedded_sdmmc::blockdevice::Block],
        start: embedded_sdmmc::blockdevice::BlockIdx,
    ) -> Result<(), Self::Error> {
        let mut flash = self.flash.borrow_mut();
        for (i, block) in blocks.iter_mut().enumerate() {
            let addr = (start.0 + i as u32) * 512;
            flash
                .read(addr, &mut block.contents)
                .map_err(|_| QspiError::Flash)?;
        }
        Ok(())
    }

    fn write(
        &self,
        blocks: &[embedded_sdmmc::blockdevice::Block],
        start: embedded_sdmmc::blockdevice::BlockIdx,
    ) -> Result<(), Self::Error> {
        let mut flash = self.flash.borrow_mut();
        for (i, block) in blocks.iter().enumerate() {
            let addr = (start.0 + i as u32) * 512;
            let ss_mask = 4096u32 - 1;
            let ss_base = addr & !ss_mask;
            let ss_off = (addr - ss_base) as usize;

            // Read-modify-write at 4KB subsector granularity
            let mut ss_buf = [0u8; 4096];
            flash
                .read(ss_base, &mut ss_buf)
                .map_err(|_| QspiError::Flash)?;
            ss_buf[ss_off..ss_off + 512].copy_from_slice(&block.contents);
            flash
                .erase_subsector(ss_base)
                .map_err(|_| QspiError::Flash)?;
            flash
                .write(ss_base, &ss_buf)
                .map_err(|_| QspiError::Flash)?;
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<embedded_sdmmc::blockdevice::BlockCount, Self::Error> {
        Ok(embedded_sdmmc::blockdevice::BlockCount(QSPI_FAT_SIZE / 512))
    }
}

// ── QSPI format check ────────────────────────────────────────────────────

/// Check if the first 1 MB of QSPI has a valid FAT filesystem.
/// If not, erase and write a minimal FAT16 boot sector + tables.
/// Returns true if formatting was performed.
#[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
pub fn ensure_qspi_formatted(flash: &Rc<RefCell<rlvgl::platform::Mt25tlFlash>>) -> bool {
    use rlvgl::platform::sd_emmc_adapter::DummyTimeSource;

    let bd = QspiBlockDev::new(flash.clone());
    let vm = embedded_sdmmc::VolumeManager::new(bd, DummyTimeSource);

    // Try to open volume 0 — if it works, we're formatted.
    if vm.open_volume(embedded_sdmmc::VolumeIdx(0)).is_ok() {
        return false;
    }

    // Not formatted — erase first 1 MB and write a minimal FAT16 image.
    {
        let mut f = flash.borrow_mut();
        // Erase in 64KB sectors for speed (16 × 64KB = 1 MB)
        for i in 0..16 {
            let _ = f.erase_sector(i * 65536);
        }
    }

    // Write a minimal FAT16 boot sector.
    // 1 MB = 2048 sectors of 512 bytes.
    // FAT16 with 1 sector/cluster, 1 reserved, 2 FATs, 512 root entries.
    let mut boot = [0u8; 512];
    // Jump boot code
    boot[0] = 0xEB;
    boot[1] = 0x3C;
    boot[2] = 0x90;
    // OEM name
    boot[3..11].copy_from_slice(b"RLVGL   ");
    // Bytes per sector: 512
    boot[11] = 0x00;
    boot[12] = 0x02;
    // Sectors per cluster: 1
    boot[13] = 1;
    // Reserved sectors: 1
    boot[14] = 1;
    boot[15] = 0;
    // Number of FATs: 2
    boot[16] = 2;
    // Root entry count: 512 (32 sectors of 16 entries each)
    boot[17] = 0x00;
    boot[18] = 0x02;
    // Total sectors (16-bit): 2048
    boot[19] = 0x00;
    boot[20] = 0x08;
    // Media type: fixed disk
    boot[21] = 0xF8;
    // FAT size in sectors: 8 (enough for 2048 clusters)
    boot[22] = 8;
    boot[23] = 0;
    // Sectors per track (dummy)
    boot[24] = 32;
    boot[25] = 0;
    // Number of heads (dummy)
    boot[26] = 2;
    boot[27] = 0;
    // Hidden sectors
    boot[28..32].copy_from_slice(&0u32.to_le_bytes());
    // Total sectors (32-bit): 0 (using 16-bit field)
    boot[32..36].copy_from_slice(&0u32.to_le_bytes());
    // Drive number
    boot[36] = 0x80;
    // Reserved
    boot[37] = 0;
    // Extended boot signature
    boot[38] = 0x29;
    // Volume serial number
    boot[39..43].copy_from_slice(&0x12345678u32.to_le_bytes());
    // Volume label
    boot[43..54].copy_from_slice(b"QSPI FLASH ");
    // File system type
    boot[54..62].copy_from_slice(b"FAT16   ");
    // Boot signature
    boot[510] = 0x55;
    boot[511] = 0xAA;

    {
        let mut f = flash.borrow_mut();
        let _ = f.write(0, &boot);

        // Initialize FAT1 and FAT2 — first two entries are reserved.
        // FAT16 entry = 2 bytes. Entry 0 = 0xFFF8 (media), entry 1 = 0xFFFF.
        let mut fat_start = [0u8; 4];
        fat_start[0] = 0xF8;
        fat_start[1] = 0xFF;
        fat_start[2] = 0xFF;
        fat_start[3] = 0xFF;
        // FAT1 at sector 1 (byte 512)
        let _ = f.write(512, &fat_start);
        // FAT2 at sector 9 (byte 512 * 9 = 4608)
        let _ = f.write(512 * 9, &fat_start);
    }

    true
}

// ── Device storage browser ────────────────────────────────────────────────

/// Real hardware storage browser.
///
/// Device 0 = QSPI Flash (FAT16 over first 1 MB)
/// Device 1 = SD Card (if present)
pub struct DeviceStorage {
    #[cfg(feature = "qspi_flash")]
    qspi: Option<Rc<RefCell<rlvgl::platform::Mt25tlFlash>>>,
    #[cfg(feature = "sd_storage")]
    sd_present: bool,
}

impl DeviceStorage {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "qspi_flash")]
            qspi: None,
            #[cfg(feature = "sd_storage")]
            sd_present: false,
        }
    }

    #[cfg(feature = "qspi_flash")]
    pub fn set_qspi(&mut self, flash: Rc<RefCell<rlvgl::platform::Mt25tlFlash>>) {
        self.qspi = Some(flash);
    }

    #[cfg(feature = "sd_storage")]
    #[allow(dead_code)]
    pub fn set_sd_present(&mut self, present: bool) {
        self.sd_present = present;
    }

    #[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
    fn list_qspi_dir(&self, path: &str) -> Result<Vec<FileEntry>, ()> {
        use rlvgl::platform::sd_emmc_adapter::DummyTimeSource;

        let flash = self.qspi.as_ref().ok_or(())?;
        let bd = QspiBlockDev::new(flash.clone());
        let vm = embedded_sdmmc::VolumeManager::new(bd, DummyTimeSource);
        let volume = vm
            .open_volume(embedded_sdmmc::VolumeIdx(0))
            .map_err(|_| ())?;
        let root_dir = volume.open_root_dir().map_err(|_| ())?;

        let mut entries = Vec::new();

        // Add ".." for sub-directory navigation
        if path != "/" {
            entries.push(FileEntry {
                name: String::from(".."),
                kind: EntryKind::Directory,
            });
        }

        // Open the target directory
        if path == "/" || path.is_empty() {
            root_dir
                .iterate_dir(|entry| {
                    push_sdmmc_entry(&mut entries, entry);
                })
                .map_err(|_| ())?;
        } else {
            let stripped = path.trim_start_matches('/');
            let sub = root_dir.open_dir(stripped).map_err(|_| ())?;
            sub.iterate_dir(|entry| {
                push_sdmmc_entry(&mut entries, entry);
            })
            .map_err(|_| ())?;
        }

        Ok(entries)
    }
}

#[cfg(feature = "sd_storage")]
fn push_sdmmc_entry(entries: &mut Vec<FileEntry>, entry: &embedded_sdmmc::DirEntry) {
    let base = entry.name.base_name();
    let ext = entry.name.extension();
    let base_s = core::str::from_utf8(base).unwrap_or("").trim_end();
    let ext_s = core::str::from_utf8(ext).unwrap_or("").trim_end();

    // Skip . and .. entries and volume labels
    if base_s == "." || base_s == ".." {
        return;
    }
    if entry.attributes.is_volume() {
        return;
    }

    let name = if ext_s.is_empty() {
        String::from(base_s)
    } else {
        let mut s = String::from(base_s);
        s.push('.');
        s.push_str(ext_s);
        s
    };

    let kind = if entry.attributes.is_directory() {
        EntryKind::Directory
    } else if ext_s.eq_ignore_ascii_case("WAV") {
        EntryKind::WavFile
    } else {
        EntryKind::OtherFile
    };

    entries.push(FileEntry { name, kind });
}

impl StorageBrowser for DeviceStorage {
    fn list_devices(&mut self) -> Vec<FileEntry> {
        let mut devices = Vec::new();

        #[cfg(feature = "qspi_flash")]
        if self.qspi.is_some() {
            devices.push(FileEntry {
                name: String::from("QSPI Flash"),
                kind: EntryKind::Device,
            });
        }

        #[cfg(feature = "sd_storage")]
        if self.sd_present {
            devices.push(FileEntry {
                name: String::from("SD Card"),
                kind: EntryKind::Device,
            });
        }

        if devices.is_empty() {
            devices.push(FileEntry {
                name: String::from("(no storage)"),
                kind: EntryKind::OtherFile,
            });
        }

        devices
    }

    fn list_directory(&mut self, device_index: usize, path: &str) -> Result<Vec<FileEntry>, ()> {
        let mut dev_idx = 0usize;

        #[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
        {
            if self.qspi.is_some() {
                if device_index == dev_idx {
                    return self.list_qspi_dir(path);
                }
                dev_idx += 1;
            }
        }

        #[cfg(feature = "sd_storage")]
        {
            if self.sd_present && device_index == dev_idx {
                let mut entries = vec![FileEntry {
                    name: String::from(".."),
                    kind: EntryKind::Directory,
                }];
                entries.push(FileEntry {
                    name: String::from("(SD not yet wired)"),
                    kind: EntryKind::OtherFile,
                });
                return Ok(entries);
            }
            let _ = dev_idx;
        }

        Err(())
    }
}
