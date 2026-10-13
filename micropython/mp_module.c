/*!
 * MicroPython module registration for rlvgl.
 *
 * Provides the VM-owned portion of the binding and forwards native work to
 * the Rust FFI. The generic Stage and Actor wrappers below deliberately carry
 * only neutral identity. Runtime-backed operations remain unavailable until
 * the MPY-05 endpoint transport is connected.
 */

#include "py/obj.h"
#include "py/objexcept.h"
#include "py/objint.h"
#include "py/runtime.h"
#include <stdbool.h>
#include <stdint.h>

#define MP_RLVGL_DEFAULT_POLL_BUDGET (16)
#define MP_RLVGL_MAX_POLL_BUDGET (64)

// The exception hook is VM-owned state, not a process-global C pointer. This
// keeps the callable alive across collections; the module initializer below
// clears it on the first import after VM initialization (including soft reset).
MP_REGISTER_ROOT_POINTER(mp_obj_t rlvgl_exception_hook);

// Successful subscription will eventually populate this registry from the
// endpoint adapter. The private conformance seam already proves that VM-owned
// callables survive collection and are released without replacement exactly
// once. It is intentionally not a cue queue.
MP_REGISTER_ROOT_POINTER(mp_obj_t rlvgl_callback_registry);

// Callback Drain Mode is VM-owned so a soft reset cannot inherit process-global
// state. A small integer permits nested conformance dispatch without a separate
// allocation or native runtime call.
MP_REGISTER_ROOT_POINTER(mp_obj_t rlvgl_callback_depth);

// MPY-06 stable exception roots implemented by this host slice. Detailed
// protocol error context is added when the encoded endpoint transport lands.
MP_DEFINE_EXCEPTION(RlvglError, RuntimeError)
MP_DEFINE_EXCEPTION(UnsupportedError, RlvglError)
MP_DEFINE_EXCEPTION(CallbackBusyError, RlvglError)

typedef struct _mp_rlvgl_stage_obj_t {
  mp_obj_base_t base;
  uint32_t stage_id;
} mp_rlvgl_stage_obj_t;

typedef struct _mp_rlvgl_actor_obj_t {
  mp_obj_base_t base;
  mp_obj_t stage;
  uint64_t object_id;
  uint32_t type_id;
} mp_rlvgl_actor_obj_t;

static const mp_obj_type_t mp_rlvgl_stage_type;
static const mp_obj_type_t mp_rlvgl_actor_type;

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

static unsigned long long mp_rlvgl_get_unsigned(mp_obj_t value,
                                                 const char *name,
                                                 size_t width,
                                                 bool allow_zero) {
  if (!mp_obj_is_int(value)) {
    mp_raise_msg_varg(&mp_type_TypeError,
                      MP_ERROR_TEXT("%s must be an integer"), name);
  }

  unsigned long long raw = 0;
  if (mp_obj_is_small_int(value)) {
    mp_int_t small = MP_OBJ_SMALL_INT_VALUE(value);
    if (small < 0) {
      mp_raise_msg_varg(&mp_type_ValueError,
                        MP_ERROR_TEXT("%s is out of range"), name);
    }
    raw = (unsigned long long)small;
  } else {
    byte encoded[sizeof(unsigned long long)] = {0};
    if (mp_obj_int_sign(value) < 0 || width == 0 || width > sizeof(encoded) ||
        !mp_obj_int_to_bytes_impl(value, true, sizeof(encoded), encoded)) {
      mp_raise_msg_varg(&mp_type_ValueError,
                        MP_ERROR_TEXT("%s is out of range"), name);
    }
    for (size_t i = 0; i < sizeof(encoded); ++i) {
      raw = (raw << 8) | encoded[i];
    }
  }

  unsigned long long maximum =
      width == sizeof(raw) ? UINT64_MAX : ((1ULL << (width * 8)) - 1);
  if (raw > maximum || (!allow_zero && raw == 0)) {
    mp_raise_msg_varg(&mp_type_ValueError,
                      MP_ERROR_TEXT("%s is out of range"), name);
  }
  return raw;
}

