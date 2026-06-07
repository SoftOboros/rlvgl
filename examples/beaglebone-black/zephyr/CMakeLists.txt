# SPDX-License-Identifier: MIT
# Zephyr application for BeagleBone Black + NHD-7.0CTP-CAPE-P rlvgl demo.
#
# Build:
#   west build -b am335x_bone_black \
#       examples/beaglebone-black/zephyr
#
# The Rust static library is built separately and linked via
# target_link_libraries. Two-step build:
#
#   1. cargo build --target armv7a-none-eabihf \
#          -p rlvgl-example-bbb --features bare_metal,zephyr --lib
#   2. west build -b am335x_bone_black examples/beaglebone-black/zephyr

cmake_minimum_required(VERSION 3.20.0)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(rlvgl_bbb_zephyr C)

target_sources(app PRIVATE src/main.c)

# Link the Rust static library built by cargo.
set(RLVGL_RUST_LIB
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../target/armv7a-none-eabihf/debug/librlvgl_example_bbb.a)

if(EXISTS ${RLVGL_RUST_LIB})
    target_link_libraries(app PUBLIC ${RLVGL_RUST_LIB})
else()
    message(WARNING "Rust library not found at ${RLVGL_RUST_LIB} — build the Rust side first.")
endif()
