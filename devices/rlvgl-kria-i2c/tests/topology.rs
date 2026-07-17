// SPDX-License-Identifier: MIT

use rlvgl_kria_i2c::{EEPROM_U6, LogicalBus, PCM3168A, PTN3460, STTS22H, VEML3235SL};

#[test]
fn corrected_ps_bus_split_is_stable() {
    assert_eq!(STTS22H.bus(), LogicalBus::PsI2c1);
    assert_eq!(STTS22H.address(), 0x38);
    assert_eq!(EEPROM_U6.bus(), LogicalBus::PsI2c0);
    assert_eq!(EEPROM_U6.address(), 0x50);
}

#[test]
fn front_panel_devices_share_one_collision_free_bus() {
    let endpoints = [VEML3235SL, PTN3460, PCM3168A];
    for endpoint in endpoints {
        assert_eq!(endpoint.bus(), LogicalBus::PlFrontPanelI2c);
    }

    let addresses = endpoints.map(|endpoint| endpoint.address());
    assert_eq!(addresses, [0x10, 0x20, 0x44]);
    assert_ne!(addresses[0], addresses[1]);
    assert_ne!(addresses[0], addresses[2]);
    assert_ne!(addresses[1], addresses[2]);
}
