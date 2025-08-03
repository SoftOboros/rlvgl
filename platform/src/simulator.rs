//! Simple simulator backend using the `pixels` crate.
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
            event_loop,
            mut pixels,
            window,
        } = self;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut pointer_pos = (0i32, 0i32);
        let mut pointer_down = false;

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
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    pointer_pos = (position.x as i32, position.y as i32);
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
