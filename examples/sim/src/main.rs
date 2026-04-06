//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.

use rlvgl::core::application::Application;
use rlvgl::core::event::Event;
use rlvgl::platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, LoadedApp, PixelFmt, Surface, WgpuBlitter,
    WgpuDisplay,
};
use std::{cell::RefCell, env, fs, path::Path, rc::Rc};

/// Default screen width in pixels.
const DEFAULT_WIDTH: usize = 320;
/// Default screen height in pixels.
const DEFAULT_HEIGHT: usize = 240;
const DEFAULT_HEADLESS_PATH: &str = "headless.txt";

/// Convert an RGBA frame buffer into a simple ASCII art representation.
///
/// Each pixel is converted to grayscale and mapped to a character ranging
/// from a space for black to `@` for white. The output contains one line per
/// row and a trailing newline.
fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let r = buffer[idx] as u16;
            let g = buffer[idx + 1] as u16;
            let b = buffer[idx + 2] as u16;
            let val = ((r + g + b) / 3) as u8;
            let ch = match val {
                0 => ' ',
                1..=63 => '.',
                64..=127 => ':',
                128..=191 => '*',
                192..=223 => '#',
                _ => '@',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

/// Run the simulator with the given application.
fn run_with_app(
    app: &Rc<RefCell<dyn Application>>,
    root: &Rc<RefCell<rlvgl::core::WidgetNode>>,
    width: usize,
    height: usize,
    use_wgpi: bool,
    headless_path: Option<String>,
    png_path: Option<String>,
) {
    #[cfg(feature = "cpu_stats")]
    let frame_count = std::cell::Cell::new(0u32);

    let frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            #[cfg(feature = "cpu_stats")]
            let _frame_start = std::time::Instant::now();

            if use_wgpi {
                let mut blitter = WgpuBlitter::new();
                let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
                let mut renderer: BlitterRenderer<'_, WgpuBlitter, 16> =
                    BlitterRenderer::new(&mut blitter, surface);
                root.borrow().draw(&mut renderer);
                renderer.planner().add(BlitRect {
                    x: 0,
                    y: 0,
                    w: w as u32,
                    h: h as u32,
                });
            } else {
                let mut blitter = CpuBlitter;
                let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
                let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                    BlitterRenderer::new(&mut blitter, surface);
                root.borrow().draw(&mut renderer);
                renderer.planner().add(BlitRect {
                    x: 0,
                    y: 0,
                    w: w as u32,
                    h: h as u32,
                });
            }

            #[cfg(feature = "cpu_stats")]
            {
                let n = frame_count.get() + 1;
                frame_count.set(n);
                if n % 60 == 0 {
                    let elapsed = _frame_start.elapsed();
                    eprintln!("cpu_stats: frame {n}  render {:.2}ms", elapsed.as_secs_f64() * 1000.0);
                }
            }
        }
    };

    // Initial flush.
    app.borrow_mut().after_event(root, &Event::Tick);

    if let Some(path) = headless_path {
        let mut frame = vec![0u8; width * height * 4];
        frame_cb(&mut frame, width, height);
        let ascii = dump_ascii_frame(&frame, width, height);
        let path = Path::new(&path);
        fs::write(path, ascii).expect("failed to write ASCII output");
        return;
    }

    if let Some(path) = png_path {
        WgpuDisplay::headless(width, height, |fb| frame_cb(fb, width, height), path)
            .expect("PNG dump failed");
        return;
    }

    WgpuDisplay::new(width, height).run(frame_cb, {
        let root = root.clone();
        let app = app.clone();
        move |evt: InputEvent| {
            root.borrow_mut().dispatch_event(&evt);
            app.borrow_mut().after_event(&root, &evt);
        }
    });
}

