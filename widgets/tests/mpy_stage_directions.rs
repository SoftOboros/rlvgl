//! MPY-04 conformance tests for atomic stage directions and snapshots.

use alloc::string::String;
use alloc::vec;

extern crate alloc;

use rlvgl_core::actor::{
    ConstructorInput, CreateDestination, RegistryError, RegistryLimits, StageId, StageRegistry,
    TypeDescriptor, ValueRef,
};
use rlvgl_core::direction::{
    ActorDirection, GeometryRole, OwnedValue, RequestedLayout, RuntimeFlag, SnapshotError,
    StageDirection,
};
use rlvgl_core::layout::{FlexConfig, ItemHints};
use rlvgl_core::object::{ObjectFlags, ObjectStates};
use rlvgl_core::widget::Rect;
use rlvgl_widgets::mpy::CATALOG;

const BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 320,
    height: 240,
};

fn limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 8,
        max_actors: 32,
        max_tree_depth: 8,
        max_children_per_actor: 8,
        max_text_bytes: 512,
        max_resources: 8,
    }
}

fn registry() -> StageRegistry {
    registry_with(limits())
}

fn registry_with(limits: RegistryLimits) -> StageRegistry {
    StageRegistry::new(StageId::new(7).unwrap(), &CATALOG, limits).unwrap()
}

fn descriptor(suffix: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(suffix))
        .unwrap()
}

fn constructor_field(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .id
}

fn property_id(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .properties
        .iter()
        .find(|property| property.name == name)
        .unwrap()
        .id
}

fn action_id(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .actions
        .iter()
        .find(|action| action.name == name)
        .unwrap()
        .id
}

fn bounds_input(descriptor: &TypeDescriptor) -> ConstructorInput<'static> {
    ConstructorInput {
        id: constructor_field(descriptor, "bounds"),
        value: ValueRef::Rect {
            x: BOUNDS.x,
            y: BOUNDS.y,
            width: BOUNDS.width,
            height: BOUNDS.height,
        },
    }
}

fn create_container(
    registry: &mut StageRegistry,
    destination: CreateDestination<'_>,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor("container::Container");
    registry
        .create(descriptor.type_id, destination, &[bounds_input(descriptor)])
        .unwrap()
}

fn create_label(
    registry: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
    text: &'static str,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor("label::Label");
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Child { parent },
            &[
                bounds_input(descriptor),
                ConstructorInput {
                    id: constructor_field(descriptor, "text"),
                    value: ValueRef::Text(text),
                },
            ],
        )
        .unwrap()
}

fn create_button(
    registry: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
    text: &'static str,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor("button::Button");
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Child { parent },
            &[
                bounds_input(descriptor),
                ConstructorInput {
                    id: constructor_field(descriptor, "text"),
                    value: ValueRef::Text(text),
                },
            ],
        )
        .unwrap()
}

fn create_slider(
    registry: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor("slider::Slider");
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Child { parent },
            &[
                bounds_input(descriptor),
                ConstructorInput {
                    id: constructor_field(descriptor, "min"),
                    value: ValueRef::I32(0),
                },
                ConstructorInput {
                    id: constructor_field(descriptor, "max"),
                    value: ValueRef::I32(100),
                },
                ConstructorInput {
                    id: constructor_field(descriptor, "value"),
                    value: ValueRef::I32(50),
                },
            ],
        )
        .unwrap()
}

fn create_list(
    registry: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor("list::List");
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Child { parent },
            &[bounds_input(descriptor)],
        )
        .unwrap()
}

#[test]
fn preparation_failure_rolls_back_every_actor_and_revision() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "old");
    let slider = create_slider(&mut registry, root);
    let label_text = property_id(descriptor("label::Label"), "text");
    let slider_min = property_id(descriptor("slider::Slider"), "min");
    let before = registry.revision();

    let error = registry
        .apply_batch(&[
            StageDirection::MutateActor {
                object_id: label,
                directions: vec![ActorDirection::SetProperty {
                    id: label_text,
                    value: OwnedValue::Text(String::from("new")),
                }],
            },
            StageDirection::MutateActor {
                object_id: slider,
                directions: vec![ActorDirection::SetProperty {
                    id: slider_min,
                    value: OwnedValue::I32(101),
                }],
            },
        ])
        .unwrap_err();

    assert_eq!(
        error,
        RegistryError::Range {
            field_id: property_id(descriptor("slider::Slider"), "max")
        }
    );
    assert_eq!(registry.revision(), before);
    assert_eq!(
        registry.property(label, label_text).unwrap(),
        OwnedValue::Text(String::from("old"))
    );
    assert_eq!(
        registry.property(slider, slider_min).unwrap(),
        OwnedValue::I32(0)
    );
}

