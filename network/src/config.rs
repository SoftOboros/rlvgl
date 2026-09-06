//! Bounded Wi-Fi configuration, persistent encoding, and store policy.

use core::{fmt, str};

/// Maximum Wi-Fi SSID length in bytes.
pub const WIFI_SSID_MAX_LEN: usize = 32;
/// Maximum Wi-Fi password or raw PSK length in bytes.
pub const WIFI_PASSWORD_MAX_LEN: usize = 64;
/// Exact byte length of the version-one persistent network record.
pub const CONFIG_RECORD_LEN: usize = 112;

const CONFIG_MAGIC: [u8; 4] = *b"RLNW";
const CONFIG_VERSION: u8 = 1;
const HEADER_LEN: usize = 12;
const PASSWORD_OFFSET: usize = HEADER_LEN + WIFI_SSID_MAX_LEN;
const CRC_OFFSET: usize = PASSWORD_OFFSET + WIFI_PASSWORD_MAX_LEN;

/// A validation error for Wi-Fi credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialsError {
    /// The SSID was empty.
    MissingSsid,
    /// The UTF-8 SSID exceeded 32 bytes.
    SsidTooLong,
    /// The UTF-8 password or raw PSK exceeded 64 bytes.
    PasswordTooLong,
}

/// Bounded Wi-Fi credentials suitable for `no_std` firmware.
#[derive(Clone, Eq, PartialEq)]
pub struct WifiCredentials {
    ssid: [u8; WIFI_SSID_MAX_LEN],
    password: [u8; WIFI_PASSWORD_MAX_LEN],
    ssid_len: u8,
    password_len: u8,
}

impl WifiCredentials {
    /// Validate and copy an SSID and password into fixed-capacity storage.
    pub fn new(ssid: &str, password: &str) -> Result<Self, CredentialsError> {
        if ssid.is_empty() {
            return Err(CredentialsError::MissingSsid);
        }
        if ssid.len() > WIFI_SSID_MAX_LEN {
            return Err(CredentialsError::SsidTooLong);
        }
        if password.len() > WIFI_PASSWORD_MAX_LEN {
            return Err(CredentialsError::PasswordTooLong);
        }

        let mut credentials = Self {
            ssid: [0; WIFI_SSID_MAX_LEN],
            password: [0; WIFI_PASSWORD_MAX_LEN],
            ssid_len: ssid.len() as u8,
            password_len: password.len() as u8,
        };
        credentials.ssid[..ssid.len()].copy_from_slice(ssid.as_bytes());
        credentials.password[..password.len()].copy_from_slice(password.as_bytes());
        Ok(credentials)
    }

    /// Return the configured SSID.
    pub fn ssid(&self) -> &str {
        // Construction and decoding both validate UTF-8 before establishing
        // the value, so this slice always preserves that invariant.
        str::from_utf8(&self.ssid[..usize::from(self.ssid_len)])
            .expect("validated Wi-Fi SSID invariant")
    }

    /// Return the configured password.
    ///
    /// Callers must not log or include this value in status output.
    pub fn password(&self) -> &str {
        // See `ssid`: this is validated at both construction boundaries.
        str::from_utf8(&self.password[..usize::from(self.password_len)])
            .expect("validated Wi-Fi password invariant")
    }
}

impl fmt::Debug for WifiCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WifiCredentials")
            .field("ssid", &self.ssid())
            .field("password", &"<redacted>")
            .finish()
    }
}

/// A versioned network configuration stored by a platform adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkConfig {
    generation: u32,
    credentials: WifiCredentials,
}

impl NetworkConfig {
    /// Construct a network configuration at the supplied generation.
    ///
    /// # Panics
    ///
    /// Panics when `generation` is zero, which is reserved for an
    /// unprovisioned record.
    pub const fn new(generation: u32, credentials: WifiCredentials) -> Self {
        assert!(
            generation > 0,
            "network configuration generation must be nonzero"
        );
        Self {
            generation,
            credentials,
        }
    }

    /// Return the monotonically increasing configuration generation.
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// Return the Wi-Fi credentials.
    pub const fn credentials(&self) -> &WifiCredentials {
        &self.credentials
    }

