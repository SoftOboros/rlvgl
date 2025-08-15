# rlvgl MicroPython module build glue.
#
# Integrates the Rust static library and C shim into the MicroPython
# build system when invoked via `USER_C_MODULES=$(RLVGL_PATH)/micropython`.
#
# Expect the Rust crate to be compiled separately, producing a
# `librlvgl_micropython.a` in a `lib/` directory beneath this one:
#
#   cargo build --release --target thumbv7em-none-eabihf
#   mkdir -p lib
#   cp target/thumbv7em-none-eabihf/release/librlvgl_micropython.a lib/
#
# With the library in place, MicroPython can link it by passing the
# location of this file to `make`:
#
#   make USER_C_MODULES=/path/to/rlvgl/micropython
#
# Variables:
#   RLVGL_MOD_DIR - Set by MicroPython's build to the module directory.
#   RLVGL_LIB_DIR - Optional override for the static library directory.
#
# This file follows the conventions used by MicroPython's user module
# example and is referenced by the `USER_C_MODULES` build flag.

RLVGL_MOD_DIR := $(USERMOD_DIR)
RLVGL_LIB_DIR ?= $(RLVGL_MOD_DIR)/lib

# C shim source
SRC_USERMOD += $(RLVGL_MOD_DIR)/mp_module.c

# Include path for the shim
CFLAGS_USERMOD += -I$(RLVGL_MOD_DIR)

# Link against the prebuilt Rust static library
LDFLAGS_USERMOD += -L$(RLVGL_LIB_DIR) -l:librlvgl_micropython.a
