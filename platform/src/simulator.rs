//! Simple simulator backend using the `pixels` crate.
#[cfg(feature = "simulator")]
use alloc::boxed::Box;
#[cfg(feature = "simulator")]
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "simulator")]
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
    event_loop: EventLoop<()>,
    pixels: Pixels<'static>,
    window: &'static winit::window::Window,
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
        let surface = SurfaceTexture::new(width as u32, height as u32, &*window);
        let pixels = Pixels::new(width as u32, height as u32, surface)
            .expect("failed to create pixel buffer");
        let window: &'static winit::window::Window = &*window;
        Self {
            event_loop,
            pixels,
            window,
        }
    }

    /// Run the simulator event loop, rendering frames with `frame_callback`.
    pub fn run(self, mut frame_callback: impl FnMut(&mut [u8]) + 'static) {
        let PixelsDisplay {
            event_loop,
            mut pixels,
            window,
        } = self;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop
            .run(move |event, target| match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => target.exit(),
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    frame_callback(pixels.frame_mut());
                    pixels.render().unwrap();
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            })
            .expect("event loop error");
    }
}