    /// Encode the configuration into the stable version-one record format.
    pub fn encode(&self) -> [u8; CONFIG_RECORD_LEN] {
        let mut record = [0_u8; CONFIG_RECORD_LEN];
        record[..4].copy_from_slice(&CONFIG_MAGIC);
        record[4] = CONFIG_VERSION;
        record[5] = self.credentials.ssid_len;
        record[6] = self.credentials.password_len;
        record[7] = 0;
        record[8..12].copy_from_slice(&self.generation.to_le_bytes());
        record[HEADER_LEN..PASSWORD_OFFSET].copy_from_slice(&self.credentials.ssid);
        record[PASSWORD_OFFSET..CRC_OFFSET].copy_from_slice(&self.credentials.password);
        let crc = crc32(&record[..CRC_OFFSET]);
        record[CRC_OFFSET..].copy_from_slice(&crc.to_le_bytes());
        record
    }

    /// Decode and validate a stable version-one record.
    pub fn decode(record: &[u8]) -> Result<Self, ConfigDecodeError> {
        if record.len() != CONFIG_RECORD_LEN {
            return Err(ConfigDecodeError::InvalidLength(record.len()));
        }
        if record[..4] != CONFIG_MAGIC {
            return Err(ConfigDecodeError::InvalidMagic);
        }
        if record[4] != CONFIG_VERSION {
            return Err(ConfigDecodeError::UnsupportedVersion(record[4]));
        }
        if record[7] != 0 {
            return Err(ConfigDecodeError::UnsupportedFlags(record[7]));
        }

        let expected_crc = u32::from_le_bytes(record[CRC_OFFSET..].try_into().unwrap());
        let actual_crc = crc32(&record[..CRC_OFFSET]);
        if expected_crc != actual_crc {
            return Err(ConfigDecodeError::ChecksumMismatch);
        }

        let ssid_len = usize::from(record[5]);
        let password_len = usize::from(record[6]);
        if ssid_len == 0 {
            return Err(ConfigDecodeError::InvalidCredentials(
                CredentialsError::MissingSsid,
            ));
        }
        if ssid_len > WIFI_SSID_MAX_LEN {
            return Err(ConfigDecodeError::InvalidCredentials(
                CredentialsError::SsidTooLong,
            ));
        }
        if password_len > WIFI_PASSWORD_MAX_LEN {
            return Err(ConfigDecodeError::InvalidCredentials(
                CredentialsError::PasswordTooLong,
            ));
        }

        let ssid = str::from_utf8(&record[HEADER_LEN..HEADER_LEN + ssid_len])
            .map_err(|_| ConfigDecodeError::InvalidUtf8)?;
        let password = str::from_utf8(&record[PASSWORD_OFFSET..PASSWORD_OFFSET + password_len])
            .map_err(|_| ConfigDecodeError::InvalidUtf8)?;
        let generation = u32::from_le_bytes(record[8..12].try_into().unwrap());
        if generation == 0 {
            return Err(ConfigDecodeError::InvalidGeneration);
        }
        let credentials =
            WifiCredentials::new(ssid, password).map_err(ConfigDecodeError::InvalidCredentials)?;
        Ok(Self::new(generation, credentials))
    }
}

/// A reason a persistent network record was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigDecodeError {
    /// The record did not have the exact version-one length.
    InvalidLength(usize),
    /// The record did not start with the rlvgl network magic value.
    InvalidMagic,
    /// The record version is not supported by this firmware.
    UnsupportedVersion(u8),
    /// Reserved flags were nonzero.
    UnsupportedFlags(u8),
    /// The reserved unprovisioned generation zero was present.
    InvalidGeneration,
    /// The record checksum did not match its contents.
    ChecksumMismatch,
    /// An SSID or password field was not valid UTF-8.
    InvalidUtf8,
    /// A decoded credential violated its capacity or presence rule.
    InvalidCredentials(CredentialsError),
}

/// Persistent storage boundary for common network configuration.
pub trait NetworkConfigStore {
    /// Platform-specific storage error.
    type Error;

    /// Load a configuration, returning `None` when none has been provisioned.
    fn load(&mut self) -> Result<Option<NetworkConfig>, Self::Error>;

    /// Atomically store or replace a complete configuration.
    fn store(&mut self, config: &NetworkConfig) -> Result<(), Self::Error>;
}

/// How the active configuration was resolved at boot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigOrigin {
    /// An unchanged persistent configuration was loaded.
    Stored,
    /// No persistent configuration existed, so the supplied seed was stored.
    Seeded,
    /// The supplied seed differed from storage and replaced it.
    Updated,
}

/// An active configuration together with its boot-time origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNetworkConfig {
    /// Complete active configuration.
    pub config: NetworkConfig,
    /// Whether storage or a seed supplied the active value.
    pub origin: ConfigOrigin,
}

