//! Read-only Modbus RTU primitives for the CCPS ESP32-P4 bench firmware.
//!
//! This module deliberately has no generic function-code enum and no write
//! request constructor. Its only encodable request is function `0x03` (read
//! holding registers), matching `INV-FWR-1` and `INV-FWR-2` in the Softoboros
//! CCPS firmware requirements. Hardware access, retries, scans, and automatic
//! transmission are outside this module.

const READ_HOLDING_REGISTERS: u8 = 0x03;
const READ_HOLDING_EXCEPTION: u8 = 0x83;
const MAX_READ_REGISTERS: u16 = 8;
const MAX_RESPONSE_LEN: usize = 3 + (MAX_READ_REGISTERS as usize * 2) + 2;
const FORBIDDEN_REGISTER_FIRST: u16 = 61_440;
const FORBIDDEN_REGISTER_LAST: u16 = 61_442;

/// Errors returned while constructing the only supported request type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestError {
    /// Modbus address zero is broadcast; addresses above 247 are not device addresses.
    InvalidDeviceAddress,
    /// Rung 0 permits a bounded block of one through eight registers.
    InvalidQuantity,
    /// The requested register range overflows the 16-bit address space.
    AddressOverflow,
    /// The ACP broadcast/configuration register range is structurally unreachable.
    ForbiddenRegister,
}

/// A bounded, read-only holding-register request with both address notations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReadHoldingRequest {
    device_address: u8,
    human_register: u16,
    pdu_address: u16,
    quantity: u16,
}

impl ReadHoldingRequest {
    /// Constructs a bounded request while retaining both human and wire addresses.
    pub(crate) fn new(
        device_address: u8,
        human_register: u16,
        pdu_address: u16,
        quantity: u16,
    ) -> Result<Self, RequestError> {
        if device_address == 0 || device_address > 247 {
            return Err(RequestError::InvalidDeviceAddress);
        }
        if quantity == 0 || quantity > MAX_READ_REGISTERS {
            return Err(RequestError::InvalidQuantity);
        }

        let human_last = human_register
            .checked_add(quantity - 1)
            .ok_or(RequestError::AddressOverflow)?;
        let pdu_last = pdu_address
            .checked_add(quantity - 1)
            .ok_or(RequestError::AddressOverflow)?;

        // Until Rung 0 resolves whether a source uses one-based register labels
        // or zero-based PDU addresses, reject the dangerous ACP range under all
        // three plausible interpretations.
        let pdu_one_based_first = pdu_address
            .checked_add(1)
            .ok_or(RequestError::AddressOverflow)?;
        let pdu_one_based_last = pdu_last
            .checked_add(1)
            .ok_or(RequestError::AddressOverflow)?;
        if overlaps_forbidden(human_register, human_last)
            || overlaps_forbidden(pdu_address, pdu_last)
            || overlaps_forbidden(pdu_one_based_first, pdu_one_based_last)
        {
            return Err(RequestError::ForbiddenRegister);
        }

        Ok(Self {
            device_address,
            human_register,
            pdu_address,
            quantity,
        })
    }

    /// Encodes the fixed eight-byte Modbus RTU application data unit.
    pub(crate) fn encode(self) -> [u8; 8] {
        let mut frame = [
            self.device_address,
            READ_HOLDING_REGISTERS,
            (self.pdu_address >> 8) as u8,
            self.pdu_address as u8,
            (self.quantity >> 8) as u8,
            self.quantity as u8,
            0,
            0,
        ];
        let crc = crc16_modbus(&frame[..6]);
        frame[6] = crc as u8;
        frame[7] = (crc >> 8) as u8;
        frame
    }

    /// Returns the human-facing register label retained in the evidence record.
    pub(crate) fn human_register(self) -> u16 {
        self.human_register
    }

    /// Returns the zero-based PDU address placed on the wire.
    pub(crate) fn pdu_address(self) -> u16 {
        self.pdu_address
    }
}

