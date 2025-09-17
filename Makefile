.PHONY: help gen-stm32h747i-disco-bsp build-disco build-disco-cm4 build-disco-all openocd openocd-erase

help:
	@echo "Convenience targets:"
	@echo "  make gen-stm32h747i-disco-bsp   # Regenerate H747I-DISCO BSP (SMPS/VOS1)"
	@echo "  make build-disco                # Build CM7 example"
	@echo "  make build-disco-cm4            # Build CM4 example"
	@echo "  make build-disco-all            # Build both cores"
	@echo "  make openocd                    # Start OpenOCD (ST-Link + STM32H7)"
	@echo "  make openocd-erase              # Full chip erase via OpenOCD (DANGER)"

gen-stm32h747i-disco-bsp:
	STM32_PWR_SUPPLY=SMPS STM32_PWR_SDLEVEL=VOS1 \
		./examples/stm32h747i-disco/gen-bsp.sh

build-disco:
	cargo build --target thumbv7em-none-eabihf \
	  --bin rlvgl-stm32h747i-disco --features stm32h747i_disco

build-disco-cm4:
	cargo build --target thumbv7em-none-eabihf \
	  --bin rlvgl-stm32h747i-disco-cm4 --features stm32h747i_disco

build-disco-all: build-disco build-disco-cm4

# Basic OpenOCD sessions; adjust interface/target as needed
openocd:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg -c init -c "reset halt"

openocd-erase:
	openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
	  -c init -c "reset halt" -c "stm32h7x mass_erase 0" -c shutdown

