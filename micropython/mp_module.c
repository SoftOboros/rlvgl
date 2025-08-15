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

// Forward declarations of the Rust FFI functions.
void mp_rlvgl_init(void);
void mp_rlvgl_stack_clear(void);
void mp_rlvgl_present(void);
void mp_rlvgl_stats(void);

// Python-exposed wrappers.
STATIC mp_obj_t mp_rlvgl_init_py(void) {
    mp_rlvgl_init();
    return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_init_obj, mp_rlvgl_init_py);

STATIC mp_obj_t mp_rlvgl_stack_clear_py(void) {
    mp_rlvgl_stack_clear();
    return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stack_clear_obj, mp_rlvgl_stack_clear_py);

STATIC mp_obj_t mp_rlvgl_present_py(void) {
    mp_rlvgl_present();
    return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_present_obj, mp_rlvgl_present_py);

STATIC mp_obj_t mp_rlvgl_stats_py(void) {
    mp_rlvgl_stats();
    return mp_const_none;
}
STATIC MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stats_obj, mp_rlvgl_stats_py);

// Module globals table.
STATIC const mp_rom_map_elem_t mp_rlvgl_module_globals_table[] = {
    { MP_ROM_QSTR(MP_QSTR___name__), MP_ROM_QSTR(MP_QSTR_mp_rlvgl) },
    { MP_ROM_QSTR(MP_QSTR_init), MP_ROM_PTR(&mp_rlvgl_init_obj) },
    { MP_ROM_QSTR(MP_QSTR_stack_clear), MP_ROM_PTR(&mp_rlvgl_stack_clear_obj) },
    { MP_ROM_QSTR(MP_QSTR_present), MP_ROM_PTR(&mp_rlvgl_present_obj) },
    { MP_ROM_QSTR(MP_QSTR_stats), MP_ROM_PTR(&mp_rlvgl_stats_obj) },
};

STATIC MP_DEFINE_CONST_DICT(mp_rlvgl_module_globals, mp_rlvgl_module_globals_table);

// Define the module.
const mp_obj_module_t mp_rlvgl_user_cmodule = {
    .base = { &mp_type_module },
    .globals = (mp_obj_dict_t*)&mp_rlvgl_module_globals,
};

// Register the module to make it available in MicroPython.
MP_REGISTER_MODULE(MP_QSTR_mp_rlvgl, mp_rlvgl_user_cmodule);