/// Errors returned by the independent last-mile wire-frame validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WireFrameError {
    /// A read-holding request is exactly eight bytes.
    InvalidLength,
    /// The frame addresses broadcast or a non-device value.
    InvalidDeviceAddress,
    /// The wire-side allowlist contains only function `0x03`.
    FunctionNotAllowed,
    /// The request quantity is outside the Rung-0 bound.
    InvalidQuantity,
    /// The requested range overflows or touches a forbidden register.
    ForbiddenRegister,
    /// The frame CRC does not match its payload.
    CrcMismatch,
}

/// Independently validates an already encoded frame immediately before transport.
pub(crate) fn validate_wire_frame(frame: &[u8]) -> Result<(), WireFrameError> {
    if frame.len() != 8 {
        return Err(WireFrameError::InvalidLength);
    }
    if frame[0] == 0 || frame[0] > 247 {
        return Err(WireFrameError::InvalidDeviceAddress);
    }
    if frame[1] != READ_HOLDING_REGISTERS {
        return Err(WireFrameError::FunctionNotAllowed);
    }

    let pdu_address = u16::from_be_bytes([frame[2], frame[3]]);
    let quantity = u16::from_be_bytes([frame[4], frame[5]]);
    if quantity == 0 || quantity > MAX_READ_REGISTERS {
        return Err(WireFrameError::InvalidQuantity);
    }
    let pdu_last = pdu_address
        .checked_add(quantity - 1)
        .ok_or(WireFrameError::ForbiddenRegister)?;
    let one_based_first = pdu_address
        .checked_add(1)
        .ok_or(WireFrameError::ForbiddenRegister)?;
    let one_based_last = pdu_last
        .checked_add(1)
        .ok_or(WireFrameError::ForbiddenRegister)?;
    if overlaps_forbidden(pdu_address, pdu_last)
        || overlaps_forbidden(one_based_first, one_based_last)
    {
        return Err(WireFrameError::ForbiddenRegister);
    }

    let expected_crc = u16::from_le_bytes([frame[6], frame[7]]);
    if crc16_modbus(&frame[..6]) != expected_crc {
        return Err(WireFrameError::CrcMismatch);
    }
    Ok(())
}

/// Evidence grade assigned to a parsed field before physical cross-checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvidenceGrade {
    /// Wire data is syntactically valid but its meaning has not been proven physically.
    Candidate,
}

/// A CRC-valid response retaining raw bytes and decoded register words together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReadHoldingResponse {
    request: ReadHoldingRequest,
    observed_at_ms: u64,
    raw: [u8; MAX_RESPONSE_LEN],
    raw_len: usize,
    registers: [u16; MAX_READ_REGISTERS as usize],
    register_count: usize,
    grade: EvidenceGrade,
}

impl ReadHoldingResponse {
    /// Returns the request, including both human and PDU register addresses.
    pub(crate) fn request(&self) -> ReadHoldingRequest {
        self.request
    }

    /// Returns the immutable wire bytes that produced the decoded values.
    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw[..self.raw_len]
    }

    /// Returns big-endian Modbus register words decoded from [`Self::raw`].
    pub(crate) fn registers(&self) -> &[u16] {
        &self.registers[..self.register_count]
    }

    /// Returns the monotonic observation timestamp supplied by the transport.
    pub(crate) fn observed_at_ms(&self) -> u64 {
        self.observed_at_ms
    }

    /// Returns the evidence grade; parsing alone can only produce a candidate.
    pub(crate) fn grade(&self) -> EvidenceGrade {
        self.grade
    }
}

/// A standard exception response retained as raw evidence rather than decoded data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionResponse {
    raw: [u8; 5],
    code: u8,
    observed_at_ms: u64,
}

impl ExceptionResponse {
    /// Returns the exception code reported by the device.
    pub(crate) fn code(self) -> u8 {
        self.code
    }

