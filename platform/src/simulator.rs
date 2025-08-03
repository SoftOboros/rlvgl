//! Simple simulator backend using the `pixels` crate.
//!
//! The window can be resized by the user and will scale the simulated
//! display while preserving the aspect ratio of the configured dimensions.
#[cfg(feature = "simulator")]
use alloc::boxed::Box;
#[cfg(feature = "simulator")]
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "simulator")]
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(feature = "simulator")]
use crate::input::InputEvent;

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
    width: usize,
    height: usize,
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
            width,
            height,
            event_loop,
            pixels,
            window,
        }
    }

    /// Run the simulator event loop.
    ///
    /// `frame_callback` is invoked whenever the window needs to be redrawn,
    /// providing mutable access to the RGBA pixel buffer. `event_callback`
    /// receives input events converted from the underlying `winit` window.
    pub fn run(
        self,
        mut frame_callback: impl FnMut(&mut [u8]) + 'static,
        mut event_callback: impl FnMut(InputEvent) + 'static,
    ) {
        let PixelsDisplay {
            width,
            height,
            event_loop,
            mut pixels,
            window,
        } = self;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut pointer_pos = (0i32, 0i32);
        let mut pointer_down = false;
        let mut surface_size = (width as u32, height as u32);
        let aspect_ratio = width as f64 / height as f64;

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
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let mut w = size.width;
                    let mut h = size.height;
                    if (w as f64 / h as f64 - aspect_ratio).abs() > f64::EPSILON {
                        if w as f64 / h as f64 > aspect_ratio {
                            w = (h as f64 * aspect_ratio).round() as u32;
                        } else {
                            h = (w as f64 / aspect_ratio).round() as u32;
                        }
                        let _ = window.request_inner_size(LogicalSize::new(w as f64, h as f64));
                    }
                    pixels
                        .resize_surface(w, h)
                        .expect("failed to resize surface");
                    surface_size = (w, h);
                }
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    pointer_pos = (
                        (position.x * width as f64 / surface_size.0 as f64) as i32,
                        (position.y * height as f64 / surface_size.1 as f64) as i32,
                    );
                    if pointer_down {
                        event_callback(InputEvent::PointerMove {
                            x: pointer_pos.0,
                            y: pointer_pos.1,
                        });
                    }
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::MouseInput {
                            state: ElementState::Pressed,
                            button: MouseButton::Left,
                            ..
                        },
                    ..
                } => {
                    pointer_down = true;
                    event_callback(InputEvent::PointerDown {
                        x: pointer_pos.0,
                        y: pointer_pos.1,
                    });
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::MouseInput {
                            state: ElementState::Released,
                            button: MouseButton::Left,
                            ..
                        },
                    ..
                } => {
                    pointer_down = false;
                    event_callback(InputEvent::PointerUp {
                        x: pointer_pos.0,
                        y: pointer_pos.1,
                    });
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            })
            .expect("event loop error");
    }
}
