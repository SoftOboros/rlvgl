// SPDX-License-Identifier: MIT
//
// rust_inline_v1 layout fragment for the STM32H747I-DISCO home screen.
// Cited by ../app.yaml as `screens[].layout`. Per
// docs/app-schema/02-generator-pipeline.md §7.7, the orchestrator copies
// this file verbatim into src/screens/home.rs.
//
// Status: candidate fragment. The widget tree lives inside the
// `rlvgl-app-disco-demo` controller crate (per the 01 §5.10 `controller:`
// field cited from ../app.yaml); this fragment is the per-frame entry
// point the prong glue calls. Identical in shape to the BBB Linux home
// fragment at examples/beaglebone-black/layouts/home.rs — that
// cross-prong consistency is the whole point of the manifest schema.
//
// The existing src/main.rs holds the equivalent driving logic inline
// (constructs `DiscoController` with `DiscoCapabilities::stm32h747i_disco()`,
// drives ERIF-gated present + DMA2D admission, runs FreeRTOS tasks for
// present/render/touch/playit when the `freertos` feature is enabled).
// Decomposing main.rs so this fragment is the canonical source-of-truth
// is follow-up work under APP-03c — until then, this file documents
// the intended screen body and IS NOT referenced by any current build.

use rlvgl_app_disco_demo::DiscoController;
use rlvgl_core::renderer::Renderer;

/// Render one frame of the H747 home screen.
///
/// Called once per tick from the prong's main glue (see
/// docs/app-schema/02-generator-pipeline.md §8.3 freertos template —
/// invoked from the `render_task`). `controller` owns the widget tree;
/// this fragment just drives the per-frame render call. Effect
/// orchestration (star crawl) and command draining live in the prong
/// glue, which acts on commands the controller emits via
/// `drain_commands()`.
pub fn render<R>(controller: &mut DiscoController, renderer: &mut R)
where
    R: Renderer,
{
    controller.render(renderer);
}
