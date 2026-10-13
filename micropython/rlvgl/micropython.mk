# micropython.mk - MicroPython v1.28 user-module collection entry point.

# MicroPython's Make build discovers one descriptor below USER_C_MODULES.
# Forward to the crate-owned descriptor while retaining its existing location.
RLVGL_MOD_DIR := $(abspath $(USERMOD_DIR)/..)
include $(RLVGL_MOD_DIR)/micropython.mk
