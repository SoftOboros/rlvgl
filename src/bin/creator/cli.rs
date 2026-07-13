//! rlgvl-creator CLI module.
//!
//! Provides CLI utilities for managing rlvgl assets. Supports the `init`, `scan`, `check`,
//! `vendor`, `convert`, `preview`, `add-target`, `sync`, `scaffold`, `apng`, `schema`, `fonts`,
//! `svg`, `lottie`, `svelte`, `sim`, and `ui` commands to bootstrap asset directories, update a
//! manifest, validate asset policies, copy assets to build outputs, regenerate feature lists,
//! generate thumbnails, register targets, build animations, pack fonts, render SVGs, import
//! Lottie animations, align Svelte tokens, and launch the desktop UI or simulator.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};

pub mod add_target;
pub mod apng;
pub mod app;
pub mod bsp_gen;
pub mod chakra;
pub mod check;
pub mod compress;
pub mod convert;
pub mod emit;
pub mod fonts;
pub mod gen_lib;
pub mod init;
pub mod lottie;
pub mod manifest;
pub mod new;
pub mod preview;
pub mod qt;
pub mod qt_scjson;
pub mod raw;
pub mod run;
pub mod scaffold;
pub mod scan;
pub mod schema;
pub mod sim;
pub mod svelte;
pub mod svg;
pub mod sync;
pub mod util;
pub mod vendor;

fn resolve_out_arg(
    positional: Option<PathBuf>,
    flagged: Option<PathBuf>,
    command: &str,
) -> Result<PathBuf> {
    positional
        .or(flagged)
        .ok_or_else(|| anyhow!("missing output path for `{command}`"))
}

/// Dual-core selector for BSP generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CoreSel {
    Cm7,
    Cm4,
}

/// Target to run.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum RunTarget {
    Sim,
}

/// Output container for `compress` / `lvgl`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutKindArg {
    /// Raw binary blob (default).
    Bin,
    /// C source byte array (`lv_image_dsc_t` for the LVGL path).
    C,
    /// Rust source `[u8; N]` array.
    Rust,
}

impl From<OutKindArg> for emit::OutKind {
    fn from(k: OutKindArg) -> Self {
        match k {
            OutKindArg::Bin => emit::OutKind::Bin,
            OutKindArg::C => emit::OutKind::C,
            OutKindArg::Rust => emit::OutKind::Rust,
        }
    }
}

/// LVGL color/alpha format for the `lvgl` converter.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum LvglCfArg {
    /// 16-bit RGB565 (most compact opaque color format).
    Rgb565,
    /// 24-bit RGB888.
    Rgb888,
    /// 32-bit ARGB8888 (keeps the alpha channel).
    Argb8888,
    /// 32-bit XRGB8888 (alpha forced opaque).
    Xrgb8888,
    /// 8-bit alpha-only coverage; fill color applied at draw time.
    A8,
    /// 4-bit dithered alpha-only coverage (half the size of A8).
    A4,
}

/// Coverage source for the alpha-only formats (`a8`/`a4`).
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CoverageSourceArg {
    /// Source alpha if the image is transparent anywhere, else luminance.
    Auto,
    /// Always the source alpha channel.
    Alpha,
    /// Always luminance (white-on-black mask art).
    Luminance,
}

impl From<CoverageSourceArg> for rlvgl_decomp::lvgl::CoverageSource {
    fn from(c: CoverageSourceArg) -> Self {
        use rlvgl_decomp::lvgl::CoverageSource;
        match c {
            CoverageSourceArg::Auto => CoverageSource::Auto,
            CoverageSourceArg::Alpha => CoverageSource::Alpha,
            CoverageSourceArg::Luminance => CoverageSource::Luminance,
        }
    }
}

impl LvglCfArg {
    /// Resolve to the encoder target, attaching the coverage source for the
    /// alpha-only formats.
    fn to_target(self, coverage: CoverageSourceArg) -> compress::LvglTarget {
        use compress::LvglTarget;
        use rlvgl_decomp::lvgl::{LvglAlphaCf, LvglCf};
        match self {
            LvglCfArg::Rgb565 => LvglTarget::Color(LvglCf::Rgb565),
            LvglCfArg::Rgb888 => LvglTarget::Color(LvglCf::Rgb888),
            LvglCfArg::Argb8888 => LvglTarget::Color(LvglCf::Argb8888),
            LvglCfArg::Xrgb8888 => LvglTarget::Color(LvglCf::Xrgb8888),
            LvglCfArg::A8 => LvglTarget::Alpha(LvglAlphaCf::A8, coverage.into()),
            LvglCfArg::A4 => LvglTarget::Alpha(LvglAlphaCf::A4, coverage.into()),
        }
    }
}

