# ── Canonical build variables ─────────────────────────────────────
# STM32H747I-DISCO firmware (Cortex-M7 / M4 dual core)
PACKAGE       := rlvgl-example-disco
BIN_CM7       := rlvgl-stm32h747i-disco
BIN_CM4       := rlvgl-stm32h747i-disco-cm4
TARGET        := thumbv7em-none-eabihf
CPU_CM7       := cortex-m7
CPU_CM4       := cortex-m4
FEATURES_CM7  := cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio
FEATURES_CM4  := cm4
CHIP          := STM32H747XIHx
FLASH_BASE    := 0x08000000
OBJCOPY       := rust-objcopy
PROBE_ID      ?= 0483:3754:004F00273133510837363734
PROBE_SPEED   ?= 1000
DISCO_EVIDENCE_DIR      := target/mpy08-evidence
DISCO_STATIC_EVIDENCE   := $(DISCO_EVIDENCE_DIR)/stm32h747i-disco-pair.json
DISCO_PHYSICAL_EVIDENCE := $(DISCO_EVIDENCE_DIR)/stm32h747i-disco-physical.json
DISCO_SETTLE_SECONDS    ?= 5
DISCO_CAPTURE_TIMEOUT   ?= 10

# Host simulators and tools
SIM_PACKAGE        := rlvgl-example-sim
SIM_BIN            := rlvgl-sim
SIM_FEATURES       := png,jpeg,gif,qrcode,fontdue
DISCO_SIM_PACKAGE  := rlvgl-example-disco-sim
DISCO_SIM_BIN      := rlvgl-disco-sim
CREATOR_PACKAGE    := rlvgl
CREATOR_BIN        := rlvgl-creator
CREATOR_FEATURES   := creator

# UEFI simulation (excluded from workspace)
UEFI_MANIFEST      := examples/uefi-disco/Cargo.toml
UEFI_TARGET        := aarch64-unknown-uefi
UEFI_BIN           := rlvgl-uefi-disco

# Host triple — detected from rustc, used for locating host binaries.
HOST_TRIPLE := $(shell rustc -vV | sed -n 's/^host: //p')

# Pinned MPY-07 host proof.
MPY_MICROPYTHON_COMMIT := e0e9fbb17ed6fd06bb76e266ae554784c9c80804
MPY_UNIX_DIR := vendor/micropython/ports/unix
MPY_HOST_BUILD := build-rlvgl-standard
MPY_HOST_BIN := $(MPY_UNIX_DIR)/$(MPY_HOST_BUILD)/micropython
MPY_HOST_CC ?= cc

# Derived paths
ELF_CM7       := target/$(TARGET)/debug/$(BIN_CM7)
ELF_CM7_REL   := target/$(TARGET)/release/$(BIN_CM7)
ELF_CM4       := target/$(TARGET)/debug/$(BIN_CM4)
DISCO_SIM_ELF := target/$(HOST_TRIPLE)/debug/$(DISCO_SIM_BIN)

.PHONY: help build-disco build-disco-release build-disco-cm4 build-disco-all \
	build-disco-freertos flash-disco-freertos release-artifacts \
	objcopy-disco objcopy-disco-release objcopy-disco-cm4 verify-disco-pair \
	test-disco-pair-tools \
	flash-disco flash-disco-cm4 flash-disco-all flash-disco-hex flash-disco-bin \
	capture-disco-pair \
	probe-rs-gdb \
	openocd openocd-dual openocd-erase \
	gen-stm32h747i-disco-bsp \
	build-sim build-disco-sim build-uefi-disco build-creator build-all-bins \
	test-disco-sim test-uefi-disco test-stm32h747i-disco test-playit-all \
	test-disco-demo test-disco-sim-rust \
	translate-all-i18n translate-all-i18n-force translate-all-i18n-dry-run \
	extract-i18n-keys extract-icons import-icons

.PHONY: mpy-host-test

