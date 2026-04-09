<!--
docs/CUSTOM-SIMULATOR.md - Custom Simulator Integration.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Integración de Simulador Personalizado

Este documento explica cómo vincular rlvgl desde tu propia aplicación, proporcionando dimensiones de pantalla personalizadas y tu propia función de configuración de demostración en lugar de la demostración integrada del simulador.

## Añadir rlvgl como dependencia

Añade el crate a tu `Cargo.toml`:

```toml
[dependencies]
rlvgl = { path = "../rlvgl" } # or `rlvgl = "0.1"` once published
```

## Proporciona tu propio punto de entrada

El ejemplo del simulador en `examples/sim` muestra cómo manejar una ventana. Para construir una variante con tu propio diseño:

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

Los puntos clave son:

- `width` y `height` definen el tamaño de la pantalla.
- `my_app::build_ui` es tu propia función de configuración de demostración.
- El callback del frame renderiza el árbol de widgets en el búfer de frame.
- El manejador de eventos despacha los eventos de entrada al widget raíz y actualiza las actualizaciones pendientes.

Estos pasos te permiten crear diferentes binarios de simulador sin depender de la demostración codificada.
