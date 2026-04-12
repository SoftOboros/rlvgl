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
pub mod bsp_gen;
pub mod chakra;
pub mod check;
pub mod compress;
pub mod convert;
pub mod fonts;
pub mod gen_lib;
pub mod init;
pub mod lottie;
pub mod manifest;
pub mod new;
pub mod preview;
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
        /// Output .rle file
        output: PathBuf,
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
pub fn run() -> Result<()> {
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
        Command::Compress { input, output } => compress::run(&input, &output)?,
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
                if !do_split && core.is_none() {
                    if let Ok(txt) = std::fs::read_to_string(&ioc) {
                        let (cm7, cm4) = crate::bsp::ioc::detect_core_projects(&txt);
                        if cm7 && cm4 {
                            do_split = true;
                        }
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
                    other => {
                        return Err(anyhow!(
                            "bsp from-yaml does not support vendor '{other}' \
                             (supported: esp, espressif, nrf, nordic, nxp, imxrt, rp, rp2040)"
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
    }

    Ok(())
}
