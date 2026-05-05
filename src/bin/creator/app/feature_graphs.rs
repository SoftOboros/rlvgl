//! Per-prong + per-generator + per-vendor + per-board feature-graph
//! and dependency tables for the orchestrator's Cargo.toml emission.
//!
//! This module implements the data layer described in
//! `docs/app-schema/APP-05-A.md` (sub-letter analysis filed
//! 2026-05-04). Chapter 02 §8 preamble's frozen rule —
//! "the per-prong template owns the graph; the manifest names leaves" —
//! is satisfied by looking up a `ProngTemplate` by
//! `(prong, generator, vendor, board)` and expanding the manifest's
//! flat `target.features` list against the template's
//! `feature_expansions` table.
//!
//! v0 is a closed allow-list: only the boards on chapter 01 §5.6
//! `hand_written` (BBB + H747) and the chipdb boards already cited by
//! the five committed round-trip manifests (chapter 03) get real
//! templates here. Anything else falls through to a placeholder
//! (preserving the prior APP-02c behaviour) so out-of-allow-list
//! manifests still emit a buildable shell.

/// One Cargo dependency line.
#[derive(Debug, Clone)]
pub struct Dep {
    pub name: &'static str,
    pub source: DepSource,
    pub default_features: bool,
    pub features: &'static [&'static str],
    pub optional: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants used by APP-05b–e templates (pending).
pub enum DepSource {
    /// Workspace-relative path, e.g. `"../../core"`.
    Path(&'static str),
    /// crates.io version, e.g. `"0.7"`.
    Version(&'static str),
    /// `package = "X", version = "Y"` rename form.
    PackageRename {
        package: &'static str,
        version: &'static str,
    },
}

/// Policy for the `default = [...]` line in `[features]`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants used by APP-05b–e templates (pending).
pub enum DefaultPolicy {
    /// `default = []` — bin must use `required-features` to gate.
    Empty,
    /// `default = <target.features>` — manifest features are on by default.
    AllManifestFeatures,
    /// `default = [<list>]` — explicit override (rarely used).
    Explicit(&'static [&'static str]),
}

#[derive(Debug, Clone)]
pub struct ExtraBin {
    pub name: &'static str,
    pub path: &'static str,
    pub required_features: &'static [&'static str],
}

/// Resolved template for one (prong, generator, vendor, board) tuple.
pub struct ProngTemplate {
    /// Unconditional `[dependencies]` entries.
    pub base_deps: &'static [Dep],
    /// `[target.'cfg(...)'.dependencies]` entries keyed by cfg
    /// expression.
    pub target_cfg_deps: &'static [(&'static str, &'static [Dep])],
    /// `[build-dependencies]` entries.
    pub build_deps: &'static [Dep],
    /// Each manifest feature's `[features]` expansion.
    /// Manifest features absent from this list emit as `feat = []`.
    pub feature_expansions: &'static [(&'static str, &'static [&'static str])],
    /// Extra known feature names that aren't in `target.features` but
    /// must still appear in the `[features]` block (sibling-intent
    /// gates, etc.). For now used to keep the [[bin]] required-features
    /// satisfiable when the user adds a custom feature locally; defaults
    /// to empty.
    pub extra_features: &'static [(&'static str, &'static [&'static str])],
    pub default_features: DefaultPolicy,
    /// `[[bin]] required-features` for the primary binary, if any.
    pub bin_required_features: &'static [&'static str],
    /// Additional `[[bin]]` entries (sibling-intent binaries).
    pub extra_bins: &'static [ExtraBin],
}

/// Look up the template for `(prong, generator, vendor, board)`.
/// Returns `None` if the orchestrator should fall back to the
/// pre-APP-05 placeholder behaviour (no graph expansion).
pub fn lookup(
    prong: &str,
    generator: &str,
    vendor: &str,
    board: &str,
) -> Option<&'static ProngTemplate> {
    for entry in TEMPLATES {
        if entry.0 == prong && entry.1 == generator && entry.2 == vendor && entry.3 == board {
            return Some(entry.4);
        }
    }
    None
}

type TemplateKey = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static ProngTemplate,
);

