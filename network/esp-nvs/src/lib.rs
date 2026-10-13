//! ESP-IDF-compatible NVS adapter for the rlvgl network configuration store.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;

use esp_nvs::{Key, Nvs, platform::Platform};
use rlvgl_network::{ConfigDecodeError, NetworkConfig, NetworkConfigStore};

#[cfg(all(feature = "esp-storage", target_arch = "riscv32"))]
mod flash;

#[cfg(all(feature = "esp-storage", target_arch = "riscv32"))]
pub use flash::{EspFlashNetworkConfigStore, OpenError, open_store};

const NAMESPACE: Key = Key::from_str("rlvgl_net");
const CONFIG_KEY: Key = Key::from_str("config_v1");

/// Failure while reading or writing an rlvgl network record in ESP NVS.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The underlying ESP NVS operation failed.
    Nvs(esp_nvs::error::Error),
    /// NVS returned a blob that failed rlvgl record validation.
    Decode(ConfigDecodeError),
}

/// Network-configuration store backed by an initialized ESP NVS partition.
pub struct EspNvsNetworkConfigStore<T: Platform> {
    nvs: Nvs<T>,
}

impl<T: Platform> EspNvsNetworkConfigStore<T> {
    /// Wrap an initialized ESP NVS instance.
    pub const fn new(nvs: Nvs<T>) -> Self {
        Self { nvs }
    }

    /// Release and return the underlying ESP NVS instance.
    pub fn into_inner(self) -> Nvs<T> {
        self.nvs
    }
}

impl<T: Platform> NetworkConfigStore for EspNvsNetworkConfigStore<T> {
    type Error = Error;

    fn load(&mut self) -> Result<Option<NetworkConfig>, Self::Error> {
        let bytes: Vec<u8> = match self.nvs.get(&NAMESPACE, &CONFIG_KEY) {
            Ok(bytes) => bytes,
            Err(esp_nvs::error::Error::NamespaceNotFound | esp_nvs::error::Error::KeyNotFound) => {
                return Ok(None);
            }
            Err(error) => return Err(Error::Nvs(error)),
        };
        NetworkConfig::decode(&bytes)
            .map(Some)
            .map_err(Error::Decode)
    }

    fn store(&mut self, config: &NetworkConfig) -> Result<(), Self::Error> {
        let record = config.encode();
        self.nvs
            .set(&NAMESPACE, &CONFIG_KEY, record.as_slice())
            .map_err(Error::Nvs)
    }
}

#[cfg(test)]
mod tests {
    use esp_nvs::{FLASH_SECTOR_SIZE, mem_flash::MemFlash};
    use rlvgl_network::WifiCredentials;

    use super::*;

    #[test]
    fn adapter_round_trips_the_portable_record() {
        let flash = MemFlash::new(3);
        let nvs = Nvs::new(0, 3 * FLASH_SECTOR_SIZE, flash).unwrap();
        let mut store = EspNvsNetworkConfigStore::new(nvs);
        let config = NetworkConfig::new(4, WifiCredentials::new("bench", "secret").unwrap());

        store.store(&config).unwrap();
        assert_eq!(store.load().unwrap(), Some(config));
    }
}