static uint32_t mp_rlvgl_get_u32(mp_obj_t value, const char *name,
                                 bool allow_zero) {
  unsigned long long raw =
      mp_rlvgl_get_unsigned(value, name, sizeof(uint32_t), allow_zero);
  return (uint32_t)raw;
}

static uint64_t mp_rlvgl_get_object_id(mp_obj_t value) {
  uint64_t raw = (uint64_t)mp_rlvgl_get_unsigned(
      value, "object_id", sizeof(uint64_t), false);
  if ((uint32_t)(raw >> 32) == 0 || (uint32_t)raw == 0) {
    mp_raise_ValueError(MP_ERROR_TEXT("object_id is out of range"));
  }
  return raw;
}

static size_t mp_rlvgl_callback_depth_get(void) {
  mp_obj_t depth = MP_STATE_VM(rlvgl_callback_depth);
  if (depth == MP_OBJ_NULL) {
    return 0;
  }
  return (size_t)MP_OBJ_SMALL_INT_VALUE(depth);
}

static void mp_rlvgl_callback_depth_set(size_t depth) {
  MP_STATE_VM(rlvgl_callback_depth) = MP_OBJ_NEW_SMALL_INT(depth);
}

static MP_NORETURN void mp_rlvgl_require_runtime_read(void) {
  if (mp_rlvgl_callback_depth_get() != 0) {
    mp_raise_msg(&mp_type_CallbackBusyError,
                 MP_ERROR_TEXT("runtime reads are unavailable during a callback"));
  }
  mp_raise_msg(&mp_type_UnsupportedError,
               MP_ERROR_TEXT("native endpoint transport is not connected"));
}

static mp_obj_t mp_rlvgl_callback_registry_get(void) {
  mp_obj_t registry = MP_STATE_VM(rlvgl_callback_registry);
  if (registry == MP_OBJ_NULL) {
    registry = mp_obj_new_dict(0);
    MP_STATE_VM(rlvgl_callback_registry) = registry;
  }
  return registry;
}

static mp_obj_t mp_rlvgl_callback_key(uint32_t callback_id) {
  return mp_obj_new_int_from_uint(callback_id);
}

static mp_map_elem_t *mp_rlvgl_callback_lookup(uint32_t callback_id) {
  mp_obj_t registry = mp_rlvgl_callback_registry_get();
  return mp_map_lookup(mp_obj_dict_get_map(registry),
                       mp_rlvgl_callback_key(callback_id), MP_MAP_LOOKUP);
}

static mp_obj_t mp_rlvgl_poll_summary(uint32_t budget) {
  // No endpoint drain hook exists yet. Returning a bounded, explicit zero-work
  // summary is truthful and gives Stage.poll() and rlvgl.poll() one ABI shape
  // without creating a binding-local queue.
  mp_obj_t summary = mp_obj_new_dict(7);
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_budget),
                    mp_obj_new_int_from_uint(budget));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_cues),
                    MP_OBJ_NEW_SMALL_INT(0));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_callbacks),
                    MP_OBJ_NEW_SMALL_INT(0));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_exceptions),
                    MP_OBJ_NEW_SMALL_INT(0));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_deferred_batches),
                    MP_OBJ_NEW_SMALL_INT(0));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_stage_counts),
                    mp_obj_new_tuple(0, NULL));
  mp_obj_dict_store(summary, MP_OBJ_NEW_QSTR(MP_QSTR_endpoint_connected),
                    mp_const_false);
  return summary;
}

static uint32_t mp_rlvgl_poll_budget(size_t n_args, const mp_obj_t *pos_args,
                                     mp_map_t *kw_args) {
  enum { ARG_max_cues };
  static const mp_arg_t allowed_args[] = {
      {MP_QSTR_max_cues, MP_ARG_OBJ, {.u_obj = mp_const_none}},
  };
  mp_arg_val_t args[MP_ARRAY_SIZE(allowed_args)];
  mp_arg_parse_all(n_args, pos_args, kw_args, MP_ARRAY_SIZE(allowed_args),
                   allowed_args, args);

  mp_obj_t selected = args[ARG_max_cues].u_obj;
  if (selected == mp_const_none) {
    return MP_RLVGL_DEFAULT_POLL_BUDGET;
  }
  uint32_t budget = mp_rlvgl_get_u32(selected, "max_cues", false);
  if (budget > MP_RLVGL_MAX_POLL_BUDGET) {
    mp_raise_ValueError(MP_ERROR_TEXT("max_cues exceeds the endpoint limit"));
  }
  return budget;
}

