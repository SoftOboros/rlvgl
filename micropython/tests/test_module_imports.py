# test_module_imports.py - Host proof for canonical and compatibility imports.

import mp_rlvgl
import rlvgl


assert rlvgl is mp_rlvgl
assert rlvgl.__name__ == "rlvgl"
assert mp_rlvgl.__name__ == "rlvgl"

assert rlvgl.api_version() == (0, 2, 1)
assert rlvgl.init() is None
assert mp_rlvgl.stack_clear() is None
assert rlvgl.present() is None
assert mp_rlvgl.stats() is None