help:
	@echo "STM32H747I-DISCO firmware:"
	@echo "  make build-disco              # Build CM7 debug + generate .hex/.bin"
	@echo "  make build-disco-release      # Build CM7 release + generate .hex/.bin"
	@echo "  make build-disco-cm4          # Build CM4 debug + generate .hex"
	@echo "  make build-disco-all          # Build and verify the paired CM7/CM4 images"
	@echo "  make verify-disco-pair        # Verify CPU, flash-bank, mailbox, and artifact metadata"
	@echo "  make test-disco-pair-tools    # Run host tests for physical capture accounting"
	@echo "  make build-disco-freertos     # Build FreeRTOS preemptive task variant"
	@echo "  make flash-disco-freertos     # Build + flash FreeRTOS variant"
	@echo "  make release-artifacts        # Build all platforms (release) into release/"
	@echo ""
	@echo "Host simulators and tools:"
	@echo "  make build-sim                # Build rlvgl-sim (generic simulator)"
	@echo "  make build-disco-sim          # Build rlvgl-disco-sim (disco demo simulator)"
	@echo "  make build-creator            # Build rlvgl-creator (asset/project tool)"
	@echo "  make build-uefi-disco         # Build rlvgl-uefi-disco (aarch64-unknown-uefi)"
	@echo "  make build-all-bins           # Build every binary target above"
	@echo ""
	@echo "Playit test runners:"
	@echo "  make test-disco-sim           # Rust + Node.js playit tests vs disco-sim"
	@echo "  make test-disco-demo          # Disco-demo unit tests (no_std controller)"
	@echo "  make test-uefi-disco          # Headless QEMU + playit tests vs UEFI"
	@echo "  make test-stm32h747i-disco    # Serial bridge + playit tests vs hardware"
	@echo "  make test-playit-all          # Run all playit test suites in sequence"
	@echo ""
	@echo "Flash & debug:"
	@echo "  make flash-disco              # Build + flash CM7 via probe-rs (ELF)"
	@echo "  make flash-disco-cm4          # Build + flash CM4 via probe-rs (ELF)"
	@echo "  make flash-disco-all          # Build, verify, flash both cores, then reset"
	@echo "  make capture-disco-pair       # Flash pair + capture physical ring progress"
	@echo "  make flash-disco-hex          # Flash from .hex"
	@echo "  make flash-disco-bin          # Flash from .bin (with base address)"
	@echo "  make probe-rs-gdb             # Flash + launch probe-rs GDB server"
	@echo ""
	@echo "OpenOCD:"
	@echo "  make openocd                  # Start OpenOCD (ST-Link + STM32H7)"
	@echo "  make openocd-dual             # Dual-core cfg (CM7 on 3333, CM4 on 3334)"
	@echo "  make openocd-erase            # Full chip erase (DANGER)"
	@echo ""
	@echo "BSP generation:"
	@echo "  make gen-stm32h747i-disco-bsp # Regenerate H747I-DISCO BSP (SMPS/VOS1)"
	@echo ""
	@echo "Zephyr build (STM32H747I-DISCO via shield mb1166-a09):"
	@echo "  make zephyr-sdk-install       # Download + register Zephyr SDK (first time)"
	@echo "  make zephyr-disco-lib         # Build Rust staticlib for Zephyr (video mode)"
	@echo "  make zephyr-disco-lib-acm     # Build Rust staticlib (adapted_cmd feature)"
	@echo "  make zephyr-disco             # Build Zephyr image (video mode)"
	@echo "  make zephyr-disco-acm         # Build Zephyr image (adapted command mode)"
	@echo "  make zephyr-disco-flash       # Flash zephyr.elf via probe-rs"
	@echo "  make zephyr-disco-help        # Show env requirements + manual commands"
	@echo ""
	@echo "i18n translation:"
	@echo "  make translate-locale-de       # Translate en.json to German"
	@echo "  make translate-all-i18n        # Translate en.json to all locales (skip existing)"
	@echo "  make translate-all-i18n-force  # Regenerate all locale translations"
	@echo "  make translate-all-i18n-dry-run# Preview translation targets"
	@echo "  make extract-i18n-keys         # Sync keys from Rust source to en.json"
	@echo ""
	@echo "Icon extraction:"
	@echo "  make extract-icons             # Extract Lucide SVGs to assets/icons/"
	@echo "  make import-icons              # Extract + convert to .raw via creator"
	@echo ""
	@echo "Documentation index:"
	@echo "  make spec-index                # Regenerate the committed rlvgl index"
	@echo "  make spec-index-check          # Verify regeneration is a no-op"
	@echo "  make spec-index-report         # Print diagnostic corpus findings"
	@echo "  make spec-test                 # Run local index unit/regression tests"
	@echo ""
	@echo "MicroPython proof:"
	@echo "  make mpy-host-test             # Clean pinned v1.28 Unix host build + import proof"