#[test]
fn accepted_batch_commits_collective_slider_text_and_flags_once() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "old");
    let slider = create_slider(&mut registry, root);
    let label_descriptor = descriptor("label::Label");
    let slider_descriptor = descriptor("slider::Slider");
    let before = registry.revision();

    let revision = registry
        .apply_batch(&[
            StageDirection::MutateActor {
                object_id: label,
                directions: vec![ActorDirection::SetProperty {
                    id: property_id(label_descriptor, "text"),
                    value: OwnedValue::Text(String::from("committed")),
                }],
            },
            StageDirection::MutateActor {
                object_id: slider,
                directions: vec![
                    ActorDirection::SetProperty {
                        id: property_id(slider_descriptor, "min"),
                        value: OwnedValue::I32(200),
                    },
                    ActorDirection::SetProperty {
                        id: property_id(slider_descriptor, "max"),
                        value: OwnedValue::I32(300),
                    },
                    ActorDirection::SetProperty {
                        id: property_id(slider_descriptor, "value"),
                        value: OwnedValue::I32(250),
                    },
                ],
            },
            StageDirection::SetFlag {
                object_id: label,
                flag: RuntimeFlag::Hidden,
                enabled: true,
            },
        ])
        .unwrap();

    assert_eq!(revision.get(), before.get() + 1);
    assert_eq!(
        registry
            .property(slider, property_id(slider_descriptor, "min"))
            .unwrap(),
        OwnedValue::I32(200)
    );
    assert!(
        registry
            .node(label)
            .unwrap()
            .flags()
            .contains(ObjectFlags::HIDDEN)
    );
    assert!(!registry.last_invalidations().is_empty());
    assert!(
        registry
            .last_commit_effects()
            .contains(rlvgl_core::actor::MutationEffects::DRAW)
    );
}

#[test]
fn public_widget_borrow_is_a_structured_busy_failure() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "old");
    let property = property_id(descriptor("label::Label"), "text");
    let handle = registry.node(label).unwrap().widget().clone();
    let guard = handle.borrow_mut();
    let before = registry.revision();

    assert_eq!(
        registry.apply_batch(&[StageDirection::MutateActor {
            object_id: label,
            directions: vec![ActorDirection::SetProperty {
                id: property,
                value: OwnedValue::Text(String::from("blocked")),
            }],
        }]),
        Err(RegistryError::DispatchBusy)
    );
    assert_eq!(registry.revision(), before);
    drop(guard);
}

#[test]
fn requested_layout_roundtrips_and_computed_geometry_is_read_only() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "layout");
    let before = registry.revision();
    let hints = ItemHints {
        flex_grow: 2,
        ..ItemHints::default()
    };

    registry
        .apply_batch(&[
            StageDirection::SetRequestedLayout {
                object_id: root,
                layout: RequestedLayout::Flex(FlexConfig::default()),
            },
            StageDirection::SetRequestedLayout {
                object_id: label,
                layout: RequestedLayout::Item(hints.clone()),
            },
        ])
        .unwrap();
    assert_eq!(registry.revision().get(), before.get() + 1);
    assert_eq!(
        registry.requested_layout(label).unwrap(),
        RequestedLayout::Item(hints)
    );
    assert_eq!(
        registry.geometry(root).unwrap().layout_role,
        GeometryRole::Container
    );
    assert_eq!(
        registry.geometry(label).unwrap().layout_role,
        GeometryRole::Item
    );

    let revision = registry.revision();
    assert_eq!(
        registry.apply_batch(&[StageDirection::SetComputedGeometry {
            object_id: label,
            bounds: Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
        }]),
        Err(RegistryError::ReadOnly)
    );
    assert_eq!(registry.revision(), revision);
}

