// SPDX-License-Identifier: MIT
//
// rust_inline_v1 layout fragment for the BBB Linux home screen.
// Cited by ../app.yaml as `screens[].layout`. Per
// docs/app-schema/02-generator-pipeline.md §7.7, the orchestrator copies
// this file verbatim into src/screens/home.rs.
//
// Status: candidate fragment. The actual widget tree lives inside the
// `rlvgl-app-disco-demo` controller crate (per the new 01 §5.10
// `controller:` field cited from ../app.yaml); this fragment is the
// per-frame entry point the prong glue calls. The existing
// src/main.rs holds the equivalent driving logic inline (it constructs
// `DiscoController` with `DiscoCapabilities::beaglebone_black_*`,
// then calls `controller.tick()` and renders into the local ARGB8888
// surface every frame). Decomposing main.rs so this fragment is the
// canonical source-of-truth is follow-up work under APP-03b — until
// then, the file documents the intended screen body and IS NOT
// referenced by any current build.

use rlvgl_app_disco_demo::DiscoController;
use rlvgl_core::renderer::Renderer;

/// Render one frame of the BBB home screen.
///
/// Called once per tick from the prong's main glue (see
/// docs/app-schema/02-generator-pipeline.md §8.1 linux template).
/// `controller` owns the widget tree; this fragment just drives the
/// per-frame render call. Effect orchestration (star crawl) and
/// command draining live in the prong glue, which acts on commands
/// the controller emits via `drain_commands()`.
pub fn render<R>(controller: &mut DiscoController, renderer: &mut R)
where
    R: Renderer,
{
    controller.render(renderer);
}
