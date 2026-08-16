//! MPY-03 production conformance tests for the descriptor catalog and Stage Registry.

use rlvgl_core::actor::{
    CapacityKind, ConstructorInput, CreateDestination, RegistryError, RegistryLimits, StageId,
    StageRegistry, TypeDescriptor, TypeId, ValueRef, ValueTag,
};
use rlvgl_core::widget::Rect;
use rlvgl_widgets::mpy::CATALOG;

const ROOT_BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 320,
    height: 240,
};

fn limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 2,
        max_actors: 16,
        max_tree_depth: 8,
        max_children_per_actor: 8,
        max_text_bytes: 256,
        max_resources: 8,
    }
}

fn registry_with(limits: RegistryLimits) -> StageRegistry {
    StageRegistry::new(StageId::new(1).unwrap(), &CATALOG, limits).unwrap()
}

fn descriptor(name: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(name))
        .unwrap()
}

fn field(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .id
}

fn rect_input(id: u32, rect: Rect) -> ConstructorInput<'static> {
    ConstructorInput {
        id,
        value: ValueRef::Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        },
    }
}

fn create_container(
    registry: &mut StageRegistry,
    destination: CreateDestination<'_>,
    bounds: Rect,
) -> rlvgl_core::actor::ObjectId {
    let actor = descriptor("container::Container");
    registry
        .create(
            actor.type_id,
            destination,
            &[rect_input(field(actor, "bounds"), bounds)],
        )
        .unwrap()
}

#[test]
fn catalog_enumerates_five_actor_local_schemas() {
    let registry = registry_with(limits());
    let names: Vec<_> = registry
        .catalog()
        .iter()
        .map(|descriptor| descriptor.stable_name)
        .collect();

    assert_eq!(
        names,
        [
            "rlvgl_widgets::container::Container",
            "rlvgl_widgets::label::Label",
            "rlvgl_widgets::button::Button",
            "rlvgl_widgets::slider::Slider",
            "rlvgl_widgets::list::List",
        ]
    );
    for descriptor in registry.catalog() {
        assert_ne!(descriptor.type_id.get(), 0);
        assert!(!descriptor.constructor_fields.is_empty());
        assert!(
            descriptor
                .targets
                .contains(rlvgl_core::actor::TargetSet::ALL)
        );
        let expected = match descriptor.stable_name.rsplit("::").next().unwrap() {
            "Container" => (1, 0, 0, 0),
            "Label" => (1, 1, 0, 0),
            "Button" => (2, 1, 0, 1),
            "Slider" => (2, 3, 0, 1),
            "List" => (2, 1, 5, 1),
            _ => unreachable!(),
        };
        assert_eq!(descriptor.schema_revision, expected.0);
        assert_eq!(descriptor.properties.len(), expected.1);
        assert_eq!(descriptor.actions.len(), expected.2);
        assert_eq!(descriptor.events.len(), expected.3);
    }
}

#[test]
fn generic_create_constructs_and_resolves_all_five_actors() {
    let mut registry = registry_with(limits());
    let root = create_container(
        &mut registry,
        CreateDestination::Root { name: "main" },
        ROOT_BOUNDS,
    );

    let label = descriptor("label::Label");
    let label_bounds = Rect {
        x: 10,
        y: 10,
        width: 100,
        height: 20,
    };
    let label_id = registry
        .create(
            label.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(label, "bounds"), label_bounds),
                ConstructorInput {
                    id: field(label, "text"),
                    value: ValueRef::Text("status"),
                },
            ],
        )
        .unwrap();

    let button = descriptor("button::Button");
    let button_id = registry
        .create(
            button.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(button, "bounds"), ROOT_BOUNDS),
                ConstructorInput {
                    id: field(button, "text"),
                    value: ValueRef::Text("Go"),
                },
            ],
        )
        .unwrap();

    let slider = descriptor("slider::Slider");
    let slider_id = registry
        .create(
            slider.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(slider, "bounds"), ROOT_BOUNDS),
                ConstructorInput {
                    id: field(slider, "min"),
                    value: ValueRef::I32(0),
                },
                ConstructorInput {
                    id: field(slider, "max"),
                    value: ValueRef::I32(100),
                },
                ConstructorInput {
                    id: field(slider, "value"),
                    value: ValueRef::I32(75),
                },
            ],
        )
        .unwrap();

    let list = descriptor("list::List");
    let list_id = registry
        .create(
            list.type_id,
            CreateDestination::Child { parent: root },
            &[rect_input(field(list, "bounds"), ROOT_BOUNDS)],
        )
        .unwrap();

    assert_eq!(registry.root_id("main"), Some(root));
    assert_eq!(registry.node(root).unwrap().children().len(), 4);
    assert_eq!(registry.actor_info(label_id).unwrap().parent, Some(root));
    assert_eq!(registry.actor_info(label_id).unwrap().depth, 2);
    assert_eq!(registry.actor_bounds(label_id).unwrap(), label_bounds);
    assert_eq!(
        registry.actor_info(button_id).unwrap().type_id,
        button.type_id
    );
    assert_eq!(
        registry.actor_info(slider_id).unwrap().type_id,
        slider.type_id
    );
    assert_eq!(registry.actor_info(list_id).unwrap().type_id, list.type_id);
    assert_eq!(
        registry
            .node(label_id)
            .unwrap()
            .actor_identity()
            .unwrap()
            .object_id,
        label_id
    );
    assert_eq!(registry.usage().actors, 5);
    assert_eq!(registry.usage().roots, 1);
    assert_eq!(registry.usage().text_bytes, 12);
}

