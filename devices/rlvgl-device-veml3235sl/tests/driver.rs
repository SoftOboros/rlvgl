// SPDX-License-Identifier: MIT

use rlvgl_device_veml3235sl::{
    ADDRESS, Config, DigitalGain, Error, Gain, IntegrationTime, Veml3235sl,
};
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockError, MockI2c};

#[test]
fn construction_is_side_effect_free_and_uses_the_fixed_address() {
    let driver = Veml3235sl::new(MockI2c::new(&[]));
    assert_eq!(driver.address(), ADDRESS);
    assert_eq!(ADDRESS, 0x10);
    assert!(!driver.is_configured());
    driver.release().done().unwrap();
}

#[test]
fn probe_checks_the_part_number_and_preserves_bus_errors() {
    let probe_ops = [
        ExpectedOperation::Write(&[0x09]),
        ExpectedOperation::Read(&[0x35, 0xab]),
    ];
    let expected = [ExpectedTransaction::success(ADDRESS, &probe_ops)];
    let mut driver = Veml3235sl::new(MockI2c::new(&expected));
    assert_eq!(driver.probe(), Ok(()));
    driver.release().done().unwrap();

    let wrong_ops = [
        ExpectedOperation::Write(&[0x09]),
        ExpectedOperation::Read(&[0x34, 0x00]),
    ];
    let wrong = [ExpectedTransaction::success(ADDRESS, &wrong_ops)];
    let mut driver = Veml3235sl::new(MockI2c::new(&wrong));
    assert_eq!(
        driver.probe(),
        Err(Error::InvalidDevice {
            expected: 0x35,
            actual: 0x34,
        })
    );
    driver.release().done().unwrap();

    let failed_ops = [
        ExpectedOperation::Write(&[0x09]),
        ExpectedOperation::Read(&[0x00, 0x00]),
    ];
    let failed = [ExpectedTransaction::failure(ADDRESS, &failed_ops)];
    let mut driver = Veml3235sl::new(MockI2c::new(&failed));
    assert_eq!(driver.probe(), Err(Error::Bus(MockError::Injected)));
    driver.release().done().unwrap();
}

#[test]
fn configuration_encodes_only_current_rev_1_4_fields() {
    let ordinary = [ExpectedOperation::Write(&[0x00, 0x10, 0x01])];
    let sensitive_shutdown = [ExpectedOperation::Write(&[0x00, 0x41, 0xb9])];
    let expected = [
        ExpectedTransaction::success(ADDRESS, &ordinary),
        ExpectedTransaction::success(ADDRESS, &sensitive_shutdown),
    ];
    let mut driver = Veml3235sl::new(MockI2c::new(&expected));

    let ordinary_config = Config::new(IntegrationTime::Ms100, Gain::X1, DigitalGain::X1);
    driver.configure(ordinary_config).unwrap();
    assert_eq!(driver.config(), Some(ordinary_config));

    let shutdown =
        Config::new(IntegrationTime::Ms800, Gain::X4, DigitalGain::X2).with_enabled(false);
    driver.configure(shutdown).unwrap();
    assert_eq!(driver.config(), Some(shutdown));
    assert!(!shutdown.enabled());
    driver.release().done().unwrap();
}

#[test]
fn failed_configuration_invalidates_local_state() {
    let first = [ExpectedOperation::Write(&[0x00, 0x10, 0x01])];
    let failed = [ExpectedOperation::Write(&[0x00, 0x20, 0x29])];
    let expected = [
        ExpectedTransaction::success(ADDRESS, &first),
        ExpectedTransaction::failure(ADDRESS, &failed),
    ];
    let mut driver = Veml3235sl::new(MockI2c::new(&expected));
    driver.configure(Config::default()).unwrap();
    assert_eq!(
        driver.configure(Config::new(
            IntegrationTime::Ms200,
            Gain::X2,
            DigitalGain::X2,
        )),
        Err(Error::Bus(MockError::Injected))
    );
    assert!(!driver.is_configured());
    driver.release().done().unwrap();
}

#[test]
fn raw_channels_use_the_documented_registers_and_little_endian_words() {
    let white_ops = [
        ExpectedOperation::Write(&[0x04]),
        ExpectedOperation::Read(&[0x34, 0x12]),
    ];
    let als_ops = [
        ExpectedOperation::Write(&[0x05]),
        ExpectedOperation::Read(&[0xcd, 0xab]),
    ];
    let expected = [
        ExpectedTransaction::success(ADDRESS, &white_ops),
        ExpectedTransaction::success(ADDRESS, &als_ops),
    ];
    let mut driver = Veml3235sl::new(MockI2c::new(&expected));
    assert_eq!(driver.read_white_raw(), Ok(0x1234));
    assert_eq!(driver.read_als_raw(), Ok(0xabcd));
    driver.release().done().unwrap();
}