# ── Documentation object index ───────────────────────────────────
# This is subrepo-owned. The parent softoboros scanner intentionally excludes
# submodules and consumes this committed projection through separate tooling.
.PHONY: spec-index spec-index-check spec-index-report spec-suspect spec-test

spec-index:
	python3 scripts/specidx/scan.py --emit-index

spec-index-check:
	python3 scripts/specidx/scan.py --check-index

spec-index-report:
	python3 scripts/specidx/scan.py

spec-suspect:
	python3 scripts/specidx/suspect.py

spec-test:
	python3 scripts/specidx/test_scan.py
	python3 scripts/specidx/test_suspect.py

# ── MicroPython host proof ────────────────────────────────────────
# A dedicated build directory keeps the proof isolated from ordinary Unix-port
# development. Cleaning it on every invocation makes compiler flags, toolchain
# changes, and Rust target configuration part of the rebuilt evidence.
mpy-host-test:
	@test "$$(git -C vendor/micropython rev-parse HEAD)" = "$(MPY_MICROPYTHON_COMMIT)"
	@test -z "$$(git -C vendor/micropython status --porcelain --untracked-files=no)"
	@echo "micropython_commit=$(MPY_MICROPYTHON_COMMIT)"
	@echo "micropython_variant=standard"
	@echo "rust_target=$(HOST_TRIPLE)"
	@rustc -vV
	@$(MPY_HOST_CC) --version | head -1
	$(MAKE) -C $(MPY_UNIX_DIR) BUILD=$(MPY_HOST_BUILD) clean
	$(MAKE) -C $(MPY_UNIX_DIR) -j2 \
		BUILD=$(MPY_HOST_BUILD) VARIANT=standard CC=$(MPY_HOST_CC) \
		USER_C_MODULES="$(CURDIR)/micropython" RLVGL_RUST_TARGET=$(HOST_TRIPLE)
	$(MPY_HOST_BIN) micropython/tests/test_module_imports.py
	$(MPY_HOST_BIN) micropython/tests/test_exception_hook.py
	@shasum -a 256 $(MPY_HOST_BIN)

# ── Build ─────────────────────────────────────────────────────────
build-disco:
	RUSTFLAGS="-C target-cpu=$(CPU_CM7)" \
	cargo build --target $(TARGET) \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_CM7)
	$(MAKE) objcopy-disco

build-disco-release:
	RUSTFLAGS="-C target-cpu=$(CPU_CM7)" \
	cargo build --target $(TARGET) --release \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_CM7)
	$(MAKE) objcopy-disco-release

build-disco-cm4:
	RUSTFLAGS="-C target-cpu=$(CPU_CM4)" \
	cargo build --target $(TARGET) \
	  -p $(PACKAGE) --bin $(BIN_CM4) --features $(FEATURES_CM4)
	$(MAKE) objcopy-disco-cm4

build-disco-all: verify-disco-pair

# FreeRTOS build: preemptive present / render / touch tasks.
FEATURES_CM7_FREERTOS := cm7,freertos,adapted_cmd,dma2d,splash,desktop

build-disco-freertos:
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_CM7_FREERTOS)
	$(MAKE) objcopy-disco

# ── Host simulators and tools ─────────────────────────────────────
build-sim:
	RUSTFLAGS="" cargo build -p $(SIM_PACKAGE) --bin $(SIM_BIN) --features $(SIM_FEATURES)

