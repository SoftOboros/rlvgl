cmake_minimum_required(VERSION 3.16)

set(TARGET DiscoBiscuit_CM7)
set(LINKER_SCRIPT ${CMAKE_SOURCE_DIR}/CM7/stm32h747xx_flash_CM7.ld)
set(STARTUP ${CMAKE_SOURCE_DIR}/CM7/Core/Startup/startup_stm32h747xx_CM7.s)

# Compiler flags
set(COMMON_FLAGS "-mcpu=cortex-m7 -mthumb -mfpu=fpv5-d16 -mfloat-abi=hard")
set(CMAKE_C_FLAGS "${COMMON_FLAGS} -O3 -g")
set(CMAKE_ASM_FLAGS "${COMMON_FLAGS} -x assembler-with-cpp")
set(CMAKE_EXE_LINKER_FLAGS "${COMMON_FLAGS} -T${LINKER_SCRIPT} -Wl,--gc-sections -Wl,-Map=${TARGET}.map")

if(ENABLE_LTO)
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -flto")
  set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -flto")
endif()

add_compile_definitions(
  STM32H747xx
  USE_HAL_DRIVER
  CORE_CM7
  LOG_LEVEL=${LOG_LEVEL}
  SMOOTHING_ALPHA=${SMOOTHING_ALPHA}
)

# Include directories
include_directories(
  ${CMAKE_SOURCE_DIR}/CM7/Core/Inc
  ${CMAKE_SOURCE_DIR}/firmware/inc
  ${CMAKE_SOURCE_DIR}/Drivers/CMSIS/Include
  ${CMAKE_SOURCE_DIR}/Drivers/CMSIS/Device/ST/STM32H7xx/Include
  ${CMAKE_SOURCE_DIR}/Drivers/STM32H7xx_HAL_Driver/Inc
)

# Source files
file(GLOB_RECURSE SOURCES
  "${CMAKE_SOURCE_DIR}/CM7/Core/Src/*.c"
  "${CMAKE_SOURCE_DIR}/firmware/src/*.c"
  "${CMAKE_SOURCE_DIR}/Drivers/STM32H7xx_HAL_Driver/Src/*.c"
  "${CMAKE_SOURCE_DIR}/Drivers/CMSIS/Device/ST/STM32H7xx/Source/Templates/system_stm32h7xx.c"
)
list(FILTER SOURCES EXCLUDE REGEX "${CMAKE_SOURCE_DIR}/CM7/Core/Src/main.c$")

# Exclude HAL timebase templates which require manual configuration
list(FILTER SOURCES EXCLUDE REGEX "stm32h7xx_hal_timebase_.*_template.c")
list(FILTER SOURCES EXCLUDE REGEX "stm32h7xx_hal_msp_template.c")

add_executable(${TARGET} ${SOURCES} ${STARTUP})
set_target_properties(${TARGET} PROPERTIES OUTPUT_NAME "${TARGET}" SUFFIX ".elf" RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
target_link_libraries(${TARGET} PRIVATE ${TOOLCHAIN_LINK_LIBRARIES})

