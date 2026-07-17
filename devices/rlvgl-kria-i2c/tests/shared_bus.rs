// SPDX-License-Identifier: MIT

use core::cell::RefCell;

use embedded_hal::i2c::I2c;
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockI2c};
use rlvgl_kria_i2c::share_i2c;

#[test]
fn independent_handles_serialize_complete_transactions() {
    let light_ops = [
        ExpectedOperation::Write(&[0x04]),
        ExpectedOperation::Read(&[0x34, 0x12]),
    ];
    let bridge_ops = [ExpectedOperation::Write(&[0x10, 0x81])];
    let codec_ops = [
        ExpectedOperation::Write(&[0x40]),
        ExpectedOperation::Read(&[0xa5]),
    ];
    let expected = [
        ExpectedTransaction::success(0x10, &light_ops),
        ExpectedTransaction::success(0x20, &bridge_ops),
        ExpectedTransaction::success(0x44, &codec_ops),
    ];
    let bus = RefCell::new(MockI2c::new(&expected));
    {
        let mut light = share_i2c(&bus);
        let mut bridge = share_i2c(&bus);
        let mut codec = share_i2c(&bus);

        let mut light_data = [0; 2];
        light.write_read(0x10, &[0x04], &mut light_data).unwrap();
        bridge.write(0x20, &[0x10, 0x81]).unwrap();
        let mut codec_status = [0; 1];
        codec.write_read(0x44, &[0x40], &mut codec_status).unwrap();

        assert_eq!(light_data, [0x34, 0x12]);
        assert_eq!(codec_status, [0xa5]);
    }
    bus.into_inner().done().unwrap();
}
