# ESP-IDF component wrapping the Rust rlvgl staticlib (BEETLE M1).
#
# The component itself ships no C sources — only the public header. It builds
# the cargo staticlib (librlvgl_app.a) via an ExternalProject and imports it,
# so anything that REQUIRES rlvgl_app links against the Rust payload and can
# call rlvgl_app_render().
#
# Prerequisites in the build environment:
#   - cargo on PATH (rustup)
#   - rustup target add riscv32imafc-unknown-none-elf   (ilp32f, matches IDF)

idf_component_register(INCLUDE_DIRS "include")

set(RUST_DIR     "${CMAKE_CURRENT_SOURCE_DIR}/rust")
set(RUST_TARGET  "riscv32imafc-unknown-none-elf")
set(RUST_PROFILE "release")
set(RUST_LIB     "${RUST_DIR}/target/${RUST_TARGET}/${RUST_PROFILE}/librlvgl_app.a")

# Build-time payload selection (SCTD-02 PCDN-02-01): the default `sctd` payload
# is the interactive SCXML tutorial demo; `idf.py -DRLVGL_PAYLOAD=disco build`
# restores the original disco-demo widget tree.
set(RLVGL_PAYLOAD "sctd" CACHE STRING "rlvgl IDF payload: sctd | disco")
if(RLVGL_PAYLOAD STREQUAL "disco")
    set(RUST_CARGO_FEATURES --no-default-features --features app_disco)
else()
    set(RUST_CARGO_FEATURES "")
endif()

include(ExternalProject)
ExternalProject_Add(rlvgl_rust_build
    PREFIX            "${CMAKE_CURRENT_BINARY_DIR}/rlvgl_rust_build"
    SOURCE_DIR        "${RUST_DIR}"
    CONFIGURE_COMMAND ""
    BUILD_COMMAND     cargo build --release
                          --target ${RUST_TARGET}
                          --manifest-path "${RUST_DIR}/Cargo.toml"
                          ${RUST_CARGO_FEATURES}
    BUILD_ALWAYS      1   # let cargo do its own incremental dependency tracking
    BUILD_IN_SOURCE   1
    INSTALL_COMMAND   ""
    BUILD_BYPRODUCTS  "${RUST_LIB}"
    LOG_BUILD         1
)

# Import the cargo output and force it to be built before the link step.
add_prebuilt_library(rlvgl_app_rust "${RUST_LIB}")
add_dependencies(rlvgl_app_rust rlvgl_rust_build)

# The registered component is INTERFACE-only (no SRCS), so propagate the import
# to consumers via the interface link set.
target_link_libraries(${COMPONENT_LIB} INTERFACE rlvgl_app_rust)