fn main() {
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut png_path = None;
    let mut headless_path: Option<String> = None;
    let mut headless = false;
    let mut use_wgpi = false;
    let mut show_qr = false;
    let mut show_png = false;
    let mut show_gif = false;
    let mut app_dylib: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(screen) = arg.strip_prefix("--screen=") {
            if let Some((w, h)) = screen.split_once('x') {
                if let (Ok(w), Ok(h)) = (w.parse::<usize>(), h.parse::<usize>()) {
                    width = w;
                    height = h;
                } else {
                    eprintln!("Invalid --screen value: {screen}");
                    return;
                }
            } else {
                eprintln!("Invalid --screen value: {screen}");
                return;
            }
        } else if arg == "--wgpi" {
            use_wgpi = true;
        } else if arg == "--qrcode" {
            show_qr = true;
        } else if arg == "--png" {
            show_png = true;
        } else if arg == "--gif" {
            show_gif = true;
        } else if arg == "--app" {
            app_dylib = args.next();
            if app_dylib.is_none() {
                eprintln!("--app requires a path argument");
                return;
            }
        } else if let Some(val) = arg.strip_prefix("--app=") {
            app_dylib = Some(val.to_string());
        } else if arg.starts_with("--headless") {
            if let Some(eq) = arg.split_once('=') {
                headless_path = Some(eq.1.to_string());
            } else {
                headless_path = Some(
                    args.next()
                        .unwrap_or_else(|| DEFAULT_HEADLESS_PATH.to_string()),
                );
            }
            headless = true;
        } else {
            png_path = Some(arg);
        }
    }

    if let Some(ref dylib_path) = app_dylib {
        // Dynamic loading path.
        let mut loaded = unsafe { LoadedApp::load(dylib_path) }
            .unwrap_or_else(|e| panic!("Failed to load app from {dylib_path}: {e}"));

        let root_node = loaded.app_mut().build(width as u32, height as u32);
        let root = Rc::new(RefCell::new(root_node));

        if headless {
            root.borrow_mut().children.clear();
        }

        // Wrap in Rc<RefCell<>> for the run loop. We use a thin wrapper that
        // delegates to LoadedApp since LoadedApp isn't Clone/Rc-compatible.
        // For the dynamic path, we use a simpler inline loop.
        let app: Rc<RefCell<dyn Application>> = Rc::new(RefCell::new(LoadedAppWrapper(loaded)));
        run_with_app(&app, &root, width, height, use_wgpi, headless_path, png_path);
    } else {
        // Static path: use the built-in demo app.
        let mut demo = rlvgl_app_demo::create_app();
        let root_node = demo.build(width as u32, height as u32);
        let root = Rc::new(RefCell::new(root_node));

        if headless {
            root.borrow_mut().children.clear();
        }

        if show_qr {
            #[cfg(feature = "qrcode")]
            root.borrow_mut()
                .children
                .push(rlvgl_app_demo::build_plugin_demo(
                    width as u32,
                    height as u32,
                ));
        }
        if show_png {
            #[cfg(feature = "png")]
            root.borrow_mut()
                .children
                .push(rlvgl_app_demo::build_png_demo(
                    width as u32,
                    height as u32,
                ));
        }
        if show_gif {
            #[cfg(feature = "gif")]
            root.borrow_mut()
                .children
                .push(rlvgl_app_demo::build_gif_demo(
                    width as u32,
                    height as u32,
                ));
        }

        let app: Rc<RefCell<dyn Application>> = Rc::new(RefCell::new(BoxedAppWrapper(demo)));
        run_with_app(&app, &root, width, height, use_wgpi, headless_path, png_path);
    };
}

/// Wrapper to put a `Box<dyn Application>` behind `Rc<RefCell<dyn Application>>`.
struct BoxedAppWrapper(Box<dyn Application>);

impl Application for BoxedAppWrapper {
    fn info(&self) -> rlvgl::core::application::AppInfo {
        self.0.info()
    }
    fn build(&mut self, width: u32, height: u32) -> rlvgl::core::WidgetNode {
        self.0.build(width, height)
    }
    fn after_event(&mut self, root: &Rc<RefCell<rlvgl::core::WidgetNode>>, event: &Event) {
        self.0.after_event(root, event);
    }
    fn tick(&mut self, root: &Rc<RefCell<rlvgl::core::WidgetNode>>) {
        self.0.tick(root);
    }
    fn destroy(&mut self) {
        self.0.destroy();
    }
}

/// Wrapper to put a `LoadedApp` behind `Rc<RefCell<dyn Application>>`.
struct LoadedAppWrapper(LoadedApp);

impl Application for LoadedAppWrapper {
    fn info(&self) -> rlvgl::core::application::AppInfo {
        self.0.app().info()
    }
    fn build(&mut self, width: u32, height: u32) -> rlvgl::core::WidgetNode {
        self.0.app_mut().build(width, height)
    }
    fn after_event(&mut self, root: &Rc<RefCell<rlvgl::core::WidgetNode>>, event: &Event) {
        self.0.app_mut().after_event(root, event);
    }
    fn tick(&mut self, root: &Rc<RefCell<rlvgl::core::WidgetNode>>) {
        self.0.app_mut().tick(root);
    }
    fn destroy(&mut self) {
        self.0.app_mut().destroy();
    }
}
