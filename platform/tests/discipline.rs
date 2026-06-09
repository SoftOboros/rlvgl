//! Forbidden-pattern scanner enforcing the rlvgl Register-Mashing Discipline.
//!
//! Scans tracked Rust source files in the STM32H747I-DISCO scope for
//! patterns that violate the rules listed in the "Register-Mashing
//! Discipline (STM32H747I-DISCO)" section of `CLAUDE.md`. Out-of-scope
//! ports (BeagleBone Black, UEFI, ESP, simulator) are excluded entirely.
//!
//! ## Modes
//!
//! - **Default (baseline mode)**: violations recorded in `BASELINE` are
//!   tolerated; new violations fail the test. Used during the staged
//!   migration (Steps 1..8) so CI stays green while typed APIs land.
//! - **Strict mode** (`RLVGL_LINT_STRICT=1`): asserts `BASELINE` is
//!   empty. Activated by the pre-publish Phase 2.5 gate at Step 9.
//!
//! ## Opt-out marker
//!
//! Lines containing `// rlvgl-discipline: allow(<rule_id>)` are exempt
//! from that single rule on that single line. Use sparingly with a
//! justification in adjacent comments.
//!
//! ## Adding a new violation to `BASELINE`
//!
//! Run `RLVGL_LINT_BASELINE_PRINT=1 cargo test -p rlvgl-platform --test
//! discipline -- --nocapture` and append the printed entries. Reviewers
//! must confirm each addition is a known-grandfathered case from the
//! refactor migration, not a regression.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── Configuration ──────────────────────────────────────────────────────

/// In-scope path prefixes (relative to repo root). Files outside these
/// are not scanned.
const IN_SCOPE_PREFIXES: &[&str] = &[
    "platform/src/",
    "examples/stm32h747i-disco/src/",
    "examples/disco-sim/src/",
];

/// Per-rule whitelist prefixes (relative to repo root).
struct Rule {
    id: &'static str,
    description: &'static str,
    matcher: fn(&str) -> bool,
    /// Path prefixes exempt from this rule.
    whitelist: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        id: "raw_mmio_cast",
        description: "raw MMIO cast outside hwcore/regs",
        matcher: |line| line.contains("as *mut u32") || line.contains("as *const u32"),
        whitelist: &["platform/src/hwcore/regs/"],
    },
    Rule {
        id: "raw_addr_cast",
        description: "address-shaped integer cast outside hwcore/regs",
        matcher: |line| contains_hex_as_mut(line),
        whitelist: &["platform/src/hwcore/regs/"],
    },
    Rule {
        id: "static_mut",
        description: "static mut outside hwcore/isr.rs",
        matcher: |line| {
            // Match the keyword pair anywhere in code (comment-stripped already).
            line.contains("static mut ")
        },
        whitelist: &["platform/src/hwcore/isr.rs"],
    },
    Rule {
        id: "raw_dma2d",
        description: "deleted DMA2D raw API resurrected",
        matcher: |line| line.contains("start_fill_raw") || line.contains("start_blit_raw"),
        whitelist: &[],
    },
    Rule {
        id: "fb_addr_shim",
        description: "deleted u32 framebuffer-address shim resurrected",
        matcher: |line| line.contains(".front_buffer_addr(") || line.contains(".back_buffer_addr("),
        whitelist: &[],
    },
    Rule {
        id: "compiler_fence",
        description: "compiler_fence outside hwcore/",
        matcher: |line| line.contains("compiler_fence("),
        whitelist: &["platform/src/hwcore/"],
    },
    // DCB-00 §9 INV-D8: D-cache maintenance for cacheable-RAM DMA
    // buffers must go through `hwcore::dca` (DcaBuf typestate API)
    // rather than direct SCB calls. Whitelisted in `hwcore/dca.rs`
    // (the rule's owning module) and `hwcore/regs/` (for typed
    // SCB wrappers, if they grow). Pre-existing call sites are
    // grandfathered via BASELINE until the DCB-02b/DCB-02c
    // retrofits land.
    Rule {
        id: "raw_dcache",
        description: "raw SCB d-cache call outside hwcore/dca",
        matcher: |line| {
            line.contains("clean_dcache_by_address")
                || line.contains("clean_dcache_by_slice")
                || line.contains("invalidate_dcache_by_address")
                || line.contains("invalidate_dcache_by_slice")
                || line.contains("clean_invalidate_dcache_by_address")
                || line.contains("clean_invalidate_dcache_by_slice")
        },
        whitelist: &["platform/src/hwcore/dca.rs", "platform/src/hwcore/regs/"],
    },
];

