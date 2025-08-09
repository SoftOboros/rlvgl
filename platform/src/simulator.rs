//! Simple simulator backend using the `pixels` crate.
//!
//! The window can be resized by the user and will scale the simulated
//! display while preserving the aspect ratio of the configured dimensions.
#[cfg(feature = "simulator")]
use alloc::{boxed::Box, format, vec::Vec};
//! Simple simulator backend using the `pixels` crate.
//!
//! The window can be resized by the user and will scale the simulated
//! display while preserving the aspect ratio of the configured dimensions.
#[cfg(feature = "simulator")]
use alloc::{boxed::Box, format, vec::Vec};
#[cfg(feature = "simulator")]
use pixels::{Pixels, SurfaceTexture};
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "simulator")]
use rfd::{MessageButtons, MessageDialog};
use rfd::{MessageButtons, MessageDialog};
#[cfg(feature = "simulator")]
use std::{backtrace::Backtrace, panic};
use std::{backtrace::Backtrace, panic};
#[cfg(feature = "simulator")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, WindowBuilder},
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, WindowBuilder},
};

#[cfg(feature = "simulator")]
use crate::input::InputEvent;

#[cfg(feature = "simulator")]
#[allow(dead_code)]
/// Region of the window that fits within GPU texture limits.
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg(feature = "simulator")]
/// Generate tiles covering the window where each tile is no larger than
/// `max_tile_size` in either dimension.
fn generate_tiles_from_window(window: &winit::window::Window, max_tile_size: u32) -> Vec<Tile> {
    let PhysicalSize { width, height } = window.inner_size();
    let mut tiles = Vec::new();
    let x_tiles = width.div_ceil(max_tile_size);
    let y_tiles = height.div_ceil(max_tile_size);

    for y in 0..y_tiles {
        for x in 0..x_tiles {
            let tile_x = x * max_tile_size;
            let tile_y = y * max_tile_size;
            let tile_w = (width - tile_x).min(max_tile_size);
            let tile_h = (height - tile_y).min(max_tile_size);
            tiles.push(Tile {
                x: tile_x,
                y: tile_y,
                width: tile_w,
                height: tile_h,
            });
        }
    }

    tiles
}

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
use crate::input::InputEvent;

#[cfg(feature = "simulator")]
#[allow(dead_code)]
/// Region of the window that fits within GPU texture limits.
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg(feature = "simulator")]
/// Generate tiles covering the window where each tile is no larger than
/// `max_tile_size` in either dimension.
fn generate_tiles_from_window(window: &winit::window::Window, max_tile_size: u32) -> Vec<Tile> {
    let PhysicalSize { width, height } = window.inner_size();
    let mut tiles = Vec::new();
    let x_tiles = width.div_ceil(max_tile_size);
    let y_tiles = height.div_ceil(max_tile_size);

    for y in 0..y_tiles {
        for x in 0..x_tiles {
            let tile_x = x * max_tile_size;
            let tile_y = y * max_tile_size;
            let tile_w = (width - tile_x).min(max_tile_size);
            let tile_h = (height - tile_y).min(max_tile_size);
            tiles.push(Tile {
                x: tile_x,
                y: tile_y,
                width: tile_w,
                height: tile_h,
            });
        }
    }

    tiles
}

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
    width: usize,
    height: usize,
    event_loop: EventLoop<()>,
    pixels: Pixels<'static>,
    window: &'static winit::window::Window,
    event_loop: EventLoop<()>,
    pixels: Pixels<'static>,
    window: &'static winit::window::Window,
}

#[cfg(feature = "simulator")]
impl PixelsDisplay {
impl PixelsDisplay {
    /// Create a new window with the given size.
    ///
    /// Any panic during simulator execution triggers a message dialog
    /// displaying the panic and a captured call stack. Selecting **OK** in
    /// the dialog terminates the process.
    ///
    /// Any panic during simulator execution triggers a message dialog
    /// displaying the panic and a captured call stack. Selecting **OK** in
    /// the dialog terminates the process.
    pub fn new(width: usize, height: usize) -> Self {
        panic::set_hook(Box::new(|info| {
            let backtrace = Backtrace::force_capture();
            let message = format!("{info}\n\n{backtrace}");
            let _ = MessageDialog::new()
                .set_title("rlvgl panic")
                .set_description(&message)
                .set_buttons(MessageButtons::Ok)
                .show();
            std::process::exit(1);
        }));
        let event_loop = EventLoop::new().expect("failed to create event loop");
        let window = WindowBuilder::new()
            .with_title("rlvgl simulator")
            .with_inner_size(LogicalSize::new(width as f64, height as f64))
            .build(&event_loop)
        panic::set_hook(Box::new(|info| {
            let backtrace = Backtrace::force_capture();
            let message = format!("{info}\n\n{backtrace}");
            let _ = MessageDialog::new()
                .set_title("rlvgl panic")
                .set_description(&message)
                .set_buttons(MessageButtons::Ok)
                .show();
            std::process::exit(1);
        }));
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
        let mut surface_offset = (0i32, 0i32);
        let aspect_ratio = width as f64 / height as f64;
        let max_dim = pixels.device().limits().max_texture_dimension_2d;
        let mut _tiles = generate_tiles_from_window(window, max_dim);
        let mut fullscreen = false;

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
                    }
                    surface_offset = (
                        ((size.width as i32 - w as i32) / 2),
                        ((size.height as i32 - h as i32) / 2),
                    );
                    _tiles = generate_tiles_from_window(window, max_dim);
                    pixels
                        .resize_surface(w.min(max_dim), h.min(max_dim))
                        .expect("failed to resize surface");
                    let old = surface_size;
                    surface_size = (w, h);
                    pointer_pos = (
                        (pointer_pos.0 as f64 * old.0 as f64 / surface_size.0 as f64) as i32,
                        (pointer_pos.1 as f64 * old.1 as f64 / surface_size.1 as f64) as i32,
                    );
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    physical_key: PhysicalKey::Code(KeyCode::F11),
                                    state: ElementState::Pressed,
                                    ..
                                },
                            ..
                        },
                    ..
                } => {
                    fullscreen = !fullscreen;
                    if fullscreen {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    let adj_x = position.x - surface_offset.0 as f64;
                    let adj_y = position.y - surface_offset.1 as f64;
                    pointer_pos = (
                        (adj_x * width as f64 / surface_size.0 as f64)
                            .clamp(0.0, width as f64 - 1.0) as i32,
                        (adj_y * height as f64 / surface_size.1 as f64)
                            .clamp(0.0, height as f64 - 1.0) as i32,
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
