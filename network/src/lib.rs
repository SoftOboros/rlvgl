//! Portable network configuration, lifecycle policy, SNTP, and clock holdover.
//!
//! This crate deliberately stops below radio drivers, sockets, flash drivers,
//! and provisioning transports. Those target-specific adapters can share the
//! same bounded credential record and connection behavior without importing an
//! operating system or a particular ESP framework.

#![no_std]
#![deny(missing_docs)]

mod config;
mod sntp;
mod state;
mod time;

pub use config::{
    CONFIG_RECORD_LEN, ConfigDecodeError, ConfigOrigin, CredentialsError, NetworkConfig,
    NetworkConfigStore, ResolvedNetworkConfig, WIFI_PASSWORD_MAX_LEN, WIFI_SSID_MAX_LEN,
    WifiCredentials, load_or_seed,
};
pub use sntp::{NTP_PACKET_LEN, NetworkTime, NtpError, ntp_request, parse_ntp_response};
pub use state::{ConnectionState, RetryPolicy};
pub use time::{HoldoverClock, UtcDateTime, unix_seconds_to_utc};