/// CLI arguments for rlgvl-creator.
#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = None,
    after_help = "Example:\n  rlgvl-creator scan assets/\n  rlgvl-creator --manifest custom.yml check assets/",
    arg_required_else_help = true
)]
struct Cli {
    /// Path to the asset manifest
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "manifest.yml",
        global = true
    )]
    manifest: PathBuf,

    /// Increase output verbosity
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,

    /// Suppress non-error output (hides splash and info messages)
    #[arg(long, global = true)]
    silent: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Command {
    /// Initialize asset directories and a default manifest
    Init,
    /// Create a new rlvgl workspace
    New {
        /// Name of the project
        name: String,
        /// Target MCU (optional)
        #[arg(long)]
        mcu: Option<String>,
    },
    /// Build and run a target
    Run {
        /// Target to run
        #[arg(value_enum)]
        target: RunTarget,
    },
    /// Scan a directory for assets and update the manifest
    Scan {
        /// Root path containing assets
        path: PathBuf,
    },
    /// Validate manifest entries against asset files
    Check {
        /// Root path containing assets
        path: PathBuf,
        /// Apply fixes to the manifest
        #[arg(long)]
        fix: bool,
    },
    /// Copy assets and generate an `rlvgl_assets.rs` module
    Vendor {
        /// Root path containing assets
        path: PathBuf,
        /// Directory to copy assets into
        #[arg(
            value_name = "OUT",
            required_unless_present = "out_flag",
            conflicts_with = "out_flag"
        )]
        out: Option<PathBuf>,
        /// Directory to copy assets into
        #[arg(
            long = "out",
            value_name = "OUT",
            required_unless_present = "out",
            conflicts_with = "out"
        )]
        out_flag: Option<PathBuf>,
        /// Allow only assets with these licenses
        #[arg(long, value_name = "LICENSE")]
        allow: Vec<String>,
        /// Deny assets with these licenses
        #[arg(long, value_name = "LICENSE")]
        deny: Vec<String>,
    },
    /// Convert assets to normalized formats and update manifest
    Convert {
        /// Root path containing assets
        path: PathBuf,
        /// Rebuild all assets even if cached
        #[arg(long)]
        force: bool,
    },
    /// Compress an image to an RLEC blob for firmware use
    Compress {
        /// Input image (PNG, BMP, etc.) or RLVGLRAW .raw file
        input: PathBuf,
        /// Output file (.rle blob, or .c/.rs source with `--emit`)
        output: PathBuf,
        /// Output container: a raw blob, or a C / Rust byte array
        #[arg(long, value_enum, default_value_t = OutKindArg::Bin)]
        emit: OutKindArg,
        /// Symbol name for `--emit c|rust` (defaults to the output file stem)
        #[arg(long)]
        name: Option<String>,
        /// Map source pixels with alpha < 128 to a magenta (#FF00FF) sentinel.
        /// The RLEC format is RGB565 (no alpha); consumers that key magenta
        /// back to transparent (e.g. the QML `qt_image` helper) thereby
        /// recover 1-bit transparency for icons with alpha edges.
        #[arg(long)]
        transparent_key: bool,
    },
    /// Convert an image to an LVGL v9 binary image (`.bin`) or compiled-in
    /// C / Rust source for handoff to an LVGL build
    Lvgl {
        /// Input image (PNG, BMP, etc.) or RLVGLRAW .raw file
        input: PathBuf,
        /// Output file (.bin, or .c/.rs source with `--emit`)
        output: PathBuf,
        /// LVGL color/alpha format of the emitted image
        #[arg(long, value_enum, default_value_t = LvglCfArg::Rgb565)]
        cf: LvglCfArg,
        /// Coverage source for `--cf a8|a4` (alpha-only formats)
        #[arg(long, value_enum, default_value_t = CoverageSourceArg::Auto)]
        coverage: CoverageSourceArg,
        /// Compress the `.bin` with LVGL RLE (ignored for `--emit c|rust`)
        #[arg(long)]
        rle: bool,
        /// Output container: an LVGL `.bin`, or a C / Rust array
        #[arg(long, value_enum, default_value_t = OutKindArg::Bin)]
        emit: OutKindArg,
        /// Symbol name for `--emit c|rust` (defaults to the output file stem)
        #[arg(long)]
        name: Option<String>,
    },
    /// Decompress an RLEC .rle blob back to a PNG image
    Decompress {
        /// Input .rle file
        input: PathBuf,
        /// Output PNG file
        output: PathBuf,
    },
    /// Generate thumbnails for quick previews
    Preview {
        /// Root path containing assets
        path: PathBuf,
    },
    /// Register a target with a vendor directory
    AddTarget {
        /// Name of the target
        name: String,
        /// Directory where assets will be vendored
        vendor_dir: PathBuf,
    },
    /// Regenerate Cargo features and an asset index from the manifest
    Sync {
        /// Directory to write generated files
        #[arg(
            value_name = "OUT",
            required_unless_present = "out_flag",
            conflicts_with = "out_flag"
        )]
        out: Option<PathBuf>,
        /// Directory to write generated files
        #[arg(
            long = "out",
            value_name = "OUT",
            required_unless_present = "out",
            conflicts_with = "out"
        )]
        out_flag: Option<PathBuf>,
        /// Print changes instead of writing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Scaffold a dual-mode assets crate
    Scaffold {
        /// Directory where the new crate will be created
        path: PathBuf,
    },
    /// Build an APNG from a sequence of PNG frames
    Apng {
        /// Directory containing PNG frames
        frames: PathBuf,
        /// Output APNG file
        out: PathBuf,
        /// Frame delay in milliseconds
        #[arg(long, default_value_t = 100)]
        delay: u16,
        /// Number of animation loops (0 = infinite)
        #[arg(long, default_value_t = 0)]
        loops: u32,
    },
    /// Output a JSON schema for the manifest structure
    Schema,
    /// Font-related commands
    Fonts {
        #[command(subcommand)]
        cmd: FontsCommand,
    },
    /// Lottie-related commands (direct import requires Linux; CLI mode works everywhere)
    Lottie {
        #[command(subcommand)]
        cmd: LottieCommand,
    },
    /// Render an SVG into raw images
    Svg {
        /// Path to the SVG file
        svg: PathBuf,
        /// Directory to write raw images into
        out: PathBuf,
        /// DPI values to render at
        #[arg(long, value_name = "DPI", action = ArgAction::Append, default_values_t = [96.0])]
        dpi: Vec<f32>,
        /// Monochrome threshold (0-255)
        #[arg(long)]
        threshold: Option<u8>,
    },
    /// Svelte alignment commands
    Svelte {
        #[command(subcommand)]
        cmd: SvelteCommand,
    },
    /// Launch the desktop UI.
    Ui,
    /// Run the desktop simulator.
    Sim(sim::SimArgs),
    /// Generate a `lib.rs` from generated BSP fragments
    GenLib {
        /// Directory containing generated modules
        #[arg(long)]
        src: PathBuf,
        /// Path to output `lib.rs`
        #[arg(long)]
        out: PathBuf,
        /// Prelude re-export form (e.g., `hal:split` or `none`)
        #[arg(long, default_value = "hal:split")]
        prelude: String,
        /// Features to gate (comma-separated)
        #[arg(
            long,
            value_delimiter = ',',
            default_value = "hal,pac,split,flat,summaries,pinreport"
        )]
        features: Vec<String>,
        /// Optional feature prefix for family gates
        #[arg(long)]
        family_feature_prefix: Option<String>,
        /// Inline includes rather than `mod` shims
        #[arg(long)]
        inline_includes: bool,
    },
    /// Board support package generation commands
    Bsp {
        #[command(subcommand)]
        cmd: BspCommand,
    },
    /// Extract BSP IR from vendor C sources (experimental)
    Ast {
        #[command(subcommand)]
        cmd: AstCommand,
    },
    /// Qt / QML ingestion commands
    Qt {
        #[command(subcommand)]
        cmd: QtCommand,
    },
    /// rlvgl Application Schema (`app.yaml`) commands per
    /// `docs/app-schema/`.
    App {
        #[command(subcommand)]
        cmd: AppCommand,
    },
}

