# ── Canonical build variables ─────────────────────────────────────
# STM32H747I-DISCO firmware (Cortex-M7 / M4 dual core)
PACKAGE       := rlvgl-example-disco
BIN_CM7       := rlvgl-stm32h747i-disco
BIN_CM4       := rlvgl-stm32h747i-disco-cm4
TARGET        := thumbv7em-none-eabihf
FEATURES_CM7  := cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio
FEATURES_CM4  := cm4
CHIP          := STM32H747XIHx
FLASH_BASE    := 0x08000000
OBJCOPY       := rust-objcopy
PROBE_ID      ?= 0483:3754:004F00273133510837363734
PROBE_SPEED   ?= 1000

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

# Derived paths
ELF_CM7       := target/$(TARGET)/debug/$(BIN_CM7)
ELF_CM7_REL   := target/$(TARGET)/release/$(BIN_CM7)
ELF_CM4       := target/$(TARGET)/debug/$(BIN_CM4)
DISCO_SIM_ELF := target/$(HOST_TRIPLE)/debug/$(DISCO_SIM_BIN)

.PHONY: help build-disco build-disco-release build-disco-cm4 build-disco-all \
	objcopy-disco objcopy-disco-release \
	flash-disco flash-disco-hex flash-disco-bin \
	probe-rs-gdb \
	openocd openocd-dual openocd-erase \
	gen-stm32h747i-disco-bsp \
	build-sim build-disco-sim build-uefi-disco build-creator build-all-bins \
	test-disco-sim test-uefi-disco test-stm32h747i-disco test-playit-all \
	test-disco-demo test-disco-sim-rust \
	translate-all-i18n translate-all-i18n-force translate-all-i18n-dry-run \
	extract-i18n-keys extract-icons import-icons

help:
	@echo "STM32H747I-DISCO firmware:"
	@echo "  make build-disco              # Build CM7 debug + generate .hex/.bin"
	@echo "  make build-disco-release      # Build CM7 release + generate .hex/.bin"
	@echo "  make build-disco-cm4          # Build CM4 debug"
	@echo "  make build-disco-all          # Build both cores"
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

# ── Build ─────────────────────────────────────────────────────────
build-disco:
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_CM7)
	$(MAKE) objcopy-disco

build-disco-release:
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) --release \
	  -p $(PACKAGE) --bin $(BIN_CM7) --features $(FEATURES_CM7)
	$(MAKE) objcopy-disco-release

build-disco-cm4:
	RUSTFLAGS="-C target-cpu=cortex-m7" \
	cargo build --target $(TARGET) \
	  -p $(PACKAGE) --bin $(BIN_CM4) --features $(FEATURES_CM4)

build-disco-all: build-disco build-disco-cm4

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

# ── Flash (probe-rs) ─────────────────────────────────────────────
flash-disco: build-disco
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