const TEMPLATES: &[TemplateKey] = &[
    // APP-05a: linux + hand_written + ti + beaglebone_black_nhd_cape.
    (
        "linux",
        "hand_written",
        "ti",
        "beaglebone_black_nhd_cape",
        &BBB_LINUX,
    ),
    // APP-05b: bare_metal + hosted + esp + beetle_esp32c3 (esp_hal).
    (
        "bare_metal",
        "hosted",
        "esp",
        "beetle_esp32c3",
        &BEETLE_ESP_HAL,
    ),
    // APP-05c: bare_metal + creator-bsp-pac + esp + beetle_esp32c3 (bsp_pac).
    (
        "bare_metal",
        "creator-bsp-pac",
        "esp",
        "beetle_esp32c3",
        &BEETLE_BSP_PAC,
    ),
    // APP-05d: freertos + hand_written + stm + stm32h747i_disco.
    // Covers app.yaml + app-with-sm.yaml (shared feature set).
    (
        "freertos",
        "hand_written",
        "stm",
        "stm32h747i_disco",
        &H747_FREERTOS,
    ),
];

// ─── APP-05a: BBB linux template ──────────────────────────────────────
//
// Reference: examples/beaglebone-black/Cargo.toml.
//
// The BBB linux manifest declares features
//   [linux, splash, desktop, playit, star_crawl]
// and the reference Cargo.toml carries all five plus sibling-intent
// gates (bare_metal, freertos, zephyr) that this single-intent emit
// does not need. v0 emits only the manifest-named features; the
// sibling-intent bare-metal binary will land via its own future
// app.yaml (per chapter 03 §6.7 "duplicate by copy" rule).

