//! rlgvl-creator CLI module.
//!
//! Provides CLI utilities for managing rlvgl assets. Supports the `init`, `scan`, `check`,
//! `vendor`, `convert`, `preview`, `add-target`, `sync`, `scaffold`, `apng`, `schema`, `fonts`,
//! `svg`, and `lottie` commands to bootstrap asset directories, update a manifest, validate
//! asset policies, copy assets to build outputs, regenerate feature lists, generate thumbnails,
//! register targets, build animations, pack fonts, render SVGs, import Lottie animations, and
//! generate dual-mode crates.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser, Subcommand};

pub mod add_target;
pub mod apng;
pub mod board_import;
pub mod bsp_gen;
pub mod check;
pub mod convert;
pub mod fonts;
pub mod gen_lib;
pub mod init;
pub mod lottie;
pub mod manifest;
pub mod preview;
pub mod raw;
pub mod scaffold;
pub mod scan;
pub mod schema;
pub mod svg;
pub mod sync;
pub mod util;
pub mod vendor;

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

    /// Subcommand to execute
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Command {
    /// Initialize asset directories and a default manifest
    Init,
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
        out: PathBuf,
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
        out: PathBuf,
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
    /// Lottie-related commands
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
    /// Board-related commands
    Board {
        #[command(subcommand)]
        cmd: BoardCommand,
    },
    /// Board support package generation commands
    Bsp {
        #[command(subcommand)]
        cmd: BspCommand,
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
enum LottieCommand {
    /// Import a Lottie JSON into PNG frames and an optional APNG via rlottie FFI
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
enum BoardCommand {
    /// Convert a CubeMX `.ioc` file into a board overlay JSON
    FromIoc {
        /// Input `.ioc` file
        ioc: PathBuf,
        /// Board name to embed in the overlay
        board: String,
        /// Output path for the generated JSON
        out: PathBuf,
        /// Embed HAL template selection
        #[arg(long, conflicts_with_all = ["pac", "template"])]
        hal: bool,
        /// Embed PAC template selection
        #[arg(long, conflicts_with_all = ["hal", "template"])]
        pac: bool,
        /// Embed a custom template path
        #[arg(long, conflicts_with_all = ["hal", "pac"])]
        template: Option<String>,
        /// Output directory for generated BSP code
        #[arg(long)]
        bsp_out: Option<PathBuf>,
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
}

/// Run the rlgvl-creator command-line interface.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if cli.verbose > 0 {
        eprintln!("Using manifest {}", cli.manifest.display());
    }

    match cli.command {
        Command::Init => init::run(&cli.manifest)?,
        Command::Scan { path } => scan::run(&path, &cli.manifest)?,
        Command::Check { path, fix } => check::run(&path, &cli.manifest, fix)?,
        Command::Vendor {
            path,
            out,
            allow,
            deny,
        } => vendor::run(&path, &cli.manifest, &out, &allow, &deny)?,
        Command::Convert { path, force } => convert::run(&path, &cli.manifest, force)?,
        Command::Preview { path } => preview::run(&path, &cli.manifest)?,
        Command::AddTarget { name, vendor_dir } => {
            add_target::run(&cli.manifest, &name, &vendor_dir)?
        }
        Command::Sync { out, dry_run } => sync::run(&cli.manifest, &out, dry_run)?,
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
        Command::Board { cmd } => match cmd {
            BoardCommand::FromIoc {
                ioc,
                board,
                out,
                hal,
                pac,
                template,
                bsp_out,
            } => {
                let tmpl_sel = if hal {
                    Some("hal")
                } else if pac {
                    Some("pac")
                } else {
                    template.as_deref()
                };
                board_import::from_ioc(&ioc, &board, &out, tmpl_sel)?;
                if let Some(dir) = bsp_out {
                    let kind = match tmpl_sel {
                        Some("pac") => bsp_gen::TemplateKind::Pac,
                        Some("hal") | None => bsp_gen::TemplateKind::Hal,
                        Some(t) => bsp_gen::TemplateKind::Custom(PathBuf::from(t)),
                    };
                    bsp_gen::from_ioc(
                        &ioc,
                        kind,
                        &dir,
                        false,
                        false,
                        false,
                        bsp_gen::Layout::OneFile,
                        false, // use_label_names
                        None,  // label_prefix
                        false, // fail_on_duplicate_labels
                        false, // emit_label_consts
                    )?;
                }
            }
        },
        Command::Bsp { cmd } => match cmd {
            BspCommand::FromIoc {
                ioc,
                out,
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
                    )?;
                }
                if per_peripheral {
                    bsp_gen::emit_board_mod(&out, emit_hal, emit_pac, false, false)?;
                }
            }
        },
    }

    Ok(())
}