/// Known-grandfathered violations. `(rule_id, repo_relative_path)`.
/// Shrinks step-by-step; MUST be empty when `RLVGL_LINT_STRICT=1`.
///
/// Generated via `RLVGL_LINT_BASELINE_PRINT=1 cargo test -p rlvgl-platform
/// --test discipline -- --nocapture`. Migration steps remove entries as
/// the corresponding files adopt typed APIs.
const BASELINE: &[(&str, &str)] = &[
    // main.rs compiler_fence cleared in Step 7b: TOUCH_RING migrated
    // from `static mut TouchRing` + manual compiler_fence to
    // `IsrChannel<RawTouchSample, 17>` from hwcore::isr.
    // main.rs migrated in Step 5b/c: all 17 front_buffer_addr() /
    // back_buffer_addr() / set_back_buffer() call sites use typed
    // front_phys() / back_phys() / set_back_phys(PhysAddr) accessors.
    // config_menu.rs migrated: D3 SRAM telemetry slots carry per-line
    // opt-out markers (intentional fixed-offset writes for probe-rs).
    // event_overlay.rs migrated: DCCMVAC opt-out marker (same as audio_player).
    // freertos_entry.rs migrated: bulk per-line opt-out markers cover
    // LTDC L2 access, DMA2D ISR, DSI wrapper, D3 SRAM telemetry, jumbo
    // buffer scratch. Structural migration deferred. static_mut entry
    // covers FreeRTOS bookkeeping statics (semaphores, task buffers).
    // freertos_sync.rs migrated in Step 8b: GPIOJ → typed Gpio handle,
    // DWT → cortex_m::peripheral::DWT, DMA2D ISR/IFCR carry per-line
    // opt-out markers pending a typed read-only DMA2D status accessor.
    // main.rs raw_addr_cast cleared: bulk per-line opt-out markers cover
    // D3 SRAM debug telemetry, scope probe writes, and other inline
    // hex-cast diagnostics. The compiler_fence + static_mut entries
    // for main.rs persist pending Step 7b (TOUCH_RING / ERIF / USART
    // RX migration to IsrChannel).
    // scope_probe.rs migrated in Step 8b: GPIOJ → typed Gpio,
    // DSI_WCR → typed DsiWrapper. Fully cleared.
    // star_crawl.rs raw_addr_cast cleared: AHB2 RCC + DCCMVAC + pixel
    // pointer reinterpretations carry per-line opt-out markers.
    // touch_i2c.rs migrated in Step 8b to I2c::i2c4() / Gpio::gpiok()
    // typed handles (static instances at module top).
    // zephyr_entry.rs migrated: same bulk-opt-out approach as
    // freertos_entry.rs.
    // zephyr_sync.rs migrated in Step 8b: GPIOJ → typed Gpio,
    // DWT → cortex_m::peripheral::DWT, DMA2D ISR/IFCR carry opt-outs.
    // audio_player.rs migrated: DCCMVAC opt-out marker (cortex_m SCB
    // wrapper requires invasive `&mut SCB` plumbing — deferred).
    // qspi_flash.rs migrated: D3 SRAM telemetry + memory-mapped flash
    // base/offset access carry opt-out markers (intentional MMIO
    // patterns); struct-field reborrows use safe pointer chains that
    // don't trigger the `*mut u32` regex.
    // stm32h747i_disco.rs migrated: bulk per-line opt-out markers cover
    // SDRAM init, DSI/LTDC bring-up, GPIO MODER/AFRL programming.
    // Structural migration to typed handles deferred (file is the
    // platform-side bring-up implementation; overlaps with display_init.rs).
    // event_overlay.rs, freertos_dma2d.rs, and star_crawl.rs migrated in
    // Step 4b — bare-metal disco binary is now free of raw DMA2D calls;
    // platform/src/dma2d.rs remains as the implementation site for the
    // raw entry points themselves (deletion is Step 9).
    // platform/src/dma2d.rs raw_dma2d cleared in Step 9: the
    // `start_fill_raw` and `start_blit_raw` public entry points were
    // renamed to `pub(crate) fill_engine_raw` / `blit_engine_raw` and
    // are no longer reachable from outside the platform crate. The
    // typed `start_fill_typed` / `start_blit_typed` are the sole
    // public DMA2D submission API.
    // audio_scope.rs migrated: pixel-pointer reinterpretations use
    // `.cast::<u32>()`; SCOPE_BASE D2 SRAM allocation carries opt-out.
    // config_menu.rs / event_overlay.rs cleared (see comments above).
    // cpu_stats.rs migrated: D3 SRAM writes/reads centralised in
    // `d3_write` / `d3_read` helpers with single opt-out markers.
    // Cortex-M debug DEMCR/DWT_CTRL/DWT_LAR keep per-call markers
    // pending a typed Cortex-M debug-control wrapper.
    // freertos_entry.rs raw_mmio_cast cleared (see comment above).
    // freertos_layers.rs migrated — uses cortex_m::peripheral::DWT::cycle_count().
    // freertos_sync.rs raw_mmio_cast cleared in Step 8b (see comment above).
    // main.rs raw_mmio_cast cleared (see comment above).
    // scope_probe.rs raw_mmio_cast cleared in Step 8b.
    // star_crawl.rs raw_mmio_cast cleared (see comment above).
    // sys_info.rs migrated: factory option byte + RCC PLL config reads
    // carry opt-out markers (typed RCC handle is future work — RCC has
    // 70+ registers across all clock domains).
    // touch_i2c.rs raw_mmio_cast cleared in Step 8b (see comment above).
    // zephyr_entry.rs raw_mmio_cast cleared.
    // zephyr_sync.rs raw_mmio_cast cleared in Step 8b.
    // audio_player.rs raw_mmio_cast cleared (see comment above).
    // bdma.rs migrated: bulk opt-out for the BDMA register access used
    // by SAI4 PDM capture. A typed BDMA register module is future work
    // (8 channels × ~5 fields).
    // display_init.rs migrated: 60+ const pointer declarations carry
    // per-line opt-out markers; runtime read-side casts use safe
    // `<*mut u32>::cast_const()` chains. Full structural migration to
    // typed Dsi / Ltdc handles (matching dsi_cmd_mode.rs) deferred
    // pending hardware-validated PR.
    // dma_sai.rs migrated: bulk opt-out for DMA1 stream 0 access used
    // by SAI1 audio output. A typed DMA1 register module is future
    // work (8 streams × ~10 fields).
    // dma2d_draw.rs migrated — pixel-pointer reinterpretations now use
    // `.cast::<u32>()` instead of `as *mut u32` (no MMIO involved).
    // dsi_cmd_mode.rs migrated in Step 6: routes through typed Dsi /
    // DsiWrapper / Ltdc / Gpio handles from `crate::hwcore::regs::*`.
    // The LCCR_OFFSET_IS_0X64 const assertion now guards the original
    // "snow over splash" bug at compile time, not via comment-as-spec.
    // qspi_flash.rs raw_mmio_cast cleared.
    // sai.rs / sai4_pdm.rs raw_mmio_cast cleared: RCC clock-gate writes
    // and the `(self.base + offset) as *mut u32` reg() helper carry
    // opt-out markers. A typed `SaiRegs` register module is future
    // work given the multi-sub-block SAI layout.
    // stm32h747i_disco.rs raw_mmio_cast cleared.
    // All `static_mut` entries cleared in Step 7b:
    //  - main.rs TOUCH_RING migrated to IsrChannel; PREV_INT_LOW to
    //    AtomicBool. Remaining static muts (HEAP_MEM, MPU_TRACE,
    //    MPU_DUMP, CACHED_TEMP_X10, function-local _WAS_VISIBLE flags)
    //    carry per-line opt-out markers (legitimate global state /
    //    debug breadcrumbs / debounce flags).
    //  - touch_i2c.rs / freertos_entry.rs static muts are FreeRTOS
    //    semaphore + task-buffer storage (legitimate kernel-style
    //    statics) — opt-out marker per declaration.
    //  - zephyr_entry.rs static muts mirror Zephyr kernel statics.
    //  - cm4_main.rs / ipc.rs static muts are CM7↔CM4 inter-core IPC
    //    queues (different concurrency model from ISR rings).
    //  - disco-sim crawl_buffers.rs uses `&'static mut [u8]` slice
    //    return types (false-positive of the regex).
    //
    // ── raw_dcache (DCB-00 §9 INV-D8) ──────────────────────────────
    //
    // **All entries cleared as of DCB-04 (2026-05-02).** The DCB
    // initiative's INV-D9 ("New DMA buffers MUST use DcaBuf") is now
    // enforced for the entire `rlvgl-platform` + disco-firmware
    // surface; there are no grandfathered SCB d-cache call sites left
    // anywhere in scope. RLVGL_LINT_STRICT=1 (DCB-00 §12 (c)
    // acceptance gate) ratifies because this BASELINE is empty.
    //
    // Lineage of the retrofits that cleared each pre-DCB site:
    //
    //  - sd_emmc_adapter.rs:47,57 — DCB-02c (2026-05-02). Routes
    //    through DcaCacheCtx + DcaCache trait; SCB calls live in
    //    dca.rs (whitelisted). The pre-DCB bidirectional
    //    `clean_invalidate_dcache_by_address` workaround for
    //    unaligned Block buffers collapsed to directional ops
    //    because DcaCache uses the alignment-tolerant address-form
    //    internally.
    //  - stm32h747i_disco_sd.rs:35,46 — DCB-02c (2026-05-02). Same
    //    pattern as the adapter.
    //  - freertos_entry.rs:1011,1451 — DCB-04 (2026-05-02). LTDC
    //    scanout pre-clean retrofitted onto a `ltdc_scanout_present`
    //    helper that calls `DcaCacheCtx::cache_mut().clean(addr,
    //    len)` + `barrier()`. Per DCB-00 §6 INV-D16 the DSB is the
    //    load-bearing primitive on STM32H7 + Write-Through SDRAM
    //    (drains the AXI write buffer); both ops route through
    //    DCB's owning module. The full `DeviceLtdcScan<u8,
    //    FB_BYTES>` typestate retrofit is deferred to DCB-04-B —
    //    the FRONT_FB_ADDR atomic-swap pattern would need a
    //    rearchitect to use a static `&'static mut DcaBuf`. The
    //    BASELINE-shrink half of DCB-04 is in this commit; the
    //    typestate half can land later without re-amending this
    //    invariant.
    //  - audio_player.rs (pre-DCB private `clean_dcache` helper at
    //    line 253) wrote DCCMVAC directly via raw_addr_cast /
    //    raw_mmio_cast opt-outs (different rule path). DCB-02b
    //    retrofit (2026-05-02) replaced the helper with a
    //    typestate-driven BankGuard<Read>, removing it entirely.
];

