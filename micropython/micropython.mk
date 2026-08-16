# rlvgl MicroPython module build glue.
#
# Integrates the Rust static library and C shim into the MicroPython
# build system when invoked via `USER_C_MODULES=$(RLVGL_PATH)/micropython`.
#
# The Rust crate is built automatically into MicroPython's selected build
# directory. MicroPython can discover this descriptor either through the
# repository root or through the forwarding descriptor in `rlvgl/`:
#
#   make USER_C_MODULES=/path/to/rlvgl
#   make USER_C_MODULES=/path/to/rlvgl/micropython
#
# Variables:
#   RLVGL_MOD_DIR - Set by the descriptor, or overridden by its forwarder.
#   RLVGL_CARGO - Optional Cargo executable override.
#   RLVGL_RUSTC - Optional rustc executable override.
#   RLVGL_RUST_TARGET - Explicit Rust target triple. Defaults to rustc's host.
#   RLVGL_CARGO_TARGET_DIR - Optional Cargo target-directory override.
#
# This file follows the conventions used by MicroPython's user module
# example and is referenced by the `USER_C_MODULES` build flag.

RLVGL_MOD_DIR ?= $(USERMOD_DIR)
RLVGL_CARGO ?= cargo
RLVGL_RUSTC ?= rustc
RLVGL_RUST_TARGET ?= $(shell $(RLVGL_RUSTC) -vV | sed -n 's/^host: //p')
RLVGL_CARGO_TARGET_DIR ?= $(BUILD)/rlvgl-cargo
RLVGL_STATICLIB := $(RLVGL_CARGO_TARGET_DIR)/$(RLVGL_RUST_TARGET)/release/librlvgl_micropython.a
RLVGL_RUST_INPUTS := $(RLVGL_MOD_DIR)/Cargo.toml \
                     $(wildcard $(RLVGL_MOD_DIR)/src/*.rs) \
                     $(RLVGL_MOD_DIR)/staticlib/Cargo.toml \
                     $(RLVGL_MOD_DIR)/staticlib/Cargo.lock \
                     $(wildcard $(RLVGL_MOD_DIR)/staticlib/src/*.rs) \
                     $(RLVGL_MOD_DIR)/../api/Cargo.toml \
                     $(wildcard $(RLVGL_MOD_DIR)/../api/src/*.rs)

# C shim source
SRC_USERMOD += $(RLVGL_MOD_DIR)/mp_module.c

# Include path for the shim
CFLAGS_USERMOD += -I$(RLVGL_MOD_DIR)

# Build and link the Rust static library. Passing the archive path directly is
# portable across the GNU and Apple linkers used by the Unix port.
$(RLVGL_STATICLIB): $(RLVGL_RUST_INPUTS)
	$(RLVGL_CARGO) build --locked --release \
		--manifest-path $(RLVGL_MOD_DIR)/staticlib/Cargo.toml \
		--target $(RLVGL_RUST_TARGET) \
		--target-dir $(abspath $(RLVGL_CARGO_TARGET_DIR))
	$(TOUCH) $(RLVGL_STATICLIB)

# MicroPython's Unix link rule consumes normal prerequisites through `$^`, so
# the archive is listed exactly once here rather than duplicated in LDFLAGS.
$(BUILD)/$(PROG): $(RLVGL_STATICLIB)