#[test]
fn create_rejects_schema_parent_and_constructor_failures_before_publication() {
    let mut registry = registry_with(limits());
    let label = descriptor("label::Label");
    let bounds_field = field(label, "bounds");
    let text_field = field(label, "text");

    assert_eq!(
        registry.create(
            label.type_id,
            CreateDestination::Root { name: "bad" },
            &[
                rect_input(bounds_field, ROOT_BOUNDS),
                ConstructorInput {
                    id: text_field,
                    value: ValueRef::Text("not-a-root"),
                },
            ],
        ),
        Err(RegistryError::InvalidParent)
    );
    assert_eq!(
        registry.create(
            label.type_id,
            CreateDestination::Root { name: "missing" },
            &[rect_input(bounds_field, ROOT_BOUNDS)],
        ),
        Err(RegistryError::MissingField {
            field_id: text_field
        })
    );
    assert_eq!(
        registry.create(
            label.type_id,
            CreateDestination::Root { name: "wrong" },
            &[
                rect_input(bounds_field, ROOT_BOUNDS),
                ConstructorInput {
                    id: text_field,
                    value: ValueRef::I32(3),
                },
            ],
        ),
        Err(RegistryError::TypeMismatch {
            field_id: text_field,
            expected: ValueTag::Text,
            actual: ValueTag::I32,
        })
    );
    assert_eq!(
        registry.create(
            label.type_id,
            CreateDestination::Root { name: "duplicate" },
            &[
                rect_input(bounds_field, ROOT_BOUNDS),
                rect_input(bounds_field, ROOT_BOUNDS),
                ConstructorInput {
                    id: text_field,
                    value: ValueRef::Text("x"),
                },
            ],
        ),
        Err(RegistryError::DuplicateField {
            field_id: bounds_field
        })
    );
    assert_eq!(
        registry.create(
            TypeId::registered(0x00ff_ffff),
            CreateDestination::Root { name: "unknown" },
            &[],
        ),
        Err(RegistryError::UnknownType {
            type_id: TypeId::registered(0x00ff_ffff)
        })
    );

    let root = create_container(
        &mut registry,
        CreateDestination::Root { name: "main" },
        ROOT_BOUNDS,
    );
    let usage = registry.usage();
    let slider = descriptor("slider::Slider");
    assert_eq!(
        registry.create(
            slider.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(slider, "bounds"), ROOT_BOUNDS),
                ConstructorInput {
                    id: field(slider, "min"),
                    value: ValueRef::I32(10),
                },
                ConstructorInput {
                    id: field(slider, "max"),
                    value: ValueRef::I32(5),
                },
            ],
        ),
        Err(RegistryError::Range {
            field_id: field(slider, "max")
        })
    );
    assert_eq!(registry.usage(), usage);
    assert!(registry.node(root).unwrap().children().is_empty());
}

