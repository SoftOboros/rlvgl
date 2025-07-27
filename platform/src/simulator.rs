#[cfg(feature = "simulator")]
use crate::{display::DisplayDriver, input::InputDevice};
#[cfg(feature = "simulator")]
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
#[cfg(feature = "simulator")]
use rlvgl_core::{
    event::Event,
    widget::{Color, Rect},
};

#[cfg(feature = "simulator")]
pub struct MinifbDisplay {
    window: Window,
    width: usize,
    height: usize,
    buffer: Vec<u32>,
    mouse_down: bool,
    last_pos: Option<(i32, i32)>,
}

#[cfg(feature = "simulator")]
impl MinifbDisplay {
    pub fn new(width: usize, height: usize) -> Self {
        let window = Window::new("rlvgl simulator", width, height, WindowOptions::default())
            .expect("failed to create window");
        let buffer = vec![0; width * height];
        Self {
            window,
            width,
            height,
            buffer,
            mouse_down: false,
            last_pos: None,
        }
    }

    fn update(&mut self) {
        let _ = self
            .window
            .update_with_buffer(&self.buffer, self.width, self.height);
    }
}

#[cfg(feature = "simulator")]
impl DisplayDriver for MinifbDisplay {
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        for y in 0..area.height as usize {
            for x in 0..area.width as usize {
                let idx = (area.y as usize + y) * self.width + (area.x as usize + x);
                let color = colors[y * area.width as usize + x];
                self.buffer[idx] =
                    ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | (color.2 as u32);
            }
        }
        self.update();
    }
}

#[cfg(feature = "simulator")]
impl InputDevice for MinifbDisplay {
    fn poll(&mut self) -> Option<Event> {
        let pos = self
            .window
            .get_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| (x as i32, y as i32));
        let down = self.window.get_mouse_down(MouseButton::Left);

        let mut event = None;
        if down != self.mouse_down {
            self.mouse_down = down;
            if down {
                if let Some((x, y)) = pos {
                    event = Some(Event::PointerDown { x, y });
                }
            } else {
                if let Some((x, y)) = pos {
                    event = Some(Event::PointerUp { x, y });
                }
            }
        } else if pos != self.last_pos {
            if let Some((x, y)) = pos {
                event = Some(Event::PointerMove { x, y });
            }
        }
        self.last_pos = pos;
        event
    }
}