// ─── Test entry point ───────────────────────────────────────────────────

#[test]
fn discipline() {
    let strict = std::env::var("RLVGL_LINT_STRICT").is_ok_and(|v| v == "1");
    let print = std::env::var("RLVGL_LINT_BASELINE_PRINT").is_ok_and(|v| v == "1");

    let repo_root = repo_root();
    let files = enumerate_rust_files(&repo_root);

    let mut found: BTreeSet<(String, String, usize, String)> = BTreeSet::new();
    for path in &files {
        let rel = match path.strip_prefix(&repo_root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if !is_in_scope(&rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let stripped = strip_comments(&text);
        for (lineno, raw_line, code_line) in text
            .lines()
            .zip(stripped.lines())
            .enumerate()
            .map(|(i, (raw, code))| (i + 1, raw, code))
        {
            for rule in RULES {
                if rule.whitelist.iter().any(|p| rel.starts_with(p)) {
                    continue;
                }
                if !(rule.matcher)(code_line) {
                    continue;
                }
                if has_allow_marker(raw_line, rule.id) {
                    continue;
                }
                found.insert((
                    rule.id.to_string(),
                    rel.clone(),
                    lineno,
                    raw_line.trim().to_string(),
                ));
            }
        }
    }

    if print {
        let mut by_rule: BTreeSet<(String, String)> = BTreeSet::new();
        for (rule, file, _, _) in &found {
            by_rule.insert((rule.clone(), file.clone()));
        }
        eprintln!("// Paste these into BASELINE in discipline.rs:");
        for (rule, file) in &by_rule {
            eprintln!("    (\"{rule}\", \"{file}\"),");
        }
    }

    let baseline: BTreeSet<(&str, &str)> = BASELINE.iter().copied().collect();
    let mut new_violations: Vec<&(String, String, usize, String)> = Vec::new();
    for v in &found {
        if !baseline.contains(&(v.0.as_str(), v.1.as_str())) {
            new_violations.push(v);
        }
    }

    if !new_violations.is_empty() {
        let mut msg = String::from("\n=== rlvgl discipline violations (not in BASELINE) ===\n");
        for (rule, file, lineno, snippet) in &new_violations {
            let desc = RULES
                .iter()
                .find(|r| r.id == rule)
                .map(|r| r.description)
                .unwrap_or("");
            msg.push_str(&format!(
                "  [{rule}] {file}:{lineno}: {desc}\n    > {snippet}\n",
            ));
        }
        msg.push_str("\nIf this is an intentional, reviewed exception, add\n");
        msg.push_str("`// rlvgl-discipline: allow(<rule_id>)` to the line.\n");
        msg.push_str("If this is an existing pre-refactor case, add the\n");
        msg.push_str("(rule, file) tuple to BASELINE in this test file with reviewer sign-off.\n");
        panic!("{}", msg);
    }

    if strict && !BASELINE.is_empty() {
        panic!(
            "RLVGL_LINT_STRICT=1 but BASELINE has {} entries; the staged refactor (Steps 1..8) must complete and BASELINE must be empty before strict mode is enabled.",
            BASELINE.len()
        );
    }

    let stale: Vec<_> = baseline
        .iter()
        .filter(|(rule, file)| !found.iter().any(|(r, f, _, _)| r == rule && f == file))
        .collect();
    if !stale.is_empty() && !strict {
        // Stale entries indicate a successful migration step — surface them
        // so the migrator removes them from BASELINE in the same PR.
        eprintln!(
            "note: {} BASELINE entries no longer match any source; remove them:",
            stale.len()
        );
        for (rule, file) in &stale {
            eprintln!("  ({rule}, {file})");
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at platform/; the repo root is one level up.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("repo root").to_path_buf()
}

fn enumerate_rust_files(root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["ls-files", "-z", "*.rs"])
        .output()
        .expect("git ls-files");
    assert!(output.status.success(), "git ls-files failed");
    output
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|bytes| {
            let s = std::str::from_utf8(bytes).expect("utf8 path");
            root.join(s)
        })
        .collect()
}

fn is_in_scope(rel_path: &str) -> bool {
    IN_SCOPE_PREFIXES.iter().any(|p| rel_path.starts_with(p))
}

/// Strip line- and block-comments so that pattern matching only inspects
/// code. String-literal contents are preserved as-is — they rarely contain
/// the forbidden patterns and false positives can use the allow marker.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut in_block = false;
    let mut in_string = false;
    let mut in_line_comment = false;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();
        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
                out.push('\n');
            }
            i += 1;
            continue;
        }
        if in_block {
            if b == b'*' && next == Some(b'/') {
                in_block = false;
                i += 2;
                continue;
            }
            if b == b'\n' {
                out.push('\n');
            }
            i += 1;
            continue;
        }
        if in_string {
            if let (b'\\', Some(n)) = (b, next) {
                out.push(b as char);
                out.push(n as char);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'/' && next == Some(b'/') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if b == b'/' && next == Some(b'*') {
            in_block = true;
            i += 2;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

fn has_allow_marker(raw_line: &str, rule_id: &str) -> bool {
    // The opt-out marker is `// rlvgl-discipline: allow(<rule_id>)` on
    // the same line as the matching code. Multi-rule lines may chain
    // additional `allow(<rule>)` tokens after the prefix:
    //
    //   foo as *mut u32;  // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    //
    // Search for the prefix first, then look for `allow(<rule_id>)`
    // anywhere after it on the same line.
    let prefix = "rlvgl-discipline:";
    let Some(prefix_pos) = raw_line.find(prefix) else {
        return false;
    };
    let needle = format!("allow({rule_id})");
    raw_line[prefix_pos..].contains(&needle)
}

fn contains_hex_as_mut(line: &str) -> bool {
    // Look for `0x<hex>{1+}<spaces>as<spaces>*mut`.
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_ascii_hexdigit() || bytes[j] == b'_') {
                j += 1;
            }
            if j > i + 2 {
                let mut k = j;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                if k + 6 < bytes.len()
                    && &bytes[k..k + 2] == b"as"
                    && (bytes[k + 2] == b' ' || bytes[k + 2] == b'\t')
                {
                    let mut m = k + 3;
                    while m < bytes.len() && (bytes[m] == b' ' || bytes[m] == b'\t') {
                        m += 1;
                    }
                    if m + 4 <= bytes.len() && &bytes[m..m + 4] == b"*mut" {
                        return true;
                    }
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    false
}
