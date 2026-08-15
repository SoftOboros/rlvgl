//! Compile evidence for the additive MPY-03 parallel actor-operations adapter.

use std::{boxed::Box, cell::RefCell, rc::Rc, string::String};

use rlvgl_core::{
    object::ObjectNode,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{
    button::Button, container::Container, label::Label, list::List, slider::Slider,
};

const BOUNDS: Rect = Rect {
    x: 1,
    y: 2,
    width: 120,
    height: 40,
};

#[derive(Debug, PartialEq, Eq)]
enum NeutralValue {
    Int(i32),
    Text(String),
    Count(usize),
}

trait ActorOpsProbe {
    fn invoke(&self, value: NeutralValue) -> NeutralValue;
}

type ActorInvoke<T> = fn(&mut T, NeutralValue) -> NeutralValue;

struct TypedActorOps<T> {
    actor: Rc<RefCell<T>>,
    invoke: ActorInvoke<T>,
}

impl<T> ActorOpsProbe for TypedActorOps<T> {
    fn invoke(&self, value: NeutralValue) -> NeutralValue {
        (self.invoke)(&mut self.actor.borrow_mut(), value)
    }
}

struct ActorPair {
    node: ObjectNode,
    ops: Box<dyn ActorOpsProbe>,
}

fn erase_actor<T>(actor: T, invoke: ActorInvoke<T>) -> ActorPair
where
    T: Widget + 'static,
{
    let typed = Rc::new(RefCell::new(actor));
    let erased: Rc<RefCell<dyn Widget>> = typed.clone();

    // A count of two proves the typed adapter and erased ObjectNode handle
    // retain the same allocation rather than duplicate actor state.
    assert_eq!(Rc::strong_count(&typed), 2);
    assert_eq!(Rc::strong_count(&erased), 2);

    ActorPair {
        node: ObjectNode::new(erased),
        ops: Box::new(TypedActorOps {
            actor: typed,
            invoke,
        }),
    }
}

fn container_op(actor: &mut Container, value: NeutralValue) -> NeutralValue {
    let NeutralValue::Int(radius) = value else {
        panic!("container probe requires Int");
    };
    actor.style.radius = radius as u8;
    NeutralValue::Int(i32::from(actor.style.radius))
}

fn label_op(actor: &mut Label, value: NeutralValue) -> NeutralValue {
    let NeutralValue::Text(text) = value else {
        panic!("label probe requires Text");
    };
    actor.set_text(text);
    NeutralValue::Text(String::from(actor.text()))
}

fn button_op(actor: &mut Button, value: NeutralValue) -> NeutralValue {
    let NeutralValue::Text(text) = value else {
        panic!("button probe requires Text");
    };
    actor.set_text(text);
    NeutralValue::Text(String::from(actor.text()))
}

fn slider_op(actor: &mut Slider, value: NeutralValue) -> NeutralValue {
    let NeutralValue::Int(value) = value else {
        panic!("slider probe requires Int");
    };
    actor.set_value(value);
    NeutralValue::Int(actor.value())
}

fn list_op(actor: &mut List, value: NeutralValue) -> NeutralValue {
    let NeutralValue::Text(text) = value else {
        panic!("list probe requires Text");
    };
    actor.add_item(text);
    NeutralValue::Count(actor.items().len())
}

#[test]
fn parallel_actor_ops_shares_all_five_native_widget_handles() {
    let actors = [
        erase_actor(Container::new(BOUNDS), container_op),
        erase_actor(Label::new("before", BOUNDS), label_op),
        erase_actor(Button::new("before", BOUNDS), button_op),
        erase_actor(Slider::new(BOUNDS, 0, 100), slider_op),
        erase_actor(List::new(BOUNDS), list_op),
    ];

    assert_eq!(
        actors[0].ops.invoke(NeutralValue::Int(7)),
        NeutralValue::Int(7)
    );
    assert_eq!(
        actors[1]
            .ops
            .invoke(NeutralValue::Text(String::from("label"))),
        NeutralValue::Text(String::from("label"))
    );
    assert_eq!(
        actors[2]
            .ops
            .invoke(NeutralValue::Text(String::from("button"))),
        NeutralValue::Text(String::from("button"))
    );
    assert_eq!(
        actors[3].ops.invoke(NeutralValue::Int(75)),
        NeutralValue::Int(75)
    );
    assert_eq!(
        actors[4]
            .ops
            .invoke(NeutralValue::Text(String::from("item"))),
        NeutralValue::Count(1)
    );

    for actor in &actors {
        assert_eq!(actor.node.widget().borrow().bounds(), BOUNDS);
    }
}
