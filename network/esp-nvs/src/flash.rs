//! ESP flash and partition discovery for the shared NVS network store.

use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
use esp_bootloader_esp_idf::partitions::{
    DataPartitionSubType, Error as PartitionError, PARTITION_TABLE_MAX_LEN, PartitionType,
    read_partition_table,
};
use esp_nvs::{Nvs, platform::Crc};
use esp_storage::{FlashStorage, FlashStorageError};

use crate::EspNvsNetworkConfigStore;

/// Failure while discovering or opening the ESP-IDF NVS partition.
pub enum OpenError {
    /// The ESP-IDF partition table could not be read or decoded.
    Partition(PartitionError),
    /// The partition table has no data/NVS entry.
    MissingNvsPartition,
    /// The NVS entry is encrypted, which `esp-storage` cannot access.
    EncryptedNvsPartition,
    /// The NVS data structure could not be initialized.
    Nvs(esp_nvs::error::Error),
}

impl core::fmt::Debug for OpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Partition(error) => formatter.debug_tuple("Partition").field(error).finish(),
            Self::MissingNvsPartition => formatter.write_str("MissingNvsPartition"),
            Self::EncryptedNvsPartition => formatter.write_str("EncryptedNvsPartition"),
            Self::Nvs(error) => formatter.debug_tuple("Nvs").field(error).finish(),
        }
    }
}

/// `esp-storage` wrapper adding the CRC implementation required by `esp-nvs`.
pub struct EspFlashPlatform(FlashStorage);

impl ErrorType for EspFlashPlatform {
    type Error = FlashStorageError;
}

impl ReadNorFlash for EspFlashPlatform {
    const READ_SIZE: usize = <FlashStorage as ReadNorFlash>::READ_SIZE;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        ReadNorFlash::read(&mut self.0, offset, bytes)
    }

    fn capacity(&self) -> usize {
        ReadNorFlash::capacity(&self.0)
    }
}

impl NorFlash for EspFlashPlatform {
    const WRITE_SIZE: usize = <FlashStorage as NorFlash>::WRITE_SIZE;
    const ERASE_SIZE: usize = <FlashStorage as NorFlash>::ERASE_SIZE;

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        NorFlash::write(&mut self.0, offset, bytes)
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        NorFlash::erase(&mut self.0, from, to)
    }
}

impl Crc for EspFlashPlatform {
    fn crc32(init: u32, data: &[u8]) -> u32 {
        esp_nvs::platform::software_crc32(init, data)
    }
}

/// Network store opened directly on an ESP-IDF data/NVS flash partition.
pub type EspFlashNetworkConfigStore = EspNvsNetworkConfigStore<EspFlashPlatform>;

/// Discover the ESP-IDF data/NVS partition and open the shared store adapter.
pub fn open_store() -> Result<EspFlashNetworkConfigStore, OpenError> {
    let mut flash = FlashStorage::new();
    let mut partition_bytes = [0_u8; PARTITION_TABLE_MAX_LEN];
    let table =
        read_partition_table(&mut flash, &mut partition_bytes).map_err(OpenError::Partition)?;
    let partition = table
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .map_err(OpenError::Partition)?
        .ok_or(OpenError::MissingNvsPartition)?;
    if partition.is_encrypted() {
        return Err(OpenError::EncryptedNvsPartition);
    }

    let nvs = Nvs::new(
        partition.offset() as usize,
        partition.len() as usize,
        EspFlashPlatform(flash),
    )
    .map_err(OpenError::Nvs)?;
    Ok(EspNvsNetworkConfigStore::new(nvs))
}