#[derive(Subcommand)]
enum AppCommand {
    /// Parse an `app.yaml`, run the chapter 01 §6 validator, and
    /// optionally emit a buildable Cargo crate scaffold per chapter
    /// 02 §6.
    FromYaml {
        /// Path to `app.yaml` (rlvgl-app/v0 manifest).
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,
        /// Output directory for orchestrator emission.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        /// Parse and validate; do not emit.
        #[arg(long)]
        validate_only: bool,
        /// Emit to a temp dir and compare against `--out` byte-for-byte.
        /// Exits non-zero on any diff. Per chapter 02 §5.2 / §9 — the
        /// CI determinism gate.
        #[arg(long, requires = "out", conflicts_with = "validate_only")]
        check: bool,
        /// Overwrite files under `--out` that are not recorded in the
        /// previous inventory at `<out>/.rlvgl-app-manifest.json`. Required
        /// when `--out` is non-empty and contains user-owned files.
        #[arg(long, requires = "out")]
        force: bool,
        /// Parallel sub-generator dispatch per chapter 02 §5.2.
        /// `1` (default) is sequential; `>1` runs the independent
        /// stage-3 sub-gens (BSP-gen, asset-pipeline, SM-gen,
        /// i18n, theme) concurrently via `std::thread::scope`.
        /// Output is byte-deterministic regardless of N
        /// (chapter 02 §9.1).
        #[arg(long, default_value_t = 1, value_name = "N")]
        jobs: usize,
    },
    /// Emit a JSON Schema for the rlvgl-app/v0 `app.yaml` manifest
    /// per chapter 01 §5. Suitable for editor validation and CI
    /// lint hooks; does not capture runtime cross-reference rules
    /// (chipdb board lookup, workspace path safety, etc.) which
    /// require the full validator.
    Schema {
        /// Output file. Defaults to stdout.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
    /// Print a human-readable summary of a manifest: target,
    /// controller, state machine, asset histogram, screens, theme,
    /// i18n, and which chapter 02 §5.1 stage-3 sub-generators
    /// would run. Validates the manifest first.
    Inspect {
        /// Path to `app.yaml` (rlvgl-app/v0 manifest).
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,
    },
    /// Scaffold a starter `app.yaml` + minimal layout file at
    /// `<DIR>/<NAME>/`. Defaults `--dir` to the current working
    /// directory (cargo-new style). Refuses to overwrite an
    /// existing path. Generated manifest validates against
    /// chapter 01 §6.
    New {
        /// Project name. Must be a valid kebab-case ref-id
        /// (chapter 01 §3 — `^[a-z][a-z0-9-]*$`, max 63 chars).
        #[arg(value_name = "NAME")]
        name: String,
        /// Parent directory under which `<NAME>/` is created.
        /// Defaults to the current working directory.
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum QtCommand {
    /// Parse a `.qml` file (or every `*.qml` under a directory, per
    /// QT-08) and emit IR to the output directory
    Ingest {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Output directory. File-mode writes `qt-ir.json`;
        /// directory-mode writes `<basename>.qt-ir.json` per file.
        out: PathBuf,
    },
    /// Parse a `.qml` file and exit non-zero on any error (no IR emitted)
    Check {
        /// Input `.qml` file
        input: PathBuf,
    },
    /// Emit JSON Schema for `qt-ir.json` (UiModule)
    Schema {
        /// Optional output file (defaults to stdout)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Lower a `.qml` to a Rust module. `--target rlvgl` (default)
    /// produces a `<basename>.rlvgl.rs` with a runnable
    /// `build_screen` function; `--target data` produces a
    /// `<basename>.rs` static-data module per phase QT-03. Accepts
    /// a directory `<input>` per QT-08 and emits one output per
    /// `*.qml` child.
    Emit {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Output directory
        out: PathBuf,
        /// Emit target shape
        #[arg(long, value_enum, default_value_t = QtEmitTarget::Rlvgl)]
        target: QtEmitTarget,
        /// QT-05g: link an externally-injected SCXML context object to a
        /// machine crate, as `<ctx>=<crate>` (e.g. `scxmlBolero=media_player`).
        /// QML predicates `<ctx>.<state>` then lower to reactive
        /// `machine.is_active("<state>")` Image bindings (istate linkage v2).
        #[arg(long)]
        scxml_context: Option<String>,
    },
    /// QT-05d: walk a `.qml` file's inline `states:`/`transitions:`
    /// blocks and write a sibling `.scjson` document. Directory
    /// mode emits per-`<basename>.scjson`. See
    /// `docs/qt-support/05d-emit-scjson.md`.
    EmitScjson {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Optional output path. File: a `.scjson` filename. Dir:
        /// a directory; emits `<basename>.scjson` per input. Defaults
        /// to the input's parent directory.
        out: Option<PathBuf>,
    },
    /// QT-05e: walk a `.qml` file's attached state-machine scripts
    /// and write a sibling `<basename>_externals.rs` containing a
    /// `pub struct ScreenExternals` with `impl Externals` stubs.
    /// See `docs/qt-support/05e-externals-stubs.md`.
    EmitExternals {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Optional output path. File: a `.rs` filename. Dir:
        /// a directory; emits `<basename>_externals.rs` per input.
        /// Defaults to the input's parent directory.
        out: Option<PathBuf>,
    },
    /// QT-06: walk a `.qml` file's root-level `property color/int/string`
    /// theme declarations and write a sibling `<basename>.tokens.yaml`
    /// matching the chakra/svelte token schema. See
    /// `docs/qt-support/06-theme-tokens.md`.
    EmitTokens {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Optional output path. File: a `.tokens.yaml` filename.
        /// Dir: a directory; emits `<basename>.tokens.yaml` per
        /// input. Defaults to the input's parent directory.
        out: Option<PathBuf>,
    },
    /// QT-07: walk a `.qml` file's `Image { source: … }` and font
    /// declarations and write a sibling `<basename>.assets.yaml`
    /// inventory of referenced assets for handoff to the
    /// `rlvgl-creator scan` / `vendor` pipeline. See
    /// `docs/qt-support/07-asset-handoff.md`.
    ListAssets {
        /// Input `.qml` file or directory containing `*.qml`
        input: PathBuf,
        /// Optional output path. File: a `.assets.yaml` filename.
        /// Dir: a directory; emits `<basename>.assets.yaml` per
        /// input. Defaults to the input's parent directory.
        out: Option<PathBuf>,
    },
    /// QT-08b: parse a `qmldir` module manifest and emit a stable
    /// YAML inventory of declared types, singletons, internals,
    /// imports, and depends. See `docs/qt-support/08b-qmldir-resolution.md`.
    ListQmldir {
        /// Input: a `qmldir` file path or a directory containing one.
        input: PathBuf,
        /// Optional output path. File: a `.yaml` filename. Dir:
        /// a directory; emits `<dirname>.qmldir.yaml`.
        out: Option<PathBuf>,
    },
    /// QT-08c: parse a `.qrc` Qt resource manifest and emit a
    /// stable YAML inventory of declared resources / files /
    /// aliases. See `docs/qt-support/08c-qrc-resources.md`.
    ListQrc {
        /// Input: a `.qrc` file path or a directory containing
        /// one or more `.qrc` files.
        input: PathBuf,
        /// Optional output path. File: a `.yaml` filename. Dir:
        /// a directory; emits `<basename>.qrc.yaml` per input.
        out: Option<PathBuf>,
    },
}

/// `qt emit --target` value selector. Mirrors `qt::EmitTarget`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum QtEmitTarget {
    /// QT-03 data-only `pub static SCREEN: Node = …;` shape.
    Data,
    /// QT-03b runnable `build_screen(bounds) -> WidgetNode` shape.
    Rlvgl,
}

impl From<QtEmitTarget> for qt::EmitTarget {
    fn from(t: QtEmitTarget) -> Self {
        match t {
            QtEmitTarget::Data => qt::EmitTarget::Data,
            QtEmitTarget::Rlvgl => qt::EmitTarget::Rlvgl,
        }
    }
}

#[derive(Subcommand)]
enum FontsCommand {
    /// Pack TTF/OTF fonts into bitmaps and metrics files
    Pack {
        /// Root path containing font files
        path: PathBuf,
        /// Point size for rasterization
        #[arg(long, default_value_t = 32)]
        size: u16,
        /// Characters to include in the pack
        #[arg(
            long,
            default_value = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
        )]
        chars: String,
    },
}

#[derive(Subcommand)]
enum SvelteCommand {
    /// Generate token outputs for web + rlvgl
    Tokens {
        /// Input token YAML
        input: PathBuf,
        /// Output directory
        out: PathBuf,
        /// Optional token mode (defaults to the first/only mode)
        #[arg(long)]
        mode: Option<String>,
    },
    /// Ingest a Chakra UI theme (.ts) and emit a tokens.yaml
    Chakra {
        /// Input Chakra theme TypeScript file
        input: PathBuf,
        /// Output directory (tokens.yaml written here)
        out: PathBuf,
    },
    /// Compile Svelte components into rlvgl output
    Compile {
        /// Input Svelte file or directory
        input: PathBuf,
        /// Output directory
        out: PathBuf,
        /// Optional token YAML to resolve styles
        #[arg(long)]
        tokens: Option<PathBuf>,
    },
    /// Emit renderer glue for Svelte → WASM → rlvgl
    Wasm {
        /// Output directory
        out: PathBuf,
        /// Optional package name for generated files
        #[arg(long)]
        name: Option<String>,
    },
    /// Output JSON schema for tokens + UI IR
    Schema {
        /// Optional output file (defaults to stdout)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Validate tokens + Svelte subset constraints
    Check {
        /// Input Svelte file or directory
        input: PathBuf,
        /// Optional token YAML
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Optional token mode (defaults to the first/only mode)
        #[arg(long)]
        mode: Option<String>,
    },
}

#[derive(Subcommand)]
enum LottieCommand {
    /// Import a Lottie JSON into PNG frames and an optional APNG using the
    /// default `lottie-cli` executable on `PATH`
    Import {
        /// Path to the Lottie JSON file
        json: PathBuf,
        /// Directory to write PNG frames into
        out: PathBuf,
        /// Optional APNG file to generate
        #[arg(long)]
        apng: Option<PathBuf>,
    },
    /// Use an external CLI to convert a Lottie JSON into frames and an optional APNG
    Cli {
        /// Path to the external CLI binary
        #[arg(long, default_value = "lottie-cli")]
        bin: PathBuf,
        /// Path to the Lottie JSON file
        json: PathBuf,
        /// Directory to write PNG frames into
        out: PathBuf,
        /// Optional APNG file to generate
        #[arg(long)]
        apng: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BspCommand {
    /// Render Rust source from a CubeMX `.ioc` file
    FromIoc {
        /// Input `.ioc` file
        ioc: PathBuf,
        /// Output directory for generated files
        #[arg(long)]
        out: PathBuf,
        /// Generate per-core outputs (cm7/ and cm4/) instead of unified
        #[arg(long)]
        split_cores: bool,
        /// Restrict output to a single core in unified mode
        #[arg(long, value_enum)]
        core: Option<CoreSel>,
        /// Override which core initializes system clocks
        #[arg(long, value_enum)]
        clock_init_core: Option<CoreSel>,
        /// Assign ownership to peripherals (comma-separated name=core pairs)
        /// Example: --periph-core usart1=cm4,spi1=cm7
        #[arg(long, value_delimiter = ',')]
        periph_core: Vec<String>,
        /// Render using the built-in HAL template
        #[arg(long)]
        emit_hal: bool,
        /// Render using the built-in PAC template
        #[arg(long)]
        emit_pac: bool,
        /// MiniJinja template to render
        #[arg(long, conflicts_with_all = ["emit_hal", "emit_pac"])]
        template: Option<PathBuf>,
        /// Collapse RCC writes by register
        #[arg(long)]
        grouped_writes: bool,
        /// Emit a single consolidated file
        #[arg(long, group = "layout")]
        one_file: bool,
        /// Emit one file per peripheral
        #[arg(long, group = "layout")]
        per_peripheral: bool,
        /// Include optional de-initialization helpers
        #[arg(long)]
        with_deinit: bool,
        /// Permit configuration of reserved SWD pins (PA13/PA14)
        #[arg(long)]
        allow_reserved: bool,
        /// Use label-based identifiers when available
        #[arg(long)]
        use_label_names: bool,
        /// Prefix to apply to label identifiers when needed
        #[arg(long)]
        label_prefix: Option<String>,
        /// Fail if two labels sanitize to the same identifier
        #[arg(long)]
        fail_on_duplicate_labels: bool,
        /// Emit a `pins` module with label constants (PAC)
        #[arg(long)]
        emit_label_consts: bool,
    },
    /// Render Rust source from vendor C sources (experimental)
    FromC {
        /// Input C files or directories (recurses)
        inputs: Vec<PathBuf>,
        /// Output directory for generated files
        #[arg(long)]
        out: PathBuf,
        /// MCU identifier (e.g., STM32H747XIHx)
        #[arg(long)]
        mcu: String,
        /// Package identifier (e.g., LQFP176)
        #[arg(long)]
        package: String,
        /// Render using the built-in HAL template
        #[arg(long)]
        emit_hal: bool,
        /// Render using the built-in PAC template
        #[arg(long)]
        emit_pac: bool,
        /// MiniJinja template to render
        #[arg(long, conflicts_with_all = ["emit_hal", "emit_pac"])]
        template: Option<PathBuf>,
        /// Collapse RCC writes by register
        #[arg(long)]
        grouped_writes: bool,
        /// Include optional de-initialization helpers
        #[arg(long)]
        with_deinit: bool,
        /// Emit a single consolidated file
        #[arg(long, group = "layout")]
        one_file: bool,
        /// Emit one file per peripheral
        #[arg(long, group = "layout")]
        per_peripheral: bool,
        /// Use label-based identifiers when available
        #[arg(long)]
        use_label_names: bool,
        /// Prefix to apply to label identifiers when needed
        #[arg(long)]
        label_prefix: Option<String>,
        /// Fail if two labels sanitize to the same identifier
        #[arg(long)]
        fail_on_duplicate_labels: bool,
        /// Emit a `pins` module with label constants (PAC)
        #[arg(long)]
        emit_label_consts: bool,
    },
    /// List available chips for a vendor.
    ListChips {
        /// Vendor key (`esp`, `espressif`).
        #[arg(long)]
        vendor: String,
    },
    /// List available boards for a vendor, optionally filtered by chip.
    ListBoards {
        /// Vendor key (`esp`, `espressif`).
        #[arg(long)]
        vendor: String,
        /// Filter boards to those targeting this chip (e.g. `ESP32-C3`).
        #[arg(long)]
        chip: Option<String>,
    },
    /// Render Rust source from a vendor YAML chip/board spec.
    ///
    /// Supports `--vendor esp` which consumes the `rlvgl-chips-esp`
    /// YAML chipdb to produce a PAC-style board support crate.
    FromYaml {
        /// Vendor key selecting the YAML adapter (`esp`, `espressif`).
        #[arg(long)]
        vendor: String,
        /// Board spec file stem in `chipdb/rlvgl-chips-esp/db/boards/`.
        #[arg(long)]
        board: String,
        /// Override the chip spec file stem (defaults to `board.chip`).
        #[arg(long)]
        chip: Option<String>,
        /// Output directory for generated files.
        #[arg(long)]
        out: PathBuf,
        /// Emit PAC-style initialization code (currently the only option).
        #[arg(long)]
        emit_pac: bool,
        /// Override the resolved CPU frequency in hertz.
        #[arg(long)]
        cpu_hz: Option<u32>,
        /// Override the console baud rate.
        #[arg(long)]
        baud: Option<u32>,
        /// Load chip spec from a YAML file instead of the embedded chipdb.
        #[arg(long)]
        chip_yaml: Option<PathBuf>,
        /// Load board spec from a YAML file instead of the embedded chipdb.
        #[arg(long)]
        board_yaml: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AstCommand {
    /// Extract IR from one or more C files
    FromC {
        /// MCU identifier (e.g., STM32H747XIHx)
        #[arg(long)]
        mcu: String,
        /// Package identifier (e.g., LQFP176)
        #[arg(long)]
        package: String,
        /// Input C files or directories (recurses)
        inputs: Vec<PathBuf>,
        /// Output path for the generated IR JSON (stdout if omitted)
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Run the rlgvl-creator command-line interface.
pub fn run(bsp_gen: app::BspGenFn) -> Result<()> {
    let cli = Cli::parse();
    if !cli.silent {
        println!("rlvgl v{} • rlvgl-creator", env!("CARGO_PKG_VERSION"));
        if cli.verbose > 0 {
            eprintln!("Using manifest {}", cli.manifest.display());
        }
    }

    match cli.command {
        Command::Init => init::run(&cli.manifest)?,
        Command::New { name, mcu } => new::run(&name, mcu.as_deref())?,
        Command::Run { target } => match target {
            RunTarget::Sim => run::sim()?,
        },
        Command::Scan { path } => scan::run(&path, &cli.manifest)?,
        Command::Check { path, fix } => check::run(&path, &cli.manifest, fix)?,
        Command::Vendor {
            path,
            out,
            out_flag,
            allow,
            deny,
        } => {
            let out = resolve_out_arg(out, out_flag, "vendor")?;
            vendor::run(&path, &cli.manifest, &out, &allow, &deny)?
        }
        Command::Convert { path, force } => convert::run(&path, &cli.manifest, force)?,
        Command::Compress {
            input,
            output,
            emit,
            name,
            transparent_key,
        } => compress::run(
            &input,
            &output,
            emit.into(),
            name.as_deref(),
            transparent_key,
        )?,
        Command::Lvgl {
            input,
            output,
            cf,
            coverage,
            rle,
            emit,
            name,
        } => compress::lvgl(
            &input,
            &output,
            cf.to_target(coverage),
            rle,
            emit.into(),
            name.as_deref(),
        )?,
        Command::Decompress { input, output } => compress::decompress(&input, &output)?,
        Command::Preview { path } => preview::run(&path, &cli.manifest)?,
        Command::AddTarget { name, vendor_dir } => {
            add_target::run(&cli.manifest, &name, &vendor_dir)?
        }
        Command::Sync {
            out,
            out_flag,
            dry_run,
        } => {
            let out = resolve_out_arg(out, out_flag, "sync")?;
            sync::run(&cli.manifest, &out, dry_run)?
        }
        Command::Scaffold { path } => scaffold::run(&path, &cli.manifest)?,
        Command::Apng {
            frames,
            out,
            delay,
            loops,
        } => apng::run(&frames, &out, delay, loops)?,
        Command::Schema => schema::run()?,
        Command::Fonts { cmd } => match cmd {
            FontsCommand::Pack { path, size, chars } => {
                fonts::pack(&path, &cli.manifest, size as f32, &chars)?
            }
        },
        Command::Lottie { cmd } => match cmd {
            LottieCommand::Import { json, out, apng } => {
                lottie::import(&json, &out, apng.as_deref())?
            }
            LottieCommand::Cli {
                bin,
                json,
                out,
                apng,
            } => lottie::import_cli(&bin, &json, &out, apng.as_deref())?,
        },
        Command::Svg {
            svg,
            out,
            dpi,
            threshold,
        } => svg::run(&svg, &out, &dpi, threshold)?,
        Command::Svelte { cmd } => match cmd {
            SvelteCommand::Chakra { input, out } => chakra::ingest(&input, &out)?,
            SvelteCommand::Tokens { input, out, mode } => {
                svelte::tokens(&input, &out, mode.as_deref())?
            }
            SvelteCommand::Compile { input, out, tokens } => {
                svelte::compile(&input, &out, tokens.as_deref())?
            }
            SvelteCommand::Wasm { out, name } => svelte::wasm(&out, name.as_deref())?,
            SvelteCommand::Schema { out } => svelte::schema(out.as_deref())?,
            SvelteCommand::Check {
                input,
                tokens,
                mode,
            } => svelte::check(&input, tokens.as_deref(), mode.as_deref())?,
        },
        Command::Ui => {
            #[cfg(feature = "creator_ui")]
            crate::ui::run()?;
            #[cfg(not(feature = "creator_ui"))]
            return Err(anyhow!("creator_ui feature is not enabled"));
        }
        Command::Sim(args) => sim::run(args)?,
        Command::GenLib {
            src,
            out,
            prelude,
            features,
            family_feature_prefix,
            inline_includes,
        } => {
            let df = if prelude == "none" {
                None
            } else {
                let parts: Vec<_> = prelude.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow!("prelude must be kind:form or 'none'"));
                }
                Some((parts[0].to_string(), parts[1].to_string()))
            };
            let df_ref = df.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
            gen_lib::emit_lib_rs(
                &src,
                &out,
                df_ref,
                &features,
                family_feature_prefix.as_deref(),
                inline_includes,
            )?;
        }
        Command::Bsp { cmd } => match cmd {
            BspCommand::FromIoc {
                ioc,
                out,
                split_cores,
                core,
                clock_init_core,
                periph_core,
                emit_hal,
                emit_pac,
                template,
                grouped_writes,
                one_file: _,
                per_peripheral,
                with_deinit,
                allow_reserved,
                use_label_names,
                label_prefix,
                fail_on_duplicate_labels,
                emit_label_consts,
            } => {
                let mut kinds = Vec::new();
                if emit_hal {
                    kinds.push(bsp_gen::TemplateKind::Hal);
                }
                if emit_pac {
                    kinds.push(bsp_gen::TemplateKind::Pac);
                }
                if let Some(t) = template {
                    kinds.push(bsp_gen::TemplateKind::Custom(t));
                }
                if kinds.is_empty() {
                    return Err(anyhow!("select --emit-hal, --emit-pac, or --template"));
                }
                let layout = if per_peripheral {
                    bsp_gen::Layout::PerPeripheral
                } else {
                    bsp_gen::Layout::OneFile
                };
                let to_ir_core = |c: CoreSel| match c {
                    CoreSel::Cm7 => crate::bsp::ir::Core::Cm7,
                    CoreSel::Cm4 => crate::bsp::ir::Core::Cm4,
                };
                // Build overrides map if provided
                let mut overrides: indexmap::IndexMap<String, crate::bsp::ir::Core> =
                    indexmap::IndexMap::new();
                for entry in periph_core {
                    if let Some((name, core_s)) = entry.split_once('=') {
                        let c = match core_s.to_ascii_lowercase().as_str() {
                            "cm7" => Some(crate::bsp::ir::Core::Cm7),
                            "cm4" => Some(crate::bsp::ir::Core::Cm4),
                            _ => None,
                        };
                        if let Some(c) = c {
                            overrides.insert(name.to_ascii_lowercase(), c);
                        } else {
                            return Err(anyhow!("invalid core in periph-core: {}", core_s));
                        }
                    } else {
                        return Err(anyhow!("periph-core entries must be name=core"));
                    }
                }
                let overrides_ref = if overrides.is_empty() {
                    None
                } else {
                    Some(&overrides)
                };
                let init_override = clock_init_core.map(to_ir_core);
                // Auto-split when both cores are present in the .ioc and no single-core was requested
                let mut do_split = split_cores;
                if !do_split
                    && core.is_none()
                    && let Ok(txt) = std::fs::read_to_string(&ioc)
                {
                    let (cm7, cm4) = crate::bsp::ioc::detect_core_projects(&txt);
                    if cm7 && cm4 {
                        do_split = true;
                    }
                }
                if do_split {
                    for (subdir, csel) in [("cm7", CoreSel::Cm7), ("cm4", CoreSel::Cm4)] {
                        let odir = out.join(subdir);
                        std::fs::create_dir_all(&odir)?;
                        for kind in &kinds {
                            bsp_gen::from_ioc(
                                &ioc,
                                kind.clone(),
                                &odir,
                                grouped_writes,
                                with_deinit,
                                allow_reserved,
                                layout.clone(),
                                use_label_names,
                                label_prefix.as_deref(),
                                fail_on_duplicate_labels,
                                emit_label_consts,
                                Some(to_ir_core(csel)),
                                init_override,
                                overrides_ref,
                            )?;
                        }
                    }
                } else {
                    let core_filter = core.map(to_ir_core);
                    for kind in kinds {
                        bsp_gen::from_ioc(
                            &ioc,
                            kind,
                            &out,
                            grouped_writes,
                            with_deinit,
                            allow_reserved,
                            layout.clone(),
                            use_label_names,
                            label_prefix.as_deref(),
                            fail_on_duplicate_labels,
                            emit_label_consts,
                            core_filter,
                            init_override,
                            overrides_ref,
                        )?;
                    }
                }
                if per_peripheral {
                    bsp_gen::emit_board_mod(&out, emit_hal, emit_pac, false, false)?;
                }
            }
            BspCommand::FromC {
                inputs,
                out,
                mcu,
                package,
                emit_hal,
                emit_pac,
                template,
                grouped_writes,
                one_file: _,
                per_peripheral,
                with_deinit,
                use_label_names,
                label_prefix,
                fail_on_duplicate_labels,
                emit_label_consts,
            } => {
                let mut files = Vec::new();
                for p in inputs {
                    let ps = if p.is_dir() {
                        crate::ast::discover_c_sources(&p)
                    } else {
                        vec![p]
                    };
                    files.extend(ps);
                }
                if files.is_empty() {
                    return Err(anyhow!("no C sources found in inputs"));
                }
                let ir_tmp = crate::ast::extract_from_c_sources(
                    &files,
                    crate::ast::ExtractOptions {
                        mcu: &mcu,
                        package: &package,
                    },
                )?;
                // Normalize to the shared IR type to avoid cfg(test) path differences
                let mut ir: crate::ir::Ir = serde_json::from_slice(&serde_json::to_vec(&ir_tmp)?)?;
                // Apply environment overrides (STM32_* variables)
                crate::bsp_gen::apply_env_overrides(&mut ir);

                let mut kinds = Vec::new();
                if emit_hal {
                    kinds.push(bsp_gen::TemplateKind::Hal);
                }
                if emit_pac {
                    kinds.push(bsp_gen::TemplateKind::Pac);
                }
                if let Some(t) = template {
                    kinds.push(bsp_gen::TemplateKind::Custom(t));
                }
                if kinds.is_empty() {
                    return Err(anyhow!("select --emit-hal, --emit-pac, or --template"));
                }
                let layout = if per_peripheral {
                    bsp_gen::Layout::PerPeripheral
                } else {
                    bsp_gen::Layout::OneFile
                };
                for kind in kinds {
                    bsp_gen::render_from_ir(
                        &ir,
                        kind,
                        &out,
                        grouped_writes,
                        with_deinit,
                        layout.clone(),
                        use_label_names,
                        label_prefix.as_deref(),
                        fail_on_duplicate_labels,
                        emit_label_consts,
                        None,
                    )?;
                }
                if per_peripheral {
                    bsp_gen::emit_board_mod(&out, emit_hal, emit_pac, false, false)?;
                }
            }
            BspCommand::ListChips { vendor } => match vendor.as_str() {
                "esp" | "espressif" => {
                    for name in rlvgl_chips_esp::chip_names() {
                        println!("{name}");
                    }
                }
                "nrf" | "nordic" => {
                    for name in rlvgl_chips_nrf::chip_names() {
                        println!("{name}");
                    }
                }
                "nxp" | "imxrt" => {
                    for name in rlvgl_chips_nxp::chip_names() {
                        println!("{name}");
                    }
                }
                "rp" | "rp2040" => {
                    for name in rlvgl_chips_rp2040::chip_names() {
                        println!("{name}");
                    }
                }
                "renesas" | "ra" => {
                    for name in rlvgl_chips_renesas::chip_names() {
                        println!("{name}");
                    }
                }
                "ti" => {
                    for name in rlvgl_chips_ti::chip_names() {
                        println!("{name}");
                    }
                }
                "microchip" => {
                    for name in rlvgl_chips_microchip::chip_names() {
                        println!("{name}");
                    }
                }
                "silabs" => {
                    for name in rlvgl_chips_silabs::chip_names() {
                        println!("{name}");
                    }
                }
                other => return Err(anyhow!("unsupported vendor: {other}")),
            },
            BspCommand::ListBoards { vendor, chip } => match vendor.as_str() {
                "esp" | "espressif" => {
                    for info in rlvgl_chips_esp::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "nrf" | "nordic" => {
                    for info in rlvgl_chips_nrf::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "nxp" | "imxrt" => {
                    for info in rlvgl_chips_nxp::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "rp" | "rp2040" => {
                    for info in rlvgl_chips_rp2040::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "renesas" | "ra" => {
                    for info in rlvgl_chips_renesas::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "ti" => {
                    for info in rlvgl_chips_ti::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "microchip" => {
                    for info in rlvgl_chips_microchip::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                "silabs" => {
                    for info in rlvgl_chips_silabs::boards() {
                        if let Some(ref filter) = chip {
                            if !info.chip.eq_ignore_ascii_case(filter) {
                                continue;
                            }
                        }
                        println!("{:<40} {}", info.board, info.chip);
                    }
                }
                other => return Err(anyhow!("unsupported vendor: {other}")),
            },
            BspCommand::FromYaml {
                vendor,
                board,
                chip,
                out,
                emit_pac,
                cpu_hz,
                baud,
                chip_yaml,
                board_yaml,
            } => {
                if !emit_pac {
                    return Err(anyhow!("bsp from-yaml currently only supports --emit-pac"));
                }
                match vendor.as_str() {
                    "esp" | "espressif" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::espressif::load_board_file(path)?
                        } else {
                            crate::bsp::espressif::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::espressif::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.to_ascii_lowercase().replace('-', ""),
                            };
                            crate::bsp::espressif::load_chip_db(&name)?
                        };
                        let mut ir = crate::bsp::espressif::merge(chip_ir, board_ir)?;
                        if let Some(hz) = cpu_hz {
                            ir.clocks.cpu_hz = hz;
                        }
                        if let Some(baud) = baud {
                            if let Some(console) = ir.board.console.as_mut() {
                                console.baud = baud;
                            }
                        }
                        let written = crate::bsp::espressif::render_esp_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "nrf" | "nordic" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::nordic::load_board_file(path)?
                        } else {
                            crate::bsp::nordic::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::nordic::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.to_ascii_lowercase().replace('-', ""),
                            };
                            crate::bsp::nordic::load_chip_db(&name)?
                        };
                        let ir = crate::bsp::nordic::merge(chip_ir, board_ir)?;
                        let written = crate::bsp::nordic::render_nrf_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "nxp" | "imxrt" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::nxp::load_board_file(path)?
                        } else {
                            crate::bsp::nxp::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::nxp::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.to_ascii_lowercase(),
                            };
                            crate::bsp::nxp::load_chip_db(&name)?
                        };
                        let ir = crate::bsp::nxp::merge(chip_ir, board_ir)?;
                        let written = crate::bsp::nxp::render_nxp_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "rp" | "rp2040" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::rp::load_board_file(path)?
                        } else {
                            crate::bsp::rp::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::rp::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.to_ascii_lowercase(),
                            };
                            crate::bsp::rp::load_chip_db(&name)?
                        };
                        let ir = crate::bsp::rp::merge(chip_ir, board_ir)?;
                        let written = crate::bsp::rp::render_rp_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "renesas" | "ra" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::renesas::load_board_file(path)?
                        } else {
                            crate::bsp::renesas::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::renesas::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.to_ascii_lowercase(),
                            };
                            crate::bsp::renesas::load_chip_db(&name)?
                        };
                        let ir = crate::bsp::renesas::merge(chip_ir, board_ir)?;
                        let written = crate::bsp::renesas::render_renesas_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "ti" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::ti::load_board_file(path)?
                        } else {
                            crate::bsp::ti::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::ti::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.clone(),
                            };
                            crate::bsp::ti::load_chip_db(&name)?
                        };
                        let mut ir = crate::bsp::ti::merge(chip_ir, board_ir)?;
                        if let Some(hz) = cpu_hz {
                            ir.clocks.cpu_hz = hz;
                        }
                        if let Some(baud) = baud {
                            if let Some(console) = ir.board.console.as_mut() {
                                console.baud = baud;
                            }
                        }
                        let written = crate::bsp::ti::render_ti_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "microchip" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::microchip::load_board_file(path)?
                        } else {
                            crate::bsp::microchip::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::microchip::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.clone(),
                            };
                            crate::bsp::microchip::load_chip_db(&name)?
                        };
                        let mut ir = crate::bsp::microchip::merge(chip_ir, board_ir)?;
                        if let Some(hz) = cpu_hz {
                            ir.clocks.cpu_hz = hz;
                        }
                        if let Some(baud) = baud {
                            if let Some(console) = ir.board.console.as_mut() {
                                console.baud = baud;
                            }
                        }
                        let written = crate::bsp::microchip::render_microchip_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    "silabs" => {
                        let board_ir = if let Some(path) = board_yaml.as_ref() {
                            crate::bsp::silabs::load_board_file(path)?
                        } else {
                            crate::bsp::silabs::load_board_db(&board)?
                        };
                        let chip_ir = if let Some(path) = chip_yaml.as_ref() {
                            crate::bsp::silabs::load_chip_file(path)?
                        } else {
                            let name = match chip.as_deref() {
                                Some(c) => c.to_string(),
                                None => board_ir.chip.clone(),
                            };
                            crate::bsp::silabs::load_chip_db(&name)?
                        };
                        let mut ir = crate::bsp::silabs::merge(chip_ir, board_ir)?;
                        if let Some(hz) = cpu_hz {
                            ir.clocks.cpu_hz = hz;
                        }
                        if let Some(baud) = baud {
                            if let Some(console) = ir.board.console.as_mut() {
                                console.baud = baud;
                            }
                        }
                        let written = crate::bsp::silabs::render_silabs_pac(&ir, &out)?;
                        if !cli.silent {
                            println!("generated {} files in {}", written.len(), out.display());
                        }
                    }
                    other => {
                        return Err(anyhow!(
                            "bsp from-yaml does not support vendor '{other}' \
                             (supported: esp, espressif, nrf, nordic, nxp, imxrt, rp, rp2040, renesas, ra, ti, microchip, silabs)"
                        ));
                    }
                }
            }
        },
        Command::Ast { cmd } => match cmd {
            AstCommand::FromC {
                mcu,
                package,
                inputs,
                out,
            } => {
                let mut files = Vec::new();
                for p in inputs {
                    let ps = if p.is_dir() {
                        crate::ast::discover_c_sources(&p)
                    } else {
                        vec![p]
                    };
                    files.extend(ps);
                }
                let ir_tmp = crate::ast::extract_from_c_sources(
                    &files,
                    crate::ast::ExtractOptions {
                        mcu: &mcu,
                        package: &package,
                    },
                )?;
                let ir: crate::ir::Ir = serde_json::from_slice(&serde_json::to_vec(&ir_tmp)?)?;
                let json = serde_json::to_string_pretty(&ir)?;
                if let Some(path) = out {
                    std::fs::write(path, json)?;
                } else {
                    println!("{}", json);
                }
            }
        },
        Command::Qt { cmd } => match cmd {
            QtCommand::Ingest { input, out } => qt::ingest(&input, &out)?,
            QtCommand::Check { input } => qt::check(&input)?,
            QtCommand::Schema { out } => qt::schema(out.as_deref())?,
            QtCommand::Emit {
                input,
                out,
                target,
                scxml_context,
            } => qt::emit(&input, &out, target.into(), scxml_context)?,
            QtCommand::EmitScjson { input, out } => qt::emit_scjson(&input, out.as_deref())?,
            QtCommand::EmitExternals { input, out } => qt::emit_externals(&input, out.as_deref())?,
            QtCommand::EmitTokens { input, out } => qt::emit_tokens(&input, out.as_deref())?,
            QtCommand::ListAssets { input, out } => qt::list_assets(&input, out.as_deref())?,
            QtCommand::ListQmldir { input, out } => qt::list_qmldir(&input, out.as_deref())?,
            QtCommand::ListQrc { input, out } => qt::list_qrc(&input, out.as_deref())?,
        },
        Command::App { cmd } => match cmd {
            AppCommand::FromYaml {
                manifest,
                out,
                validate_only,
                check,
                force,
                jobs,
            } => app::run_from_yaml(
                &manifest,
                out.as_deref(),
                validate_only,
                check,
                force,
                jobs,
                bsp_gen,
            )?,
            AppCommand::Schema { out } => {
                let body = app::app_schema_json()?;
                if let Some(path) = out {
                    std::fs::write(&path, &body)?;
                    if !cli.silent {
                        eprintln!("wrote {}", path.display());
                    }
                } else {
                    println!("{body}");
                }
            }
            AppCommand::Inspect { manifest } => app::inspect(&manifest)?,
            AppCommand::New { name, dir } => {
                let parent = dir.unwrap_or_else(|| PathBuf::from("."));
                let manifest = app::new_scaffold(&parent, &name)?;
                if !cli.silent {
                    eprintln!(
                        "scaffolded {} (run `rlvgl-creator app inspect {0}` to verify)",
                        manifest.display()
                    );
                }
            }
        },
    }

    Ok(())
}