    /// Returns the exact five-byte exception frame.
    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the monotonic observation timestamp supplied by the transport.
    pub(crate) fn observed_at_ms(self) -> u64 {
        self.observed_at_ms
    }
}

/// A successful parse is either register data or a retained Modbus exception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadResponse {
    /// A normal function-`0x03` response.
    Data(ReadHoldingResponse),
    /// A standard function-`0x83` exception response.
    Exception(ExceptionResponse),
}

/// Errors returned while parsing a response to one exact request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseError {
    /// The response is too short or too long for the bounded request.
    InvalidLength,
    /// The response CRC does not match its payload.
    CrcMismatch,
    /// The response came from a different Modbus device.
    DeviceAddressMismatch,
    /// The function is neither `0x03` nor its standard exception `0x83`.
    FunctionMismatch,
    /// The byte count does not exactly match the requested register quantity.
    ByteCountMismatch,
}

/// Parses a response to one exact request and retains its unmodified wire bytes.
pub(crate) fn parse_response(
    request: ReadHoldingRequest,
    frame: &[u8],
    observed_at_ms: u64,
) -> Result<ReadResponse, ResponseError> {
    if frame.len() < 5 || frame.len() > MAX_RESPONSE_LEN {
        return Err(ResponseError::InvalidLength);
    }
    let expected_crc = u16::from_le_bytes([frame[frame.len() - 2], frame[frame.len() - 1]]);
    if crc16_modbus(&frame[..frame.len() - 2]) != expected_crc {
        return Err(ResponseError::CrcMismatch);
    }
    if frame[0] != request.device_address {
        return Err(ResponseError::DeviceAddressMismatch);
    }

    if frame[1] == READ_HOLDING_EXCEPTION {
        if frame.len() != 5 {
            return Err(ResponseError::InvalidLength);
        }
        let mut raw = [0; 5];
        raw.copy_from_slice(frame);
        return Ok(ReadResponse::Exception(ExceptionResponse {
            raw,
            code: frame[2],
            observed_at_ms,
        }));
    }
    if frame[1] != READ_HOLDING_REGISTERS {
        return Err(ResponseError::FunctionMismatch);
    }

    let expected_bytes = request.quantity as usize * 2;
    if frame[2] as usize != expected_bytes || frame.len() != expected_bytes + 5 {
        return Err(ResponseError::ByteCountMismatch);
    }

    let mut raw = [0; MAX_RESPONSE_LEN];
    raw[..frame.len()].copy_from_slice(frame);
    let mut registers = [0; MAX_READ_REGISTERS as usize];
    for (index, chunk) in frame[3..3 + expected_bytes].chunks_exact(2).enumerate() {
        registers[index] = u16::from_be_bytes([chunk[0], chunk[1]]);
    }

    Ok(ReadResponse::Data(ReadHoldingResponse {
        request,
        observed_at_ms,
        raw,
        raw_len: frame.len(),
        registers,
        register_count: request.quantity as usize,
        grade: EvidenceGrade::Candidate,
    }))
}

