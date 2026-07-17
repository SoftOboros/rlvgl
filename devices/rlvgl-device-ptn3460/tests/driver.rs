// SPDX-License-Identifier: MIT

use rlvgl_device_ptn3460::{
    Address, ClockSpread, Error, LvdsElectricalConfig, OutputSwing, Ptn3460,
};
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockError, MockI2c};

#[test]
fn construction_is_side_effect_free_and_addresses_are_seven_bit() {
    let driver = Ptn3460::new(MockI2c::new(&[]));
    assert_eq!(driver.address(), Address::DevCfgLow);
    assert_eq!(driver.address() as u8, 0x20);
    driver.release().done().unwrap();

    let driver = Ptn3460::with_address(MockI2c::new(&[]), Address::DevCfgOpen);
    assert_eq!(driver.address() as u8, 0x60);
    driver.release().done().unwrap();
}

#[test]
fn probe_reports_configuration_magic_without_claiming_device_identity() {
    let valid_ops = [
        ExpectedOperation::Write(&[0xec]),
        ExpectedOperation::Read(&[0x12, 0x34, 0x56, 0x78]),
    ];
    let valid = [ExpectedTransaction::success(0x20, &valid_ops)];
    let mut driver = Ptn3460::new(MockI2c::new(&valid));
    let probe = driver.probe().unwrap();
    assert_eq!(probe.configuration_magic(), 0x1234_5678);
    assert!(probe.configuration_valid());
    driver.release().done().unwrap();

    let invalid_ops = [
        ExpectedOperation::Write(&[0xec]),
        ExpectedOperation::Read(&[0, 0, 0, 0]),
    ];
    let invalid = [ExpectedTransaction::success(0x20, &invalid_ops)];
    let mut driver = Ptn3460::new(MockI2c::new(&invalid));
    let probe = driver.probe().unwrap();
    assert_eq!(probe.configuration_magic(), 0);
    assert!(!probe.configuration_valid());
    driver.release().done().unwrap();

    let failed_ops = [
        ExpectedOperation::Write(&[0xec]),
        ExpectedOperation::Read(&[0, 0, 0, 0]),
    ];
    let failed = [ExpectedTransaction::failure(0x20, &failed_ops)];
    let mut driver = Ptn3460::new(MockI2c::new(&failed));
    assert_eq!(driver.probe(), Err(Error::Bus(MockError::Injected)));
    driver.release().done().unwrap();
}

#[test]
fn electrical_configuration_writes_only_register_0x82() {
    let default_ops = [ExpectedOperation::Write(&[0x82, 0x03])];
    let maximum_ops = [ExpectedOperation::Write(&[0x82, 0x2e])];
    let expected = [
        ExpectedTransaction::success(0x20, &default_ops),
        ExpectedTransaction::success(0x20, &maximum_ops),
    ];
    let mut driver = Ptn3460::new(MockI2c::new(&expected));

    driver
        .configure_lvds_electrical(LvdsElectricalConfig::default())
        .unwrap();
    driver
        .configure_lvds_electrical(LvdsElectricalConfig::new(
            ClockSpread::TwoAndHalfPercent,
            OutputSwing::Millivolts450,
        ))
        .unwrap();
    driver.release().done().unwrap();
}

#[test]
fn electrical_configuration_read_decodes_a_valid_register() {
    let operations = [
        ExpectedOperation::Write(&[0x82]),
        ExpectedOperation::Read(&[0x2e]),
    ];
    let expected = [ExpectedTransaction::success(0x20, &operations)];
    let mut driver = Ptn3460::new(MockI2c::new(&expected));

    let config = driver.read_lvds_electrical().unwrap();
    assert_eq!(config.clock_spread(), ClockSpread::TwoAndHalfPercent);
    assert_eq!(config.output_swing(), OutputSwing::Millivolts450);
    assert_eq!(config.register_value(), 0x2e);
    driver.release().done().unwrap();
}

#[test]
fn invalid_or_reserved_register_encodings_are_rejected() {
    for value in [0x40, 0x30, 0x07] {
        assert_eq!(
            LvdsElectricalConfig::from_register_value(value),
            Err(Error::<MockError>::InvalidElectricalConfig { value })
        );
    }

    let operations = [
        ExpectedOperation::Write(&[0x82]),
        ExpectedOperation::Read(&[0x07]),
    ];
    let expected = [ExpectedTransaction::success(0x20, &operations)];
    let mut driver = Ptn3460::new(MockI2c::new(&expected));
    assert_eq!(
        driver.read_lvds_electrical(),
        Err(Error::InvalidElectricalConfig { value: 0x07 })
    );
    driver.release().done().unwrap();
}

#[test]
fn every_admitted_electrical_encoding_round_trips() {
    let spreads = [
        ClockSpread::Disabled,
        ClockSpread::HalfPercent,
        ClockSpread::OnePercent,
        ClockSpread::OneAndHalfPercent,
        ClockSpread::TwoPercent,
        ClockSpread::TwoAndHalfPercent,
    ];
    let swings = [
        OutputSwing::Millivolts150,
        OutputSwing::Millivolts200,
        OutputSwing::Millivolts250,
        OutputSwing::Millivolts300,
        OutputSwing::Millivolts350,
        OutputSwing::Millivolts400,
        OutputSwing::Millivolts450,
    ];

    for spread in spreads {
        for swing in swings {
            let expected = LvdsElectricalConfig::new(spread, swing);
            assert_eq!(
                LvdsElectricalConfig::from_register_value::<MockError>(expected.register_value(),),
                Ok(expected)
            );
        }
    }
}

#[test]
fn backend_failures_are_preserved_for_read_and_write() {
    let read_ops = [
        ExpectedOperation::Write(&[0x82]),
        ExpectedOperation::Read(&[0]),
    ];
    let write_ops = [ExpectedOperation::Write(&[0x82, 0x03])];
    let expected = [
        ExpectedTransaction::failure(0x20, &read_ops),
        ExpectedTransaction::failure(0x20, &write_ops),
    ];
    let mut driver = Ptn3460::new(MockI2c::new(&expected));

    assert_eq!(
        driver.read_lvds_electrical(),
        Err(Error::Bus(MockError::Injected))
    );
    assert_eq!(
        driver.configure_lvds_electrical(LvdsElectricalConfig::default()),
        Err(Error::Bus(MockError::Injected))
    );
    driver.release().done().unwrap();
}