#[test]
fn capacity_failures_leave_the_registry_unchanged() {
    let mut constrained = limits();
    constrained.max_roots = 1;
    constrained.max_actors = 4;
    constrained.max_tree_depth = 2;
    constrained.max_children_per_actor = 1;
    constrained.max_text_bytes = 8;
    let mut registry = registry_with(constrained);
    let root = create_container(
        &mut registry,
        CreateDestination::Root { name: "r" },
        ROOT_BOUNDS,
    );
    let child = create_container(
        &mut registry,
        CreateDestination::Child { parent: root },
        ROOT_BOUNDS,
    );
    let usage = registry.usage();

    assert_eq!(
        registry.create(
            descriptor("list::List").type_id,
            CreateDestination::Child { parent: root },
            &[rect_input(
                field(descriptor("list::List"), "bounds"),
                ROOT_BOUNDS,
            )],
        ),
        Err(RegistryError::Capacity {
            kind: CapacityKind::Children
        })
    );
    assert_eq!(
        registry.create(
            descriptor("list::List").type_id,
            CreateDestination::Child { parent: child },
            &[rect_input(
                field(descriptor("list::List"), "bounds"),
                ROOT_BOUNDS,
            )],
        ),
        Err(RegistryError::Capacity {
            kind: CapacityKind::TreeDepth
        })
    );

    assert_eq!(registry.usage(), usage);
    assert_eq!(registry.node(root).unwrap().children().len(), 1);

    let mut text_limits = limits();
    text_limits.max_text_bytes = 4;
    let mut text_registry = registry_with(text_limits);
    let text_root = create_container(
        &mut text_registry,
        CreateDestination::Root { name: "r" },
        ROOT_BOUNDS,
    );
    let text_usage = text_registry.usage();
    let label = descriptor("label::Label");
    assert_eq!(
        text_registry.create(
            label.type_id,
            CreateDestination::Child { parent: text_root },
            &[
                rect_input(field(label, "bounds"), ROOT_BOUNDS),
                ConstructorInput {
                    id: field(label, "text"),
                    value: ValueRef::Text("long"),
                },
            ],
        ),
        Err(RegistryError::Capacity {
            kind: CapacityKind::TextBytes
        })
    );
    assert_eq!(text_registry.usage(), text_usage);
    assert!(text_registry.node(text_root).unwrap().children().is_empty());
}

#[test]
fn subtree_delete_advances_generations_and_preserves_unrelated_actors() {
    let mut registry = registry_with(limits());
    let root = create_container(
        &mut registry,
        CreateDestination::Root { name: "main" },
        ROOT_BOUNDS,
    );
    let child = create_container(
        &mut registry,
        CreateDestination::Child { parent: root },
        ROOT_BOUNDS,
    );
    let label = descriptor("label::Label");
    let grandchild = registry
        .create(
            label.type_id,
            CreateDestination::Child { parent: child },
            &[
                rect_input(field(label, "bounds"), ROOT_BOUNDS),
                ConstructorInput {
                    id: field(label, "text"),
                    value: ValueRef::Text("nested"),
                },
            ],
        )
        .unwrap();
    let unrelated = create_container(
        &mut registry,
        CreateDestination::Root { name: "other" },
        ROOT_BOUNDS,
    );

    assert_eq!(registry.delete(child).unwrap(), 2);
    assert_eq!(
        registry.actor_info(child),
        Err(RegistryError::StaleObject { object_id: child })
    );
    assert_eq!(
        registry.actor_info(grandchild),
        Err(RegistryError::StaleObject {
            object_id: grandchild
        })
    );
    assert!(registry.actor_info(root).is_ok());
    assert!(registry.actor_info(unrelated).is_ok());

    let replacement = create_container(
        &mut registry,
        CreateDestination::Child { parent: root },
        ROOT_BOUNDS,
    );
    assert_eq!(replacement.slot(), child.slot());
    assert_eq!(replacement.generation(), child.generation() + 1);
    assert_ne!(replacement, child);

    assert_eq!(registry.teardown().unwrap(), 3);
    assert_eq!(
        registry.actor_info(unrelated),
        Err(RegistryError::StageClosed)
    );
    assert_eq!(registry.usage().actors, 0);
    assert_eq!(
        registry.create(
            descriptor("container::Container").type_id,
            CreateDestination::Root { name: "closed" },
            &[rect_input(
                field(descriptor("container::Container"), "bounds"),
                ROOT_BOUNDS,
            )],
        ),
        Err(RegistryError::StageClosed)
    );
}
