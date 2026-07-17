// SPDX-License-Identifier: MIT

use rlvgl_device_stts22h::{
    Address, Averaging, Config, Error, FreeRunRate, Stts22h, Threshold, ThresholdError,
};
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockError, MockI2c};

#[test]
fn constructors_are_side_effect_free_and_release_the_bus() {
    let driver = Stts22h::new(MockI2c::new(&[]));
    assert_eq!(driver.address(), Address::Vdd);
    driver.release().done().unwrap();

    let driver = Stts22h::with_address(MockI2c::new(&[]), Address::PullUp15K);
    assert_eq!(driver.address(), Address::PullUp15K);
    driver.release().done().unwrap();
}

#[test]
fn probe_checks_the_documented_identity_and_preserves_bus_errors() {
    let probe_ops = [
        ExpectedOperation::Write(&[0x01]),
        ExpectedOperation::Read(&[0xa0]),
    ];
    let expected = [ExpectedTransaction::success(0x38, &probe_ops)];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    assert_eq!(driver.probe(), Ok(()));
    driver.release().done().unwrap();

    let wrong_ops = [
        ExpectedOperation::Write(&[0x01]),
        ExpectedOperation::Read(&[0x00]),
    ];
    let wrong = [ExpectedTransaction::success(0x38, &wrong_ops)];
    let mut driver = Stts22h::new(MockI2c::new(&wrong));
    assert_eq!(
        driver.probe(),
        Err(Error::InvalidDevice {
            expected: 0xa0,
            actual: 0x00,
        })
    );
    driver.release().done().unwrap();

    let failed_ops = [
        ExpectedOperation::Write(&[0x01]),
        ExpectedOperation::Read(&[0x00]),
    ];
    let failed = [ExpectedTransaction::failure(0x38, &failed_ops)];
    let mut driver = Stts22h::new(MockI2c::new(&failed));
    assert_eq!(driver.probe(), Err(Error::Bus(MockError::Injected)));
    driver.release().done().unwrap();
}

#[test]
fn configuration_transitions_through_power_down_before_freerun() {
    let power_down = [ExpectedOperation::Write(&[0x04, 0x68])];
    let freerun = [ExpectedOperation::Write(&[0x04, 0x6c])];
    let expected = [
        ExpectedTransaction::success(0x38, &power_down),
        ExpectedTransaction::success(0x38, &freerun),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));

    driver
        .configure(Config::free_run(FreeRunRate::Hz100))
        .unwrap();
    assert!(driver.is_configured());
    driver.release().done().unwrap();
}

#[test]
fn one_shot_requires_one_shot_configuration() {
    let configure = [ExpectedOperation::Write(&[0x04, 0x48])];
    let trigger = [ExpectedOperation::Write(&[0x04, 0x49])];
    let expected = [
        ExpectedTransaction::success(0x38, &configure),
        ExpectedTransaction::success(0x38, &trigger),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    driver
        .configure(Config::one_shot(Averaging::Samples8))
        .unwrap();
    assert_eq!(driver.start_one_shot(), Ok(()));
    driver.release().done().unwrap();

    let power_down = [ExpectedOperation::Write(&[0x04, 0x48])];
    let freerun = [ExpectedOperation::Write(&[0x04, 0x4c])];
    let expected = [
        ExpectedTransaction::success(0x38, &power_down),
        ExpectedTransaction::success(0x38, &freerun),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    driver
        .configure(Config::free_run(FreeRunRate::Hz25))
        .unwrap();
    assert_eq!(driver.start_one_shot(), Err(Error::WrongMode));
    driver.release().done().unwrap();
}

#[test]
fn temperature_read_is_coherent_little_endian_signed_centi_celsius() {
    let configure = [ExpectedOperation::Write(&[0x04, 0x48])];
    let positive_read = [
        ExpectedOperation::Write(&[0x06]),
        ExpectedOperation::Read(&[0xe6, 0x09]),
    ];
    let negative_read = [
        ExpectedOperation::Write(&[0x06]),
        ExpectedOperation::Read(&[0xda, 0xfd]),
    ];
    let expected = [
        ExpectedTransaction::success(0x38, &configure),
        ExpectedTransaction::success(0x38, &positive_read),
        ExpectedTransaction::success(0x38, &negative_read),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    assert_eq!(driver.read_temperature(), Err(Error::NotConfigured));
    driver.configure(Config::default()).unwrap();

    assert_eq!(driver.read_temperature().unwrap().centi_celsius(), 2534);
    assert_eq!(driver.read_temperature().unwrap().centi_celsius(), -550);
    driver.release().done().unwrap();
}

#[test]
fn status_exposes_busy_and_both_read_to_clear_alert_flags() {
    let status_ops = [
        ExpectedOperation::Write(&[0x05]),
        ExpectedOperation::Read(&[0x07]),
    ];
    let expected = [ExpectedTransaction::success(0x38, &status_ops)];
    let mut driver = Stts22h::new(MockI2c::new(&expected));

    let status = driver.read_status().unwrap();
    assert!(status.busy());
    assert!(status.over_high_limit());
    assert!(status.under_low_limit());
    driver.release().done().unwrap();
}

#[test]
fn thresholds_are_exactly_quantized_and_written_to_their_registers() {
    assert_eq!(Threshold::disabled().register_value(), 0);
    assert_eq!(Threshold::disabled().centi_celsius(), None);

    let zero = Threshold::from_centi_celsius(0).unwrap();
    assert_eq!(zero.register_value(), 63);
    assert_eq!(zero.centi_celsius(), Some(0));
    assert_eq!(
        Threshold::from_centi_celsius(-3968)
            .unwrap()
            .register_value(),
        1
    );
    assert_eq!(
        Threshold::from_centi_celsius(12288)
            .unwrap()
            .register_value(),
        255
    );
    assert_eq!(
        Threshold::from_centi_celsius(1),
        Err(ThresholdError { centi_celsius: 1 })
    );
    assert_eq!(
        Threshold::from_centi_celsius(12352),
        Err(ThresholdError {
            centi_celsius: 12352,
        })
    );

    let high_write = [ExpectedOperation::Write(&[0x02, 0xff])];
    let low_write = [ExpectedOperation::Write(&[0x03, 0x01])];
    let expected = [
        ExpectedTransaction::success(0x38, &high_write),
        ExpectedTransaction::success(0x38, &low_write),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    driver
        .set_high_threshold(Threshold::from_centi_celsius(12288).unwrap())
        .unwrap();
    driver
        .set_low_threshold(Threshold::from_centi_celsius(-3968).unwrap())
        .unwrap();
    driver.release().done().unwrap();
}

#[test]
fn failed_configuration_invalidates_the_local_configuration_state() {
    let first_configure = [ExpectedOperation::Write(&[0x04, 0x48])];
    let failed_reconfigure = [ExpectedOperation::Write(&[0x04, 0x58])];
    let expected = [
        ExpectedTransaction::success(0x38, &first_configure),
        ExpectedTransaction::failure(0x38, &failed_reconfigure),
    ];
    let mut driver = Stts22h::new(MockI2c::new(&expected));
    driver.configure(Config::default()).unwrap();
    assert_eq!(
        driver.configure(Config::low_odr(Averaging::Samples4)),
        Err(Error::Bus(MockError::Injected))
    );
    assert!(!driver.is_configured());
    driver.release().done().unwrap();
}