build-disco-sim:
	RUSTFLAGS="" cargo build -p $(DISCO_SIM_PACKAGE) --bin $(DISCO_SIM_BIN)

build-creator:
	RUSTFLAGS="" cargo build -p $(CREATOR_PACKAGE) --bin $(CREATOR_BIN) --features $(CREATOR_FEATURES)

build-uefi-disco:
	cargo build --manifest-path $(UEFI_MANIFEST) --target $(UEFI_TARGET) --bin $(UEFI_BIN)

build-all-bins: build-sim build-disco-sim build-creator build-uefi-disco build-disco build-disco-cm4

# ── Playit test runners ───────────────────────────────────────────
test-disco-demo:
	RUSTFLAGS="" cargo test -p rlvgl-app-disco-demo

test-disco-sim-rust:
	RUSTFLAGS="" cargo test -p $(DISCO_SIM_PACKAGE)

test-disco-sim: build-disco-sim test-disco-demo test-disco-sim-rust
	cd playit/node && RLVGL_DISCO_SIM_BIN="$(CURDIR)/$(DISCO_SIM_ELF)" node --test test/disco-sim.test.js test/disco-navigation.test.js

test-uefi-disco:
	bash scripts/test-uefi-aarch64-playit.sh

test-stm32h747i-disco:
	bash scripts/test-stm32h747i-disco-playit.sh

test-playit-all: test-disco-sim test-uefi-disco test-stm32h747i-disco

# ── Objcopy (hex + trimmed bin) ───────────────────────────────────
objcopy-disco:
	$(OBJCOPY) -O ihex $(ELF_CM7) $(ELF_CM7).hex
	$(OBJCOPY) -O binary -R .noinit $(ELF_CM7) $(ELF_CM7).bin
	@echo "── artifacts ──"
	@ls -lh $(ELF_CM7) $(ELF_CM7).hex $(ELF_CM7).bin

objcopy-disco-release:
	$(OBJCOPY) -O ihex $(ELF_CM7_REL) $(ELF_CM7_REL).hex
	$(OBJCOPY) -O binary -R .noinit $(ELF_CM7_REL) $(ELF_CM7_REL).bin
	@echo "── artifacts ──"
	@ls -lh $(ELF_CM7_REL) $(ELF_CM7_REL).hex $(ELF_CM7_REL).bin

objcopy-disco-cm4:
	$(OBJCOPY) -O ihex $(ELF_CM4) $(ELF_CM4).hex
	@echo "── CM4 artifact ──"
	@ls -lh $(ELF_CM4) $(ELF_CM4).hex

verify-disco-pair: build-disco build-disco-cm4
	python3 scripts/test_capture_stm32h747i_disco_pair.py
	python3 scripts/verify_stm32h747i_disco_pair.py \
	  --cm7 $(ELF_CM7) --cm4 $(ELF_CM4) \
	  --json-out $(DISCO_STATIC_EVIDENCE)

test-disco-pair-tools:
	python3 scripts/test_capture_stm32h747i_disco_pair.py

# ── Release artifacts ────────────────────────────────────────────
#
# Build all platform variants in release mode, rename with platform
# suffix, and stage in release/. Suitable for attaching to a GitHub
# release via `gh release upload`.
#
# Usage:
#   make release-artifacts
#   gh release create v0.2.0 release/*
#
RELEASE_DIR       := release
FEATURES_REL_BM   := cm7,dma2d,splash,desktop,audio
FEATURES_REL_RTOS := cm7,freertos,adapted_cmd,dma2d,splash,desktop

release-artifacts:
	@rm -rf $(RELEASE_DIR) && mkdir -p $(RELEASE_DIR)
	@echo "── bare-metal release ──"
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) --release \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_REL_BM)
	$(OBJCOPY) -O ihex $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-bare-metal.hex
	$(OBJCOPY) -O binary -R .noinit $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-bare-metal.bin
	cp $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-bare-metal.elf
	@echo "── FreeRTOS release ──"
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) --release \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_REL_RTOS)
	$(OBJCOPY) -O ihex $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-freertos.hex
	$(OBJCOPY) -O binary -R .noinit $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-freertos.bin
	cp $(ELF_CM7_REL) $(RELEASE_DIR)/$(BIN_CM7)-freertos.elf
	@echo "── manifest ──"
	@ls -lh $(RELEASE_DIR)/
	@echo ""
	@echo "Upload: gh release create <tag> $(RELEASE_DIR)/*"

