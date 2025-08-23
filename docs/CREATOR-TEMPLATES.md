# rlvgl-creator Templates and Hooks

Developer documentation describing how the creator uses embedded Tera templates for scaffolding assets crates and where to extend the conversion pipeline.

## Templates
The `scaffold` command builds an assets crate using Minjinja templates that are embedded as string constants in [`src/bin/creator/scaffold.rs`](../src/bin/creator/scaffold.rs). These templates cover files such as `Cargo.toml`, `lib.rs`, `build.rs`, and `README.md`. Modify the corresponding constants to change the generated crate layout or add new files.

## Pipeline Hooks
Conversion logic lives in modular Rust files like [`convert.rs`](../src/bin/creator/convert.rs), [`fonts.rs`](../src/bin/creator/fonts.rs), and [`lottie.rs`](../src/bin/creator/lottie.rs). New pipeline stages can hook into the process by adding a module and invoking it from `convert.rs`. Each step receives asset metadata and may emit outputs into `.cache` for reuse.
