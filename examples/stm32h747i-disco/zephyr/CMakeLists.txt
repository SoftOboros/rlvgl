# SPDX-License-Identifier: MIT
# Zephyr application for STM32H747I-DISCO rlvgl demo.
#
# Build:
#   west build -b stm32h747i_disco/stm32h747xx/m7 \
#       examples/stm32h747i-disco/zephyr \
#       -- -DSHIELD=st_b_lcd40_dsi1_mb1166_a09
#
# The Rust static library is built separately and linked via
# target_link_libraries. See README for the two-step build flow.

cmake_minimum_required(VERSION 3.20.0)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(rlvgl_disco_zephyr C)

target_sources(app PRIVATE src/main.c)

# Link the Rust static library built by cargo.
# The library path assumes cargo was invoked from the workspace root:
#   RUSTFLAGS="-C target-cpu=cortex-m7" cargo build \
#       --target thumbv7em-none-eabihf \
#       -p rlvgl-example-disco \
#       --features cm7,dma2d,splash,desktop,zephyr \
#       --lib
#
# Adjust the path if your build layout differs.
set(RLVGL_RUST_LIB
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../../target/thumbv7em-none-eabihf/debug/librlvgl_example_disco.a)

if(EXISTS ${RLVGL_RUST_LIB})
    target_link_libraries(app PUBLIC ${RLVGL_RUST_LIB})
else()
    message(WARNING "Rust library not found at ${RLVGL_RUST_LIB} — build the Rust side first.")
endif()