# ── Flash (probe-rs) ─────────────────────────────────────────────
flash-disco: build-disco
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset \
	  --probe $(PROBE_ID) $(ELF_CM7)
	probe-rs reset --chip $(CHIP) --probe $(PROBE_ID)

flash-disco-cm4: build-disco-cm4
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset --verify \
	  --probe $(PROBE_ID) $(ELF_CM4)
	probe-rs reset --chip $(CHIP) --probe $(PROBE_ID)

flash-disco-all: build-disco-all
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset --verify \
	  --probe $(PROBE_ID) $(ELF_CM7)
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset --verify \
	  --probe $(PROBE_ID) $(ELF_CM4)
	probe-rs reset --chip $(CHIP) --probe $(PROBE_ID)

capture-disco-pair: flash-disco-all
	python3 scripts/capture_stm32h747i_disco_pair.py \
	  --chip $(CHIP) --probe $(PROBE_ID) --speed $(PROBE_SPEED) \
	  --settle-seconds $(DISCO_SETTLE_SECONDS) \
	  --timeout-seconds $(DISCO_CAPTURE_TIMEOUT) \
	  --static-evidence $(DISCO_STATIC_EVIDENCE) \
	  --json-out $(DISCO_PHYSICAL_EVIDENCE)

flash-disco-freertos: build-disco-freertos
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset \
	  --probe $(PROBE_ID) $(ELF_CM7)
	probe-rs reset --chip $(CHIP) --probe $(PROBE_ID)

flash-disco-hex: build-disco
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset \
	  --probe $(PROBE_ID) \
	  --binary-format iHex $(ELF_CM7).hex

flash-disco-bin: build-disco
	probe-rs download --chip $(CHIP) \
	  --protocol swd --speed $(PROBE_SPEED) \
	  --non-interactive --connect-under-reset \
	  --probe $(PROBE_ID) \
	  --binary-format bin --base-address $(FLASH_BASE) $(ELF_CM7).bin

# ── Debug (build + flash + GDB server) ───────────────────────────
probe-rs-gdb: flash-disco
	probe-rs gdb --chip $(CHIP) \
	  --protocol swd --speed 50 \
	  --non-interactive --connect-under-reset --reset-halt \
	  --probe $(PROBE_ID)

# ── BSP generation ───────────────────────────────────────────────
gen-stm32h747i-disco-bsp:
	STM32_PWR_SUPPLY=SMPS STM32_PWR_SDLEVEL=VOS1 \
		./examples/stm32h747i-disco/gen-bsp.sh

# ── OpenOCD ──────────────────────────────────────────────────────
openocd:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c init -c "reset halt"

openocd-dual:
	openocd -f openocd/stm32h747_dual_core.cfg

openocd-erase:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
	  -c init -c "reset halt" -c "stm32h7x mass_erase 0" -c shutdown

# ── i18n translation ─────────────────────────────────────────────
translate-locale-%:
	python3 i18n/translate_locale.py --locale $*

translate-all-i18n:
	python3 i18n/translate_locale.py --locale all

translate-all-i18n-force:
	python3 i18n/translate_locale.py --locale all --force

translate-all-i18n-dry-run:
	python3 i18n/translate_locale.py --locale all --dry-run

extract-i18n-keys:
	python3 i18n/extract_keys.py

# ── Icon extraction ──────────────────────────────────────────────
extract-icons:
	npx tsx scripts/extract-lucide-icons.ts