static mp_obj_t mp_rlvgl_poll_py(size_t n_args, const mp_obj_t *pos_args,
                                 mp_map_t *kw_args) {
  return mp_rlvgl_poll_summary(
      mp_rlvgl_poll_budget(n_args, pos_args, kw_args));
}
static MP_DEFINE_CONST_FUN_OBJ_KW(mp_rlvgl_poll_obj, 0, mp_rlvgl_poll_py);

static mp_obj_t mp_rlvgl_stage_poll_py(size_t n_args,
                                       const mp_obj_t *pos_args,
                                       mp_map_t *kw_args) {
  mp_rlvgl_stage_obj_t *self = MP_OBJ_TO_PTR(pos_args[0]);
  (void)self;
  return mp_rlvgl_poll_summary(
      mp_rlvgl_poll_budget(n_args - 1, pos_args + 1, kw_args));
}
static MP_DEFINE_CONST_FUN_OBJ_KW(mp_rlvgl_stage_poll_obj, 1,
                                  mp_rlvgl_stage_poll_py);

static mp_obj_t mp_rlvgl_stage_types_py(mp_obj_t self_in) {
  (void)self_in;
  mp_rlvgl_require_runtime_read();
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_stage_types_obj,
                                 mp_rlvgl_stage_types_py);

static mp_obj_t mp_rlvgl_stage_describe_type_py(mp_obj_t self_in,
                                                 mp_obj_t name_or_id) {
  (void)self_in;
  (void)name_or_id;
  mp_rlvgl_require_runtime_read();
}
static MP_DEFINE_CONST_FUN_OBJ_2(mp_rlvgl_stage_describe_type_obj,
                                 mp_rlvgl_stage_describe_type_py);

static mp_obj_t mp_rlvgl_stage_snapshot_py(mp_obj_t self_in) {
  (void)self_in;
  mp_rlvgl_require_runtime_read();
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_stage_snapshot_obj,
                                 mp_rlvgl_stage_snapshot_py);

static void mp_rlvgl_stage_attr(mp_obj_t self_in, qstr attr, mp_obj_t *dest) {
  if (dest[0] != MP_OBJ_NULL) {
    return;
  }
  mp_rlvgl_stage_obj_t *self = MP_OBJ_TO_PTR(self_in);
  if (attr == MP_QSTR_id || attr == MP_QSTR_stage_id) {
    dest[0] = mp_obj_new_int_from_uint(self->stage_id);
  } else {
    dest[1] = MP_OBJ_SENTINEL;
  }
}

static const mp_rom_map_elem_t mp_rlvgl_stage_locals_table[] = {
    {MP_ROM_QSTR(MP_QSTR_poll), MP_ROM_PTR(&mp_rlvgl_stage_poll_obj)},
    {MP_ROM_QSTR(MP_QSTR_types), MP_ROM_PTR(&mp_rlvgl_stage_types_obj)},
    {MP_ROM_QSTR(MP_QSTR_describe_type),
     MP_ROM_PTR(&mp_rlvgl_stage_describe_type_obj)},
    {MP_ROM_QSTR(MP_QSTR_snapshot), MP_ROM_PTR(&mp_rlvgl_stage_snapshot_obj)},
};
static MP_DEFINE_CONST_DICT(mp_rlvgl_stage_locals,
                            mp_rlvgl_stage_locals_table);

static MP_DEFINE_CONST_OBJ_TYPE(mp_rlvgl_stage_type, MP_QSTR_Stage,
                                MP_TYPE_FLAG_NONE, attr, mp_rlvgl_stage_attr,
                                locals_dict, &mp_rlvgl_stage_locals);

