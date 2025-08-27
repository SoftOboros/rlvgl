<!--
docs/CUSTOM-SIMULATOR.md - Custom Simulator Integration.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Custom Simulator Integration

This document explains how to link to rlvgl from your own application while providing custom screen dimensions and your own demo setup function instead of the built-in simulator demo.

## Add rlvgl as a dependency

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
rlvgl = { path = "../rlvgl" } # or `rlvgl = "0.1"` once published
```

## Provide your own entry point

The simulator example in `examples/sim` shows how to drive a window. To build a variant with your own layout:

```rust
use rlvgl::platform::{BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuDisplay};

fn main() {
    // Pick any resolution.
    let width = 480;
    let height = 320;

    // Replace this call with your own function that builds the widget tree.
    let demo = my_app::build_ui(width as i32, height as i32);
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    let mut frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            let mut blitter = CpuBlitter;
            let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
            let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect { x: 0, y: 0, w: w as u32, h: h as u32 });
        }
    };

    // Run the display with your frame callback and event handler.
    WgpuDisplay::new(width, height).run(frame_cb, move |evt: InputEvent| {
        root.borrow_mut().dispatch_event(&evt);
        rlvgl_examples_common_demo::flush_pending(&root, &pending, &to_remove);
    });
}
```

The key points are:

- `width` and `height` define the screen size.
- `my_app::build_ui` is your own demo setup function.
- The frame callback renders the widget tree into the frame buffer.
- The event handler dispatches input events to the root widget and flushes pending updates.

These steps allow you to create different simulator binaries without relying on the hard‑coded demo.
