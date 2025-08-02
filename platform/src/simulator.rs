//! Simple simulator backend using the pixels crate.
#[cfg(feature = "simulator")]
use crate::{display::DisplayDriver, input::InputDevice};
#[cfg(feature = "simulator")]
use alloc::boxed::Box;
#[cfg(feature = "simulator")]
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "simulator")]
use rlvgl_core::{
    event::Event,
    widget::{Color, Rect},
};
#[cfg(feature = "simulator")]
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
    _event_loop: EventLoop<()>,
    pixels: Pixels<'static>,
    width: usize,
    _height: usize,
}

#[cfg(feature = "simulator")]
impl PixelsDisplay {
    /// Create a new window with the given size.
    pub fn new(width: usize, height: usize) -> Self {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        let window = WindowBuilder::new()
            .with_title("rlvgl simulator")
            .with_inner_size(LogicalSize::new(width as f64, height as f64))
            .build(&event_loop)
            .expect("failed to create window");
        let window = Box::leak(Box::new(window));
        let surface = SurfaceTexture::new(width as u32, height as u32, window);
        let pixels = Pixels::new(width as u32, height as u32, surface)
            .expect("failed to create pixel buffer");
        Self {
            _event_loop: event_loop,
            pixels,
            width,
            _height: height,
        }
    }

    /// Present the internal buffer to the window.
    fn update(&mut self) {
        let _ = self.pixels.render();
    }
}

#[cfg(feature = "simulator")]
impl DisplayDriver for PixelsDisplay {
    /// Copy a region of pixels into the window buffer.
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        {
            let frame = self.pixels.frame_mut();
            for y in 0..area.height as usize {
                for x in 0..area.width as usize {
                    let idx = ((area.y as usize + y) * self.width + (area.x as usize + x)) * 4;
                    let color = colors[y * area.width as usize + x];
                    frame[idx] = color.0;
                    frame[idx + 1] = color.1;
                    frame[idx + 2] = color.2;
                    frame[idx + 3] = 0xFF;
                }
            }
        }
        self.update();
    }
}

#[cfg(feature = "simulator")]
impl InputDevice for PixelsDisplay {
    /// Convert window input into [`Event`]s understood by the core runtime.
    fn poll(&mut self) -> Option<Event> {
        None
    }
}
