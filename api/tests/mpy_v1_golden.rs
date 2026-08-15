// mpy_v1_golden.rs - Conformance tests for committed MPY v1 canonical byte vectors.

use rlvgl_api::protocol::{
    Batch, BatchBudget, Capabilities, CodecError, Command, Completion, CompletionStatus, Cue,
    DiscriminantDomain, ErrorClass, FrameRef, Hello, Limits, MPY_V1, OpcodeList, ProtocolVersion,
    RuntimeNotice, ValueRef, decode_frame, decode_value, encode_frame, encode_value,
};

const OPCODES: &[u32] = &[0x10, 0x1020_3040];

fn limits() -> Limits {
    Limits {
        max_frame_bytes: 256,
        max_text_bytes: 128,
        max_byte_payload: 64,
        max_fields_per_command: 8,
        max_values_per_result: 8,
    }
}

fn values() -> Vec<(&'static str, ValueRef<'static>)> {
    vec![
        ("value.none", ValueRef::None),
        ("value.bool_true", ValueRef::Bool(true)),
        ("value.i32", ValueRef::I32(-2)),
        ("value.u32", ValueRef::U32(0x1234_5678)),
        ("value.i64", ValueRef::I64(-2)),
        ("value.u64", ValueRef::U64(0x0123_4567_89ab_cdef)),
        ("value.precise", ValueRef::Precise(-125)),
        ("value.color", ValueRef::Color(0xff11_2233)),
        ("value.point", ValueRef::Point { x: -1, y: 2 }),
        (
            "value.size",
            ValueRef::Size {
                width: 320,
                height: 240,
            },
        ),
        (
            "value.rect",
            ValueRef::Rect {
                x: -1,
                y: -2,
                width: 3,
                height: 4,
            },
        ),
        (
            "value.enum",
            ValueRef::Enum {
                domain: 2,
                value: 5,
            },
        ),
        ("value.text", ValueRef::Text("Hi μ")),
        ("value.bytes", ValueRef::Bytes(&[0xde, 0xad, 0x00, 0xbe])),
        ("value.object", ValueRef::Object(0x0000_0002_0000_0001)),
        (
            "value.resource",
            ValueRef::Resource {
                kind: 3,
                id: 0x0102_0304_0506_0708,
            },
        ),
        ("value.batch_object", ValueRef::BatchObject(7)),
    ]
}

fn frames() -> Vec<(&'static str, FrameRef<'static>)> {
    vec![
        (
            "frame.hello",
            FrameRef::Hello(Hello {
                schema_version: ProtocolVersion {
                    major: 1,
                    minor: 2,
                    patch: 3,
                },
                limits: limits(),
                features: 5,
            }),
        ),
        (
            "frame.capabilities",
            FrameRef::Capabilities(Capabilities {
                schema_version: ProtocolVersion {
                    major: 1,
                    minor: 2,
                    patch: 3,
                },
                limits: limits(),
                features: 5,
                value_tags: 0x0001_ffff,
                opcodes: OpcodeList::from_slice(OPCODES),
            }),
        ),
        (
            "frame.command",
            FrameRef::Command(Command {
                stage_id: 1,
                request_id: 2,
                opcode: 3,
                flags: 4,
                payload: &[0x00, 0x01],
            }),
        ),
        (
            "frame.batch",
            FrameRef::Batch(Batch {
                stage_id: 1,
                request_id: 2,
                flags: 4,
                budget: BatchBudget {
                    actors: 2,
                    text_bytes: 32,
                    resources: 1,
                    result_bytes: 64,
                },
                operations: &[0x03, 0x02, 0x01],
            }),
        ),
        (
            "frame.result",
            FrameRef::Result(Completion {
                request_id: 2,
                status: CompletionStatus::Error(ErrorClass::Capacity),
                operation_index: Some(5),
                field_id: Some(9),
                diagnostic: "cap",
                payload: &[],
            }),
        ),
        (
            "frame.cue",
            FrameRef::Cue(Cue {
                sequence: 1,
                stage_id: 2,
                object_id: 0x0000_0003_0000_0004,
                subscription_id: 5,
                callback_id: 6,
                event_id: 7,
                flags: 8,
                payload: &[0x01, 0x00],
            }),
        ),
        (
            "frame.runtime_notice",
            FrameRef::RuntimeNotice(RuntimeNotice {
                sequence: 1,
                kind: 2,
                diagnostic: "log",
                payload: &[0xaa, 0x55],
            }),
        ),
    ]
}

fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    include_str!("fixtures/mpy_v1_vectors.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, hex) = line.split_once('|').expect("fixture separator");
            let bytes = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    let text = core::str::from_utf8(pair).expect("ASCII fixture");
                    u8::from_str_radix(text, 16).expect("hex fixture")
                })
                .collect();
            (name, bytes)
        })
        .collect()
}

fn fixture(name: &str) -> Vec<u8> {
    fixtures()
        .into_iter()
        .find_map(|(candidate, bytes)| (candidate == name).then_some(bytes))
        .unwrap_or_else(|| panic!("missing fixture {name}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn every_value_tag_matches_committed_golden_bytes() {
    for (name, value) in values() {
        let mut encoded = [0u8; 256];
        let length = encode_value(value, &mut encoded).expect(name);
        if std::env::var_os("RLVGL_MPY_DUMP_GOLDENS").is_some() {
            println!("{name}|{}", hex(&encoded[..length]));
            continue;
        }
        assert_eq!(&encoded[..length], fixture(name), "{name}");
        let (decoded, consumed) = decode_value(&encoded[..length]).expect(name);
        assert_eq!(decoded, value, "{name}");
        assert_eq!(consumed, length, "{name}");
    }
}

#[test]
fn every_frame_class_matches_committed_golden_bytes() {
    for (name, frame) in frames() {
        let mut encoded = [0u8; 256];
        let length = encode_frame(MPY_V1, frame, &mut encoded).expect(name);
        if std::env::var_os("RLVGL_MPY_DUMP_GOLDENS").is_some() {
            println!("{name}|{}", hex(&encoded[..length]));
            continue;
        }
        assert_eq!(&encoded[..length], fixture(name), "{name}");
        let decoded = decode_frame(&encoded[..length]).expect(name);
        assert_eq!(decoded.version, MPY_V1, "{name}");
        assert_eq!(decoded.frame, frame, "{name}");

        let mut round_trip = [0u8; 256];
        let round_trip_length =
            encode_frame(decoded.version, decoded.frame, &mut round_trip).expect(name);
        assert_eq!(
            &round_trip[..round_trip_length],
            &encoded[..length],
            "{name}"
        );
    }
}

#[test]
fn rejects_noncanonical_and_invalid_encodings() {
    assert_eq!(
        encode_value(ValueRef::None, &mut []),
        Err(CodecError::BufferTooSmall)
    );
    assert_eq!(decode_value(&[0x01, 0x02]), Err(CodecError::InvalidFrame));
    assert_eq!(
        decode_value(&[0x0c, 0x01, 0, 0, 0, 0xff]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_value(&[0xff]),
        Err(CodecError::UnsupportedDiscriminant {
            domain: DiscriminantDomain::ValueTag,
            value: 0xff,
        })
    );
    assert_eq!(
        encode_value(ValueRef::Object(1), &mut [0; 16]),
        Err(CodecError::InvalidFrame)
    );
    let invalid_result = FrameRef::Result(Completion {
        request_id: 1,
        status: CompletionStatus::Error(ErrorClass::UnknownProperty),
        operation_index: None,
        field_id: Some(0),
        diagnostic: "field",
        payload: &[],
    });
    assert_eq!(
        encode_frame(MPY_V1, invalid_result, &mut [0; 64]),
        Err(CodecError::InvalidFrame)
    );

    let frame = fixture("frame.command");
    assert_eq!(
        decode_frame(&frame[..frame.len() - 1]),
        Err(CodecError::Truncated)
    );
    let mut trailing = frame.clone();
    trailing.push(0);
    assert_eq!(decode_frame(&trailing), Err(CodecError::InvalidFrame));
}

#[test]
fn interoperability_floor_values_fit_the_floor_frame() {
    const TEXT: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    assert_eq!(TEXT.len(), 128);
    let mut value = [0u8; 256];
    let value_length = encode_value(ValueRef::Text(TEXT), &mut value).expect("128-byte text");
    assert_eq!(value_length, 133);

    let command = FrameRef::Command(Command {
        stage_id: 1,
        request_id: 1,
        opcode: 1,
        flags: 0,
        payload: &value[..value_length],
    });
    let mut frame = [0u8; 256];
    let frame_length = encode_frame(MPY_V1, command, &mut frame).expect("floor frame");
    assert!(frame_length <= frame.len());
}