/// Computes the Modbus RTU CRC-16 using polynomial `0xA001`.
pub(crate) fn crc16_modbus(bytes: &[u8]) -> u16 {
    let mut crc = 0xffff_u16;
    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn overlaps_forbidden(first: u16, last: u16) -> bool {
    first <= FORBIDDEN_REGISTER_LAST && last >= FORBIDDEN_REGISTER_FIRST
}

/// Builds one bounded CCPS read-holding-register request without touching hardware.
///
/// This is the only Modbus request constructor exported by the P4 payload. It
/// retains the human-facing register number separately from the zero-based PDU
/// address and refuses broadcast/configuration ranges. The caller must still
/// pass the resulting frame through [`rlvgl_ccps_modbus_wire_frame_is_allowed`]
/// immediately before any future transport write.
///
/// Returns `8` on success, `-1` for an invalid request, or `-2` for an invalid
/// output pointer/capacity. This function performs no UART or GPIO access.
///
/// # Safety
/// On success, `out` must be valid for at least eight writable bytes and must
/// not alias memory Rust accesses concurrently.
#[no_mangle]
pub unsafe extern "C" fn rlvgl_ccps_modbus_prepare_read(
    device_address: u8,
    human_register: u16,
    pdu_address: u16,
    quantity: u16,
    out: *mut u8,
    out_capacity: usize,
) -> i32 {
    if out.is_null() || out_capacity < 8 {
        return -2;
    }
    let Ok(request) =
        ReadHoldingRequest::new(device_address, human_register, pdu_address, quantity)
    else {
        return -1;
    };
    let frame = request.encode();
    // SAFETY: the caller guarantees eight writable bytes and this branch has
    // already rejected null/short output buffers.
    unsafe { core::ptr::copy_nonoverlapping(frame.as_ptr(), out, frame.len()) };
    frame.len() as i32
}

/// Applies the independent function/address/range/CRC allowlist to a wire frame.
///
/// Returns `1` only for an exact, bounded function-`0x03` request and `0` for
/// every other byte sequence. It performs no UART or GPIO access.
///
/// # Safety
/// `frame` must be valid for `frame_len` readable bytes for the duration of the
/// call and must not alias memory Rust accesses concurrently.
#[no_mangle]
pub unsafe extern "C" fn rlvgl_ccps_modbus_wire_frame_is_allowed(
    frame: *const u8,
    frame_len: usize,
) -> i32 {
    if frame.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees `frame_len` readable bytes.
    let frame = unsafe { core::slice::from_raw_parts(frame, frame_len) };
    i32::from(validate_wire_frame(frame).is_ok())
}

/// C-compatible response record retaining request identity, raw bytes, and words.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CcpsModbusObservation {
    device_address: u8,
    evidence_grade: u8,
    raw_len: u8,
    register_count: u8,
    human_register: u16,
    pdu_address: u16,
    observed_at_ms: u64,
    raw: [u8; MAX_RESPONSE_LEN],
    registers: [u16; MAX_READ_REGISTERS as usize],
    exception_code: u8,
}

impl CcpsModbusObservation {
    fn empty() -> Self {
        Self {
            device_address: 0,
            evidence_grade: 0,
            raw_len: 0,
            register_count: 0,
            human_register: 0,
            pdu_address: 0,
            observed_at_ms: 0,
            raw: [0; MAX_RESPONSE_LEN],
            registers: [0; MAX_READ_REGISTERS as usize],
            exception_code: 0,
        }
    }
}

/// Parses one response into a C-compatible evidence record without hardware access.
///
/// Returns `1` for data, `2` for a retained Modbus exception, `-1` for a
/// refused request, `-2` for invalid pointers/capacity, or `-3` for an invalid
/// response. Parsed data remains candidate evidence until physically verified.
///
/// # Safety
/// `frame` must identify `frame_len` readable bytes, and `out` must identify one
/// writable [`CcpsModbusObservation`]. Neither may alias concurrent Rust access.
#[no_mangle]
pub unsafe extern "C" fn rlvgl_ccps_modbus_parse_response(
    device_address: u8,
    human_register: u16,
    pdu_address: u16,
    quantity: u16,
    frame: *const u8,
    frame_len: usize,
    observed_at_ms: u64,
    out: *mut CcpsModbusObservation,
) -> i32 {
    if frame.is_null() || out.is_null() || frame_len > MAX_RESPONSE_LEN {
        return -2;
    }
    let Ok(request) =
        ReadHoldingRequest::new(device_address, human_register, pdu_address, quantity)
    else {
        return -1;
    };
    // SAFETY: the caller guarantees `frame_len` readable bytes.
    let frame = unsafe { core::slice::from_raw_parts(frame, frame_len) };
    let Ok(response) = parse_response(request, frame, observed_at_ms) else {
        return -3;
    };

    let mut observation = CcpsModbusObservation::empty();
    observation.device_address = device_address;
    observation.human_register = request.human_register();
    observation.pdu_address = request.pdu_address();
    observation.observed_at_ms = observed_at_ms;
    match response {
        ReadResponse::Data(response) => {
            observation.evidence_grade = match response.grade() {
                EvidenceGrade::Candidate => 0,
            };
            observation.raw_len = response.raw().len() as u8;
            observation.raw[..response.raw().len()].copy_from_slice(response.raw());
            observation.register_count = response.registers().len() as u8;
            observation.registers[..response.registers().len()]
                .copy_from_slice(response.registers());
            debug_assert_eq!(response.request(), request);
            debug_assert_eq!(response.observed_at_ms(), observed_at_ms);
            // SAFETY: `out` points to one writable observation by contract.
            unsafe { out.write(observation) };
            1
        }
        ReadResponse::Exception(response) => {
            observation.raw_len = response.raw().len() as u8;
            observation.raw[..response.raw().len()].copy_from_slice(response.raw());
            observation.exception_code = response.code();
            debug_assert_eq!(response.observed_at_ms(), observed_at_ms);
            // SAFETY: `out` points to one writable observation by contract.
            unsafe { out.write(observation) };
            2
        }
    }
}