#[test]
fn tree_directions_preserve_identity_order_depth_policy_and_cycles() {
    let mut registry = registry();
    let left = create_container(&mut registry, CreateDestination::Root { name: "left" });
    let right = create_container(&mut registry, CreateDestination::Root { name: "right" });
    let panel = create_container(&mut registry, CreateDestination::Child { parent: left });
    let label = create_label(&mut registry, panel, "nested");
    let button = create_button(&mut registry, right, "button");
    let before = registry.revision();

    registry
        .apply_batch(&[
            StageDirection::Reparent {
                object_id: panel,
                new_parent: right,
                index: 0,
            },
            StageDirection::Reorder {
                object_id: button,
                index: 0,
            },
        ])
        .unwrap();
    assert_eq!(registry.revision().get(), before.get() + 1);
    assert_eq!(registry.children(right).unwrap(), [button, panel]);
    assert_eq!(registry.actor_info(panel).unwrap().depth, 2);
    assert_eq!(registry.actor_info(label).unwrap().depth, 3);

    let revision = registry.revision();
    assert_eq!(
        registry.apply_batch(&[StageDirection::Reparent {
            object_id: right,
            new_parent: panel,
            index: 0,
        }]),
        Err(RegistryError::InvalidParent)
    );
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.root_id("right"), Some(right));

    registry
        .apply_batch(&[StageDirection::PromoteRoot {
            object_id: panel,
            name: String::from("panel"),
            index: 1,
        }])
        .unwrap();
    assert_eq!(registry.root_id("panel"), Some(panel));
    assert_eq!(registry.actor_info(label).unwrap().depth, 2);
    assert_eq!(registry.actor_info(button).unwrap().parent, Some(right));

    let actors = registry.usage().actors;
    registry
        .apply_batch(&[StageDirection::Delete { object_id: panel }])
        .unwrap();
    assert_eq!(registry.usage().actors, actors - 2);
    assert!(matches!(
        registry.actor_info(label),
        Err(RegistryError::StaleObject { .. })
    ));
    assert_eq!(registry.root_id("panel"), None);
}

#[test]
fn tree_directions_validate_final_root_child_and_depth_capacity() {
    let mut constrained = limits();
    constrained.max_roots = 2;
    constrained.max_tree_depth = 3;
    let mut registry = registry_with(constrained);
    let left = create_container(&mut registry, CreateDestination::Root { name: "left" });
    let right = create_container(&mut registry, CreateDestination::Root { name: "right" });
    let panel = create_container(&mut registry, CreateDestination::Child { parent: left });
    let _label = create_label(&mut registry, panel, "nested");
    let nested = create_container(&mut registry, CreateDestination::Child { parent: right });
    let revision = registry.revision();

    assert_eq!(
        registry.apply_batch(&[StageDirection::PromoteRoot {
            object_id: panel,
            name: String::from("third"),
            index: 2,
        }]),
        Err(RegistryError::Capacity {
            kind: rlvgl_core::actor::CapacityKind::Roots
        })
    );
    assert_eq!(
        registry.apply_batch(&[StageDirection::Reparent {
            object_id: panel,
            new_parent: nested,
            index: 0,
        }]),
        Err(RegistryError::Capacity {
            kind: rlvgl_core::actor::CapacityKind::TreeDepth
        })
    );
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.actor_info(panel).unwrap().parent, Some(left));

    let mut child_limits = limits();
    child_limits.max_children_per_actor = 1;
    let mut registry = registry_with(child_limits);
    let left = create_container(&mut registry, CreateDestination::Root { name: "left" });
    let right = create_container(&mut registry, CreateDestination::Root { name: "right" });
    let panel = create_container(&mut registry, CreateDestination::Child { parent: left });
    let _occupied = create_container(&mut registry, CreateDestination::Child { parent: right });
    assert_eq!(
        registry.apply_batch(&[StageDirection::Reparent {
            object_id: panel,
            new_parent: right,
            index: 1,
        }]),
        Err(RegistryError::Capacity {
            kind: rlvgl_core::actor::CapacityKind::Children
        })
    );
}

