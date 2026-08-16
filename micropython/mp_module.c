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

// The exception hook is VM-owned state, not a process-global C pointer. This
// keeps the callable alive across collections; the module initializer below
// clears it on the first import after VM initialization (including soft reset).
MP_REGISTER_ROOT_POINTER(mp_obj_t rlvgl_exception_hook);

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
extern const mp_obj_module_t mp_rlvgl_user_cmodule;

// Helper to convert FFI status codes into MicroPython exceptions.
static void mp_rlvgl_check(int status) {
  if (status < 0) {
    mp_raise_ValueError(MP_ERROR_TEXT("mp_rlvgl error"));
  }
}

// Python-exposed wrappers.
static mp_obj_t mp_rlvgl_init_py(void) {
  mp_rlvgl_check(mp_rlvgl_init());
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_init_obj, mp_rlvgl_init_py);

static mp_obj_t mp_rlvgl_stack_clear_py(void) {
  mp_rlvgl_check(mp_rlvgl_stack_clear());
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stack_clear_obj,
                                 mp_rlvgl_stack_clear_py);

static mp_obj_t mp_rlvgl_present_py(void) {
  mp_rlvgl_check(mp_rlvgl_present());
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_present_obj, mp_rlvgl_present_py);

static mp_obj_t mp_rlvgl_stats_py(void) {
  mp_rlvgl_check(mp_rlvgl_stats());
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_stats_obj, mp_rlvgl_stats_py);

static mp_obj_t mp_rlvgl_api_version_py(void) {
  mp_rlvgl_api_version_t v = mp_rlvgl_api_version();
  mp_obj_t tuple[3];
  tuple[0] = mp_obj_new_int(v.major);
  tuple[1] = mp_obj_new_int(v.minor);
  tuple[2] = mp_obj_new_int(v.patch);
  return mp_obj_new_tuple(3, tuple);
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_api_version_obj,
                                 mp_rlvgl_api_version_py);

#if MICROPY_MODULE_BUILTIN_INIT
static mp_obj_t mp_rlvgl_module_init_py(void) {
  // Built-in module initialization runs before the new module name is entered
  // in sys.modules. If neither alias is already loaded, this is the first
  // import in the current VM and any pre-soft-reset pointer must be cleared.
  mp_map_t *loaded = &MP_STATE_VM(mp_loaded_modules_dict).map;
  bool canonical_loaded =
      mp_map_lookup(loaded, MP_OBJ_NEW_QSTR(MP_QSTR_rlvgl), MP_MAP_LOOKUP) !=
      NULL;
  bool alias_loaded =
      mp_map_lookup(loaded, MP_OBJ_NEW_QSTR(MP_QSTR_mp_rlvgl), MP_MAP_LOOKUP) !=
      NULL;
  if (!canonical_loaded && !alias_loaded) {
    MP_STATE_VM(rlvgl_exception_hook) = mp_const_none;
  }
  // MicroPython does not otherwise cache non-extensible built-in modules in
  // the loaded-module dictionary. Add the canonical name as this VM's marker
  // so importing the compatibility alias cannot reset shared module state.
  mp_obj_dict_store(MP_OBJ_FROM_PTR(&MP_STATE_VM(mp_loaded_modules_dict)),
                    MP_OBJ_NEW_QSTR(MP_QSTR_rlvgl),
                    MP_OBJ_FROM_PTR(&mp_rlvgl_user_cmodule));
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_module_init_obj,
                                 mp_rlvgl_module_init_py);
#endif

static mp_obj_t mp_rlvgl_set_exception_hook_py(mp_obj_t hook) {
  if (hook != mp_const_none && !mp_obj_is_callable(hook)) {
    mp_raise_TypeError(MP_ERROR_TEXT("exception hook must be callable or None"));
  }

  MP_STATE_VM(rlvgl_exception_hook) = hook;
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_set_exception_hook_obj,
                                 mp_rlvgl_set_exception_hook_py);

