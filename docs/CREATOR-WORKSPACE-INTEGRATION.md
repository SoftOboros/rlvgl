<!-- CREATOR-WORKSPACE-INTEGRATION.md - Workspace scaffolding and simulator integration. -->

# Creator Workspace Integration

This document outlines the roadmap for transitioning `rlvgl-creator` from a component generator (assets, BSPs) to a full project lifecycle manager. The goal is to provide a "zero-friction" workspace where user interface code is shared between a high-performance desktop simulator and the target embedded hardware.

## Vision: The Workspace-First Approach

We will move to a `cargo workspace` structure as the standard unit of a project. This ensures strict separation of concerns while enabling shared logic.

### Canonical Workspace Structure
```text
my-project/
├── Cargo.toml          # Workspace root
├── assets/             # Raw assets (png, svg, fonts)
├── creator.yml         # Project configuration (display res, target MCU, etc.)
├── crates/
│   ├── app-core/       # [no_std] UI logic, widgets, state management
│   └── bsp/            # [no_std] Generated hardware definitions (from IOC/SVD)
└── apps/
    ├── sim/            # [std] Desktop simulator runner (wgpu/winit)
    └── firmware/       # [no_std] Embedded firmware entry point
```

---

## Phase 1: Workspace Scaffolding
Implement the `new` command and core MiniJinja templates to bootstrap a working project.

- [x] **CLI Command: `rlvgl-creator new <name>`**
    - [x] Accept optional `--mcu <model>` or `--preset <name>` arguments.
    - [x] Create directory structure.
    - [x] Initialize git repository.

- [x] **Template: Workspace Root**
    - [x] Generate `Cargo.toml` with `[workspace]` members.
    - [x] Generate `creator.yml` with sensible defaults (320x240, etc.).

- [x] **Template: App Core (`crates/app-core`)**
    - [x] `lib.rs` with `no_std` attribute.
    - [x] Define entry point trait/function (e.g., `fn create_ui() -> impl Widget`).
    - [x] Dependency on `rlvgl`.

- [x] **Template: Simulator (`apps/sim`)**
    - [x] `main.rs` using `rlvgl::platform::WgpuDisplay`.
    - [x] Dependency on `app-core` and `rlvgl` (with `simulator` feature).
    - [x] Boilerplate to wire `WgpuDisplay` input events to `app-core`.

- [x] **Template: Firmware Stub (`apps/firmware`)**
    - [x] Basic `main.rs` with `cortex-m-rt` entry point.
    - [x] Dependency on `app-core` and `bsp`.
    - [x] Placeholder for display driver initialization.

---

## Phase 2: Abstraction & Bridging
Ensure the user's UI code ("Core") is strictly decoupled from the execution environment.

- [x] **Hardware Abstraction Layer (HAL) Traits**
    - [x] Define specific traits for UI I/O in `rlvgl::interface` (or similar).
        - [x] `DisplayDriver` (already exists roughly as `Surface`/`Blitter`).
        - [x] `InputDriver` (Event polling).
        - [x] `TimeSource` (Monotonic clock).
    - [x] Ensure these traits are `no_std` compatible.

- [x] **Simulator Implementation**
    - [x] Implement HAL traits for `WgpuDisplay` (Desktop).
    - [x] Ensure `apps/sim` maps OS window events (Keyboard/Mouse) to `rlvgl` Input Events.

- [x] **Embedded Implementation**
    - [x] Create shared adapters for common embedded patterns (e.g., `embedded-hal` SPI -> `rlvgl` Display).
    - [x] Template code in `apps/firmware` to wire the generated BSP pins to these adapters.

---

## Phase 3: The "Rebuild" Loop
Enable the Creator tool to drive the development loop by building and running the simulator.

- [x] **Creator Build Command**
    - [x] Implement `rlvgl-creator run sim`.
    - [x] invoke `cargo run -p <project>-sim` using the project's temporary target dir to avoid locking.

- [ ] **Dynamic Configuration Loading**
    - [ ] Allow `apps/sim` to read `creator.yml` at runtime (if present) to configure window size/title.
    - [ ] *Optimization:* Generate a `config.rs` in `app-core` so constants (screen size) are known at compile time for both targets.

- [ ] **Hot-Reloading Investigation (Optional/Future)**
    - [ ] *Research:* Can `app-core` be compiled as a `.dylib` and reloaded by the Creator UI without restarting?
        - [ ] Pros: Instant feedback.
        - [ ] Cons: ABI stability, state migration complex.
    - [ ] *Fallback:* "Fast Restart" where Creator just re-spawns the sim process on file change.

---

## Phase 4: Embedded Integration
Connect the Scaffolding to the existing BSP generation logic.

- [ ] **BSP Integration**
    - [ ] Update `rlvgl-creator bsp from-ioc` to target `crates/bsp` by default in a workspace.
    - [ ] Ensure `apps/firmware` correctly re-exports or uses the generated `bsp` crate.

- [ ] **Board Presets**
    - [ ] Allow `rlvgl-creator new` to accept a board preset (e.g., `stm32h747i-disco`).
    - [ ] Pre-populate `apps/firmware` with the correct driver instantiation for that board's display/touch controller.
