use platform::display::{BufferDisplay, DisplayDriver};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::container::Container;

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
fn container_layout_stress() {
    let mut display = BufferDisplay::new(20, 20);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };
    let mut rng = StdRng::seed_from_u64(0);
    for _ in 0..100 {
        let width = rng.gen_range(0..=20);
        let height = rng.gen_range(0..=20);
        let x = rng.gen_range(0..=20 - width);
        let y = rng.gen_range(0..=20 - height);
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        let container = Container::new(rect);
        container.draw(&mut renderer);
        let b = container.bounds();
        assert_eq!(b.x, rect.x);
        assert_eq!(b.y, rect.y);
        assert_eq!(b.width, rect.width);
        assert_eq!(b.height, rect.height);
    }
}