static mp_obj_t mp_rlvgl_actor_get_py(mp_obj_t self_in, mp_obj_t property) {
  (void)self_in;
  (void)property;
  mp_rlvgl_require_runtime_read();
}
static MP_DEFINE_CONST_FUN_OBJ_2(mp_rlvgl_actor_get_obj,
                                 mp_rlvgl_actor_get_py);

static mp_obj_t mp_rlvgl_actor_children_py(mp_obj_t self_in) {
  (void)self_in;
  mp_rlvgl_require_runtime_read();
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_actor_children_obj,
                                 mp_rlvgl_actor_children_py);

static void mp_rlvgl_actor_attr(mp_obj_t self_in, qstr attr, mp_obj_t *dest) {
  if (dest[0] != MP_OBJ_NULL) {
    return;
  }
  mp_rlvgl_actor_obj_t *self = MP_OBJ_TO_PTR(self_in);
  if (attr == MP_QSTR_stage) {
    dest[0] = self->stage;
  } else if (attr == MP_QSTR_id || attr == MP_QSTR_object_id) {
    dest[0] = mp_obj_new_int_from_ull(self->object_id);
  } else if (attr == MP_QSTR_type_id) {
    dest[0] = mp_obj_new_int_from_uint(self->type_id);
  } else {
    dest[1] = MP_OBJ_SENTINEL;
  }
}

static const mp_rom_map_elem_t mp_rlvgl_actor_locals_table[] = {
    {MP_ROM_QSTR(MP_QSTR_get), MP_ROM_PTR(&mp_rlvgl_actor_get_obj)},
    {MP_ROM_QSTR(MP_QSTR_children), MP_ROM_PTR(&mp_rlvgl_actor_children_obj)},
};
static MP_DEFINE_CONST_DICT(mp_rlvgl_actor_locals,
                            mp_rlvgl_actor_locals_table);

static MP_DEFINE_CONST_OBJ_TYPE(mp_rlvgl_actor_type, MP_QSTR_Actor,
                                MP_TYPE_FLAG_NONE, attr, mp_rlvgl_actor_attr,
                                locals_dict, &mp_rlvgl_actor_locals);

// Private factories are the host-conformance stand-in for wrappers normally
// created by decoded runtime results. They allocate no native Stage or Actor.
static mp_obj_t mp_rlvgl_wrap_stage_py(mp_obj_t stage_id) {
  mp_rlvgl_stage_obj_t *stage =
      mp_obj_malloc(mp_rlvgl_stage_obj_t, &mp_rlvgl_stage_type);
  stage->stage_id = mp_rlvgl_get_u32(stage_id, "stage_id", false);
  return MP_OBJ_FROM_PTR(stage);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_wrap_stage_obj,
                                 mp_rlvgl_wrap_stage_py);

static mp_obj_t mp_rlvgl_wrap_actor_py(mp_obj_t stage_in, mp_obj_t object_id,
                                       mp_obj_t type_id) {
  if (!mp_obj_is_type(stage_in, &mp_rlvgl_stage_type)) {
    mp_raise_TypeError(MP_ERROR_TEXT("stage must be an rlvgl.Stage"));
  }
  mp_rlvgl_actor_obj_t *actor =
      mp_obj_malloc(mp_rlvgl_actor_obj_t, &mp_rlvgl_actor_type);
  actor->stage = stage_in;
  actor->object_id = mp_rlvgl_get_object_id(object_id);
  actor->type_id = mp_rlvgl_get_u32(type_id, "type_id", false);
  return MP_OBJ_FROM_PTR(actor);
}
static MP_DEFINE_CONST_FUN_OBJ_3(mp_rlvgl_wrap_actor_obj,
                                 mp_rlvgl_wrap_actor_py);

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
    MP_STATE_VM(rlvgl_callback_registry) = mp_obj_new_dict(0);
    mp_rlvgl_callback_depth_set(0);
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

