.PHONY: help gen-stm32h747i-disco-bsp build-disco build-disco-cm4 build-disco-all \
	openocd openocd-dual openocd-erase probe-rs-gdb \
	translate-all-i18n translate-all-i18n-force translate-all-i18n-dry-run \
	extract-i18n-keys extract-icons import-icons

ELF_CM7 := target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco
PROBE_ID ?= 0483:374e:004F00273133510837363734
PROBE_SPEED_KHZ ?= 50

help:
	@echo "Convenience targets:"
	@echo "  make gen-stm32h747i-disco-bsp   # Regenerate H747I-DISCO BSP (SMPS/VOS1)"
	@echo "  make build-disco                # Build CM7 example"
	@echo "  make build-disco-cm4            # Build CM4 example"
	@echo "  make build-disco-all            # Build both cores"
	@echo "  make openocd                    # Start OpenOCD (ST-Link + STM32H7)"
	@echo "  make openocd-dual               # Start OpenOCD with dual-core cfg (CM7 on 3333, CM4 on 3334)"
	@echo "  make openocd-erase              # Full chip erase via OpenOCD (DANGER)"
	@echo "  make probe-rs-gdb               # Flash CM7 image, then launch probe-rs GDB server"
	@echo ""
	@echo "i18n translation:"
	@echo "  make translate-locale-de         # Translate en.json to German"
	@echo "  make translate-all-i18n          # Translate en.json to all locales (skip existing)"
	@echo "  make translate-all-i18n-force    # Regenerate all locale translations"
	@echo "  make translate-all-i18n-dry-run  # Preview translation targets"
	@echo "  make extract-i18n-keys           # Sync keys from Rust source to en.json"
	@echo ""
	@echo "Icon extraction:"
	@echo "  make extract-icons               # Extract Lucide SVGs to assets/icons/"
	@echo "  make import-icons                # Extract + convert to .raw via creator"

gen-stm32h747i-disco-bsp:
	STM32_PWR_SUPPLY=SMPS STM32_PWR_SDLEVEL=VOS1 \
		./examples/stm32h747i-disco/gen-bsp.sh

build-disco:
	cargo build --target thumbv7em-none-eabihf \
	  --bin rlvgl-stm32h747i-disco --features stm32h747i_disco_cm7,splash,desktop

build-disco-cm4:
	cargo build --target thumbv7em-none-eabihf \
	  --bin rlvgl-stm32h747i-disco-cm4 --features stm32h747i_disco_cm4

build-disco-all: build-disco build-disco-cm4

probe-rs-gdb: build-disco
	probe-rs download --chip STM32H747XIHx \
	  --protocol swd --speed $(PROBE_SPEED_KHZ) \
	  --non-interactive --connect-under-reset \
	  --probe $(PROBE_ID) $(ELF_CM7) && \
	probe-rs gdb --chip STM32H747XIHx \
	  --protocol swd --speed $(PROBE_SPEED_KHZ) \
	  --non-interactive --connect-under-reset --reset-halt \
	  --probe $(PROBE_ID)

# Basic OpenOCD sessions; adjust interface/target as needed
openocd:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c init -c "reset halt"

openocd-dual:
	openocd -f openocd/stm32h747_dual_core.cfg

openocd-erase:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
	  -c init -c "reset halt" -c "stm32h7x mass_erase 0" -c shutdown

# ── i18n translation ───────────────────────────────────────────────
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

# ── Icon extraction ────────────────────────────────────────────────
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
