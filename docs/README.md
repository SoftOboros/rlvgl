<!--
docs/README.md - Index of project documentation.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Documentation Index

Guides and design documents for rlvgl.

See crate overviews for:
- [core](../core/README.md)
- [widgets](../widgets/README.md)
- [platform](../platform/README.md)
- [ui](../ui/README.md)
- [chip database](../chipdb/README.md)

## Release & Project
- [CHANGELOG.md](./CHANGELOG.md) — Version history (current: v0.2.5).
- [v0.2.7 family wave three](./releases/v0.2.7-family-wave3.md) — candidate source/package progress and remaining owning decisions.
- [v0.2.5 release notes](./releases/v0.2.5.md) — Current release notes.
- [v0.1.9 release notes](./releases/v0.1.9.md) — Archived v0.1.9 release notes.
- [Pre-v0.2 roadmap](./releases/roadmap-pre-v0.2.md) — Archived high-level project roadmap and work streams.

## Creator Tool
- [CLI.md](./creator/CLI.md) — Command-line reference and workflows.
- [TEMPLATES.md](./creator/TEMPLATES.md) — MiniJinja template guidelines for BSP generation.
- [ASSET-PIPELINE.md](./creator/ASSET-PIPELINE.md) — Asset manifests, packing, and dual-mode crates.
- [BSP-STATUS.md](./creator/BSP-STATUS.md) — BSP generator task status (all vendors).
- [UI-DESIGN.md](./creator/UI-DESIGN.md) — Desktop UI menus, wizards, and command palette.
- [WORKSPACE-INTEGRATION.md](./creator/WORKSPACE-INTEGRATION.md) — Workspace scaffolding and simulator wiring.
- [QT-INGEST.md](./creator/QT-INGEST.md) — Qt/QML ingest notes.

## BSP & Chip Support
- [STM32.md](./bsp/STM32.md) — STM32 BSP generation behavior, flags, and roadmap.
- [IOC-IR-ALIGNMENT.md](./bsp/IOC-IR-ALIGNMENT.md) — Aligning CubeMX IOC data with the internal IR.
- [CHIP-SUPPORT.md](./bsp/CHIP-SUPPORT.md) — Vendor chip/board support: IR per vendor, parsers, chipdb.

## Rendering & Plugins
- [BACKEND-ARCHITECTURE.md](./rendering/BACKEND-ARCHITECTURE.md) — Pluggable blitter backends (CPU, DMA2D, wgpu).
- [ALPHA-BLENDING.md](./rendering/ALPHA-BLENDING.md) — Alpha-blending and layered widget rendering.
- [Ratatui on rlvgl](./ratatui-tutorial/README.md) — Retained cell backend, rlvgl-hosted pane, hybrid composition, and bare-metal proof.
- [WLD native Wayland backend initiative](./wayland/README.md) — WLD-00/01 are ratified with deterministic and compositor evidence; remaining WLD-01 acceptance and Draft WLD-02 input/release work are separately gated.
- [IMAGE-COMPRESSION-FORMAT.md](./assets/IMAGE-COMPRESSION-FORMAT.md) — Palette + RLE codec for embedded assets.
- [PLUGIN-ECOSYSTEM.md](./PLUGIN-ECOSYSTEM.md) — Media plugins: PNG, JPEG, GIF, Lottie, QR, fonts.

## UI Framework
- [UI-COMPONENT-ARCHITECTURE.md](./UI-COMPONENT-ARCHITECTURE.md) — Theme, components, Chakra-inspired widgets.
- [SVELTE-DESIGN-TOKEN-ALIGNMENT.md](./SVELTE-DESIGN-TOKEN-ALIGNMENT.md) — Svelte design tokens and component IR.
- [MPY stage-and-actors initiative](./concepts/MPY-00-CONCEPTS.md) — MPY-01 through MPY-05 are ratified with bounded native proof; MPY-04-017, interpreter, cross-driver and board/release closure remain separately gated in the [concepts index](./concepts/README.md).
- [CPY embedded-Linux and host CPython initiative](./cpython/README.md) — CPY-00 through CPY-02 are ratified and the native-only handoff is retained; CPY-03 through CPY-09 stay Draft pending physical/runtime, binding, Frame Lease, deployment and release proof.

## Hardware Targets
- [STM32H747I-DISCO hardware notes](../examples/stm32h747i-disco/HARDWARE.md) — Board reference (display, touch, pinout).
- [STM32H747I-DISCO bring-up](../examples/stm32h747i-disco/BRINGUP.md) — Hardware bring-up checklist and status.
- [ZEPHYR.md](./ZEPHYR.md) — Zephyr RTOS integration: SDK install, build, video mode, adapted command mode.
- [FreeRTOS Platform Guide](./disco-freertos-guide/README.md) — Volume IV: FreeRTOS preemptive tasks, interrupt-driven I2C4 touch, single-buffer rendering, joystick navigation.
- [Zephyr Platform Guide](./disco-zephyr-guide/README.md) — Volume V: Zephyr C+Rust hybrid, video mode vs adapted command mode, DMA2D pipeline, CSleep/LPENR fix.
- [FILESYSTEM-ASSET-LOADING.md](./assets/FILESYSTEM-ASSET-LOADING.md) — FAT32 asset loading on SD card + simulator.
- [MICROPYTHON-INTEGRATION.md](./future/MICROPYTHON-INTEGRATION.md) — Historical hardware-first sketch for MicroPython on CM7 + rlvgl on CM4; MPY-00 owns the active runtime plan.
- [WIFI-TELEMETRY.md](../examples/stm32h747i-disco/WIFI-TELEMETRY.md) — Future WiFi telemetry design and D3 SRAM layout.

## Tutorials
- [Disco Demo Tutorial](./disco-tutorial/README.md) — Desktop, icons, wings, and indicators.
- [State Chart → Reactive UI](./sctd-tutorial/README.md) — MCP iState generation, Qt media-player graphics, reactive bindings, and embedded hosts.
- [Ratatui on rlvgl](./ratatui-tutorial/README.md) — Rust-only Ratatui integration from host verification to STM32H747I-DISCO bare metal.

## Build & Test
- [SPEC-INDEX.md](./spec-index/README.md) — Subrepo-owned deterministic documentation-object index and local checks.
- [EMBEDDED-TOOLING.md](./EMBEDDED-TOOLING.md) — Install guide for ARM/STM32, ESP32 (RISC-V + Xtensa), and AVR toolchains, with Intel-macOS workarounds.
- [MAKE.md](./MAKE.md) — Makefile convenience targets.
- [CROSS-TESTING.md](./CROSS-TESTING.md) — Cross-target test linker requirements.
- [CUSTOM-SIMULATOR.md](./CUSTOM-SIMULATOR.md) — Building a custom simulator backend.
- [TEST-STRATEGY.md](./TEST-STRATEGY.md) — Testing workstream: unit, integration, hardware, CI.