#[test]
fn snapshots_are_preorder_bounded_busy_stale_and_explicitly_truncated() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "a long value");
    let button = create_button(&mut registry, root, "button");
    let second = create_container(&mut registry, CreateDestination::Root { name: "second" });
    let token = registry.snapshot_begin().unwrap();
    assert_eq!(registry.snapshot_begin(), Err(SnapshotError::Busy));

    let first = registry.snapshot_read(token, 2, 0).unwrap();
    assert_eq!(first.sequence, 0);
    assert_eq!(first.records[0].object_id, root);
    assert_eq!(first.records[1].object_id, label);
    assert!(first.records[1].truncated);
    assert!(!first.ended);

    registry
        .apply_batch(&[StageDirection::SetFlag {
            object_id: button,
            flag: RuntimeFlag::Clickable,
            enabled: true,
        }])
        .unwrap();
    assert!(matches!(
        registry.snapshot_read(token, 2, 16),
        Err(SnapshotError::Stale { .. })
    ));

    let token = registry.snapshot_begin().unwrap();
    let mut ids = Vec::new();
    loop {
        let page = registry.snapshot_read(token, 1, 64).unwrap();
        ids.extend(page.records.into_iter().map(|record| record.object_id));
        if page.ended {
            break;
        }
    }
    assert_eq!(ids, [root, label, button, second]);
}

#[test]
fn list_actions_prepare_transactionally_and_property_matrix_is_descriptor_driven() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let list = create_list(&mut registry, root);
    let label = create_label(&mut registry, root, "label");
    let button = create_button(&mut registry, root, "button");
    let slider = create_slider(&mut registry, root);
    let list_descriptor = descriptor("list::List");
    let count = property_id(list_descriptor, "item_count");

    registry
        .apply_batch(&[StageDirection::MutateActor {
            object_id: list,
            directions: vec![
                ActorDirection::InvokeAction {
                    id: action_id(list_descriptor, "append"),
                    arguments: vec![OwnedValue::Text(String::from("first"))],
                },
                ActorDirection::InvokeAction {
                    id: action_id(list_descriptor, "select"),
                    arguments: vec![OwnedValue::U32(0)],
                },
            ],
        }])
        .unwrap();
    assert_eq!(registry.property(list, count).unwrap(), OwnedValue::U32(1));

    let revision = registry.revision();
    assert!(matches!(
        registry.apply_batch(&[StageDirection::MutateActor {
            object_id: list,
            directions: vec![
                ActorDirection::InvokeAction {
                    id: action_id(list_descriptor, "append"),
                    arguments: vec![OwnedValue::Text(String::from("rolled back"))],
                },
                ActorDirection::InvokeAction {
                    id: action_id(list_descriptor, "remove"),
                    arguments: vec![OwnedValue::U32(99)],
                },
            ],
        }]),
        Err(RegistryError::Range { .. })
    ));
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.property(list, count).unwrap(), OwnedValue::U32(1));

    assert_eq!(
        registry.apply_batch(&[StageDirection::MutateActor {
            object_id: list,
            directions: vec![ActorDirection::SetProperty {
                id: count,
                value: OwnedValue::U32(2),
            }],
        }]),
        Err(RegistryError::ReadOnly)
    );
    assert_eq!(
        registry.apply_batch(&[StageDirection::SetFlag {
            object_id: root,
            flag: RuntimeFlag::Focusable,
            enabled: true,
        }]),
        Err(RegistryError::Unsupported)
    );
    assert_eq!(registry.node(root).unwrap().states(), ObjectStates::DEFAULT);

    let revision = registry.revision();
    for object_id in [root, label, button, slider, list] {
        assert_eq!(
            registry.apply_batch(&[StageDirection::MutateActor {
                object_id,
                directions: vec![ActorDirection::SetProperty {
                    id: 0xffff,
                    value: OwnedValue::I32(0),
                }],
            }]),
            Err(RegistryError::UnknownProperty {
                property_id: 0xffff
            })
        );
    }
    assert_eq!(registry.revision(), revision);
    assert_eq!(
        registry.apply_batch(&[StageDirection::MutateActor {
            object_id: label,
            directions: vec![ActorDirection::SetProperty {
                id: property_id(descriptor("label::Label"), "text"),
                value: OwnedValue::I32(7),
            }],
        }]),
        Err(RegistryError::TypeMismatch {
            field_id: property_id(descriptor("label::Label"), "text"),
            expected: rlvgl_core::actor::ValueTag::Text,
            actual: rlvgl_core::actor::ValueTag::I32,
        })
    );
    assert_eq!(
        registry.apply_batch(&[StageDirection::SetLocalStyle {
            object_id: button,
            part_id: 0,
            state_mask: 0,
            property_id: 1,
            value: OwnedValue::Color(0xff00_0000),
        }]),
        Err(RegistryError::Unsupported)
    );
}
