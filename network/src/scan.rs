//! Radio-neutral, bounded scan results and open-network selection.

use crate::{CredentialsError, WIFI_SSID_MAX_LEN};

/// A copyable UTF-8 SSID, including the empty name of a hidden access point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WifiSsid {
    bytes: [u8; WIFI_SSID_MAX_LEN],
    len: u8,
}

impl WifiSsid {
    /// Copy an SSID without allocating or truncating its UTF-8 encoding.
    pub fn new(ssid: &str) -> Result<Self, CredentialsError> {
        if ssid.len() > WIFI_SSID_MAX_LEN {
            return Err(CredentialsError::SsidTooLong);
        }
        let mut name = Self {
            bytes: [0; WIFI_SSID_MAX_LEN],
            len: ssid.len() as u8,
        };
        name.bytes[..ssid.len()].copy_from_slice(ssid.as_bytes());
        Ok(name)
    }

    /// Return the complete, untruncated network name.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("validated Wi-Fi SSID invariant")
    }
}

/// Authentication advertised by a scanned access point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiSecurity {
    /// The radio explicitly reported no authentication.
    Open,
    /// Authentication is required; automatic passwordless attempts are forbidden.
    Protected,
    /// The radio could not classify authentication; do not assume it is open.
    Unknown,
}

/// One observed access point, independent of any radio-driver type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WifiAccessPoint {
    /// Complete SSID; an empty name denotes a hidden access point.
    pub ssid: WifiSsid,
    /// Access-point MAC address, used to distinguish same-name radios.
    pub bssid: [u8; 6],
    /// Primary radio channel.
    pub channel: u8,
    /// Received signal strength in dBm.
    pub rssi: i8,
    /// Advertised authentication classification.
    pub security: WifiSecurity,
}

impl WifiAccessPoint {
    /// Whether this named access point may be tried without credentials.
    pub fn is_open_candidate(&self) -> bool {
        self.security == WifiSecurity::Open && !self.ssid.as_str().is_empty()
    }
}

/// Fixed-capacity, strongest-first scan results, deduplicated by BSSID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiScan<const N: usize> {
    entries: [Option<WifiAccessPoint>; N],
    len: usize,
}

impl<const N: usize> WifiScan<N> {
    /// Construct an empty scan list.
    pub const fn new() -> Self {
        Self {
            entries: [None; N],
            len: 0,
        }
    }

    /// Retain the strongest `N` access points, replacing duplicate observations.
    pub fn insert(&mut self, access_point: WifiAccessPoint) {
        let duplicate = self.iter().position(|ap| ap.bssid == access_point.bssid);
        if let Some(index) = duplicate {
            self.entries.copy_within(index + 1..self.len, index);
            self.len -= 1;
            self.entries[self.len] = None;
        }
        let index = self
            .iter()
            .position(|ap| ap.rssi < access_point.rssi)
            .unwrap_or(self.len);
        if index >= N {
            return;
        }
        let end = self.len.min(N - 1);
        self.entries.copy_within(index..end, index + 1);
        self.entries[index] = Some(access_point);
        self.len = (self.len + 1).min(N);
    }

    /// Return the number of retained access points.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no access points were retained.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterate over all retained access points in descending RSSI order.
    pub fn iter(&self) -> impl Iterator<Item = &WifiAccessPoint> {
        self.entries[..self.len].iter().flatten()
    }

    /// Iterate only explicitly open, named access points, strongest first.
    ///
    /// This never produces persistent credentials: discovery is ephemeral and
    /// must not overwrite a user's provisioned network configuration.
    pub fn open_candidates(&self) -> impl Iterator<Item = &WifiAccessPoint> {
        self.iter().filter(|ap| ap.is_open_candidate())
    }
}

impl<const N: usize> Default for WifiScan<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn ap(id: u8, ssid: &str, rssi: i8, security: WifiSecurity) -> WifiAccessPoint {
        WifiAccessPoint {
            ssid: WifiSsid::new(ssid).unwrap(),
            bssid: [id; 6],
            channel: 1,
            rssi,
            security,
        }
    }

    #[test]
    fn scan_retains_strongest_and_deduplicates_radios_not_names() {
        let mut scan = WifiScan::<3>::new();
        for (id, rssi) in [(1, -80), (2, -30), (3, -60), (4, -40), (5, -90)] {
            scan.insert(ap(id, "same name", rssi, WifiSecurity::Open));
        }
        assert_eq!(
            scan.iter()
                .map(|ap| ap.bssid[0])
                .collect::<std::vec::Vec<_>>(),
            [2, 4, 3]
        );
        scan.insert(ap(3, "renamed", -20, WifiSecurity::Protected));
        assert_eq!(scan.len(), 3);
        assert_eq!(scan.iter().next().unwrap().ssid.as_str(), "renamed");
        assert_eq!(scan.open_candidates().count(), 2);
    }

    #[test]
    fn only_explicitly_open_named_networks_are_candidates() {
        let mut scan = WifiScan::<4>::new();
        scan.insert(ap(1, "locked", -20, WifiSecurity::Protected));
        scan.insert(ap(2, "unknown", -30, WifiSecurity::Unknown));
        scan.insert(ap(3, "", -40, WifiSecurity::Open));
        scan.insert(ap(4, "guest", -50, WifiSecurity::Open));
        assert_eq!(
            scan.open_candidates()
                .map(|ap| ap.ssid.as_str())
                .collect::<std::vec::Vec<_>>(),
            ["guest"]
        );
    }

    #[test]
    fn ssids_preserve_utf8_and_zero_capacity_is_safe() {
        let name = WifiSsid::new("café").unwrap();
        assert_eq!(name.as_str(), "café");
        assert_eq!(
            WifiSsid::new(&"x".repeat(33)),
            Err(CredentialsError::SsidTooLong)
        );
        let mut scan = WifiScan::<0>::new();
        scan.insert(ap(1, "guest", -20, WifiSecurity::Open));
        assert!(scan.is_empty());
    }
}