/// Load persistent configuration and optionally seed or update it.
///
/// Unchanged seeds do not write flash. A new or changed seed advances the
/// generation and is stored before being returned. Supplying no seed therefore
/// supports ordinary credential-free firmware rebuilds after one provisioning
/// pass.
pub fn load_or_seed<S>(
    store: &mut S,
    seed: Option<WifiCredentials>,
) -> Result<Option<ResolvedNetworkConfig>, S::Error>
where
    S: NetworkConfigStore,
{
    let stored = store.load()?;
    match (stored, seed) {
        (Some(config), None) => Ok(Some(ResolvedNetworkConfig {
            config,
            origin: ConfigOrigin::Stored,
        })),
        (Some(config), Some(seed)) if config.credentials() == &seed => {
            Ok(Some(ResolvedNetworkConfig {
                config,
                origin: ConfigOrigin::Stored,
            }))
        }
        (Some(config), Some(seed)) => {
            let generation = config.generation().wrapping_add(1).max(1);
            let config = NetworkConfig::new(generation, seed);
            store.store(&config)?;
            Ok(Some(ResolvedNetworkConfig {
                config,
                origin: ConfigOrigin::Updated,
            }))
        }
        (None, Some(seed)) => {
            let config = NetworkConfig::new(1, seed);
            store.store(&config)?;
            Ok(Some(ResolvedNetworkConfig {
                config,
                origin: ConfigOrigin::Seeded,
            }))
        }
        (None, None) => Ok(None),
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{format, vec::Vec};

    use super::*;

    #[derive(Default)]
    struct Store {
        config: Option<NetworkConfig>,
        writes: Vec<NetworkConfig>,
    }

    impl NetworkConfigStore for Store {
        type Error = ();

        fn load(&mut self) -> Result<Option<NetworkConfig>, Self::Error> {
            Ok(self.config.clone())
        }

        fn store(&mut self, config: &NetworkConfig) -> Result<(), Self::Error> {
            self.config = Some(config.clone());
            self.writes.push(config.clone());
            Ok(())
        }
    }

    #[test]
    fn record_round_trip_and_checksum_validation() {
        let config = NetworkConfig::new(7, WifiCredentials::new("bench", "secret").unwrap());
        let mut encoded = config.encode();
        assert_eq!(NetworkConfig::decode(&encoded), Ok(config));

        encoded[20] ^= 0x80;
        assert_eq!(
            NetworkConfig::decode(&encoded),
            Err(ConfigDecodeError::ChecksumMismatch)
        );
    }

    #[test]
    fn record_rejects_the_unprovisioned_generation() {
        let config = NetworkConfig::new(1, WifiCredentials::new("bench", "secret").unwrap());
        let mut encoded = config.encode();
        encoded[8..12].copy_from_slice(&0_u32.to_le_bytes());
        let checksum = crc32(&encoded[..CRC_OFFSET]);
        encoded[CRC_OFFSET..].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(
            NetworkConfig::decode(&encoded),
            Err(ConfigDecodeError::InvalidGeneration)
        );
    }

    #[test]
    fn debug_never_exposes_password() {
        let credentials = WifiCredentials::new("bench", "do-not-print-this").unwrap();
        let debug = format!("{credentials:?}");
        assert!(debug.contains("bench"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("do-not-print-this"));
    }

    #[test]
    fn load_or_seed_only_writes_new_or_changed_credentials() {
        let first = WifiCredentials::new("bench", "one").unwrap();
        let second = WifiCredentials::new("bench", "two").unwrap();
        let mut store = Store::default();

        let seeded = load_or_seed(&mut store, Some(first.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(seeded.origin, ConfigOrigin::Seeded);
        assert_eq!(seeded.config.generation(), 1);
        assert_eq!(store.writes.len(), 1);

        let stored = load_or_seed(&mut store, Some(first)).unwrap().unwrap();
        assert_eq!(stored.origin, ConfigOrigin::Stored);
        assert_eq!(store.writes.len(), 1);

        let updated = load_or_seed(&mut store, Some(second)).unwrap().unwrap();
        assert_eq!(updated.origin, ConfigOrigin::Updated);
        assert_eq!(updated.config.generation(), 2);
        assert_eq!(store.writes.len(), 2);

        let loaded = load_or_seed(&mut store, None).unwrap().unwrap();
        assert_eq!(loaded.origin, ConfigOrigin::Stored);
        assert_eq!(loaded.config.generation(), 2);
        assert_eq!(store.writes.len(), 2);
    }
}
