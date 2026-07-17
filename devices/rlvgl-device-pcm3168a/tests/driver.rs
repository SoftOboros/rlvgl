// SPDX-License-Identifier: MIT

use rlvgl_device_pcm3168a::{
    Address, AudioFormat, Error, HardwareReady, Pcm3168a, ReadinessError, Ready, SamplingMode,
};
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockError, MockI2c};

#[test]
fn construction_is_side_effect_free_and_all_addresses_are_seven_bit() {
    let driver = Pcm3168a::new(MockI2c::new(&[]));
    assert_eq!(driver.address(), Address::BothLow);
    assert_eq!(Address::BothLow as u8, 0x44);
    assert_eq!(Address::Adr0High as u8, 0x45);
    assert_eq!(Address::Adr1High as u8, 0x46);
    assert_eq!(Address::BothHigh as u8, 0x47);
    driver.release().done().unwrap();

    let driver = Pcm3168a::with_address(MockI2c::new(&[]), Address::BothHigh);
    assert_eq!(driver.address(), Address::BothHigh);
    driver.release().done().unwrap();
}

#[test]
fn readiness_requires_rails_reset_and_synchronous_clocks() {
    assert_eq!(
        HardwareReady::new(false, true, true),
        Err(ReadinessError::RailsUnstable)
    );
    assert_eq!(
        HardwareReady::new(true, false, true),
        Err(ReadinessError::ResetAsserted)
    );
    assert_eq!(
        HardwareReady::new(true, true, false),
        Err(ReadinessError::ClocksUnsynchronized)
    );

    let token = HardwareReady::new(true, true, true).unwrap();
    let driver: Pcm3168a<_, Ready> = Pcm3168a::new(MockI2c::new(&[])).into_ready(token);
    driver.release().done().unwrap();
}

#[test]
fn probe_reports_reset_state_without_claiming_device_identity() {
    let normal_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xc0]),
    ];
    let expected = [ExpectedTransaction::success(0x44, &normal_ops)];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&expected)).into_ready(token);
    let probe = driver.probe().unwrap();
    assert!(probe.mode_control_normal());
    assert!(probe.system_normal());
    assert_eq!(probe.sampling_mode(), SamplingMode::Auto);
    driver.release().done().unwrap();

    let invalid_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xc4]),
    ];
    let invalid = [ExpectedTransaction::success(0x44, &invalid_ops)];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&invalid)).into_ready(token);
    assert_eq!(
        driver.probe(),
        Err(Error::InvalidResetControl { value: 0xc4 })
    );
    driver.release().done().unwrap();

    let failed_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0]),
    ];
    let failed = [ExpectedTransaction::failure(0x44, &failed_ops)];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&failed)).into_ready(token);
    assert_eq!(driver.probe(), Err(Error::Bus(MockError::Injected)));
    driver.release().done().unwrap();
}

#[test]
fn common_slave_audio_formats_write_exact_dac_and_adc_registers() {
    let dac_i2s = [ExpectedOperation::Write(&[0x41, 0x00])];
    let adc_i2s = [ExpectedOperation::Write(&[0x51, 0x00])];
    let dac_tdm = [ExpectedOperation::Write(&[0x41, 0x07])];
    let adc_tdm = [ExpectedOperation::Write(&[0x51, 0x07])];
    let expected = [
        ExpectedTransaction::success(0x44, &dac_i2s),
        ExpectedTransaction::success(0x44, &adc_i2s),
        ExpectedTransaction::success(0x44, &dac_tdm),
        ExpectedTransaction::success(0x44, &adc_tdm),
    ];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&expected)).into_ready(token);

    driver.configure_dac_interface(AudioFormat::I2s24).unwrap();
    driver.configure_adc_interface(AudioFormat::I2s24).unwrap();
    driver
        .configure_dac_interface(AudioFormat::TdmLeftJustified24)
        .unwrap();
    driver
        .configure_adc_interface(AudioFormat::TdmLeftJustified24)
        .unwrap();
    driver.release().done().unwrap();
}

#[test]
fn every_common_audio_format_has_the_documented_code() {
    let cases = [
        (AudioFormat::I2s24, 0),
        (AudioFormat::LeftJustified24, 1),
        (AudioFormat::RightJustified24, 2),
        (AudioFormat::RightJustified16, 3),
        (AudioFormat::DspI2s24, 4),
        (AudioFormat::DspLeftJustified24, 5),
        (AudioFormat::TdmI2s24, 6),
        (AudioFormat::TdmLeftJustified24, 7),
    ];

    for (format, value) in cases {
        assert_eq!(format.register_value(), value);
    }
}

#[test]
fn resynchronize_preserves_sampling_mode_and_triggers_only_system_reset() {
    let read_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xc2]),
    ];
    let write_ops = [ExpectedOperation::Write(&[0x40, 0x82])];
    let expected = [
        ExpectedTransaction::success(0x44, &read_ops),
        ExpectedTransaction::success(0x44, &write_ops),
    ];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&expected)).into_ready(token);

    driver.resynchronize().unwrap();
    driver.release().done().unwrap();
}

#[test]
fn invalid_reset_state_prevents_resynchronization_write() {
    let read_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xc8]),
    ];
    let expected = [ExpectedTransaction::success(0x44, &read_ops)];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&expected)).into_ready(token);

    assert_eq!(
        driver.resynchronize(),
        Err(Error::InvalidResetControl { value: 0xc8 })
    );
    driver.release().done().unwrap();
}

#[test]
fn backend_failures_are_preserved_for_configuration_and_resynchronization() {
    let dac_ops = [ExpectedOperation::Write(&[0x41, 0x00])];
    let adc_ops = [ExpectedOperation::Write(&[0x51, 0x00])];
    let reset_read = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0]),
    ];
    let expected = [
        ExpectedTransaction::failure(0x44, &dac_ops),
        ExpectedTransaction::failure(0x44, &adc_ops),
        ExpectedTransaction::failure(0x44, &reset_read),
    ];
    let token = HardwareReady::new(true, true, true).unwrap();
    let mut driver = Pcm3168a::new(MockI2c::new(&expected)).into_ready(token);

    assert_eq!(
        driver.configure_dac_interface(AudioFormat::I2s24),
        Err(Error::Bus(MockError::Injected))
    );
    assert_eq!(
        driver.configure_adc_interface(AudioFormat::I2s24),
        Err(Error::Bus(MockError::Injected))
    );
    assert_eq!(driver.resynchronize(), Err(Error::Bus(MockError::Injected)));
    driver.release().done().unwrap();
}