static void mp_rlvgl_report_callback_exception(mp_obj_t exception) {
  mp_obj_t hook = MP_STATE_VM(rlvgl_exception_hook);
  if (hook == MP_OBJ_NULL || hook == mp_const_none) {
    mp_obj_print_exception(MICROPY_ERROR_PRINTER, exception);
    return;
  }

  nlr_buf_t hook_nlr;
  if (nlr_push(&hook_nlr) == 0) {
    mp_call_function_1(hook, exception);
    nlr_pop();
  } else {
    // The hook is deliberately not re-entered. Report the callback exception
    // first and the hook exception second, then return to callback draining.
    mp_obj_print_exception(MICROPY_ERROR_PRINTER, exception);
    mp_obj_print_exception(MICROPY_ERROR_PRINTER,
                           MP_OBJ_FROM_PTR(hook_nlr.ret_val));
  }
}

// Conformance seam for MPY-06-003 host proof. The endpoint-wide poll adapter
// will call this same containment path once MPY-05 cue delivery is wired; this
// underscored helper is not a second queue or a public scheduling contract.
static mp_obj_t mp_rlvgl_dispatch_callback_py(mp_obj_t callback) {
  if (!mp_obj_is_callable(callback)) {
    mp_raise_TypeError(MP_ERROR_TEXT("callback must be callable"));
  }

  nlr_buf_t callback_nlr;
  if (nlr_push(&callback_nlr) == 0) {
    mp_call_function_0(callback);
    nlr_pop();
  } else {
    mp_rlvgl_report_callback_exception(MP_OBJ_FROM_PTR(callback_nlr.ret_val));
  }
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_dispatch_callback_obj,
                                 mp_rlvgl_dispatch_callback_py);

// Module globals table.
static const mp_rom_map_elem_t mp_rlvgl_module_globals_table[] = {
    {MP_ROM_QSTR(MP_QSTR___name__), MP_ROM_QSTR(MP_QSTR_rlvgl)},
#if MICROPY_MODULE_BUILTIN_INIT
    {MP_ROM_QSTR(MP_QSTR___init__), MP_ROM_PTR(&mp_rlvgl_module_init_obj)},
#endif
    {MP_ROM_QSTR(MP_QSTR_init), MP_ROM_PTR(&mp_rlvgl_init_obj)},
    {MP_ROM_QSTR(MP_QSTR_stack_clear), MP_ROM_PTR(&mp_rlvgl_stack_clear_obj)},
    {MP_ROM_QSTR(MP_QSTR_present), MP_ROM_PTR(&mp_rlvgl_present_obj)},
    {MP_ROM_QSTR(MP_QSTR_stats), MP_ROM_PTR(&mp_rlvgl_stats_obj)},
    {MP_ROM_QSTR(MP_QSTR_api_version), MP_ROM_PTR(&mp_rlvgl_api_version_obj)},
    {MP_ROM_QSTR(MP_QSTR_set_exception_hook),
     MP_ROM_PTR(&mp_rlvgl_set_exception_hook_obj)},
    {MP_ROM_QSTR(MP_QSTR__dispatch_callback),
     MP_ROM_PTR(&mp_rlvgl_dispatch_callback_obj)},
};

static MP_DEFINE_CONST_DICT(mp_rlvgl_module_globals,
                            mp_rlvgl_module_globals_table);

// Define the module.
const mp_obj_module_t mp_rlvgl_user_cmodule = {
    .base = {&mp_type_module},
    .globals = (mp_obj_dict_t *)&mp_rlvgl_module_globals,
};

// Register the canonical module and its 0.x compatibility alias. Both names
// resolve to this exact object, globals dictionary, and native runtime state.
MP_REGISTER_MODULE(MP_QSTR_rlvgl, mp_rlvgl_user_cmodule);
MP_REGISTER_MODULE(MP_QSTR_mp_rlvgl, mp_rlvgl_user_cmodule);