/// Checks one observation against a caller-declared maximum age.
///
/// Returns `1` only when `now_ms` is not earlier than the observation and its
/// age is within `max_age_ms`; otherwise returns `0`.
///
/// # Safety
/// `observation` must point to one initialized [`CcpsModbusObservation`].
#[no_mangle]
pub unsafe extern "C" fn rlvgl_ccps_modbus_observation_is_fresh(
    observation: *const CcpsModbusObservation,
    now_ms: u64,
    max_age_ms: u64,
) -> i32 {
    if observation.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees one initialized observation.
    let observation = unsafe { &*observation };
    i32::from(
        now_ms
            .checked_sub(observation.observed_at_ms)
            .is_some_and(|age| age <= max_age_ms),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ReadHoldingRequest {
        ReadHoldingRequest::new(0xf7, 5_043, 5_043, 1).unwrap()
    }

    fn with_crc(mut bytes: Vec<u8>) -> Vec<u8> {
        let crc = crc16_modbus(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes
    }

    #[test]
    fn candidate_request_matches_retained_rung_zero_vector() {
        let request = request();
        assert_eq!(request.human_register(), 5_043);
        assert_eq!(request.pdu_address(), 5_043);
        assert_eq!(request.quantity, 1);
        assert_eq!(
            request.encode(),
            [0xf7, 0x03, 0x13, 0xb3, 0x00, 0x01, 0x65, 0xff]
        );
    }

    #[test]
    fn broadcast_and_non_device_addresses_are_unconstructable() {
        assert_eq!(
            ReadHoldingRequest::new(0, 5_043, 5_043, 1),
            Err(RequestError::InvalidDeviceAddress)
        );
        assert_eq!(
            ReadHoldingRequest::new(248, 5_043, 5_043, 1),
            Err(RequestError::InvalidDeviceAddress)
        );
    }

    #[test]
    fn scans_and_acp_configuration_registers_are_unconstructable() {
        assert_eq!(
            ReadHoldingRequest::new(0xf7, 5_000, 5_000, 9),
            Err(RequestError::InvalidQuantity)
        );
        assert_eq!(
            ReadHoldingRequest::new(0xf7, 61_440, 61_440, 1),
            Err(RequestError::ForbiddenRegister)
        );
        assert_eq!(
            ReadHoldingRequest::new(0xf7, 61_440, 61_439, 1),
            Err(RequestError::ForbiddenRegister)
        );
    }

    #[test]
    fn independent_wire_guard_rejects_another_function_with_valid_crc() {
        let mut frame = request().encode();
        frame[1] = 0x06;
        let crc = crc16_modbus(&frame[..6]);
        frame[6..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(
            validate_wire_frame(&frame),
            Err(WireFrameError::FunctionNotAllowed)
        );
        assert!(validate_wire_frame(&request().encode()).is_ok());
    }

    #[test]
    fn response_retains_wire_bytes_and_candidate_grade() {
        let frame = with_crc(vec![0xf7, 0x03, 0x02, 0x05, 0x00]);
        let response = parse_response(request(), &frame, 1_000).unwrap();
        let ReadResponse::Data(response) = response else {
            panic!("expected data response");
        };
        assert_eq!(response.request(), request());
        assert_eq!(response.raw(), frame);
        assert_eq!(response.registers(), [0x0500]);
        assert_eq!(response.observed_at_ms(), 1_000);
        assert_eq!(response.grade(), EvidenceGrade::Candidate);
    }

    #[test]
    fn corrupt_crc_and_mismatched_device_are_refused() {
        let mut corrupt = with_crc(vec![0xf7, 0x03, 0x02, 0x05, 0x00]);
        corrupt[3] ^= 1;
        assert_eq!(
            parse_response(request(), &corrupt, 0),
            Err(ResponseError::CrcMismatch)
        );

        let other = with_crc(vec![0x01, 0x03, 0x02, 0x05, 0x00]);
        assert_eq!(
            parse_response(request(), &other, 0),
            Err(ResponseError::DeviceAddressMismatch)
        );
    }

    #[test]
    fn exception_response_is_retained_without_becoming_telemetry() {
        let frame = with_crc(vec![0xf7, 0x83, 0x02]);
        let response = parse_response(request(), &frame, 2_000).unwrap();
        let ReadResponse::Exception(response) = response else {
            panic!("expected exception response");
        };
        assert_eq!(response.code(), 0x02);
        assert_eq!(response.raw(), frame);
        assert_eq!(response.observed_at_ms(), 2_000);
    }

    #[test]
    fn c_abi_prepares_and_rechecks_only_the_read_frame() {
        let mut out = [0_u8; 8];
        // SAFETY: `out` is a live eight-byte writable array.
        let len = unsafe {
            rlvgl_ccps_modbus_prepare_read(0xf7, 5_043, 5_043, 1, out.as_mut_ptr(), out.len())
        };
        assert_eq!(len, 8);
        // SAFETY: `out` is a live eight-byte readable array.
        assert_eq!(
            unsafe { rlvgl_ccps_modbus_wire_frame_is_allowed(out.as_ptr(), out.len()) },
            1
        );
        // SAFETY: the output pointer is null, so no memory is dereferenced.
        assert_eq!(
            unsafe {
                rlvgl_ccps_modbus_prepare_read(0xf7, 5_043, 5_043, 1, core::ptr::null_mut(), 0)
            },
            -2
        );
    }

    #[test]
    fn c_abi_response_keeps_raw_bytes_and_refuses_stale_data() {
        let frame = with_crc(vec![0xf7, 0x03, 0x02, 0x05, 0x00]);
        let mut observation = CcpsModbusObservation::empty();
        // SAFETY: the frame and output record are live for the call.
        let status = unsafe {
            rlvgl_ccps_modbus_parse_response(
                0xf7,
                5_043,
                5_043,
                1,
                frame.as_ptr(),
                frame.len(),
                1_000,
                &mut observation,
            )
        };
        assert_eq!(status, 1);
        assert_eq!(&observation.raw[..observation.raw_len as usize], frame);
        assert_eq!(observation.registers[0], 0x0500);
        // SAFETY: `observation` is initialized above.
        assert_eq!(
            unsafe { rlvgl_ccps_modbus_observation_is_fresh(&observation, 1_050, 50) },
            1
        );
        // SAFETY: `observation` is initialized above.
        assert_eq!(
            unsafe { rlvgl_ccps_modbus_observation_is_fresh(&observation, 1_051, 50) },
            0
        );
        // SAFETY: `observation` is initialized above.
        assert_eq!(
            unsafe { rlvgl_ccps_modbus_observation_is_fresh(&observation, 999, 50) },
            0
        );
    }
}