static mp_obj_t mp_rlvgl_retain_callback_py(mp_obj_t callback_id_in,
                                             mp_obj_t callback) {
  uint32_t callback_id =
      mp_rlvgl_get_u32(callback_id_in, "callback_id", false);
  if (!mp_obj_is_callable(callback)) {
    mp_raise_TypeError(MP_ERROR_TEXT("callback must be callable"));
  }
  if (mp_rlvgl_callback_lookup(callback_id) != NULL) {
    mp_raise_msg(&mp_type_RlvglError,
                 MP_ERROR_TEXT("callback_id is already active"));
  }
  mp_obj_dict_store(mp_rlvgl_callback_registry_get(),
                    mp_rlvgl_callback_key(callback_id), callback);
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mp_rlvgl_retain_callback_obj,
                                 mp_rlvgl_retain_callback_py);

static mp_obj_t mp_rlvgl_release_callback_py(mp_obj_t callback_id_in) {
  uint32_t callback_id =
      mp_rlvgl_get_u32(callback_id_in, "callback_id", false);
  mp_obj_t registry = mp_rlvgl_callback_registry_get();
  mp_map_elem_t *removed =
      mp_map_lookup(mp_obj_dict_get_map(registry),
                    mp_rlvgl_callback_key(callback_id),
                    MP_MAP_LOOKUP_REMOVE_IF_FOUND);
  bool found = removed != NULL;
  if (found) {
    // REMOVE_IF_FOUND deliberately leaves the removed value available to its
    // caller. Clear it explicitly so the VM root dictionary stops retaining
    // the callback at the exact successful-release boundary.
    removed->value = MP_OBJ_NULL;
  }
  return mp_obj_new_bool(found);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_release_callback_obj,
                                 mp_rlvgl_release_callback_py);

static mp_obj_t mp_rlvgl_reset_callbacks_py(void) {
  mp_obj_t registry = mp_rlvgl_callback_registry_get();
  size_t released = mp_obj_dict_len(registry);
  MP_STATE_VM(rlvgl_callback_registry) = mp_obj_new_dict(0);
  return mp_obj_new_int_from_uint(released);
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_reset_callbacks_obj,
                                 mp_rlvgl_reset_callbacks_py);

static mp_obj_t mp_rlvgl_callback_count_py(void) {
  return mp_obj_new_int_from_uint(
      mp_obj_dict_len(mp_rlvgl_callback_registry_get()));
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_callback_count_obj,
                                 mp_rlvgl_callback_count_py);

static mp_obj_t mp_rlvgl_callback_storage_clean_py(void) {
  mp_map_t *map = mp_obj_dict_get_map(mp_rlvgl_callback_registry_get());
  for (size_t i = 0; i < map->alloc; ++i) {
    mp_map_elem_t *entry = &map->table[i];
    if ((entry->key == MP_OBJ_NULL || entry->key == MP_OBJ_SENTINEL) &&
        entry->value != MP_OBJ_NULL) {
      return mp_const_false;
    }
  }
  return mp_const_true;
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_callback_storage_clean_obj,
                                 mp_rlvgl_callback_storage_clean_py);

static mp_obj_t mp_rlvgl_in_callback_py(void) {
  return mp_obj_new_bool(mp_rlvgl_callback_depth_get() != 0);
}
static MP_DEFINE_CONST_FUN_OBJ_0(mp_rlvgl_in_callback_obj,
                                 mp_rlvgl_in_callback_py);

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

static void mp_rlvgl_invoke_callback(mp_obj_t callback) {
  size_t previous_depth = mp_rlvgl_callback_depth_get();
  mp_rlvgl_callback_depth_set(previous_depth + 1);

  nlr_buf_t callback_nlr;
  if (nlr_push(&callback_nlr) == 0) {
    mp_call_function_0(callback);
    nlr_pop();
    mp_rlvgl_callback_depth_set(previous_depth);
  } else {
    mp_rlvgl_callback_depth_set(previous_depth);
    mp_rlvgl_report_callback_exception(MP_OBJ_FROM_PTR(callback_nlr.ret_val));
  }
}

