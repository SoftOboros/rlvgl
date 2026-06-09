//! Simulator runner for rlvgl-creator.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

#[cfg(feature = "simulator")]
use std::fs;

#[cfg(feature = "simulator")]
const DEFAULT_HEADLESS_PATH: &str = "headless.txt";

/// CLI arguments for the simulator runner.
#[derive(Args, Debug)]
pub(crate) struct SimArgs {
    /// Screen resolution in WIDTHxHEIGHT form.
    #[arg(long, default_value = "320x240", value_name = "WxH")]
    pub(crate) screen: String,
    /// Use the wgpu accelerated blitter.
    #[arg(long = "wgpu", alias = "wgpi")]
    pub(crate) wgpu: bool,
    /// Show the QR code demo.
    #[arg(long)]
    pub(crate) qrcode: bool,
    /// Show the PNG demo.
    #[arg(long)]
    pub(crate) png: bool,
    /// Show the JPEG demo.
    #[arg(long)]
    pub(crate) jpeg: bool,
    /// Show the GIF demo.
    #[arg(long)]
    pub(crate) gif: bool,
    /// Write ASCII output to a file instead of opening a window.
    #[arg(long)]
    pub(crate) headless: bool,
    /// Path for ASCII output when using --headless.
    #[arg(long, value_name = "PATH")]
    pub(crate) headless_path: Option<PathBuf>,
    /// Output path for a single PNG frame instead of opening a window.
    #[arg(value_name = "PNG")]
    pub(crate) out: Option<PathBuf>,
}

#[cfg(feature = "simulator")]
pub(crate) fn run(args: SimArgs) -> Result<()> {
    use rlvgl::core::event::Event;
    use rlvgl::platform::{
        BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuBlitter,
        WgpuDisplay,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    let (width, height) = parse_screen(&args.screen)?;
    let mut app = rlvgl_app_demo::create_app();
    let root_node = app.build(width, height);
    let root = Rc::new(RefCell::new(root_node));

    if args.headless {
        // In headless mode, start with a blank scene to keep output deterministic.
        root.borrow_mut().children.clear();
    }

    if args.qrcode {
        #[cfg(feature = "qrcode")]
        root.borrow_mut()
            .children
            .push(rlvgl_app_demo::build_plugin_demo(width, height));
    }
    if args.png {
        #[cfg(feature = "png")]
        root.borrow_mut()
            .children
            .push(rlvgl_app_demo::build_png_demo(width, height));
    }
    if args.gif {
        #[cfg(feature = "gif")]
        root.borrow_mut()
            .children
            .push(rlvgl_app_demo::build_gif_demo(width, height));
    }
    if args.jpeg {
        #[cfg(feature = "jpeg")]
        root.borrow_mut()
            .children
            .push(rlvgl_app_demo::build_jpeg_demo(width, height));
    }

    let frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            if args.wgpu {
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
        }
    };

    // Initial flush.
    app.after_event(&root, &Event::Tick);

    if args.headless {
        let mut frame = vec![0u8; width as usize * height as usize * 4];
        frame_cb(&mut frame, width as usize, height as usize);
        let ascii = dump_ascii_frame(&frame, width as usize, height as usize);
        let path = args
            .headless_path
            .unwrap_or_else(|| PathBuf::from(DEFAULT_HEADLESS_PATH));
        fs::write(path, ascii)?;
        return Ok(());
    }

    if let Some(path) = args.out {
        WgpuDisplay::headless(
            width as usize,
            height as usize,
            |fb| frame_cb(fb, width as usize, height as usize),
            path,
        )
        .map_err(|e| anyhow!(e.to_string()))?;
        return Ok(());
    }

    let app = Rc::new(RefCell::new(app));
    WgpuDisplay::new(width as usize, height as usize).run(frame_cb, {
        let root = root.clone();
        let app = app.clone();
        move |evt: InputEvent| {
            root.borrow_mut().dispatch_event(&evt);
            app.borrow_mut().after_event(&root, &evt);
        }
    });

    Ok(())
}

#[cfg(not(feature = "simulator"))]
pub(crate) fn run(_args: SimArgs) -> Result<()> {
    Err(anyhow!(
        "simulator feature is not enabled; rebuild with --features simulator"
    ))
}

#[cfg(feature = "simulator")]
fn parse_screen(value: &str) -> Result<(u32, u32)> {
    let (w, h) = value
        .split_once('x')
        .ok_or_else(|| anyhow!("invalid screen value: {value}"))?;
    let width: u32 = w.parse().map_err(|_| anyhow!("invalid width: {w}"))?;
    let height: u32 = h.parse().map_err(|_| anyhow!("invalid height: {h}"))?;
    if width == 0 || height == 0 {
        return Err(anyhow!("screen dimensions must be non-zero"));
    }
    Ok((width, height))
}

/// Convert an RGBA frame buffer into a simple ASCII art representation.
#[cfg(feature = "simulator")]
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
