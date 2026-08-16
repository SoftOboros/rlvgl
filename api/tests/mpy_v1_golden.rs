// mpy_v1_golden.rs - Conformance tests for committed MPY v1 canonical byte vectors.

use rlvgl_api::protocol::{
    Batch, BatchBudget, Capabilities, CodecError, Command, Completion, CompletionStatus, Cue,
    DiscriminantDomain, ErrorClass, FrameRef, Hello, Limits, MPY_V1, OpcodeList, OperationList,
    OperationRef, ProtocolVersion, RuntimeNotice, ValueRef, decode_frame, decode_frame_with_limits,
    decode_operation_list, decode_operation_list_with_limit, decode_value, encode_frame,
    encode_frame_with_limits, encode_operation_list, encode_operation_list_with_limit,
    encode_value, opcode,
};

const OPCODES: &[u32] = &[0x10, 0x1020_3040];
const BATCH_OPERATIONS: &[OperationRef<'static>] = &[
    OperationRef {
        opcode: opcode::CREATE,
        flags: 0,
        payload: &[0x03, 0x02, 0x01],
    },
    OperationRef {
        opcode: opcode::DELETE,
        flags: 0,
        payload: &[],
    },
];
const REGISTRY_OPERATIONS: &[OperationRef<'static>] = &[
    empty_operation(opcode::CREATE),
    empty_operation(opcode::SET_PROPERTIES),
    empty_operation(opcode::RESET_PROPERTIES),
    empty_operation(opcode::INVOKE_ACTION),
    empty_operation(opcode::SET_FLAG),
    empty_operation(opcode::SET_REQUESTED_LAYOUT),
    empty_operation(opcode::REPARENT),
    empty_operation(opcode::PROMOTE_ROOT),
    empty_operation(opcode::REORDER),
    empty_operation(opcode::DELETE),
    empty_operation(opcode::SET_LOCAL_STYLE),
];

const fn empty_operation(opcode: u32) -> OperationRef<'static> {
    OperationRef {
        opcode,
        flags: 0,
        payload: &[],
    }
}

fn limits() -> Limits {
    Limits {
        max_frame_bytes: 256,
        max_text_bytes: 128,
        max_byte_payload: 64,
        max_items_per_command: 8,
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
                operations: OperationList::from_slice(BATCH_OPERATIONS),
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
    for invalid_opcode in [opcode::INVALID, opcode::RESERVED] {
        let invalid_command = FrameRef::Command(Command {
            stage_id: 1,
            request_id: 1,
            opcode: invalid_opcode,
            flags: 0,
            payload: &[],
        });
        assert_eq!(
            encode_frame(MPY_V1, invalid_command, &mut [0; 64]),
            Err(CodecError::InvalidFrame)
        );
    }

    let frame = fixture("frame.command");
    assert_eq!(
        decode_frame(&frame[..frame.len() - 1]),
        Err(CodecError::Truncated)
    );
    let mut trailing = frame.clone();
    trailing.push(0);
    assert_eq!(decode_frame(&trailing), Err(CodecError::InvalidFrame));

    let mut malformed_batch = fixture("frame.batch");
    malformed_batch[36..38].copy_from_slice(&3u16.to_le_bytes());
    assert_eq!(
        decode_frame(&malformed_batch),
        Err(CodecError::InvalidFrame)
    );
}

#[test]
fn counted_operation_records_match_the_stable_registry() {
    assert_eq!(opcode::CREATE, 1);
    assert_eq!(opcode::SET_PROPERTIES, 2);
    assert_eq!(opcode::RESET_PROPERTIES, 3);
    assert_eq!(opcode::INVOKE_ACTION, 4);
    assert_eq!(opcode::SET_FLAG, 5);
    assert_eq!(opcode::SET_REQUESTED_LAYOUT, 6);
    assert_eq!(opcode::REPARENT, 7);
    assert_eq!(opcode::PROMOTE_ROOT, 8);
    assert_eq!(opcode::REORDER, 9);
    assert_eq!(opcode::DELETE, 10);
    assert_eq!(opcode::SET_LOCAL_STYLE, 11);

    let mut encoded = [0u8; 256];
    let length =
        encode_operation_list(OperationList::from_slice(BATCH_OPERATIONS), &mut encoded).unwrap();
    let decoded = decode_operation_list(&encoded[..length]).unwrap();
    assert_eq!(decoded, OperationList::from_slice(BATCH_OPERATIONS));
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded.iter().collect::<Vec<_>>(), BATCH_OPERATIONS);

    let registry_length =
        encode_operation_list(OperationList::from_slice(REGISTRY_OPERATIONS), &mut encoded)
            .unwrap();
    assert_eq!(
        &encoded[..registry_length],
        fixture("operation.initial_registry")
    );

    let invalid_opcode = [OperationRef {
        opcode: opcode::INVALID,
        flags: 0,
        payload: &[],
    }];
    assert_eq!(
        encode_operation_list(OperationList::from_slice(&invalid_opcode), &mut encoded),
        Err(CodecError::InvalidFrame)
    );
    let invalid_flags = [OperationRef {
        opcode: opcode::CREATE,
        flags: 1,
        payload: &[],
    }];
    assert_eq!(
        encode_operation_list(OperationList::from_slice(&invalid_flags), &mut encoded),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_operation_list(&[1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_operation_list(&[0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
}

#[test]
fn active_item_limit_rejects_a_ninth_operation() {
    let operation = empty_operation(opcode::DELETE);
    let operations = [operation; 9];
    let mut encoded_operations = [0u8; 128];
    assert_eq!(
        encode_operation_list_with_limit(
            OperationList::from_slice(&operations),
            8,
            &mut encoded_operations,
        ),
        Err(CodecError::LimitExceeded)
    );
    let operation_length = encode_operation_list(
        OperationList::from_slice(&operations),
        &mut encoded_operations,
    )
    .unwrap();
    assert_eq!(
        decode_operation_list_with_limit(&encoded_operations[..operation_length], 8),
        Err(CodecError::LimitExceeded)
    );

    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 2,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    let mut encoded_frame = [0u8; 256];
    assert_eq!(
        encode_frame_with_limits(MPY_V1, batch, limits(), &mut encoded_frame),
        Err(CodecError::LimitExceeded)
    );
    let frame_length = encode_frame(MPY_V1, batch, &mut encoded_frame).unwrap();
    assert_eq!(
        decode_frame_with_limits(&encoded_frame[..frame_length], limits()),
        Err(CodecError::LimitExceeded)
    );
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

    let empty = OperationRef {
        opcode: opcode::DELETE,
        flags: 0,
        payload: &[],
    };
    let operations = [empty; 8];
    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 2,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    let batch_length = encode_frame(MPY_V1, batch, &mut frame).expect("eight-operation floor");
    assert!(batch_length <= frame.len());
    assert_eq!(limits().max_fields_per_command(), 8);
}
