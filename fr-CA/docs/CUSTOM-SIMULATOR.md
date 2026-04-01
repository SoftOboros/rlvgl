<!--
docs/CUSTOM-SIMULATOR.md - Intégration du simulateur personnalisé.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Intégration du simulateur personnalisé

Ce document explique comment lier rlvgl à votre propre application tout en fournissant des dimensions d'écran personnalisées et votre propre fonction de configuration de démonstration au lieu de la démonstration du simulateur intégrée.

## Ajouter rlvgl comme dépendance

Ajoutez la crate à votre `Cargo.toml` :

```toml
[dependencies]
rlvgl = { path = "../rlvgl" } # ou `rlvgl = "0.1"` une fois publié
```

## Fournir votre propre point d'entrée

L'exemple de simulateur dans `examples/sim` montre comment piloter une fenêtre. Pour construire une variante avec votre propre disposition :

```rust
use rlvgl::platform::{BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuDisplay};

fn main() {
    // Choisissez n'importe quelle résolution.
    let width = 480;
    let height = 320;

    // Remplacez cet appel par votre propre fonction qui construit l'arbre de widgets.
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

    // Exécutez l'affichage avec votre rappel de cadre et votre gestionnaire d'événements.
    WgpuDisplay::new(width, height).run(frame_cb, move |evt: InputEvent| {
        root.borrow_mut().dispatch_event(&evt);
        rlvgl_examples_common_demo::flush_pending(&root, &pending, &to_remove);
    });
}
```

Les points clés sont :

- `width` et `height` définissent la taille de l'écran.
- `my_app::build_ui` est votre propre fonction de configuration de démonstration.
- Le rappel de cadre rend l'arbre de widgets dans le tampon d'affichage.
- Le gestionnaire d'événements distribue les événements d'entrée au widget racine et vide les mises à jour en attente.

Ces étapes vous permettent de créer différents binaires de simulateur sans dépendre de la démonstration codée en dur.
