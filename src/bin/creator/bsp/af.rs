//! Alternate-function lookup trait used by the BSP tests.
//!
//! Implementations provide a mapping from (MCU, pin, signal) to
//! an alternate function number. The tests supply a stub provider
//! with hard-coded values.

use std::collections::HashMap;

/// Trait returning an alternate function number for a pin/signal pair.
pub trait AfProvider {
    /// Lookup the alternate function number for `pin` serving `func` on `mcu`.
    /// Returns `None` if the function is not available on that pin.
    fn lookup_af(&self, mcu: &str, pin: &str, func: &str) -> Option<u8>;
}

/// JSON-backed alternate-function database used by integration tests.
///
/// The JSON structure matches the output of
/// `tools/afdb/st_extract_af.py`:
/// `{ "MCU": { "PIN": { "SIGNAL": AF }}}`.
#[allow(dead_code)]
pub struct JsonAfDb {
    map: HashMap<String, HashMap<String, HashMap<String, u8>>>,
}

impl JsonAfDb {
    /// Load the database from `path`.
    #[allow(dead_code)]
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let map = serde_json::from_str(&data)?;
        Ok(Self { map })
    }
}

impl AfProvider for JsonAfDb {
    fn lookup_af(&self, mcu: &str, pin: &str, func: &str) -> Option<u8> {
        self.map
            .get(mcu)
            .and_then(|p| p.get(pin))
            .and_then(|m| m.get(func))
            .copied()
    }
}
