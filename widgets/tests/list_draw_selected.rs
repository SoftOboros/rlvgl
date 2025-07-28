use platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::list::List;

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

#[test]

fn list_draw_selected_highlight() {
    let mut display = BufferDisplay::new(20, 16);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let mut list = List::new(Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 16,
    });
    list.add_item("a");
    list.add_item("b");

    let evt = Event::PointerUp { x: 5, y: 0 };
    assert!(list.handle_event(&evt));
    list.draw(&mut renderer);

    assert_eq!(list.selected(), Some(0));
    assert_eq!(list.items().len(), 2);
}
