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
- [CHANGELOG.md](./CHANGELOG.md) — Version history (current: v0.2.0).
- [RELEASE-v0.1.9.md](./RELEASE-v0.1.9.md) — Archived v0.1.9 release notes.
- [PROJECT-ROADMAP.md](./PROJECT-ROADMAP.md) — High-level project roadmap and work streams.

## Creator Tool
- [CREATOR-CLI.md](./CREATOR-CLI.md) — Command-line reference and workflows.
- [CREATOR-TEMPLATES.md](./CREATOR-TEMPLATES.md) — MiniJinja template guidelines for BSP generation.
- [CREATOR-ASSET-PIPELINE.md](./CREATOR-ASSET-PIPELINE.md) — Asset manifests, packing, and dual-mode crates.
- [CREATOR-BSP-STATUS.md](./CREATOR-BSP-STATUS.md) — BSP generator task status (all vendors).
- [CREATOR-UI-DESIGN.md](./CREATOR-UI-DESIGN.md) — Desktop UI menus, wizards, and command palette.
- [CREATOR-WORKSPACE-INTEGRATION.md](./CREATOR-WORKSPACE-INTEGRATION.md) — Workspace scaffolding and simulator wiring.

## BSP & Chip Support
- [STM_BSP_GENERATION.md](./STM_BSP_GENERATION.md) — STM32 BSP generation behavior, flags, and roadmap.
- [IOC-IR-ALIGNMENT.md](./IOC-IR-ALIGNMENT.md) — Aligning CubeMX IOC data with the internal IR.
- [CHIP-SUPPORT.md](./CHIP-SUPPORT.md) — Vendor chip/board support: IR per vendor, parsers, chipdb.

## Rendering & Plugins
- [RENDERING-BACKEND-ARCHITECTURE.md](./RENDERING-BACKEND-ARCHITECTURE.md) — Pluggable blitter backends (CPU, DMA2D, wgpu).
- [RENDERING-ALPHA-BLENDING.md](./RENDERING-ALPHA-BLENDING.md) — Alpha-blending and layered widget rendering.
- [IMAGE-COMPRESSION-FORMAT.md](./IMAGE-COMPRESSION-FORMAT.md) — Palette + RLE codec for embedded assets.
- [PLUGIN-ECOSYSTEM.md](./PLUGIN-ECOSYSTEM.md) — Media plugins: PNG, JPEG, GIF, Lottie, QR, fonts.

## UI Framework
- [UI-COMPONENT-ARCHITECTURE.md](./UI-COMPONENT-ARCHITECTURE.md) — Theme, components, Chakra-inspired widgets.
- [SVELTE-DESIGN-TOKEN-ALIGNMENT.md](./SVELTE-DESIGN-TOKEN-ALIGNMENT.md) — Svelte design tokens and component IR.

## Hardware Targets
- [STM32H747I-DISCO.md](./STM32H747I-DISCO.md) — Board reference (display, touch, pinout).
- [STM32H747I-DISCO-BRINGUP.md](./STM32H747I-DISCO-BRINGUP.md) — Hardware bring-up checklist and status.
- [FILESYSTEM-ASSET-LOADING.md](./FILESYSTEM-ASSET-LOADING.md) — FAT32 asset loading on SD card + simulator.
- [MICROPYTHON-INTEGRATION.md](./MICROPYTHON-INTEGRATION.md) — MicroPython on CM7 + rlvgl on CM4.
- [wifi-telemetry.md](./wifi-telemetry.md) — Future WiFi telemetry design and D3 SRAM layout.

## Build & Test
- [MAKE.md](./MAKE.md) — Makefile convenience targets.
- [CROSS-TESTING.md](./CROSS-TESTING.md) — Cross-target test linker requirements.
- [CUSTOM-SIMULATOR.md](./CUSTOM-SIMULATOR.md) — Building a custom simulator backend.
- [TEST-STRATEGY.md](./TEST-STRATEGY.md) — Testing workstream: unit, integration, hardware, CI.
