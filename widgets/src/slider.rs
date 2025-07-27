use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

pub struct Slider {
    bounds: Rect,
    pub style: Style,
    pub knob_color: Color,
    min: i32,
    max: i32,
    value: i32,
}

impl Slider {
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            bounds,
            style: Style::default(),
            knob_color: Color(0, 0, 0),
            min,
            max,
            value: min,
        }
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn set_value(&mut self, val: i32) {
        self.value = val.clamp(self.min, self.max);
    }

    fn position_from_value(&self) -> i32 {
        let range = self.max - self.min;
        if range == 0 {
            return self.bounds.x;
        }
        let ratio = (self.value - self.min) as f32 / range as f32;
        self.bounds.x + (ratio * self.bounds.width as f32) as i32
    }
}

impl Widget for Slider {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);

        // Draw track
        let track_height = 4;
        let track_y = self.bounds.y + (self.bounds.height - track_height) / 2;
        let track_rect = Rect {
            x: self.bounds.x,
            y: track_y,
            width: self.bounds.width,
            height: track_height,
        };
        renderer.fill_rect(track_rect, self.style.border_color);

        // Draw knob
        let knob_x = self.position_from_value();
        let knob_size = 10;
        let knob_rect = Rect {
            x: knob_x - knob_size / 2,
            y: self.bounds.y + (self.bounds.height - knob_size) / 2,
            width: knob_size,
            height: knob_size,
        };
        renderer.fill_rect(knob_rect, self.knob_color);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Event::PointerUp { x, y } = event {
            if *y >= self.bounds.y
                && *y < self.bounds.y + self.bounds.height
                && *x >= self.bounds.x
                && *x < self.bounds.x + self.bounds.width
            {
                let relative = *x - self.bounds.x;
                let ratio = relative as f32 / self.bounds.width as f32;
                let new_value = self.min + ((self.max - self.min) as f32 * ratio) as i32;
                self.set_value(new_value);
            }
        }
    }
}
