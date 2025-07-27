use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
    WidgetNode,
};
use std::cell::RefCell;
use std::rc::Rc;

struct TestWidget {
    bounds: Rect,
    draw_counter: Rc<RefCell<usize>>,
    event_counter: Rc<RefCell<usize>>,
}

impl TestWidget {
    fn new(bounds: Rect, draw: Rc<RefCell<usize>>, event: Rc<RefCell<usize>>) -> Self {
        Self {
            bounds,
            draw_counter: draw,
            event_counter: event,
        }
    }
}

impl Widget for TestWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, _renderer: &mut dyn Renderer) {
        *self.draw_counter.borrow_mut() += 1;
    }
    fn handle_event(&mut self, _event: &Event) -> bool {
        *self.event_counter.borrow_mut() += 1;
        false
    }
}

struct DummyRenderer;

impl Renderer for DummyRenderer {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

#[test]
fn dispatch_and_draw_tree() {
    let draw_root = Rc::new(RefCell::new(0));
    let event_root = Rc::new(RefCell::new(0));
    let root_widget = Rc::new(RefCell::new(TestWidget::new(
        Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
        draw_root.clone(),
        event_root.clone(),
    )));
    let mut root = WidgetNode {
        widget: root_widget,
        children: Vec::new(),
    };

    let draw_child = Rc::new(RefCell::new(0));
    let event_child = Rc::new(RefCell::new(0));
    let child_widget = Rc::new(RefCell::new(TestWidget::new(
        Rect {
            x: 1,
            y: 1,
            width: 5,
            height: 5,
        },
        draw_child.clone(),
        event_child.clone(),
    )));
    root.children.push(WidgetNode {
        widget: child_widget,
        children: Vec::new(),
    });

    let mut renderer = DummyRenderer;
    root.draw(&mut renderer);
    assert_eq!(*draw_root.borrow(), 1);
    assert_eq!(*draw_child.borrow(), 1);

    assert!(!root.dispatch_event(&Event::Tick));
    assert_eq!(*event_root.borrow(), 1);
    assert_eq!(*event_child.borrow(), 1);
}

#[test]
fn tree_mutation_and_drop() {
    let widget = Rc::new(RefCell::new(TestWidget::new(
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        Rc::new(RefCell::new(0)),
        Rc::new(RefCell::new(0)),
    )));
    let mut root = WidgetNode {
        widget,
        children: Vec::new(),
    };

    // Push and pop children to test mutation APIs
    for _ in 0..5 {
        let child = WidgetNode {
            widget: Rc::new(RefCell::new(TestWidget::new(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                Rc::new(RefCell::new(0)),
                Rc::new(RefCell::new(0)),
            ))),
            children: Vec::new(),
        };
        root.children.push(child);
    }
    assert_eq!(root.children.len(), 5);
    root.children.pop();
    assert_eq!(root.children.len(), 4);

    // dropping root at end of scope should not panic
}

#[test]
fn stop_propagation() {
    struct StopWidget(Rc<RefCell<usize>>);

    impl Widget for StopWidget {
        fn bounds(&self) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }
        }

        fn draw(&self, _renderer: &mut dyn Renderer) {}

        fn handle_event(&mut self, _event: &Event) -> bool {
            *self.0.borrow_mut() += 1;
            true
        }
    }

    let counter_parent = Rc::new(RefCell::new(0));
    let counter_child = Rc::new(RefCell::new(0));

    let mut root = WidgetNode {
        widget: Rc::new(RefCell::new(StopWidget(counter_parent.clone()))),
        children: vec![WidgetNode {
            widget: Rc::new(RefCell::new(TestWidget::new(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                Rc::new(RefCell::new(0)),
                counter_child.clone(),
            ))),
            children: Vec::new(),
        }],
    };

    assert!(root.dispatch_event(&Event::Tick));
    assert_eq!(*counter_parent.borrow(), 1);
    // child should not receive the event
    assert_eq!(*counter_child.borrow(), 0);
}
