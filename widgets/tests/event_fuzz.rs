//! Fuzz tests for random widget events.
use rand::{Rng, SeedableRng, rngs::StdRng};
use rlvgl_core::{
    WidgetNode,
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::{button::Button, container::Container};
use std::cell::RefCell;
use std::rc::Rc;

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl<'a> Renderer for DisplayRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

fn make_button(x: i32, y: i32, counter: Rc<RefCell<usize>>) -> WidgetNode {
    let mut button = Button::new(
        "btn",
        Rect {
            x,
            y,
            width: 10,
            height: 10,
        },
    );
    let c = counter.clone();
    button.set_on_click(move |_: &mut Button| {
        *c.borrow_mut() += 1;
    });
    WidgetNode {
        widget: Rc::new(RefCell::new(button)),
        children: Vec::new(),
    }
}

#[test]
fn event_fuzz_random() {
    let mut display = BufferDisplay::new(64, 64);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let root_widget = Rc::new(RefCell::new(Container::new(Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    })));
    let mut root = WidgetNode {
        widget: root_widget,
        children: Vec::new(),
    };

    let counter1 = Rc::new(RefCell::new(0));
    let counter2 = Rc::new(RefCell::new(0));

    root.children.push(make_button(5, 5, counter1.clone()));
    root.children.push(make_button(20, 20, counter2.clone()));

    let mut rng = StdRng::seed_from_u64(0);
    for _ in 0..1000 {
        let event = match rng.gen_range(0..3) {
            0 => Event::PointerDown {
                x: rng.gen_range(0..64),
                y: rng.gen_range(0..64),
            },
            1 => Event::PointerUp {
                x: rng.gen_range(0..64),
                y: rng.gen_range(0..64),
            },
            _ => Event::PointerMove {
                x: rng.gen_range(0..64),
                y: rng.gen_range(0..64),
            },
        };
        root.dispatch_event(&event);
        root.draw(&mut renderer);
    }

    let total = *counter1.borrow() + *counter2.borrow();
    assert!(total <= 1000);
}
