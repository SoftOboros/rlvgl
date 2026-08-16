// mpy_v1_golden.rs - Conformance tests for committed MPY v1 canonical byte vectors.

use rlvgl_api::protocol::{
    Batch, BatchBudget, BatchSuccess, Capabilities, CodecError, Command, Completion,
    CompletionStatus, Cue, DiscriminantDomain, ErrorClass, FieldList, FieldRef, FrameRef, Hello,
    Limits, MPY_V1, ObjectReference, ObjectReferenceError, OpcodeList, OperationList, OperationRef,
    OperationResultList, OperationResultRef, ProtocolVersion, RuntimeNotice, ValueList, ValueRef,
    ValueTag, decode_batch_success, decode_batch_success_with_limits, decode_field_list,
    decode_field_list_with_limits, decode_frame, decode_frame_with_limits, decode_object_reference,
    decode_operation_list, decode_operation_list_with_limit, decode_value, decode_value_list,
    decode_value_list_with_limits, encode_batch_success, encode_batch_success_with_limit,
    encode_batch_success_with_limits, encode_field_list, encode_field_list_with_limit,
    encode_field_list_with_limits, encode_frame, encode_frame_with_limits, encode_object_reference,
    encode_operation_list, encode_operation_list_with_limit, encode_value, encode_value_list,
    encode_value_list_with_limit, encode_value_list_with_limits, opcode,
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
