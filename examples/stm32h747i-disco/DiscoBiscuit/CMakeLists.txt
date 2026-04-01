cmake_minimum_required(VERSION 3.16)

# Use the GNU Arm Embedded toolchain for cross compilation
set(CMAKE_TOOLCHAIN_FILE ${CMAKE_CURRENT_SOURCE_DIR}/gcc-arm-none-eabi.cmake CACHE FILEPATH "")

project(DiscoBiscuit C ASM)

set(LOG_LEVEL 2 CACHE STRING "Logging verbosity (0=off)")
set(SMOOTHING_ALPHA 0.2 CACHE STRING "Temporal smoothing factor (0-1)")
option(ENABLE_LTO "Enable link-time optimization" OFF)

add_subdirectory(firmware)

