// SPDX-License-Identifier: MIT

use embedded_hal::i2c::I2c;
use rlvgl_i2c_test_support::{ExpectedOperation, ExpectedTransaction, MockError, MockI2c};

#[test]
fn records_exact_write_and_write_read_transactions() {
    let write_ops = [ExpectedOperation::Write(&[0x01, 0x80])];
    let read_ops = [
        ExpectedOperation::Write(&[0x06]),
        ExpectedOperation::Read(&[0x34, 0x12]),
    ];
    let expected = [
        ExpectedTransaction::success(0x38, &write_ops),
        ExpectedTransaction::success(0x38, &read_ops),
    ];
    let mut i2c = MockI2c::new(&expected);

    i2c.write(0x38, &[0x01, 0x80]).unwrap();
    let mut temperature = [0; 2];
    i2c.write_read(0x38, &[0x06], &mut temperature).unwrap();

    assert_eq!(temperature, [0x34, 0x12]);
    assert_eq!(i2c.pending(), 0);
    i2c.done().unwrap();
}

#[test]
fn rejects_an_unexpected_address_without_consuming_the_expectation() {
    let operations = [ExpectedOperation::Write(&[0x00])];
    let expected = [ExpectedTransaction::success(0x10, &operations)];
    let mut i2c = MockI2c::new(&expected);

    assert_eq!(
        i2c.write(0x11, &[0x00]),
        Err(MockError::Address {
            expected: 0x10,
            actual: 0x11,
        })
    );
    assert_eq!(i2c.pending(), 1);
}

#[test]
fn rejects_reordered_or_changed_operations() {
    let operations = [
        ExpectedOperation::Write(&[0x04]),
        ExpectedOperation::Read(&[0xaa]),
    ];
    let expected = [ExpectedTransaction::success(0x10, &operations)];
    let mut i2c = MockI2c::new(&expected);
    let mut result = [0; 1];

    assert_eq!(
        i2c.write_read(0x10, &[0x05], &mut result),
        Err(MockError::OperationMismatch { index: 0 })
    );
    assert_eq!(i2c.pending(), 1);
}

#[test]
fn injects_a_backend_failure_after_matching_the_transaction() {
    let operations = [ExpectedOperation::Read(&[0u8; 2])];
    let expected = [ExpectedTransaction::failure(0x40, &operations)];
    let mut i2c = MockI2c::new(&expected);
    let mut result = [0; 2];

    assert_eq!(i2c.read(0x40, &mut result), Err(MockError::Injected));
    i2c.done().unwrap();
}

#[test]
fn reports_missing_and_extra_transactions() {
    let operations = [ExpectedOperation::Write(&[0x00])];
    let expected = [ExpectedTransaction::success(0x10, &operations)];

    assert_eq!(
        MockI2c::new(&expected).done(),
        Err(MockError::PendingExpectations { remaining: 1 })
    );

    let mut empty = MockI2c::new(&[]);
    assert_eq!(
        empty.write(0x10, &[0x00]),
        Err(MockError::UnexpectedTransaction {
            actual_address: 0x10,
        })
    );
}
