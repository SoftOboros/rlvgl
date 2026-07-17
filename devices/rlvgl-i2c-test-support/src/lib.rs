// SPDX-License-Identifier: MIT
//! Strict, allocation-free I2C transaction expectations for driver tests.

#![no_std]
#![deny(missing_docs)]

use embedded_hal::i2c::{Error as I2cError, ErrorKind, ErrorType, I2c, Operation, SevenBitAddress};

/// One expected operation within an I2C transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedOperation<'a> {
    /// Requires an exact write of the borrowed bytes.
    Write(&'a [u8]),
    /// Requires a read of this length and supplies the borrowed response.
    Read(&'a [u8]),
}

/// One ordered I2C transaction expectation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedTransaction<'a> {
    address: SevenBitAddress,
    operations: &'a [ExpectedOperation<'a>],
    fail: bool,
}

impl<'a> ExpectedTransaction<'a> {
    /// Creates an expectation that succeeds after an exact match.
    pub const fn success(
        address: SevenBitAddress,
        operations: &'a [ExpectedOperation<'a>],
    ) -> Self {
        Self {
            address,
            operations,
            fail: false,
        }
    }

    /// Creates an expectation that returns [`MockError::Injected`] after an
    /// exact match.
    pub const fn failure(
        address: SevenBitAddress,
        operations: &'a [ExpectedOperation<'a>],
    ) -> Self {
        Self {
            address,
            operations,
            fail: true,
        }
    }
}

/// A strict transaction-recorder failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockError {
    /// A transaction targeted an address other than the next expectation.
    Address {
        /// Expected seven-bit address.
        expected: SevenBitAddress,
        /// Observed seven-bit address.
        actual: SevenBitAddress,
    },
    /// A transaction contained the wrong number of operations.
    OperationCount {
        /// Expected operation count.
        expected: usize,
        /// Observed operation count.
        actual: usize,
    },
    /// An operation kind or write payload differed from its expectation.
    OperationMismatch {
        /// Zero-based operation index within the transaction.
        index: usize,
    },
    /// A read buffer had a different length than its admitted response.
    ReadLength {
        /// Zero-based operation index within the transaction.
        index: usize,
        /// Expected read length.
        expected: usize,
        /// Observed read length.
        actual: usize,
    },
    /// A matched failure expectation injected a backend error.
    Injected,
    /// [`MockI2c::done`] was called before all expectations were consumed.
    PendingExpectations {
        /// Number of unconsumed expectations.
        remaining: usize,
    },
    /// A transaction occurred after all expectations were consumed.
    UnexpectedTransaction {
        /// Address targeted by the unexpected transaction.
        actual_address: SevenBitAddress,
    },
}

impl I2cError for MockError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

/// An `embedded-hal` 1.0 I2C implementation backed by ordered expectations.
///
/// A mismatch never advances the recorder. A matched failure expectation does
/// advance it, then returns [`MockError::Injected`].
#[derive(Debug)]
pub struct MockI2c<'a> {
    expectations: &'a [ExpectedTransaction<'a>],
    position: usize,
}

impl<'a> MockI2c<'a> {
    /// Creates a recorder over an ordered borrowed expectation slice.
    pub const fn new(expectations: &'a [ExpectedTransaction<'a>]) -> Self {
        Self {
            expectations,
            position: 0,
        }
    }

    /// Returns the number of unconsumed expectations.
    pub const fn pending(&self) -> usize {
        self.expectations.len() - self.position
    }

    /// Consumes the recorder and verifies that every expectation was matched.
    pub fn done(self) -> Result<(), MockError> {
        let remaining = self.pending();
        if remaining == 0 {
            Ok(())
        } else {
            Err(MockError::PendingExpectations { remaining })
        }
    }
}

impl ErrorType for MockI2c<'_> {
    type Error = MockError;
}

impl I2c<SevenBitAddress> for MockI2c<'_> {
    fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let Some(expected) = self.expectations.get(self.position) else {
            return Err(MockError::UnexpectedTransaction {
                actual_address: address,
            });
        };

        if address != expected.address {
            return Err(MockError::Address {
                expected: expected.address,
                actual: address,
            });
        }
        if operations.len() != expected.operations.len() {
            return Err(MockError::OperationCount {
                expected: expected.operations.len(),
                actual: operations.len(),
            });
        }

        for (index, (expected_operation, actual_operation)) in expected
            .operations
            .iter()
            .zip(operations.iter())
            .enumerate()
        {
            match (expected_operation, actual_operation) {
                (ExpectedOperation::Write(expected_bytes), Operation::Write(actual_bytes))
                    if expected_bytes == actual_bytes => {}
                (ExpectedOperation::Read(response), Operation::Read(actual_buffer)) => {
                    if response.len() != actual_buffer.len() {
                        return Err(MockError::ReadLength {
                            index,
                            expected: response.len(),
                            actual: actual_buffer.len(),
                        });
                    }
                }
                _ => return Err(MockError::OperationMismatch { index }),
            }
        }

        for (expected_operation, actual_operation) in
            expected.operations.iter().zip(operations.iter_mut())
        {
            if let (ExpectedOperation::Read(response), Operation::Read(actual_buffer)) =
                (expected_operation, actual_operation)
            {
                actual_buffer.copy_from_slice(response);
            }
        }

        let fail = expected.fail;
        self.position += 1;
        if fail {
            Err(MockError::Injected)
        } else {
            Ok(())
        }
    }
}