import-icons: extract-icons
	@for svg in assets/icons/*.svg; do \
		cargo run --bin rlvgl-creator --features creator -- svg "$$svg" assets/icons/raw --dpi 96; \
	done
	@mkdir -p assets/icons/rle
	@for raw in assets/icons/raw/*.raw; do \
		name=$$(basename "$$raw" .raw); \
		cargo run --bin rlvgl-creator --features creator -- compress "$$raw" "assets/icons/rle/$${name}.rle"; \
	done

# ── Zephyr build for STM32H747I-DISCO ────────────────────────────────
#
# Requires:
#   - Zephyr (zephyrproject) checked out at $(ZEPHYR_BASE) (default ~/zephyrproject)
#   - west, cmake, ninja, python3 in PATH
#   - Zephyr SDK installed at $(ZEPHYR_SDK_INSTALL_DIR), registered with
#     cmake via `setup.sh -c` (creates ~/.cmake/packages/Zephyr-sdk)
#
# Default toolchain is `zephyr` — uses the SDK's bundled arm-zephyr-eabi
# (or aarch64-zephyr-elf for AArch64 boards). Override with
# `ZEPHYR_TOOLCHAIN_VARIANT=gnuarmemb GNUARMEMB_TOOLCHAIN_PATH=...` to
# use the stm32cube bundled GCC instead.
#
# First-time SDK install (if not already done): see `make zephyr-sdk-install`.
# Zephyr 4.1.x requires SDK 0.16+; SDK 1.0+ refuses pre-1.0 requests, so we
# pin to 0.17.4. Override ZEPHYR_SDK_VERSION below to upgrade.
ZEPHYR_BASE             ?= $(HOME)/zephyrproject
ZEPHYR_BUILD            ?= $(ZEPHYR_BASE)/build
ZEPHYR_APP              := $(CURDIR)/examples/stm32h747i-disco/zephyr
ZEPHYR_BOARD            := stm32h747i_disco/stm32h747xx/m7
ZEPHYR_SHIELD           := st_b_lcd40_dsi1_mb1166_a09
ZEPHYR_OVERLAY_ACM      := $(ZEPHYR_APP)/adapted_cmd.overlay
ZEPHYR_SDK_INSTALL_DIR  ?= $(HOME)/zephyr-sdk-0.16.8
# Default to SDK's `zephyr` toolchain. Override to `gnuarmemb` to use
# external arm-none-eabi-gcc (e.g. stm32cube bundle).
ZEPHYR_TOOLCHAIN_VARIANT?= zephyr
GNUARMEMB_TOOLCHAIN_PATH?= $(HOME)/Library/Application Support/stm32cube/bundles/gnu-tools-for-stm32/13.3.1+st.9

# Cargo features for the Rust staticlib that Zephyr links in.
ZEPHYR_DISCO_FEATURES     := cm7,splash,desktop,dma2d,zephyr
ZEPHYR_DISCO_FEATURES_ACM := $(ZEPHYR_DISCO_FEATURES),adapted_cmd

# Common cargo build invocation for the staticlib.
ZEPHYR_RUSTFLAGS := -C target-cpu=cortex-m7
ZEPHYR_CARGO_TARGET := thumbv7em-none-eabihf
ZEPHYR_LIB := target/$(ZEPHYR_CARGO_TARGET)/debug/librlvgl_example_disco.a

zephyr-disco-help:
	@echo "Zephyr build for STM32H747I-DISCO via mb1166-a09 shield."
	@echo ""
	@echo "Targets:"
	@echo "  make zephyr-disco-lib       — Rust staticlib (features: $(ZEPHYR_DISCO_FEATURES))"
	@echo "  make zephyr-disco-lib-acm   — Rust staticlib (adds adapted_cmd)"
	@echo "  make zephyr-disco           — Zephyr image, video mode"
	@echo "  make zephyr-disco-acm       — Zephyr image, adapted command mode"
	@echo "  make zephyr-disco-flash     — probe-rs download zephyr.elf + reset"
	@echo "  make zephyr-disco-clean     — Remove Zephyr build directory"
	@echo "  make zephyr-disco-sdk-check — Verify Zephyr SDK is registered with cmake"
	@echo ""
	@echo "Variables (override on command line):"
	@echo "  ZEPHYR_BASE              = $(ZEPHYR_BASE)"
	@echo "  ZEPHYR_BUILD             = $(ZEPHYR_BUILD)"
	@echo "  ZEPHYR_BOARD             = $(ZEPHYR_BOARD)"
	@echo "  ZEPHYR_SHIELD            = $(ZEPHYR_SHIELD)"
	@echo "  ZEPHYR_SDK_INSTALL_DIR   = $(ZEPHYR_SDK_INSTALL_DIR)"
	@echo "  ZEPHYR_TOOLCHAIN_VARIANT = $(ZEPHYR_TOOLCHAIN_VARIANT)"
	@echo "  GNUARMEMB_TOOLCHAIN_PATH = $(GNUARMEMB_TOOLCHAIN_PATH)"
	@echo ""
	@echo "Notes:"
	@echo "  - Default toolchain variant is 'zephyr' (uses the SDK toolchain)."
	@echo "    Override with ZEPHYR_TOOLCHAIN_VARIANT=gnuarmemb to use the stm32cube"
	@echo "    bundled arm-none-eabi-gcc instead."
	@echo "  - Adapted command mode disables Zephyr's video-mode DSI/LTDC drivers"
	@echo "    via adapted_cmd.overlay; Rust does the full DSI bring-up using"
	@echo "    platform/src/display_init.rs. Needed for DMA2D M2M (star crawl)"
	@echo "    which deadlocks under Zephyr's continuous-scan video mode."
	@echo "  - First-time SDK install: 'make zephyr-sdk-install' downloads"
	@echo "    the minimal SDK + arm-zephyr-eabi + xtensa-espressif toolchains"
	@echo "    + aarch64-zephyr-elf and registers with cmake."

zephyr-disco-sdk-check:
	@if [ ! -d "$(ZEPHYR_SDK_INSTALL_DIR)" ]; then \
		echo "ERROR: Zephyr SDK not found at $(ZEPHYR_SDK_INSTALL_DIR)"; \
		echo "Run 'make zephyr-sdk-install' to install."; \
		exit 1; \
	fi
	@if [ ! -d "$(HOME)/.cmake/packages/Zephyr-sdk" ]; then \
		echo "WARNING: Zephyr SDK not registered with cmake."; \
		echo "Run: cd $(ZEPHYR_SDK_INSTALL_DIR) && ./setup.sh -c"; \
		exit 1; \
	fi
	@echo "OK: Zephyr SDK at $(ZEPHYR_SDK_INSTALL_DIR), registered with cmake."

# Zephyr SDK install — minimal SDK + arm-zephyr-eabi (Cortex-M),
# aarch64-zephyr-elf (ARM 64-bit), and xtensa toolchains for ESP32.
# Other targets (riscv64, microblaze, etc.) can be added as needed.
ZEPHYR_SDK_VERSION ?= 0.16.8
ZEPHYR_SDK_HOST    ?= macos-aarch64
ZEPHYR_SDK_BASE    := https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v$(ZEPHYR_SDK_VERSION)
ZEPHYR_SDK_TCS     ?= arm-zephyr-eabi aarch64-zephyr-elf \
                       xtensa-espressif_esp32_zephyr-elf \
                       xtensa-espressif_esp32s2_zephyr-elf \
                       xtensa-espressif_esp32s3_zephyr-elf

zephyr-sdk-install:
	@echo "Installing Zephyr SDK $(ZEPHYR_SDK_VERSION) ($(ZEPHYR_SDK_HOST))"
	@echo "Toolchains: $(ZEPHYR_SDK_TCS)"
	@cd $(HOME) && \
	  curl -fL -# -o zephyr-sdk-$(ZEPHYR_SDK_VERSION)_$(ZEPHYR_SDK_HOST)_minimal.tar.xz \
	    $(ZEPHYR_SDK_BASE)/zephyr-sdk-$(ZEPHYR_SDK_VERSION)_$(ZEPHYR_SDK_HOST)_minimal.tar.xz && \
	  tar xf zephyr-sdk-$(ZEPHYR_SDK_VERSION)_$(ZEPHYR_SDK_HOST)_minimal.tar.xz
	@for tc in $(ZEPHYR_SDK_TCS); do \
	  echo "==> downloading $$tc"; \
	  cd $(HOME) && \
	    curl -fL -# -o tc_$$tc.tar.xz \
	      $(ZEPHYR_SDK_BASE)/toolchain_$(ZEPHYR_SDK_HOST)_$$tc.tar.xz && \
	    tar xf tc_$$tc.tar.xz -C zephyr-sdk-$(ZEPHYR_SDK_VERSION) && \
	    rm tc_$$tc.tar.xz; \
	done
	@cd $(HOME)/zephyr-sdk-$(ZEPHYR_SDK_VERSION) && ./setup.sh -c
	@echo ""
	@echo "Zephyr SDK $(ZEPHYR_SDK_VERSION) installed at $(HOME)/zephyr-sdk-$(ZEPHYR_SDK_VERSION)"
	@echo "Run 'make zephyr-disco-sdk-check' to verify."

zephyr-disco-lib:
	RUSTFLAGS="$(ZEPHYR_RUSTFLAGS)" cargo build \
		--target $(ZEPHYR_CARGO_TARGET) \
		-p rlvgl-example-disco --lib \
		--features $(ZEPHYR_DISCO_FEATURES)

zephyr-disco-lib-acm:
	RUSTFLAGS="$(ZEPHYR_RUSTFLAGS)" cargo build \
		--target $(ZEPHYR_CARGO_TARGET) \
		-p rlvgl-example-disco --lib \
		--features $(ZEPHYR_DISCO_FEATURES_ACM)

# Video-mode Zephyr build. Stock STM32 DSI driver in continuous video scan.
zephyr-disco: zephyr-disco-lib zephyr-disco-sdk-check
	cd $(ZEPHYR_BASE) && env \
		ZEPHYR_SDK_INSTALL_DIR=$(ZEPHYR_SDK_INSTALL_DIR) \
		ZEPHYR_TOOLCHAIN_VARIANT=$(ZEPHYR_TOOLCHAIN_VARIANT) \
		"GNUARMEMB_TOOLCHAIN_PATH=$(GNUARMEMB_TOOLCHAIN_PATH)" \
		west build -p auto -b $(ZEPHYR_BOARD) $(ZEPHYR_APP) \
			-- -DSHIELD=$(ZEPHYR_SHIELD)

# Adapted command mode build. Rust does full DSI+LTDC init from scratch
# (display_init.rs); Zephyr provides clocks, SDRAM, GPIO, I2C, kernel.
zephyr-disco-acm: zephyr-disco-lib-acm zephyr-disco-sdk-check
	cd $(ZEPHYR_BASE) && env \
		ZEPHYR_SDK_INSTALL_DIR=$(ZEPHYR_SDK_INSTALL_DIR) \
		ZEPHYR_TOOLCHAIN_VARIANT=$(ZEPHYR_TOOLCHAIN_VARIANT) \
		"GNUARMEMB_TOOLCHAIN_PATH=$(GNUARMEMB_TOOLCHAIN_PATH)" \
		west build -p auto -b $(ZEPHYR_BOARD) $(ZEPHYR_APP) \
			-- -DSHIELD=$(ZEPHYR_SHIELD) \
			   -DEXTRA_DTC_OVERLAY_FILE=$(ZEPHYR_OVERLAY_ACM)

zephyr-disco-flash:
	probe-rs download --chip STM32H747XIHx $(ZEPHYR_BUILD)/zephyr/zephyr.elf
	probe-rs reset --chip STM32H747XIHx

zephyr-disco-clean:
	rm -rf $(ZEPHYR_BUILD)

.PHONY: zephyr-disco-lib zephyr-disco-lib-acm zephyr-disco zephyr-disco-acm \
        zephyr-disco-flash zephyr-disco-clean zephyr-disco-help \
        zephyr-disco-sdk-check zephyr-sdk-install
