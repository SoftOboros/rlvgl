/*!
 * MicroPython module registration for rlvgl.
 *
 * Provides placeholder bindings that forward to the Rust FFI.
 * Board-specific behavior lives behind Cargo feature flags in
 * the Rust crate; this C shim only wires the module table and
 * basic call stubs.
 */

#include "py/obj.h"
#include "py/runtime.h"
#include <stdint.h>

// Forward declarations of the Rust FFI functions.
int mp_rlvgl_init(void);
int mp_rlvgl_stack_clear(void);
int mp_rlvgl_present(void);
int mp_rlvgl_stats(void);
typedef struct {
  uint8_t major;
  uint8_t minor;
  uint8_t patch;
} mp_rlvgl_api_version_t;
mp_rlvgl_api_version_t mp_rlvgl_api_version(void);

// Helper to convert FFI status codes into MicroPython exceptions.
STATIC void mp_rlvgl_check(int status) {
  if (status < 0) {
    mp_raise_ValueError(MP_ERROR_TEXT("mp_rlvgl error"));
  }
}

// Python-exposed wrappers.
STATIC mp_obj_t mp_rlvgl_init_py(void) {
  mp_rlvgl_check(mp_rlvgl_init());
  return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_init_obj, mp_rlvgl_init_py);

STATIC mp_obj_t mp_rlvgl_stack_clear_py(void) {
  mp_rlvgl_check(mp_rlvgl_stack_clear());
  return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stack_clear_obj,
                                 mp_rlvgl_stack_clear_py);

STATIC mp_obj_t mp_rlvgl_present_py(void) {
  mp_rlvgl_check(mp_rlvgl_present());
  return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_present_obj, mp_rlvgl_present_py);

STATIC mp_obj_t mp_rlvgl_stats_py(void) {
  mp_rlvgl_check(mp_rlvgl_stats());
  return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stats_obj, mp_rlvgl_stats_py);

STATIC mp_obj_t mp_rlvgl_api_version_py(void) {
  mp_rlvgl_api_version_t v = mp_rlvgl_api_version();
  mp_obj_t tuple[3];
  tuple[0] = mp_obj_new_int(v.major);
  tuple[1] = mp_obj_new_int(v.minor);
  tuple[2] = mp_obj_new_int(v.patch);
  return mp_obj_new_tuple(3, tuple);
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_api_version_obj,
                                 mp_rlvgl_api_version_py);

// Module globals table.
STATIC const mp_rom_map_elem_t mp_rlvgl_module_globals_table[] = {
    {MP_ROM_QSTR(MP_QSTR___name__), MP_ROM_QSTR(MP_QSTR_mp_rlvgl)},
    {MP_ROM_QSTR(MP_QSTR_init), MP_ROM_PTR(&mp_rlvgl_init_obj)},
    {MP_ROM_QSTR(MP_QSTR_stack_clear), MP_ROM_PTR(&mp_rlvgl_stack_clear_obj)},
    {MP_ROM_QSTR(MP_QSTR_present), MP_ROM_PTR(&mp_rlvgl_present_obj)},
    {MP_ROM_QSTR(MP_QSTR_stats), MP_ROM_PTR(&mp_rlvgl_stats_obj)},
    {MP_ROM_QSTR(MP_QSTR_api_version), MP_ROM_PTR(&mp_rlvgl_api_version_obj)},
};

STATIC MP_DEFINE_CONST_DICT(mp_rlvgl_module_globals,
                            mp_rlvgl_module_globals_table);

// Define the module.
const mp_obj_module_t mp_rlvgl_user_cmodule = {
    .base = {&mp_type_module},
    .globals = (mp_obj_dict_t *)&mp_rlvgl_module_globals,
};

// Register the module to make it available in MicroPython.
MP_REGISTER_MODULE(MP_QSTR_mp_rlvgl, mp_rlvgl_user_cmodule);