#[test]
fn illuminance_requires_a_successful_active_configuration() {
    let configure = [ExpectedOperation::Write(&[0x00, 0x10, 0x01])];
    let als_ops = [
        ExpectedOperation::Write(&[0x05]),
        ExpectedOperation::Read(&[0xc8, 0x05]),
    ];
    let shutdown = [ExpectedOperation::Write(&[0x00, 0x11, 0x81])];
    let expected = [
        ExpectedTransaction::success(ADDRESS, &configure),
        ExpectedTransaction::success(ADDRESS, &als_ops),
        ExpectedTransaction::success(ADDRESS, &shutdown),
    ];
    let mut driver = Veml3235sl::new(MockI2c::new(&expected));

    assert_eq!(driver.read_illuminance(), Err(Error::NotConfigured));
    driver.configure(Config::default()).unwrap();
    let illuminance = driver.read_illuminance().unwrap();
    assert_eq!(illuminance.raw_count(), 1480);
    assert_eq!(illuminance.micro_lux(), 201_753_600);

    driver
        .configure(Config::default().with_enabled(false))
        .unwrap();
    assert_eq!(driver.read_illuminance(), Err(Error::Shutdown));
    driver.release().done().unwrap();
}

#[test]
fn complete_resolution_matrix_matches_the_datasheet_table() {
    let cases = [
        (IntegrationTime::Ms800, Gain::X4, DigitalGain::X2, 2_130),
        (IntegrationTime::Ms800, Gain::X2, DigitalGain::X2, 4_260),
        (IntegrationTime::Ms800, Gain::X1, DigitalGain::X2, 8_520),
        (IntegrationTime::Ms400, Gain::X4, DigitalGain::X2, 4_260),
        (IntegrationTime::Ms400, Gain::X2, DigitalGain::X2, 8_520),
        (IntegrationTime::Ms400, Gain::X1, DigitalGain::X2, 17_040),
        (IntegrationTime::Ms200, Gain::X4, DigitalGain::X2, 8_520),
        (IntegrationTime::Ms200, Gain::X2, DigitalGain::X2, 17_040),
        (IntegrationTime::Ms200, Gain::X1, DigitalGain::X2, 34_080),
        (IntegrationTime::Ms100, Gain::X4, DigitalGain::X2, 17_040),
        (IntegrationTime::Ms100, Gain::X2, DigitalGain::X2, 34_080),
        (IntegrationTime::Ms100, Gain::X1, DigitalGain::X2, 68_160),
        (IntegrationTime::Ms50, Gain::X4, DigitalGain::X2, 34_080),
        (IntegrationTime::Ms50, Gain::X2, DigitalGain::X2, 68_160),
        (IntegrationTime::Ms50, Gain::X1, DigitalGain::X2, 136_320),
        (IntegrationTime::Ms800, Gain::X4, DigitalGain::X1, 4_260),
        (IntegrationTime::Ms800, Gain::X2, DigitalGain::X1, 8_520),
        (IntegrationTime::Ms800, Gain::X1, DigitalGain::X1, 17_040),
        (IntegrationTime::Ms400, Gain::X4, DigitalGain::X1, 8_520),
        (IntegrationTime::Ms400, Gain::X2, DigitalGain::X1, 17_040),
        (IntegrationTime::Ms400, Gain::X1, DigitalGain::X1, 34_080),
        (IntegrationTime::Ms200, Gain::X4, DigitalGain::X1, 17_040),
        (IntegrationTime::Ms200, Gain::X2, DigitalGain::X1, 34_080),
        (IntegrationTime::Ms200, Gain::X1, DigitalGain::X1, 68_160),
        (IntegrationTime::Ms100, Gain::X4, DigitalGain::X1, 34_080),
        (IntegrationTime::Ms100, Gain::X2, DigitalGain::X1, 68_160),
        (IntegrationTime::Ms100, Gain::X1, DigitalGain::X1, 136_320),
        (IntegrationTime::Ms50, Gain::X4, DigitalGain::X1, 68_160),
        (IntegrationTime::Ms50, Gain::X2, DigitalGain::X1, 136_320),
        (IntegrationTime::Ms50, Gain::X1, DigitalGain::X1, 272_640),
    ];

    for (integration_time, gain, digital_gain, expected) in cases {
        let config = Config::new(integration_time, gain, digital_gain);
        assert_eq!(config.micro_lux_per_count(), expected);
    }
}

#[test]
fn exact_integer_conversion_handles_the_full_scale_without_overflow() {
    let config = Config::new(IntegrationTime::Ms50, Gain::X1, DigitalGain::X1);
    let illuminance = config.illuminance_from_raw(u16::MAX);
    assert_eq!(illuminance.raw_count(), u16::MAX);
    assert_eq!(illuminance.micro_lux(), 17_867_462_400);
}