static BBB_LINUX: ProngTemplate = ProngTemplate {
    base_deps: &[
        Dep {
            name: "rlvgl-core",
            source: DepSource::Path("../../core"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-platform",
            source: DepSource::Path("../../platform"),
            default_features: false,
            features: &[],
            optional: false,
        },
        // Controller dep is emitted separately (chapter 02 §7.8).
        Dep {
            name: "rlvgl-decomp",
            source: DepSource::Path("../../rlvgl-decomp"),
            default_features: true,
            features: &[],
            optional: true,
        },
        Dep {
            name: "rlvgl-playit",
            source: DepSource::Path("../../playit"),
            default_features: false,
            features: &["std"],
            optional: true,
        },
        Dep {
            name: "rlvgl-widgets",
            source: DepSource::Path("../../widgets"),
            default_features: false,
            features: &[],
            optional: true,
        },
        Dep {
            name: "libc",
            source: DepSource::Version("0.2"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "heapless",
            source: DepSource::Version("0.8"),
            default_features: false,
            features: &[],
            optional: false,
        },
    ],
    target_cfg_deps: &[],
    build_deps: &[Dep {
        name: "cc",
        source: DepSource::Version("1"),
        default_features: true,
        features: &[],
        optional: false,
    }],
    feature_expansions: &[
        ("linux", &["rlvgl-platform/linux_fbdev"]),
        ("splash", &["rlvgl-platform/splash", "dep:rlvgl-decomp"]),
        ("desktop", &[]),
        ("playit", &["dep:rlvgl-playit"]),
        ("star_crawl", &["dep:rlvgl-widgets"]),
    ],
    extra_features: &[],
    default_features: DefaultPolicy::AllManifestFeatures,
    bin_required_features: &[],
    extra_bins: &[],
};

// ─── APP-05b: beetle ESP32-C3 esp_hal hosted template ─────────────────
//
// Reference: examples/beetle-esp32c3/Cargo.toml (esp_hal half).
//
// The beetle reference Cargo.toml carries TWO mutually-exclusive
// `[[bin]]` entries gated by `required-features = ["esp_hal"]` and
// `["bsp_pac"]`. APP-05b emits ONLY the esp_hal half — the bsp_pac
// sibling lands via APP-05c from `app-bsp-pac.yaml`. Per chapter 03
// §6.7 "duplicate by copy", each manifest's emit is single-intent.
//
// Reference [features].esp_hal expansion:
//   ["dep:esp-hal", "dep:esp-backtrace", "dep:esp-println",
//    "dep:esp-alloc", "dep:ssd1306", "dep:rlvgl-core",
//    "dep:rlvgl-platform", "dep:rlvgl-widgets", "rlvgl-platform/ssd1306"]
//
// Cross-compile-only deps (esp-hal family, ssd1306 transport) live
// under [target.'cfg(target_arch = "riscv32")'.dependencies].

static BEETLE_ESP_HAL: ProngTemplate = ProngTemplate {
    base_deps: &[
        Dep {
            name: "rlvgl-core",
            source: DepSource::Path("../../core"),
            default_features: false,
            features: &[],
            optional: true,
        },
        Dep {
            name: "rlvgl-platform",
            source: DepSource::Path("../../platform"),
            default_features: false,
            features: &[],
            optional: true,
        },
        Dep {
            name: "rlvgl-widgets",
            source: DepSource::Path("../../widgets"),
            default_features: false,
            features: &[],
            optional: true,
        },
        Dep {
            name: "ssd1306",
            source: DepSource::Version("0.9"),
            default_features: false,
            features: &["graphics"],
            optional: true,
        },
    ],
    target_cfg_deps: &[(
        "cfg(target_arch = \"riscv32\")",
        &[
            Dep {
                name: "esp-hal",
                source: DepSource::Version("=1.0.0-beta.0"),
                default_features: true,
                features: &["esp32c3", "unstable"],
                optional: true,
            },
            Dep {
                name: "esp-backtrace",
                source: DepSource::Version("0.15"),
                default_features: true,
                features: &["esp32c3", "panic-handler", "println"],
                optional: true,
            },
            Dep {
                name: "esp-println",
                source: DepSource::Version("0.13"),
                default_features: true,
                features: &["esp32c3", "log"],
                optional: true,
            },
            Dep {
                name: "esp-alloc",
                source: DepSource::Version("0.6"),
                default_features: true,
                features: &[],
                optional: true,
            },
        ],
    )],
    build_deps: &[],
    feature_expansions: &[(
        "esp_hal",
        &[
            "dep:esp-hal",
            "dep:esp-backtrace",
            "dep:esp-println",
            "dep:esp-alloc",
            "dep:ssd1306",
            "dep:rlvgl-core",
            "dep:rlvgl-platform",
            "dep:rlvgl-widgets",
            "rlvgl-platform/ssd1306",
        ],
    )],
    extra_features: &[],
    default_features: DefaultPolicy::Empty,
    bin_required_features: &["esp_hal"],
    extra_bins: &[],
};

// ─── APP-05c: beetle ESP32-C3 bsp_pac creator-bsp-pac template ────────
//
// Reference: examples/beetle-esp32c3/Cargo.toml (bsp_pac half).
//
// The bsp_pac intent is "headless" — a raw-PAC LED-blink proof of
// the chipdb → generator → boot pipeline, with no display, no rlvgl
// render stack, no widget tree (chapter 03 §6.13 documents this as
// "bsp_pac stretches the screen abstraction"). Per the reference
// comment block: "rlvgl deps are only pulled in by the esp_hal
// feature — the bsp_pac demo uses only the raw esp32c3 PAC".
//
// Reference [features].bsp_pac expansion:
//   ["dep:esp32c3", "dep:esp-riscv-rt", "dep:riscv-rt",
//    "dep:riscv", "dep:panic-halt"]

static BEETLE_BSP_PAC: ProngTemplate = ProngTemplate {
    base_deps: &[],
    target_cfg_deps: &[(
        "cfg(target_arch = \"riscv32\")",
        &[
            Dep {
                name: "esp32c3",
                source: DepSource::Version("0.31"),
                default_features: true,
                features: &["critical-section", "rt"],
                optional: true,
            },
            Dep {
                name: "esp-riscv-rt",
                source: DepSource::Version("0.13"),
                default_features: true,
                features: &[],
                optional: true,
            },
            Dep {
                name: "riscv-rt",
                source: DepSource::Version("0.16"),
                default_features: true,
                features: &["memory"],
                optional: true,
            },
            Dep {
                name: "riscv",
                source: DepSource::Version("0.15"),
                default_features: true,
                features: &["critical-section-single-hart"],
                optional: true,
            },
            Dep {
                name: "panic-halt",
                source: DepSource::Version("1"),
                default_features: true,
                features: &[],
                optional: true,
            },
        ],
    )],
    build_deps: &[],
    feature_expansions: &[(
        "bsp_pac",
        &[
            "dep:esp32c3",
            "dep:esp-riscv-rt",
            "dep:riscv-rt",
            "dep:riscv",
            "dep:panic-halt",
        ],
    )],
    extra_features: &[],
    default_features: DefaultPolicy::Empty,
    bin_required_features: &["bsp_pac"],
    extra_bins: &[],
};

// ─── APP-05d: STM32H747I-DISCO freertos hand_written template ─────────
//
// Reference: examples/stm32h747i-disco/Cargo.toml.
//
// Two manifests share this template (chapter 03 §6.5 amendments —
// app.yaml + app-with-sm.yaml carry the same target.features set
// `[cm7, freertos, adapted_cmd, dma2d, splash, desktop]` and differ
// only in their `state_machine:` block, which is orthogonal to
// Cargo.toml emission). APP-05e (zephyr prong) shares this same
// chip + board but with a different feature set.
//
// Reference [features] expansions for the manifest's six features:
//   cm7 = ["rlvgl-platform/stm32h747i_disco", "stm32h7/stm32h747cm7",
//          "dep:stm32h7xx-hal", "dep:embedded-hal",
//          "dep:embedded-hal-02", "dep:embedded-sdmmc"]
//   freertos = ["rlvgl-platform/freertos"]
//   adapted_cmd = []
//   dma2d = ["rlvgl-platform/dma2d"]
//   splash = ["rlvgl-platform/splash"]
//   desktop = []
//
// The reference also carries 16 sibling-intent features
// (cm4, clock_demo, cpu_stats, audio, qspi_flash, sd_storage,
// c_hal, c_hal_cm4, pac_sdram_init, sdram_ramtest, hal_sdram,
// backlight_pwm, semihosting, bsp_log, zephyr) — those belong to
// other manifests (the cm4 idle binary, the audio profile, etc.)
// per chapter 03 §6.7 "duplicate by copy" and are NOT emitted here.
//
// Base [dependencies]: rlvgl runtime crates + cortex-m runtime +
// allocator + panic handler + stm32h7 PAC + critical-section. The
// optional `cortex-m-semihosting` / `stm32-fmc` / `rlvgl-bsps-stm`
// / `rlvgl-app-demo` deps from the reference are gated by features
// the manifest doesn't enable, so they're skipped at single-intent
// emit.
//
// Cross-compile-only deps under
// [target.'cfg(any(target_arch = "arm", target_os = "none"))'.dependencies]:
// stm32h7xx-hal / embedded-hal / embedded-hal-02 (package rename) /
// embedded-sdmmc — all gated by the cm7 feature.

static H747_FREERTOS: ProngTemplate = ProngTemplate {
    base_deps: &[
        Dep {
            name: "rlvgl-core",
            source: DepSource::Path("../../core"),
            default_features: false,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-platform",
            source: DepSource::Path("../../platform"),
            default_features: false,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-widgets",
            source: DepSource::Path("../../widgets"),
            default_features: false,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-ui",
            source: DepSource::Path("../../ui"),
            default_features: false,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-i18n",
            source: DepSource::Path("../../i18n"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-decomp",
            source: DepSource::Path("../../rlvgl-decomp"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "rlvgl-playit",
            source: DepSource::Path("../../playit"),
            default_features: false,
            features: &[],
            optional: false,
        },
        Dep {
            name: "cortex-m-rt",
            source: DepSource::Version("0.7"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "cortex-m",
            source: DepSource::Version("0.7"),
            default_features: true,
            features: &["critical-section-single-core"],
            optional: false,
        },
        Dep {
            name: "embedded-alloc",
            source: DepSource::Version("=0.5.1"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "panic-halt",
            source: DepSource::Version("1"),
            default_features: true,
            features: &[],
            optional: false,
        },
        Dep {
            name: "stm32h7",
            source: DepSource::Version("0.15.1"),
            default_features: true,
            features: &["rt"],
            optional: false,
        },
        Dep {
            name: "critical-section",
            source: DepSource::Version("1.1.2"),
            default_features: true,
            features: &[],
            optional: false,
        },
    ],
    target_cfg_deps: &[(
        "cfg(any(target_arch = \"arm\", target_os = \"none\"))",
        &[
            Dep {
                name: "stm32h7xx-hal",
                source: DepSource::Version("0.16"),
                default_features: true,
                features: &["stm32h747cm7", "sdmmc", "fmc", "xspi"],
                optional: true,
            },
            Dep {
                name: "embedded-hal",
                source: DepSource::Version("1"),
                default_features: true,
                features: &[],
                optional: true,
            },
            Dep {
                name: "embedded-hal-02",
                source: DepSource::PackageRename {
                    package: "embedded-hal",
                    version: "0.2.7",
                },
                default_features: true,
                features: &["unproven"],
                optional: true,
            },
            Dep {
                name: "embedded-sdmmc",
                source: DepSource::Version("0.9"),
                default_features: false,
                features: &[],
                optional: true,
            },
        ],
    )],
    build_deps: &[],
    feature_expansions: &[
        (
            "cm7",
            &[
                "rlvgl-platform/stm32h747i_disco",
                "stm32h7/stm32h747cm7",
                "dep:stm32h7xx-hal",
                "dep:embedded-hal",
                "dep:embedded-hal-02",
                "dep:embedded-sdmmc",
            ],
        ),
        ("freertos", &["rlvgl-platform/freertos"]),
        ("adapted_cmd", &[]),
        ("dma2d", &["rlvgl-platform/dma2d"]),
        ("splash", &["rlvgl-platform/splash"]),
        ("desktop", &[]),
    ],
    extra_features: &[],
    default_features: DefaultPolicy::Empty,
    bin_required_features: &["cm7"],
    extra_bins: &[],
};