// Conformance seam for MPY-06-003 host proof. The endpoint-wide poll adapter
// will call this same containment path once MPY-05 cue delivery is wired; this
// underscored helper is not a second queue or a public scheduling contract.
static mp_obj_t mp_rlvgl_dispatch_callback_py(mp_obj_t callback) {
  if (!mp_obj_is_callable(callback)) {
    mp_raise_TypeError(MP_ERROR_TEXT("callback must be callable"));
  }
  mp_rlvgl_invoke_callback(callback);
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_dispatch_callback_obj,
                                 mp_rlvgl_dispatch_callback_py);

static mp_obj_t mp_rlvgl_dispatch_registered_py(mp_obj_t callback_id_in) {
  uint32_t callback_id =
      mp_rlvgl_get_u32(callback_id_in, "callback_id", false);
  mp_map_elem_t *entry = mp_rlvgl_callback_lookup(callback_id);
  if (entry == NULL) {
    mp_raise_msg(&mp_type_RlvglError,
                 MP_ERROR_TEXT("callback_id is not active"));
  }
  mp_rlvgl_invoke_callback(entry->value);
  return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mp_rlvgl_dispatch_registered_obj,
                                 mp_rlvgl_dispatch_registered_py);

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
    {MP_ROM_QSTR(MP_QSTR_poll), MP_ROM_PTR(&mp_rlvgl_poll_obj)},
    {MP_ROM_QSTR(MP_QSTR_DEFAULT_POLL_BUDGET),
     MP_ROM_INT(MP_RLVGL_DEFAULT_POLL_BUDGET)},
    {MP_ROM_QSTR(MP_QSTR_MAX_POLL_BUDGET),
     MP_ROM_INT(MP_RLVGL_MAX_POLL_BUDGET)},
    {MP_ROM_QSTR(MP_QSTR_Stage), MP_ROM_PTR(&mp_rlvgl_stage_type)},
    {MP_ROM_QSTR(MP_QSTR_Actor), MP_ROM_PTR(&mp_rlvgl_actor_type)},
    {MP_ROM_QSTR(MP_QSTR_RlvglError), MP_ROM_PTR(&mp_type_RlvglError)},
    {MP_ROM_QSTR(MP_QSTR_UnsupportedError),
     MP_ROM_PTR(&mp_type_UnsupportedError)},
    {MP_ROM_QSTR(MP_QSTR_CallbackBusyError),
     MP_ROM_PTR(&mp_type_CallbackBusyError)},
    {MP_ROM_QSTR(MP_QSTR_set_exception_hook),
     MP_ROM_PTR(&mp_rlvgl_set_exception_hook_obj)},
    {MP_ROM_QSTR(MP_QSTR__wrap_stage), MP_ROM_PTR(&mp_rlvgl_wrap_stage_obj)},
    {MP_ROM_QSTR(MP_QSTR__wrap_actor), MP_ROM_PTR(&mp_rlvgl_wrap_actor_obj)},
    {MP_ROM_QSTR(MP_QSTR__retain_callback),
     MP_ROM_PTR(&mp_rlvgl_retain_callback_obj)},
    {MP_ROM_QSTR(MP_QSTR__release_callback),
     MP_ROM_PTR(&mp_rlvgl_release_callback_obj)},
    {MP_ROM_QSTR(MP_QSTR__reset_callbacks),
     MP_ROM_PTR(&mp_rlvgl_reset_callbacks_obj)},
    {MP_ROM_QSTR(MP_QSTR__callback_count),
     MP_ROM_PTR(&mp_rlvgl_callback_count_obj)},
    {MP_ROM_QSTR(MP_QSTR__callback_storage_clean),
     MP_ROM_PTR(&mp_rlvgl_callback_storage_clean_obj)},
    {MP_ROM_QSTR(MP_QSTR__in_callback),
     MP_ROM_PTR(&mp_rlvgl_in_callback_obj)},
    {MP_ROM_QSTR(MP_QSTR__dispatch_callback),
     MP_ROM_PTR(&mp_rlvgl_dispatch_callback_obj)},
    {MP_ROM_QSTR(MP_QSTR__dispatch_registered),
     MP_ROM_PTR(&mp_rlvgl_dispatch_registered_obj)},
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
