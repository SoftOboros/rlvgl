// mpy_v1_golden.rs - Conformance tests for committed MPY v1 canonical byte vectors.

use rlvgl_api::protocol::{
    Batch, BatchBudget, BatchSuccess, Capabilities, CodecError, Command, Completion,
    CompletionStatus, CreateDestinationRef, CreatePayload, Cue, DiscriminantDomain, ErrorClass,
    FieldList, FieldRef, FrameRef, Hello, Limits, MPY_V1, MutationTargetEnvelope, ObjectReference,
    ObjectReferenceError, OpcodeList, OperationList, OperationRef, OperationResultList,
    OperationResultRef, PromoteRootPayload, ProtocolVersion, ReorderPayload, ReparentPayload,
    RuntimeFlag, RuntimeNotice, SetFlagPayload, ValueList, ValueRef, ValueTag,
    create_result_object, decode_batch_success, decode_batch_success_with_limits,
    decode_create_operation_with_limits, decode_create_payload, decode_create_payload_with_limits,
    decode_delete_operation, decode_delete_payload, decode_field_list,
    decode_field_list_with_limits, decode_frame, decode_frame_with_limits,
    decode_mutation_operation_target, decode_mutation_target_envelope, decode_object_reference,
    decode_operation_list, decode_operation_list_with_limit, decode_promote_root_operation,
    decode_promote_root_operation_with_limits, decode_promote_root_payload,
    decode_promote_root_payload_with_limits, decode_reorder_operation, decode_reorder_payload,
    decode_reparent_operation, decode_reparent_payload, decode_set_flag_operation,
    decode_set_flag_payload, decode_value, decode_value_list, decode_value_list_with_limits,
    encode_batch_success, encode_batch_success_with_limit, encode_batch_success_with_limits,
    encode_create_payload, encode_create_payload_with_limits, encode_delete_payload,
    encode_field_list, encode_field_list_with_limit, encode_field_list_with_limits, encode_frame,
    encode_frame_with_limits, encode_mutation_target_envelope, encode_object_reference,
    encode_operation_list, encode_operation_list_with_limit, encode_promote_root_payload,
    encode_promote_root_payload_with_limits, encode_reorder_payload, encode_reparent_payload,
    encode_set_flag_payload, encode_value, encode_value_list, encode_value_list_with_limit,
    encode_value_list_with_limits, is_batch_mutation_opcode, opcode, validate_delete_result_absent,
    validate_promote_root_result_absent, validate_reorder_result_absent,
    validate_reparent_result_absent, validate_set_flag_result_absent,
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
const LIST_VALUES: &[ValueRef<'static>] = &[
    ValueRef::Bool(true),
    ValueRef::I32(-2),
    ValueRef::BatchObject(7),
];
const TYPED_FIELDS: &[FieldRef<'static>] = &[
    FieldRef {
        id: 1,
        value: ValueRef::U32(0x1234_5678),
    },
    FieldRef {
        id: 9,
        value: ValueRef::Text("Hi"),
    },
    FieldRef {
        id: 10,
        value: ValueRef::Object(0x0000_0002_0000_0001),
    },
];
const FIRST_RESULT_VALUES: &[ValueRef<'static>] =
    &[ValueRef::U32(7), ValueRef::Object(0x0000_0002_0000_0001)];
const MUTATION_RESULT_VALUES: &[ValueRef<'static>] = &[ValueRef::Bool(true)];
const BATCH_RESULTS: &[OperationResultRef<'static>] = &[
    OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(FIRST_RESULT_VALUES),
    },
    OperationResultRef {
        operation_index: 2,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    },
];
const CREATE_FIELDS: &[FieldRef<'static>] = &[
    FieldRef {
        id: 1,
        value: ValueRef::U32(5),
    },
    FieldRef {
        id: 2,
        value: ValueRef::Text("Hi"),
    },
];
const CREATE_RESULT_VALUES: &[ValueRef<'static>] = &[ValueRef::Object(0x0000_0002_0000_0001)];
const MUTATION_OPCODES: &[u32] = &[
    opcode::SET_PROPERTIES,
    opcode::RESET_PROPERTIES,
    opcode::INVOKE_ACTION,
    opcode::SET_FLAG,
    opcode::SET_REQUESTED_LAYOUT,
    opcode::REPARENT,
    opcode::PROMOTE_ROOT,
    opcode::REORDER,
    opcode::DELETE,
    opcode::SET_LOCAL_STYLE,
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
fn counted_typed_lists_match_golden_bytes_and_round_trip_zero_copy() {
    let mut encoded = [0u8; 256];

    let value_length = encode_value_list(ValueList::from_slice(LIST_VALUES), &mut encoded).unwrap();
    assert_eq!(&encoded[..value_length], fixture("payload.value_list"));
    let decoded_values = decode_value_list(&encoded[..value_length]).unwrap();
    assert_eq!(decoded_values, ValueList::from_slice(LIST_VALUES));
    assert_eq!(decoded_values.iter().collect::<Vec<_>>(), LIST_VALUES);

    let field_length =
        encode_field_list(FieldList::from_slice(TYPED_FIELDS), &mut encoded).unwrap();
    assert_eq!(&encoded[..field_length], fixture("payload.field_list"));
    let decoded_fields = decode_field_list(&encoded[..field_length]).unwrap();
    assert_eq!(decoded_fields, FieldList::from_slice(TYPED_FIELDS));
    assert_eq!(decoded_fields.iter().collect::<Vec<_>>(), TYPED_FIELDS);
    let borrowed_text = match decoded_fields.iter().nth(1).unwrap().value {
        ValueRef::Text(value) => value,
        value => panic!("expected text, got {value:?}"),
    };
    let encoded_start = encoded.as_ptr() as usize;
    let encoded_end = encoded_start + field_length;
    assert!((encoded_start..encoded_end).contains(&(borrowed_text.as_ptr() as usize)));
}

#[test]
fn contextual_object_references_reuse_value_tags_and_preserve_error_classification() {
    for (reference, fixture_name) in [
        (
            ObjectReference::Object(0x0000_0002_0000_0001),
            "value.object",
        ),
        (ObjectReference::BatchObject(7), "value.batch_object"),
    ] {
        let mut encoded = [0u8; 16];
        let length = encode_object_reference(reference, &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(
            decode_object_reference(&encoded[..length]),
            Ok((reference, length))
        );
    }

    assert_eq!(
        decode_object_reference(&[ValueTag::Bool as u8, 1]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    assert_eq!(
        decode_object_reference(&[ValueTag::Object as u8, 1, 0, 0, 0, 0, 0, 0, 0]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_object_reference(&[ValueTag::Object as u8]),
        Err(ObjectReferenceError::Codec(CodecError::Truncated))
    );
}

#[test]
fn create_payloads_match_golden_bytes_and_decode_zero_copy() {
    let fields = FieldList::from_slice(CREATE_FIELDS);
    let root = CreatePayload {
        batch_ref: 7,
        type_id: 0x1122_3344,
        destination: CreateDestinationRef::Root("main"),
        constructor_fields: fields,
    };
    let child = CreatePayload {
        batch_ref: 8,
        type_id: 0x1122_3344,
        destination: CreateDestinationRef::Child(ObjectReference::BatchObject(7)),
        constructor_fields: fields,
    };

    for (name, payload) in [
        ("payload.create_root", root),
        ("payload.create_child", child),
    ] {
        let mut encoded = [0u8; 256];
        let length = encode_create_payload_with_limits(payload, limits(), &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(name));
        let decoded = decode_create_payload_with_limits(&encoded[..length], limits()).unwrap();
        assert_eq!(decoded, payload);

        if let CreateDestinationRef::Root(name) = decoded.destination {
            let encoded_range = encoded.as_ptr() as usize..encoded.as_ptr() as usize + length;
            assert!(encoded_range.contains(&(name.as_ptr() as usize)));
        }
        let borrowed_text = match decoded.constructor_fields.iter().nth(1).unwrap().value {
            ValueRef::Text(value) => value,
            value => panic!("expected text, got {value:?}"),
        };
        let encoded_range = encoded.as_ptr() as usize..encoded.as_ptr() as usize + length;
        assert!(encoded_range.contains(&(borrowed_text.as_ptr() as usize)));

        let operation = OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &encoded[..length],
        };
        assert_eq!(
            decode_create_operation_with_limits(operation, limits()),
            Ok(payload)
        );
    }

    let root_bytes = fixture("payload.create_root");
    for operation in [
        OperationRef {
            opcode: opcode::DELETE,
            flags: 0,
            payload: &root_bytes,
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 1,
            payload: &root_bytes,
        },
    ] {
        assert_eq!(
            decode_create_operation_with_limits(operation, limits()),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
}

#[test]
fn create_payload_rejects_malformed_destinations_bindings_and_fields() {
    let valid = fixture("payload.create_root");

    for offset in [0usize, 2] {
        let mut malformed = valid.clone();
        if offset == 0 {
            malformed[0..2].copy_from_slice(&0u16.to_le_bytes());
        } else {
            malformed[2..6].copy_from_slice(&0u32.to_le_bytes());
        }
        assert_eq!(
            decode_create_payload(&malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }

    let mut invalid_destination = valid.clone();
    invalid_destination[6] = 0;
    assert_eq!(
        decode_create_payload(&invalid_destination),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    invalid_destination[6] = 0xff;
    assert_eq!(
        decode_create_payload(&invalid_destination),
        Err(ObjectReferenceError::Codec(
            CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::CreateDestination,
                value: 0xff,
            }
        ))
    );

    let empty_root = [1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        decode_create_payload(&empty_root),
        Ok(CreatePayload {
            batch_ref: 1,
            type_id: 1,
            destination: CreateDestinationRef::Root(""),
            constructor_fields: FieldList::from_slice(&[]),
        }),
        "the runtime retains InvalidParent authority over an empty root name"
    );
    let invalid_utf8_root = [1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0xff, 0, 0];
    for malformed in [&invalid_utf8_root[..], &valid[..valid.len() - 1]] {
        assert_eq!(
            decode_create_payload(malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        decode_create_payload(&trailing),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    let mut duplicate_wire_fields = fixture("payload.create_root");
    duplicate_wire_fields[26..30].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        decode_create_payload(&duplicate_wire_fields),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    let child_bool = [1, 0, 1, 0, 0, 0, 2, ValueTag::Bool as u8, 1, 0, 0];
    assert_eq!(
        decode_create_payload(&child_bool),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    let invalid_child_object = [
        1,
        0,
        1,
        0,
        0,
        0,
        2,
        ValueTag::Object as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    assert_eq!(
        decode_create_payload(&invalid_child_object),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    let duplicate_fields = [
        FieldRef {
            id: 1,
            value: ValueRef::None,
        },
        FieldRef {
            id: 1,
            value: ValueRef::None,
        },
    ];
    let malformed = CreatePayload {
        batch_ref: 0,
        type_id: 1,
        destination: CreateDestinationRef::Root("main"),
        constructor_fields: FieldList::from_slice(&duplicate_fields),
    };
    let mut one_item = limits();
    one_item.max_items_per_command = 1;
    assert_eq!(
        encode_create_payload_with_limits(malformed, one_item, &mut [0; 64]),
        Err(CodecError::InvalidFrame),
        "structural errors precede negotiated limits"
    );
}

#[test]
fn create_payload_enforces_full_negotiated_limits() {
    let payload = CreatePayload {
        batch_ref: 7,
        type_id: 0x1122_3344,
        destination: CreateDestinationRef::Root("main"),
        constructor_fields: FieldList::from_slice(CREATE_FIELDS),
    };
    let mut encoded = [0u8; 256];
    let length = encode_create_payload(payload, &mut encoded).unwrap();

    let mut short_root = limits();
    short_root.max_text_bytes = 3;
    assert_eq!(
        encode_create_payload_with_limits(payload, short_root, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_create_payload_with_limits(&encoded[..length], short_root),
        Err(ObjectReferenceError::Codec(CodecError::LimitExceeded))
    );

    let mut one_field = limits();
    one_field.max_items_per_command = 1;
    assert_eq!(
        encode_create_payload_with_limits(payload, one_field, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_create_payload_with_limits(&encoded[..length], one_field),
        Err(ObjectReferenceError::Codec(CodecError::LimitExceeded))
    );

    let mut short_field_text = limits();
    short_field_text.max_text_bytes = 1;
    assert_eq!(
        encode_create_payload_with_limits(payload, short_field_text, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_create_payload_with_limits(&encoded[..length], short_field_text),
        Err(ObjectReferenceError::Codec(CodecError::LimitExceeded))
    );

    let bytes_fields = [FieldRef {
        id: 1,
        value: ValueRef::Bytes(&[1, 2]),
    }];
    let bytes_payload = CreatePayload {
        batch_ref: 8,
        type_id: 1,
        destination: CreateDestinationRef::Root("b"),
        constructor_fields: FieldList::from_slice(&bytes_fields),
    };
    let bytes_length = encode_create_payload(bytes_payload, &mut encoded).unwrap();
    let mut short_bytes = limits();
    short_bytes.max_byte_payload = 1;
    assert_eq!(
        encode_create_payload_with_limits(bytes_payload, short_bytes, &mut [0; 64]),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_create_payload_with_limits(&encoded[..bytes_length], short_bytes),
        Err(ObjectReferenceError::Codec(CodecError::LimitExceeded))
    );
}

#[test]
fn create_success_is_exactly_one_correlated_stable_object() {
    let result = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let success = BatchSuccess {
        result_revision: 10,
        results: OperationResultList::from_slice(&result),
    };
    let mut encoded = [0u8; 64];
    let length = encode_batch_success_with_limits(success, 3, limits(), &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.create_success"));
    let decoded = decode_batch_success_with_limits(&encoded[..length], 3, limits()).unwrap();
    let record = decoded.results.iter().next().unwrap();
    assert_eq!(record.operation_index, 1);
    assert_eq!(
        create_result_object(record.values),
        Ok(0x0000_0002_0000_0001)
    );

    assert_eq!(
        create_result_object(ValueList::from_slice(&[])),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        create_result_object(ValueList::from_slice(&[
            ValueRef::Object(0x0000_0002_0000_0001),
            ValueRef::None,
        ])),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    for value in [ValueRef::BatchObject(7), ValueRef::U32(7)] {
        assert_eq!(
            create_result_object(ValueList::from_slice(&[value])),
            Err(ObjectReferenceError::TypeMismatch {
                actual: value.tag(),
            })
        );
    }
}

#[test]
fn mutation_target_envelope_matches_golden_bytes_and_borrows_remainder() {
    let envelope = MutationTargetEnvelope {
        target: ObjectReference::BatchObject(7),
        remainder: &[0xaa, 0x55, 0x00],
    };
    let mut encoded = [0u8; 32];
    let length = encode_mutation_target_envelope(envelope, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.mutation_target"));
    let decoded = decode_mutation_target_envelope(&encoded[..length]).unwrap();
    assert_eq!(decoded, envelope);
    assert_eq!(decoded.remainder.as_ptr(), encoded[3..].as_ptr());

    for operation_opcode in MUTATION_OPCODES {
        assert!(is_batch_mutation_opcode(*operation_opcode));
        let operation_length = encode_mutation_target_envelope(envelope, &mut encoded).unwrap();
        let operation = OperationRef {
            opcode: *operation_opcode,
            flags: 0,
            payload: &encoded[..operation_length],
        };
        assert_eq!(decode_mutation_operation_target(operation), Ok(envelope));
    }

    let stable = MutationTargetEnvelope {
        target: ObjectReference::Object(0x0000_0002_0000_0001),
        remainder: &[],
    };
    let stable_length = encode_mutation_target_envelope(stable, &mut encoded).unwrap();
    assert_eq!(
        decode_mutation_target_envelope(&encoded[..stable_length]),
        Ok(stable)
    );
}

#[test]
fn mutation_target_preserves_object_reference_error_boundaries_and_context() {
    assert_eq!(
        decode_mutation_target_envelope(&[]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_mutation_target_envelope(&[ValueTag::Object as u8, 1]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_mutation_target_envelope(&[ValueTag::BatchObject as u8, 0, 0]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_mutation_target_envelope(&[ValueTag::Bool as u8, 1, 0xaa]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    assert_eq!(
        decode_mutation_target_envelope(&[0xff]),
        Err(ObjectReferenceError::Codec(
            CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::ValueTag,
                value: 0xff,
            }
        ))
    );

    for target in [ObjectReference::Object(1), ObjectReference::BatchObject(0)] {
        assert_eq!(
            encode_mutation_target_envelope(
                MutationTargetEnvelope {
                    target,
                    remainder: &[0xaa],
                },
                &mut [0; 16],
            ),
            Err(CodecError::InvalidFrame)
        );
    }
    assert_eq!(
        encode_mutation_target_envelope(
            MutationTargetEnvelope {
                target: ObjectReference::BatchObject(7),
                remainder: &[0xaa],
            },
            &mut [0; 3],
        ),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.mutation_target");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::EXPERIMENTAL_FIRST, 0),
        (opcode::SET_PROPERTIES, 1),
    ] {
        assert_eq!(
            decode_mutation_operation_target(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    assert!(!is_batch_mutation_opcode(opcode::CREATE));
}

#[test]
fn delete_payloads_are_exact_target_only_golden_vectors() {
    for (target, fixture_name) in [
        (
            ObjectReference::Object(0x0000_0002_0000_0001),
            "payload.delete_object",
        ),
        (
            ObjectReference::BatchObject(7),
            "payload.delete_batch_object",
        ),
    ] {
        let mut encoded = [0u8; 16];
        let length = encode_delete_payload(target, &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_delete_payload(&encoded[..length]), Ok(target));
        assert_eq!(
            decode_delete_operation(OperationRef {
                opcode: opcode::DELETE,
                flags: 0,
                payload: &encoded[..length],
            }),
            Ok(target)
        );
    }
}

#[test]
fn delete_rejects_remainders_bad_context_and_output_records() {
    let mut trailing = fixture("payload.delete_batch_object");
    trailing.push(0xaa);
    assert_eq!(
        decode_delete_payload(&trailing),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    for malformed in [
        &[][..],
        &[ValueTag::Object as u8, 1][..],
        &[ValueTag::BatchObject as u8, 0, 0][..],
    ] {
        assert_eq!(
            decode_delete_payload(malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    assert_eq!(
        decode_delete_payload(&[ValueTag::Bool as u8, 1]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    for target in [ObjectReference::Object(1), ObjectReference::BatchObject(0)] {
        assert_eq!(
            encode_delete_payload(target, &mut [0; 16]),
            Err(CodecError::InvalidFrame)
        );
    }
    assert_eq!(
        encode_delete_payload(ObjectReference::BatchObject(7), &mut [0; 2]),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.delete_object");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::SET_PROPERTIES, 0),
        (opcode::DELETE, 1),
    ] {
        assert_eq!(
            decode_delete_operation(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }

    let outputless = BatchSuccess {
        result_revision: 11,
        results: OperationResultList::from_slice(&[]),
    };
    let mut encoded = [0u8; 64];
    let length = encode_batch_success(outputless, 1, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.delete_success"));
    let decoded = decode_batch_success(&encoded[..length], 1).unwrap();
    assert_eq!(decoded.result_revision, 11);
    assert!(decoded.results.is_empty());
    assert_eq!(validate_delete_result_absent(decoded, 1, 0), Ok(()));

    let other_output = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let mixed = BatchSuccess {
        result_revision: 12,
        results: OperationResultList::from_slice(&other_output),
    };
    assert_eq!(validate_delete_result_absent(mixed, 2, 0), Ok(()));
    assert_eq!(
        validate_delete_result_absent(mixed, 2, 1),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        validate_delete_result_absent(mixed, 2, 2),
        Err(CodecError::InvalidFrame)
    );

    let forbidden_delete_output = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    }];
    assert_eq!(
        validate_delete_result_absent(
            BatchSuccess {
                result_revision: 12,
                results: OperationResultList::from_slice(&forbidden_delete_output),
            },
            1,
            0,
        ),
        Err(CodecError::InvalidFrame)
    );
}

#[test]
fn reorder_payloads_are_exact_target_plus_little_endian_index_vectors() {
    for (target, fixture_name) in [
        (
            ObjectReference::Object(0x0000_0002_0000_0001),
            "payload.reorder_object",
        ),
        (
            ObjectReference::BatchObject(7),
            "payload.reorder_batch_object",
        ),
    ] {
        let payload = ReorderPayload { target, index: 3 };
        let mut encoded = [0u8; 16];
        let length = encode_reorder_payload(payload, &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_reorder_payload(&encoded[..length]), Ok(payload));
        assert_eq!(
            decode_reorder_operation(OperationRef {
                opcode: opcode::REORDER,
                flags: 0,
                payload: &encoded[..length],
            }),
            Ok(payload)
        );
    }

    let maximum = ReorderPayload {
        target: ObjectReference::BatchObject(7),
        index: u32::MAX,
    };
    let mut encoded = [0u8; 16];
    let length = encode_reorder_payload(maximum, &mut encoded).unwrap();
    assert_eq!(decode_reorder_payload(&encoded[..length]), Ok(maximum));
}

#[test]
fn reorder_rejects_bad_lengths_context_and_output_records() {
    for malformed_target in [
        &[][..],
        &[ValueTag::Object as u8, 1][..],
        &[ValueTag::BatchObject as u8, 0, 0, 0, 0, 0, 0][..],
    ] {
        assert_eq!(
            decode_reorder_payload(malformed_target),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    let target = fixture("payload.delete_batch_object");
    for remainder in [
        &[][..],
        &[0][..],
        &[0, 0][..],
        &[0, 0, 0][..],
        &[0, 0, 0, 0, 0][..],
    ] {
        let mut malformed = target.clone();
        malformed.extend_from_slice(remainder);
        assert_eq!(
            decode_reorder_payload(&malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    assert_eq!(
        decode_reorder_payload(&[ValueTag::Bool as u8, 1, 0, 0, 0, 0]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    assert_eq!(
        encode_reorder_payload(
            ReorderPayload {
                target: ObjectReference::Object(1),
                index: 0,
            },
            &mut [0; 16],
        ),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        encode_reorder_payload(
            ReorderPayload {
                target: ObjectReference::BatchObject(7),
                index: 0,
            },
            &mut [0; 6],
        ),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.reorder_object");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::DELETE, 0),
        (opcode::REORDER, 1),
    ] {
        assert_eq!(
            decode_reorder_operation(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }

    let outputless = BatchSuccess {
        result_revision: 13,
        results: OperationResultList::from_slice(&[]),
    };
    let mut encoded = [0u8; 64];
    let length = encode_batch_success(outputless, 1, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.reorder_success"));
    let decoded = decode_batch_success(&encoded[..length], 1).unwrap();
    assert_eq!(validate_reorder_result_absent(decoded, 1, 0), Ok(()));

    let other_output = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let mixed = BatchSuccess {
        result_revision: 14,
        results: OperationResultList::from_slice(&other_output),
    };
    assert_eq!(validate_reorder_result_absent(mixed, 2, 0), Ok(()));
    assert_eq!(
        validate_reorder_result_absent(mixed, 2, 1),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        validate_reorder_result_absent(mixed, 2, 2),
        Err(CodecError::InvalidFrame)
    );

    let forbidden_reorder_output = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    }];
    assert_eq!(
        validate_reorder_result_absent(
            BatchSuccess {
                result_revision: 14,
                results: OperationResultList::from_slice(&forbidden_reorder_output),
            },
            1,
            0,
        ),
        Err(CodecError::InvalidFrame)
    );

    let range_error = FrameRef::Result(Completion {
        request_id: 1,
        status: CompletionStatus::Error(ErrorClass::Range),
        operation_index: Some(0),
        field_id: None,
        diagnostic: "index",
        payload: &[],
    });
    let length = encode_frame(MPY_V1, range_error, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("frame.reorder_range"));
    assert_eq!(decode_frame(&encoded[..length]).unwrap().frame, range_error);
}

#[test]
fn reparent_payloads_cover_all_contextual_reference_pairs() {
    let cases = [
        (
            ReparentPayload {
                target: ObjectReference::Object(0x0000_0002_0000_0001),
                new_parent: ObjectReference::Object(0x0000_0003_0000_0002),
                index: 5,
            },
            "payload.reparent_object_object",
            22,
        ),
        (
            ReparentPayload {
                target: ObjectReference::Object(0x0000_0002_0000_0001),
                new_parent: ObjectReference::BatchObject(7),
                index: 5,
            },
            "payload.reparent_object_batch",
            16,
        ),
        (
            ReparentPayload {
                target: ObjectReference::BatchObject(7),
                new_parent: ObjectReference::Object(0x0000_0003_0000_0002),
                index: 5,
            },
            "payload.reparent_batch_object",
            16,
        ),
        (
            ReparentPayload {
                target: ObjectReference::BatchObject(7),
                new_parent: ObjectReference::BatchObject(8),
                index: 5,
            },
            "payload.reparent_batch_batch",
            10,
        ),
    ];
    for (payload, fixture_name, expected_length) in cases {
        let mut encoded = [0u8; 32];
        let length = encode_reparent_payload(payload, &mut encoded).unwrap();
        assert_eq!(length, expected_length);
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_reparent_payload(&encoded[..length]), Ok(payload));
        assert_eq!(
            decode_reparent_operation(OperationRef {
                opcode: opcode::REPARENT,
                flags: 0,
                payload: &encoded[..length],
            }),
            Ok(payload)
        );
    }

    let maximum = ReparentPayload {
        target: ObjectReference::BatchObject(7),
        new_parent: ObjectReference::BatchObject(8),
        index: u32::MAX,
    };
    let mut encoded = [0u8; 16];
    let length = encode_reparent_payload(maximum, &mut encoded).unwrap();
    assert_eq!(decode_reparent_payload(&encoded[..length]), Ok(maximum));
}

#[test]
fn reparent_rejects_malformed_fields_in_target_first_order() {
    let target_type_mismatch_before_bad_parent = [
        ValueTag::Bool as u8,
        1,
        ValueTag::BatchObject as u8,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    assert_eq!(
        decode_reparent_payload(&target_type_mismatch_before_bad_parent),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    let target_invalid_before_parent_type = [
        ValueTag::BatchObject as u8,
        0,
        0,
        ValueTag::Bool as u8,
        1,
        0,
        0,
        0,
        0,
    ];
    assert_eq!(
        decode_reparent_payload(&target_invalid_before_parent_type),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    let parent_type_mismatch = [
        ValueTag::BatchObject as u8,
        7,
        0,
        ValueTag::Bool as u8,
        1,
        0,
        0,
        0,
        0,
    ];
    assert_eq!(
        decode_reparent_payload(&parent_type_mismatch),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    let parent_truncated = [ValueTag::BatchObject as u8, 7, 0, ValueTag::Object as u8, 1];
    assert_eq!(
        decode_reparent_payload(&parent_truncated),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    let pair = [
        ValueTag::BatchObject as u8,
        7,
        0,
        ValueTag::BatchObject as u8,
        8,
        0,
    ];
    for index_bytes in [
        &[][..],
        &[0][..],
        &[0, 0][..],
        &[0, 0, 0][..],
        &[0, 0, 0, 0, 0][..],
    ] {
        let mut malformed = pair.to_vec();
        malformed.extend_from_slice(index_bytes);
        assert_eq!(
            decode_reparent_payload(&malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }

    for payload in [
        ReparentPayload {
            target: ObjectReference::Object(1),
            new_parent: ObjectReference::BatchObject(8),
            index: 0,
        },
        ReparentPayload {
            target: ObjectReference::BatchObject(7),
            new_parent: ObjectReference::BatchObject(0),
            index: 0,
        },
    ] {
        assert_eq!(
            encode_reparent_payload(payload, &mut [0; 32]),
            Err(CodecError::InvalidFrame)
        );
    }
    assert_eq!(
        encode_reparent_payload(
            ReparentPayload {
                target: ObjectReference::BatchObject(7),
                new_parent: ObjectReference::BatchObject(8),
                index: 0,
            },
            &mut [0; 9],
        ),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.reparent_object_object");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::REORDER, 0),
        (opcode::REPARENT, 1),
    ] {
        assert_eq!(
            decode_reparent_operation(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
}

#[test]
fn reparent_is_outputless_and_errors_are_operation_attributed() {
    let outputless = BatchSuccess {
        result_revision: 15,
        results: OperationResultList::from_slice(&[]),
    };
    let mut encoded = [0u8; 128];
    let length = encode_batch_success(outputless, 1, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.reparent_success"));
    let decoded = decode_batch_success(&encoded[..length], 1).unwrap();
    assert_eq!(validate_reparent_result_absent(decoded, 1, 0), Ok(()));

    let other_output = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let mixed = BatchSuccess {
        result_revision: 16,
        results: OperationResultList::from_slice(&other_output),
    };
    assert_eq!(validate_reparent_result_absent(mixed, 2, 0), Ok(()));
    assert_eq!(
        validate_reparent_result_absent(mixed, 2, 1),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        validate_reparent_result_absent(mixed, 2, 2),
        Err(CodecError::InvalidFrame)
    );

    let forbidden_reparent_output = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    }];
    assert_eq!(
        validate_reparent_result_absent(
            BatchSuccess {
                result_revision: 16,
                results: OperationResultList::from_slice(&forbidden_reparent_output),
            },
            1,
            0,
        ),
        Err(CodecError::InvalidFrame)
    );

    for (status, diagnostic, fixture_name) in [
        (ErrorClass::Range, "index", "frame.reparent_range"),
        (
            ErrorClass::InvalidParent,
            "parent",
            "frame.reparent_invalid_parent",
        ),
        (ErrorClass::Capacity, "children", "frame.reparent_capacity"),
    ] {
        let error = FrameRef::Result(Completion {
            request_id: 1,
            status: CompletionStatus::Error(status),
            operation_index: Some(0),
            field_id: None,
            diagnostic,
            payload: &[],
        });
        let length = encode_frame(MPY_V1, error, &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_frame(&encoded[..length]).unwrap().frame, error);
    }
}

#[test]
fn promote_root_payloads_round_trip_stable_batch_and_empty_names() {
    let cases = [
        (
            PromoteRootPayload {
                target: ObjectReference::Object(0x0000_0002_0000_0001),
                name: "main",
                index: 2,
            },
            "payload.promote_root_object",
            21,
        ),
        (
            PromoteRootPayload {
                target: ObjectReference::BatchObject(7),
                name: "main",
                index: 2,
            },
            "payload.promote_root_batch_object",
            15,
        ),
        (
            PromoteRootPayload {
                target: ObjectReference::BatchObject(7),
                name: "",
                index: 0,
            },
            "payload.promote_root_empty_name",
            11,
        ),
    ];

    for (payload, fixture_name, expected_length) in cases {
        let mut encoded = [0u8; 32];
        let length = encode_promote_root_payload(payload, &mut encoded).unwrap();
        assert_eq!(length, expected_length);
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_promote_root_payload(&encoded[..length]), Ok(payload));
        assert_eq!(
            decode_promote_root_operation(OperationRef {
                opcode: opcode::PROMOTE_ROOT,
                flags: 0,
                payload: &encoded[..length],
            }),
            Ok(payload)
        );
        assert_eq!(
            decode_promote_root_operation_with_limits(
                OperationRef {
                    opcode: opcode::PROMOTE_ROOT,
                    flags: 0,
                    payload: &encoded[..length],
                },
                limits(),
            ),
            Ok(payload)
        );
    }

    let payload = fixture("payload.promote_root_batch_object");
    let decoded = decode_promote_root_payload(&payload).unwrap();
    assert_eq!(decoded.name.as_ptr(), payload[7..].as_ptr());
    assert_eq!(decoded.index, 2);

    let maximum = PromoteRootPayload {
        target: ObjectReference::BatchObject(7),
        name: "r",
        index: u32::MAX,
    };
    let mut encoded = [0u8; 16];
    let length = encode_promote_root_payload(maximum, &mut encoded).unwrap();
    assert_eq!(decode_promote_root_payload(&encoded[..length]), Ok(maximum));
}

#[test]
fn promote_root_rejects_malformed_fields_in_wire_order() {
    assert_eq!(
        decode_promote_root_payload(&[]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_promote_root_payload(&[ValueTag::Bool as u8, 1]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    assert_eq!(
        decode_promote_root_payload(&[0xff]),
        Err(ObjectReferenceError::Codec(
            CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::ValueTag,
                value: 0xff,
            }
        ))
    );

    let target = [ValueTag::BatchObject as u8, 7, 0];
    let mut truncated_name = target.to_vec();
    truncated_name.extend_from_slice(&4u32.to_le_bytes());
    truncated_name.push(b'm');
    assert_eq!(
        decode_promote_root_payload(&truncated_name),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    let mut invalid_utf8 = target.to_vec();
    invalid_utf8.extend_from_slice(&1u32.to_le_bytes());
    invalid_utf8.push(0xff);
    invalid_utf8.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        decode_promote_root_payload(&invalid_utf8),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    let mut prefix = target.to_vec();
    prefix.extend_from_slice(&1u32.to_le_bytes());
    prefix.push(b'r');
    for index_bytes in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 0][..]] {
        let mut malformed = prefix.clone();
        malformed.extend_from_slice(index_bytes);
        assert_eq!(
            decode_promote_root_payload(&malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
    let mut trailing = prefix;
    trailing.extend_from_slice(&0u32.to_le_bytes());
    trailing.push(0);
    assert_eq!(
        decode_promote_root_payload(&trailing),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );

    assert_eq!(
        encode_promote_root_payload(
            PromoteRootPayload {
                target: ObjectReference::Object(1),
                name: "root",
                index: 0,
            },
            &mut [0; 32],
        ),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        encode_promote_root_payload(
            PromoteRootPayload {
                target: ObjectReference::BatchObject(7),
                name: "root",
                index: 0,
            },
            &mut [0; 14],
        ),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.promote_root_batch_object");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::REPARENT, 0),
        (opcode::PROMOTE_ROOT, 1),
    ] {
        assert_eq!(
            decode_promote_root_operation(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
        assert_eq!(
            decode_promote_root_operation_with_limits(
                OperationRef {
                    opcode: operation_opcode,
                    flags,
                    payload: &valid,
                },
                limits(),
            ),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
}

#[test]
fn promote_root_enforces_text_limits_after_structural_validation() {
    let payload = PromoteRootPayload {
        target: ObjectReference::BatchObject(7),
        name: "main",
        index: 2,
    };
    let mut encoded = [0u8; 32];
    let length = encode_promote_root_payload(payload, &mut encoded).unwrap();
    let mut short_text = limits();
    short_text.max_text_bytes = 3;
    assert_eq!(
        encode_promote_root_payload_with_limits(payload, short_text, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_promote_root_payload_with_limits(&encoded[..length], short_text),
        Err(ObjectReferenceError::Codec(CodecError::LimitExceeded))
    );

    let unicode = PromoteRootPayload {
        target: ObjectReference::BatchObject(7),
        name: "μ",
        index: 0,
    };
    let mut one_byte = limits();
    one_byte.max_text_bytes = 1;
    assert_eq!(
        encode_promote_root_payload_with_limits(unicode, one_byte, &mut encoded),
        Err(CodecError::LimitExceeded),
        "max_text_bytes counts UTF-8 bytes"
    );

    let structurally_invalid = PromoteRootPayload {
        target: ObjectReference::Object(1),
        name: "main",
        index: 0,
    };
    assert_eq!(
        encode_promote_root_payload_with_limits(structurally_invalid, short_text, &mut encoded),
        Err(CodecError::InvalidFrame),
        "structural errors precede negotiated limits"
    );
    let mut malformed = fixture("payload.promote_root_batch_object").to_vec();
    malformed.push(0);
    assert_eq!(
        decode_promote_root_payload_with_limits(&malformed, short_text),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame)),
        "complete structural decoding precedes negotiated limits"
    );
}

#[test]
fn promote_root_is_outputless_and_errors_are_operation_attributed() {
    let outputless = BatchSuccess {
        result_revision: 17,
        results: OperationResultList::from_slice(&[]),
    };
    let mut encoded = [0u8; 128];
    let length = encode_batch_success(outputless, 1, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.promote_root_success"));
    let decoded = decode_batch_success(&encoded[..length], 1).unwrap();
    assert_eq!(validate_promote_root_result_absent(decoded, 1, 0), Ok(()));

    let other_output = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let mixed = BatchSuccess {
        result_revision: 18,
        results: OperationResultList::from_slice(&other_output),
    };
    assert_eq!(validate_promote_root_result_absent(mixed, 2, 0), Ok(()));
    assert_eq!(
        validate_promote_root_result_absent(mixed, 2, 1),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        validate_promote_root_result_absent(mixed, 2, 2),
        Err(CodecError::InvalidFrame)
    );

    let forbidden_output = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    }];
    assert_eq!(
        validate_promote_root_result_absent(
            BatchSuccess {
                result_revision: 18,
                results: OperationResultList::from_slice(&forbidden_output),
            },
            1,
            0,
        ),
        Err(CodecError::InvalidFrame)
    );

    for (status, diagnostic, fixture_name) in [
        (ErrorClass::Range, "index", "frame.promote_root_range"),
        (
            ErrorClass::InvalidParent,
            "name",
            "frame.promote_root_invalid_parent",
        ),
        (ErrorClass::Capacity, "roots", "frame.promote_root_capacity"),
    ] {
        let error = FrameRef::Result(Completion {
            request_id: 1,
            status: CompletionStatus::Error(status),
            operation_index: Some(0),
            field_id: None,
            diagnostic,
            payload: &[],
        });
        let length = encode_frame(MPY_V1, error, &mut encoded).unwrap();
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_frame(&encoded[..length]).unwrap().frame, error);
    }
}

#[test]
fn set_flag_payloads_freeze_ids_boolean_bytes_and_reference_forms() {
    let cases = [
        (
            SetFlagPayload {
                target: ObjectReference::Object(0x0000_0002_0000_0001),
                flag: RuntimeFlag::Hidden,
                enabled: false,
            },
            "payload.set_flag_hidden_object",
            11,
        ),
        (
            SetFlagPayload {
                target: ObjectReference::BatchObject(7),
                flag: RuntimeFlag::Enabled,
                enabled: true,
            },
            "payload.set_flag_enabled_batch_object",
            5,
        ),
        (
            SetFlagPayload {
                target: ObjectReference::BatchObject(7),
                flag: RuntimeFlag::Clickable,
                enabled: true,
            },
            "payload.set_flag_clickable_batch_object",
            5,
        ),
        (
            SetFlagPayload {
                target: ObjectReference::BatchObject(7),
                flag: RuntimeFlag::Focusable,
                enabled: false,
            },
            "payload.set_flag_focusable_batch_object",
            5,
        ),
    ];

    for (payload, fixture_name, expected_length) in cases {
        let mut encoded = [0u8; 16];
        let length = encode_set_flag_payload(payload, &mut encoded).unwrap();
        assert_eq!(length, expected_length);
        assert_eq!(&encoded[..length], fixture(fixture_name));
        assert_eq!(decode_set_flag_payload(&encoded[..length]), Ok(payload));
        assert_eq!(
            decode_set_flag_operation(OperationRef {
                opcode: opcode::SET_FLAG,
                flags: 0,
                payload: &encoded[..length],
            }),
            Ok(payload)
        );
    }
}

#[test]
fn set_flag_rejects_bad_target_flag_boolean_and_operation_envelope() {
    assert_eq!(
        decode_set_flag_payload(&[]),
        Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
    );
    assert_eq!(
        decode_set_flag_payload(&[ValueTag::Bool as u8, 1, 1, 1]),
        Err(ObjectReferenceError::TypeMismatch {
            actual: ValueTag::Bool,
        })
    );
    assert_eq!(
        decode_set_flag_payload(&[0xff, 1, 1]),
        Err(ObjectReferenceError::Codec(
            CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::ValueTag,
                value: 0xff,
            }
        ))
    );

    let target = [ValueTag::BatchObject as u8, 7, 0];
    for remainder in [&[][..], &[0][..], &[0, 1][..], &[1, 2][..], &[1, 1, 0][..]] {
        let mut malformed = target.to_vec();
        malformed.extend_from_slice(remainder);
        assert_eq!(
            decode_set_flag_payload(&malformed),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }

    let mut unsupported = target.to_vec();
    unsupported.extend_from_slice(&[5, 1]);
    assert_eq!(
        decode_set_flag_payload(&unsupported),
        Err(ObjectReferenceError::Codec(
            CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::RuntimeFlag,
                value: 5,
            }
        ))
    );

    assert_eq!(
        encode_set_flag_payload(
            SetFlagPayload {
                target: ObjectReference::Object(1),
                flag: RuntimeFlag::Hidden,
                enabled: true,
            },
            &mut [0; 16],
        ),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        encode_set_flag_payload(
            SetFlagPayload {
                target: ObjectReference::BatchObject(7),
                flag: RuntimeFlag::Hidden,
                enabled: true,
            },
            &mut [0; 4],
        ),
        Err(CodecError::BufferTooSmall)
    );

    let valid = fixture("payload.set_flag_hidden_object");
    for (operation_opcode, flags) in [
        (opcode::CREATE, 0),
        (opcode::SET_PROPERTIES, 0),
        (opcode::SET_FLAG, 1),
    ] {
        assert_eq!(
            decode_set_flag_operation(OperationRef {
                opcode: operation_opcode,
                flags,
                payload: &valid,
            }),
            Err(ObjectReferenceError::Codec(CodecError::InvalidFrame))
        );
    }
}

#[test]
fn set_flag_is_outputless_and_control_rejection_is_operation_attributed() {
    let outputless = BatchSuccess {
        result_revision: 19,
        results: OperationResultList::from_slice(&[]),
    };
    let mut encoded = [0u8; 128];
    let length = encode_batch_success(outputless, 1, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.set_flag_success"));
    let decoded = decode_batch_success(&encoded[..length], 1).unwrap();
    assert_eq!(validate_set_flag_result_absent(decoded, 1, 0), Ok(()));

    let other_output = [OperationResultRef {
        operation_index: 1,
        values: ValueList::from_slice(CREATE_RESULT_VALUES),
    }];
    let mixed = BatchSuccess {
        result_revision: 20,
        results: OperationResultList::from_slice(&other_output),
    };
    assert_eq!(validate_set_flag_result_absent(mixed, 2, 0), Ok(()));
    assert_eq!(
        validate_set_flag_result_absent(mixed, 2, 1),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        validate_set_flag_result_absent(mixed, 2, 2),
        Err(CodecError::InvalidFrame)
    );

    let forbidden_output = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    }];
    assert_eq!(
        validate_set_flag_result_absent(
            BatchSuccess {
                result_revision: 20,
                results: OperationResultList::from_slice(&forbidden_output),
            },
            1,
            0,
        ),
        Err(CodecError::InvalidFrame)
    );

    let unsupported = FrameRef::Result(Completion {
        request_id: 1,
        status: CompletionStatus::Error(ErrorClass::Unsupported),
        operation_index: Some(0),
        field_id: None,
        diagnostic: "control",
        payload: &[],
    });
    let length = encode_frame(MPY_V1, unsupported, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("frame.set_flag_unsupported"));
    assert_eq!(decode_frame(&encoded[..length]).unwrap().frame, unsupported);
}

#[test]
fn successful_batch_payload_matches_golden_bytes_and_correlates_results() {
    let success = BatchSuccess {
        result_revision: 9,
        results: OperationResultList::from_slice(BATCH_RESULTS),
    };
    let mut encoded = [0u8; 256];
    let length = encode_batch_success(success, 3, &mut encoded).unwrap();
    assert_eq!(&encoded[..length], fixture("payload.batch_success"));
    let decoded = decode_batch_success(&encoded[..length], 3).unwrap();
    assert_eq!(decoded, success);
    assert_eq!(decoded.results.iter().collect::<Vec<_>>(), BATCH_RESULTS);

    let empty = BatchSuccess {
        result_revision: 5,
        results: OperationResultList::from_slice(&[]),
    };
    let empty_length = encode_batch_success(empty, 3, &mut encoded).unwrap();
    assert_eq!(
        &encoded[..empty_length],
        fixture("payload.batch_success_empty")
    );
    assert_eq!(decode_batch_success(&encoded[..empty_length], 3), Ok(empty));
}

#[test]
fn typed_payloads_reject_malformed_order_counts_and_trailing_bytes() {
    let duplicate_fields = [
        FieldRef {
            id: 1,
            value: ValueRef::None,
        },
        FieldRef {
            id: 1,
            value: ValueRef::None,
        },
    ];
    assert_eq!(
        encode_field_list(FieldList::from_slice(&duplicate_fields), &mut [0; 32]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        encode_field_list_with_limit(FieldList::from_slice(&duplicate_fields), 1, &mut [0; 32],),
        Err(CodecError::InvalidFrame),
        "structural errors precede negotiated limits"
    );
    assert_eq!(
        decode_field_list(&[2, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_field_list(&[1, 0, 0, 0, 0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_field_list(&[2, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(
        decode_field_list(&[1, 0, 1, 0, 0, 0]),
        Err(CodecError::InvalidFrame)
    );
    assert_eq!(decode_field_list(&[0, 0, 0]), Err(CodecError::InvalidFrame));

    let malformed_values = [ValueRef::Object(1), ValueRef::None];
    assert_eq!(
        encode_value_list_with_limit(ValueList::from_slice(&malformed_values), 1, &mut [0; 32],),
        Err(CodecError::InvalidFrame),
        "structural errors precede negotiated limits"
    );
    assert_eq!(decode_value_list(&[1, 0]), Err(CodecError::InvalidFrame));
    assert_eq!(decode_value_list(&[0, 0, 0]), Err(CodecError::InvalidFrame));
}

#[test]
fn typed_payload_limits_apply_per_request_list_and_across_the_batch_result() {
    let mut encoded = [0u8; 256];
    let mut two_items = limits();
    two_items.max_items_per_command = 2;
    assert_eq!(
        encode_value_list_with_limits(ValueList::from_slice(LIST_VALUES), two_items, &mut encoded,),
        Err(CodecError::LimitExceeded)
    );
    let value_length = encode_value_list(ValueList::from_slice(LIST_VALUES), &mut encoded).unwrap();
    assert_eq!(
        decode_value_list_with_limits(&encoded[..value_length], two_items),
        Err(CodecError::LimitExceeded)
    );

    assert_eq!(
        encode_field_list_with_limits(FieldList::from_slice(TYPED_FIELDS), two_items, &mut encoded,),
        Err(CodecError::LimitExceeded)
    );
    let field_length =
        encode_field_list(FieldList::from_slice(TYPED_FIELDS), &mut encoded).unwrap();
    assert_eq!(
        decode_field_list_with_limits(&encoded[..field_length], two_items),
        Err(CodecError::LimitExceeded)
    );

    let success = BatchSuccess {
        result_revision: 9,
        results: OperationResultList::from_slice(BATCH_RESULTS),
    };
    let mut two_result_values = limits();
    two_result_values.max_values_per_result = 2;
    assert_eq!(
        encode_batch_success_with_limits(success, 3, two_result_values, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    let success_length = encode_batch_success(success, 3, &mut encoded).unwrap();
    assert_eq!(
        decode_batch_success_with_limits(&encoded[..success_length], 3, two_result_values,),
        Err(CodecError::LimitExceeded)
    );

    let mut tiny_payloads = limits();
    tiny_payloads.max_text_bytes = 1;
    tiny_payloads.max_byte_payload = 1;
    let text_values = [ValueRef::Text("Hi")];
    assert_eq!(
        encode_value_list_with_limits(
            ValueList::from_slice(&text_values),
            tiny_payloads,
            &mut encoded,
        ),
        Err(CodecError::LimitExceeded)
    );
    let text_length = encode_value_list(ValueList::from_slice(&text_values), &mut encoded).unwrap();
    assert_eq!(
        decode_value_list_with_limits(&encoded[..text_length], tiny_payloads),
        Err(CodecError::LimitExceeded)
    );

    let byte_fields = [FieldRef {
        id: 1,
        value: ValueRef::Bytes(&[1, 2]),
    }];
    assert_eq!(
        encode_field_list_with_limits(
            FieldList::from_slice(&byte_fields),
            tiny_payloads,
            &mut encoded,
        ),
        Err(CodecError::LimitExceeded)
    );

    let text_result = [OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(&text_values),
    }];
    assert_eq!(
        encode_batch_success_with_limits(
            BatchSuccess {
                result_revision: 1,
                results: OperationResultList::from_slice(&text_result),
            },
            1,
            tiny_payloads,
            &mut encoded,
        ),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn batch_success_rejects_zero_value_order_range_and_byte_failures() {
    let empty_values = OperationResultRef {
        operation_index: 0,
        values: ValueList::from_slice(&[]),
    };
    assert_eq!(
        encode_batch_success(
            BatchSuccess {
                result_revision: 1,
                results: OperationResultList::from_slice(&[empty_values]),
            },
            1,
            &mut [0; 32],
        ),
        Err(CodecError::InvalidFrame)
    );

    let duplicate_results = [BATCH_RESULTS[0], BATCH_RESULTS[0]];
    assert_eq!(
        encode_batch_success_with_limit(
            BatchSuccess {
                result_revision: 1,
                results: OperationResultList::from_slice(&duplicate_results),
            },
            3,
            1,
            &mut [0; 64],
        ),
        Err(CodecError::InvalidFrame),
        "structural errors precede aggregate result limits"
    );
    let decreasing_results = [BATCH_RESULTS[1], BATCH_RESULTS[0]];
    assert_eq!(
        encode_batch_success(
            BatchSuccess {
                result_revision: 1,
                results: OperationResultList::from_slice(&decreasing_results),
            },
            3,
            &mut [0; 64],
        ),
        Err(CodecError::InvalidFrame)
    );
    let out_of_range = OperationResultRef {
        operation_index: 3,
        values: ValueList::from_slice(MUTATION_RESULT_VALUES),
    };
    assert_eq!(
        encode_batch_success(
            BatchSuccess {
                result_revision: 1,
                results: OperationResultList::from_slice(&[out_of_range]),
            },
            3,
            &mut [0; 32],
        ),
        Err(CodecError::InvalidFrame)
    );

    let zero_value_record = [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    assert_eq!(
        decode_batch_success(&zero_value_record, 1),
        Err(CodecError::InvalidFrame)
    );
    let duplicate_wire = [
        0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0,
    ];
    assert_eq!(
        decode_batch_success(&duplicate_wire, 1),
        Err(CodecError::InvalidFrame)
    );
    let out_of_range_wire = [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 3, 0, 1, 0, 0];
    assert_eq!(
        decode_batch_success(&out_of_range_wire, 3),
        Err(CodecError::InvalidFrame)
    );

    let valid = fixture("payload.batch_success");
    assert_eq!(
        decode_batch_success(&valid[..valid.len() - 1], 3),
        Err(CodecError::InvalidFrame)
    );
    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        decode_batch_success(&trailing, 3),
        Err(CodecError::InvalidFrame)
    );
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

#[test]
fn eight_worst_case_mutation_target_prefixes_fit_the_floor_frame() {
    let mut payloads = [[0u8; 9]; 8];
    let mut lengths = [0usize; 8];
    for (index, output) in payloads.iter_mut().enumerate() {
        lengths[index] = encode_mutation_target_envelope(
            MutationTargetEnvelope {
                target: ObjectReference::Object((2u64 << 32) | u64::try_from(index + 1).unwrap()),
                remainder: &[],
            },
            output,
        )
        .unwrap();
        assert_eq!(lengths[index], 9);
    }
    let operations = [
        OperationRef {
            opcode: MUTATION_OPCODES[0],
            flags: 0,
            payload: &payloads[0][..lengths[0]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[1],
            flags: 0,
            payload: &payloads[1][..lengths[1]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[2],
            flags: 0,
            payload: &payloads[2][..lengths[2]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[3],
            flags: 0,
            payload: &payloads[3][..lengths[3]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[4],
            flags: 0,
            payload: &payloads[4][..lengths[4]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[5],
            flags: 0,
            payload: &payloads[5][..lengths[5]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[6],
            flags: 0,
            payload: &payloads[6][..lengths[6]],
        },
        OperationRef {
            opcode: MUTATION_OPCODES[7],
            flags: 0,
            payload: &payloads[7][..lengths[7]],
        },
    ];
    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    let mut encoded = [0u8; 256];
    let length = encode_frame_with_limits(MPY_V1, batch, limits(), &mut encoded).unwrap();
    assert_eq!(length, 206);

    let decoded = decode_frame_with_limits(&encoded[..length], limits()).unwrap();
    let FrameRef::Batch(decoded_batch) = decoded.frame else {
        panic!("expected Batch");
    };
    for operation in decoded_batch.operations.iter() {
        assert!(
            decode_mutation_operation_target(operation)
                .unwrap()
                .remainder
                .is_empty()
        );
    }

    let mut too_small = limits();
    too_small.max_frame_bytes = 205;
    assert_eq!(
        encode_frame_with_limits(MPY_V1, batch, too_small, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_frame_with_limits(&encoded[..length], too_small),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn eight_worst_case_reorder_operations_fit_the_floor_frame() {
    let mut payloads = [[0u8; 13]; 8];
    let mut lengths = [0usize; 8];
    for (index, output) in payloads.iter_mut().enumerate() {
        lengths[index] = encode_reorder_payload(
            ReorderPayload {
                target: ObjectReference::Object((2u64 << 32) | u64::try_from(index + 1).unwrap()),
                index: u32::try_from(index).unwrap(),
            },
            output,
        )
        .unwrap();
        assert_eq!(lengths[index], 13);
    }
    let operations = [
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[0][..lengths[0]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[1][..lengths[1]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[2][..lengths[2]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[3][..lengths[3]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[4][..lengths[4]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[5][..lengths[5]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[6][..lengths[6]],
        },
        OperationRef {
            opcode: opcode::REORDER,
            flags: 0,
            payload: &payloads[7][..lengths[7]],
        },
    ];
    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    let mut encoded = [0u8; 256];
    let length = encode_frame_with_limits(MPY_V1, batch, limits(), &mut encoded).unwrap();
    assert_eq!(length, 238);

    let decoded = decode_frame_with_limits(&encoded[..length], limits()).unwrap();
    let FrameRef::Batch(decoded_batch) = decoded.frame else {
        panic!("expected Batch");
    };
    for operation in decoded_batch.operations.iter() {
        assert!(decode_reorder_operation(operation).is_ok());
    }

    let mut too_small = limits();
    too_small.max_frame_bytes = 237;
    assert_eq!(
        encode_frame_with_limits(MPY_V1, batch, too_small, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_frame_with_limits(&encoded[..length], too_small),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn reparent_floor_proof_distinguishes_smallest_and_largest_reference_pairs() {
    let mut smallest_payload = [0u8; 10];
    let smallest_length = encode_reparent_payload(
        ReparentPayload {
            target: ObjectReference::BatchObject(7),
            new_parent: ObjectReference::BatchObject(8),
            index: 0,
        },
        &mut smallest_payload,
    )
    .unwrap();
    let smallest_operation = OperationRef {
        opcode: opcode::REPARENT,
        flags: 0,
        payload: &smallest_payload[..smallest_length],
    };
    let smallest_operations = [smallest_operation; 8];
    let smallest_batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&smallest_operations),
    });
    let mut floor_frame = [0u8; 256];
    let smallest_frame_length =
        encode_frame_with_limits(MPY_V1, smallest_batch, limits(), &mut floor_frame).unwrap();
    assert_eq!(smallest_frame_length, 214);

    let mut largest_payload = [0u8; 22];
    let largest_length = encode_reparent_payload(
        ReparentPayload {
            target: ObjectReference::Object(0x0000_0002_0000_0001),
            new_parent: ObjectReference::Object(0x0000_0003_0000_0002),
            index: 0,
        },
        &mut largest_payload,
    )
    .unwrap();
    let largest_operation = OperationRef {
        opcode: opcode::REPARENT,
        flags: 0,
        payload: &largest_payload[..largest_length],
    };
    let largest_operations = [largest_operation; 8];
    let largest_batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&largest_operations),
    });
    let mut oversized_frame = [0u8; 320];
    let largest_frame_length = encode_frame(MPY_V1, largest_batch, &mut oversized_frame).unwrap();
    assert_eq!(largest_frame_length, 310);
    assert_eq!(
        encode_frame_with_limits(MPY_V1, largest_batch, limits(), &mut oversized_frame),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn eight_largest_set_flag_operations_fit_the_floor_frame() {
    let mut payloads = [[0u8; 11]; 8];
    let mut lengths = [0usize; 8];
    for (index, output) in payloads.iter_mut().enumerate() {
        lengths[index] = encode_set_flag_payload(
            SetFlagPayload {
                target: ObjectReference::Object((2u64 << 32) | u64::try_from(index + 1).unwrap()),
                flag: RuntimeFlag::Focusable,
                enabled: index % 2 == 0,
            },
            output,
        )
        .unwrap();
        assert_eq!(lengths[index], 11);
    }
    let operations: [OperationRef<'_>; 8] = core::array::from_fn(|index| OperationRef {
        opcode: opcode::SET_FLAG,
        flags: 0,
        payload: &payloads[index][..lengths[index]],
    });
    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 0,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    let mut encoded = [0u8; 256];
    let length = encode_frame_with_limits(MPY_V1, batch, limits(), &mut encoded).unwrap();
    assert_eq!(length, 222);

    let decoded = decode_frame_with_limits(&encoded[..length], limits()).unwrap();
    let FrameRef::Batch(decoded_batch) = decoded.frame else {
        panic!("expected Batch");
    };
    for operation in decoded_batch.operations.iter() {
        assert!(decode_set_flag_operation(operation).is_ok());
    }

    let mut too_small = limits();
    too_small.max_frame_bytes = 221;
    assert_eq!(
        encode_frame_with_limits(MPY_V1, batch, too_small, &mut encoded),
        Err(CodecError::LimitExceeded)
    );
    assert_eq!(
        decode_frame_with_limits(&encoded[..length], too_small),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn promote_root_size_proof_respects_text_and_frame_floors_independently() {
    let name = "r".repeat(128);
    let payload = PromoteRootPayload {
        target: ObjectReference::Object(0x0000_0002_0000_0001),
        name: &name,
        index: 0,
    };
    let mut payload_bytes = [0u8; 145];
    let payload_length =
        encode_promote_root_payload_with_limits(payload, limits(), &mut payload_bytes).unwrap();
    assert_eq!(payload_length, 145);
    let operation = OperationRef {
        opcode: opcode::PROMOTE_ROOT,
        flags: 0,
        payload: &payload_bytes[..payload_length],
    };
    let single_operation = [operation];
    let single_batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 128,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&single_operation),
    });
    let mut encoded = [0u8; 320];
    assert_eq!(
        encode_frame_with_limits(MPY_V1, single_batch, limits(), &mut encoded),
        Ok(195)
    );

    let overlong_name = "r".repeat(129);
    assert_eq!(
        encode_promote_root_payload_with_limits(
            PromoteRootPayload {
                target: payload.target,
                name: &overlong_name,
                index: 0,
            },
            limits(),
            &mut [0; 146],
        ),
        Err(CodecError::LimitExceeded)
    );

    let names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let mut payloads = [[0u8; 18]; 8];
    let mut lengths = [0usize; 8];
    for (index, output) in payloads.iter_mut().enumerate() {
        lengths[index] = encode_promote_root_payload_with_limits(
            PromoteRootPayload {
                target: ObjectReference::Object((2u64 << 32) | u64::try_from(index + 1).unwrap()),
                name: names[index],
                index: u32::try_from(index).unwrap(),
            },
            limits(),
            output,
        )
        .unwrap();
        assert_eq!(lengths[index], 18);
    }
    let operations: [OperationRef<'_>; 8] = core::array::from_fn(|index| OperationRef {
        opcode: opcode::PROMOTE_ROOT,
        flags: 0,
        payload: &payloads[index][..lengths[index]],
    });
    let seven_batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 7,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations[..7]),
    });
    assert_eq!(
        encode_frame_with_limits(MPY_V1, seven_batch, limits(), &mut encoded),
        Ok(248)
    );

    let eight_batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 0,
            text_bytes: 8,
            resources: 0,
            result_bytes: 0,
        },
        operations: OperationList::from_slice(&operations),
    });
    assert_eq!(encode_frame(MPY_V1, eight_batch, &mut encoded), Ok(278));
    assert_eq!(
        encode_frame_with_limits(MPY_V1, eight_batch, limits(), &mut encoded),
        Err(CodecError::LimitExceeded)
    );
}

#[test]
fn eight_minimal_creates_and_one_object_results_fit_the_floor() {
    let root_names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let mut create_payloads = [[0u8; 32]; 8];
    let mut create_lengths = [0usize; 8];
    for (index, output) in create_payloads.iter_mut().enumerate() {
        create_lengths[index] = encode_create_payload_with_limits(
            CreatePayload {
                batch_ref: u16::try_from(index + 1).unwrap(),
                type_id: 1,
                destination: CreateDestinationRef::Root(root_names[index]),
                constructor_fields: FieldList::from_slice(&[]),
            },
            limits(),
            output,
        )
        .unwrap();
    }
    let operations = [
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[0][..create_lengths[0]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[1][..create_lengths[1]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[2][..create_lengths[2]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[3][..create_lengths[3]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[4][..create_lengths[4]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[5][..create_lengths[5]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[6][..create_lengths[6]],
        },
        OperationRef {
            opcode: opcode::CREATE,
            flags: 0,
            payload: &create_payloads[7][..create_lengths[7]],
        },
    ];
    let batch = FrameRef::Batch(Batch {
        stage_id: 1,
        request_id: 1,
        flags: 0,
        budget: BatchBudget {
            actors: 8,
            text_bytes: 8,
            resources: 0,
            result_bytes: 256,
        },
        operations: OperationList::from_slice(&operations),
    });
    let mut frame = [0u8; 256];
    let batch_length = encode_frame_with_limits(MPY_V1, batch, limits(), &mut frame).unwrap();
    assert!(batch_length <= frame.len());

    let result_value_0 = [ValueRef::Object(0x0000_0002_0000_0001)];
    let result_value_1 = [ValueRef::Object(0x0000_0002_0000_0002)];
    let result_value_2 = [ValueRef::Object(0x0000_0002_0000_0003)];
    let result_value_3 = [ValueRef::Object(0x0000_0002_0000_0004)];
    let result_value_4 = [ValueRef::Object(0x0000_0002_0000_0005)];
    let result_value_5 = [ValueRef::Object(0x0000_0002_0000_0006)];
    let result_value_6 = [ValueRef::Object(0x0000_0002_0000_0007)];
    let result_value_7 = [ValueRef::Object(0x0000_0002_0000_0008)];
    let result_records = [
        OperationResultRef {
            operation_index: 0,
            values: ValueList::from_slice(&result_value_0),
        },
        OperationResultRef {
            operation_index: 1,
            values: ValueList::from_slice(&result_value_1),
        },
        OperationResultRef {
            operation_index: 2,
            values: ValueList::from_slice(&result_value_2),
        },
        OperationResultRef {
            operation_index: 3,
            values: ValueList::from_slice(&result_value_3),
        },
        OperationResultRef {
            operation_index: 4,
            values: ValueList::from_slice(&result_value_4),
        },
        OperationResultRef {
            operation_index: 5,
            values: ValueList::from_slice(&result_value_5),
        },
        OperationResultRef {
            operation_index: 6,
            values: ValueList::from_slice(&result_value_6),
        },
        OperationResultRef {
            operation_index: 7,
            values: ValueList::from_slice(&result_value_7),
        },
    ];
    let success = BatchSuccess {
        result_revision: 1,
        results: OperationResultList::from_slice(&result_records),
    };
    let mut success_payload = [0u8; 256];
    let success_length =
        encode_batch_success_with_limits(success, 8, limits(), &mut success_payload).unwrap();
    let completion = FrameRef::Result(Completion {
        request_id: 1,
        status: CompletionStatus::Success,
        operation_index: None,
        field_id: None,
        diagnostic: "",
        payload: &success_payload[..success_length],
    });
    let result_length = encode_frame_with_limits(MPY_V1, completion, limits(), &mut frame).unwrap();
    assert!(result_length <= frame.len());

    let mut seven_values = limits();
    seven_values.max_values_per_result = 7;
    assert_eq!(
        encode_batch_success_with_limits(success, 8, seven_values, &mut success_payload),
        Err(CodecError::LimitExceeded)
    );
}
