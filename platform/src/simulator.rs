//! Simple simulator backend using the `pixels` crate.
//!
//! The window can be resized by the user and will scale the simulated
//! display while preserving the aspect ratio of the configured dimensions.
#[cfg(feature = "simulator")]
use alloc::{boxed::Box, format, string::String, vec::Vec};
#[cfg(feature = "simulator")]
use eframe::{self, egui};
#[cfg(feature = "simulator")]
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "simulator")]
use std::{backtrace::Backtrace, panic};
#[cfg(feature = "simulator")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window},
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
fn generate_tiles_from_window(window: &Window, max_tile_size: u32) -> Vec<Tile> {
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
/// Display a panic message in a scrollable window limited to the screen size.
fn show_panic_window(message: String) {
    /// Simple `eframe` application rendering the panic text.
    struct PanicApp {
        msg: String,
    }

    impl eframe::App for PanicApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("rlvgl panic");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.msg).desired_width(f32::INFINITY));
                });
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ctx.output_mut(|o| o.copied_text = self.msg.clone());
                    }
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        }
    }

    let event_loop = EventLoop::new().expect("failed to create event loop");
    #[allow(deprecated)]
    let hidden_window = event_loop
        .create_window(Window::default_attributes().with_visible(false))
        .expect("failed to create window");
    let monitor_size = hidden_window
        .current_monitor()
        .map(|m| m.size())
        .unwrap_or(PhysicalSize::new(800, 600));
    drop(hidden_window);
    drop(event_loop);

    let max = egui::vec2(monitor_size.width as f32, monitor_size.height as f32);
    let initial = egui::vec2(max.x * 0.8, max.y * 0.8);

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(initial)
        .with_max_inner_size(max)
        .with_decorations(true)
        .with_resizable(true);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "rlvgl panic",
        options,
        Box::new(|_| Box::new(PanicApp { msg: message })),
    );
}

#[cfg(feature = "simulator")]
/// Desktop simulator display backed by the `pixels` crate.
pub struct PixelsDisplay {
    width: usize,
    height: usize,
    event_loop: EventLoop<()>,
    pixels: Pixels<'static>,
    window: &'static Window,
}

#[cfg(feature = "simulator")]
impl PixelsDisplay {
    /// Create a new window with the given size.
    ///
    /// Any panic during simulator execution opens a resizable window
    /// displaying the panic and a captured call stack. The window is
    /// constrained to the visible screen, provides a scrollbar for long
    /// messages, and offers copy and close controls. Closing the window
    /// terminates the process.
    pub fn new(width: usize, height: usize) -> Self {
        panic::set_hook(Box::new(|info| {
            let backtrace = Backtrace::force_capture();
            let message = format!("{info}\n\n{backtrace}");
            show_panic_window(message);
            std::process::exit(1);
        }));
        let event_loop = EventLoop::new().expect("failed to create event loop");
        #[allow(deprecated)]
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("rlvgl simulator")
                    .with_inner_size(LogicalSize::new(width as f64, height as f64)),
            )
            .expect("failed to create window");
        let window = Box::leak(Box::new(window));
        let surface = SurfaceTexture::new(width as u32, height as u32, &*window);
        let pixels = Pixels::new(width as u32, height as u32, surface)
            .expect("failed to create pixel buffer");
        let window: &'static Window = &*window;
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
        let mut surface_offset = (0.0f64, 0.0f64);
        // Ratio between window pixels and logical display coordinates
        let mut scale = (1.0f64, 1.0f64);
        let aspect_ratio = width as f64 / height as f64;
        let max_dim = pixels.device().limits().max_texture_dimension_2d;
        let mut _tiles = generate_tiles_from_window(window, max_dim);
        let mut fullscreen = false;

        #[allow(deprecated)]
        event_loop
            .run(move |event, target: &ActiveEventLoop| match event {
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
                        (size.width as f64 - w as f64) / 2.0,
                        (size.height as f64 - h as f64) / 2.0,
                    );
                    _tiles = generate_tiles_from_window(window, max_dim);
                    pixels
                        .resize_surface(w.min(max_dim), h.min(max_dim))
                        .expect("failed to resize surface");
                    let old_scale = scale;
                    scale = (w as f64 / width as f64, h as f64 / height as f64);
                    pointer_pos = (
                        (pointer_pos.0 as f64 * old_scale.0 / scale.0) as i32,
                        (pointer_pos.1 as f64 * old_scale.1 / scale.1) as i32,
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
                    let adj_x = position.x - surface_offset.0;
                    let adj_y = position.y - surface_offset.1;
                    pointer_pos = (
                        (adj_x / scale.0).clamp(0.0, width as f64 - 1.0) as i32,
                        (adj_y / scale.1).clamp(0.0, height as f64 - 1.0) as i32,
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
