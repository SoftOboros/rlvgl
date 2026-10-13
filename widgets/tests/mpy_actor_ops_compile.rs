//! Compile evidence for the production MPY-03 parallel actor-operations adapter.

use rlvgl_core::{actor::construct_native_actor, widget::Rect};
use rlvgl_widgets::{
    button::{Button, MPY_TYPE_ID as BUTTON_TYPE},
    container::{Container, MPY_TYPE_ID as CONTAINER_TYPE},
    label::{Label, MPY_TYPE_ID as LABEL_TYPE},
    list::{List, MPY_TYPE_ID as LIST_TYPE},
    slider::{MPY_TYPE_ID as SLIDER_TYPE, Slider},
};

const BOUNDS: Rect = Rect {
    x: 1,
    y: 2,
    width: 120,
    height: 40,
};

#[test]
fn production_actor_ops_erases_all_five_native_widget_types() {
    let actors = [
        construct_native_actor(CONTAINER_TYPE, Container::new(BOUNDS)),
        construct_native_actor(LABEL_TYPE, Label::new("label", BOUNDS)),
        construct_native_actor(BUTTON_TYPE, Button::new("button", BOUNDS)),
        construct_native_actor(SLIDER_TYPE, Slider::new(BOUNDS, 0, 100)),
        construct_native_actor(LIST_TYPE, List::new(BOUNDS)),
    ];
    let expected_types = [
        CONTAINER_TYPE,
        LABEL_TYPE,
        BUTTON_TYPE,
        SLIDER_TYPE,
        LIST_TYPE,
    ];

    for (actor, expected_type) in actors.iter().zip(expected_types) {
        assert_eq!(actor.type_id(), expected_type);
        assert_eq!(actor.actor_bounds(), BOUNDS);
        assert_eq!(actor.node().widget().borrow().bounds(), BOUNDS);
    }
}
