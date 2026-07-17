// SPDX-License-Identifier: MIT

use rlvgl_device_pcm3168a::HardwareReady;
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockI2c};
use rlvgl_kria_i2c::{
    KriaI2cBuses, LogicalBus, PCM3168A, PTN3460, PhysicalBusMap, STTS22H, VEML3235SL,
    probe_pcm3168a, probe_ptn3460, probe_stts22h, probe_veml3235sl,
};

#[test]
fn physical_bus_mapping_is_explicit_and_caller_owned() {
    let map = PhysicalBusMap::new("/dev/ps0", "/dev/ps1", "/dev/pl-front");
    assert_eq!(map.get(LogicalBus::PsI2c0), &"/dev/ps0");
    assert_eq!(map.get(LogicalBus::PsI2c1), &"/dev/ps1");
    assert_eq!(map.get(LogicalBus::PlFrontPanelI2c), &"/dev/pl-front");
}

#[test]
fn typed_smoke_probes_use_separate_ps_buses_and_one_shared_pl_bus() {
    let stts_ops = [
        ExpectedOperation::Write(&[0x01]),
        ExpectedOperation::Read(&[0xa0]),
    ];
    let ps1_expected = [ExpectedTransaction::success(0x38, &stts_ops)];

    let light_ops = [
        ExpectedOperation::Write(&[0x09]),
        ExpectedOperation::Read(&[0x35, 0x00]),
    ];
    let bridge_ops = [
        ExpectedOperation::Write(&[0xec]),
        ExpectedOperation::Read(&[0x12, 0x34, 0x56, 0x78]),
    ];
    let codec_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xc0]),
    ];
    let pl_expected = [
        ExpectedTransaction::success(0x10, &light_ops),
        ExpectedTransaction::success(0x20, &bridge_ops),
        ExpectedTransaction::success(0x44, &codec_ops),
    ];

    let buses = KriaI2cBuses::new(
        MockI2c::new(&[]),
        MockI2c::new(&ps1_expected),
        MockI2c::new(&pl_expected),
    );

    let stts_diagnostic = {
        let mut driver = buses.stts22h();
        probe_stts22h(&mut driver)
    };
    assert_eq!(stts_diagnostic.endpoint(), STTS22H);
    assert!(stts_diagnostic.is_ok());

    let light_diagnostic = {
        let mut driver = buses.veml3235sl();
        probe_veml3235sl(&mut driver)
    };
    assert_eq!(light_diagnostic.endpoint(), VEML3235SL);
    assert!(light_diagnostic.is_ok());

    let bridge_diagnostic = {
        let mut driver = buses.ptn3460();
        probe_ptn3460(&mut driver)
    };
    assert_eq!(bridge_diagnostic.endpoint(), PTN3460);
    assert!(bridge_diagnostic.result().unwrap().configuration_valid());

    let codec_diagnostic = {
        let token = HardwareReady::new(true, true, true).unwrap();
        let mut driver = buses.pcm3168a().into_ready(token);
        probe_pcm3168a(&mut driver)
    };
    assert_eq!(codec_diagnostic.endpoint(), PCM3168A);
    assert!(codec_diagnostic.result().unwrap().system_normal());

    let (ps0, ps1, pl) = buses.release();
    ps0.done().unwrap();
    ps1.done().unwrap();
    pl.done().unwrap();
}

#[test]
fn structured_diagnostic_preserves_leaf_failure() {
    let wrong_id_ops = [
        ExpectedOperation::Write(&[0x09]),
        ExpectedOperation::Read(&[0x00, 0x00]),
    ];
    let pl_expected = [ExpectedTransaction::success(0x10, &wrong_id_ops)];
    let buses = KriaI2cBuses::new(
        MockI2c::new(&[]),
        MockI2c::new(&[]),
        MockI2c::new(&pl_expected),
    );

    let diagnostic = {
        let mut driver = buses.veml3235sl();
        probe_veml3235sl(&mut driver)
    };
    assert_eq!(diagnostic.endpoint(), VEML3235SL);
    assert!(!diagnostic.is_ok());
    assert!(diagnostic.result().is_err());

    let (ps0, ps1, pl) = buses.release();
    ps0.done().unwrap();
    ps1.done().unwrap();
    pl.done().unwrap();
}
