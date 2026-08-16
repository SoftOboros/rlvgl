# test_exception_hook.py - Host proof for callback exception containment.

import gc

import rlvgl


events = []


def fail_default():
    events.append("default-callback")
    raise ValueError("default-path")


assert rlvgl.set_exception_hook(None) is None
assert rlvgl._dispatch_callback(fail_default) is None
events.append("after-default")
assert events == ["default-callback", "after-default"]


seen = []


class RecordingHook:
    def __call__(self, exception):
        seen.append(exception)


hook = RecordingHook()
assert rlvgl.set_exception_hook(hook) is None
del hook
gc.collect()

# Loading the compatibility alias later must not reset same-VM module state.
import mp_rlvgl

assert mp_rlvgl is rlvgl

original = RuntimeError("original-callback")


def fail_custom():
    raise original


assert rlvgl._dispatch_callback(fail_custom) is None
assert len(seen) == 1
assert seen[0] is original


hook_events = []


def failing_hook(exception):
    hook_events.append(exception)
    raise OSError("hook-path")


assert rlvgl.set_exception_hook(failing_hook) is None
hook_original = LookupError("hook-original")


def fail_for_hook():
    raise hook_original


assert rlvgl._dispatch_callback(fail_for_hook) is None
assert hook_events == [hook_original]


def later_callback():
    events.append("later-callback")


assert rlvgl._dispatch_callback(later_callback) is None
assert events[-1] == "later-callback"

assert rlvgl.set_exception_hook(None) is None

try:
    rlvgl.set_exception_hook(42)
    raise AssertionError("non-callable exception hook was accepted")
except TypeError:
    pass

try:
    rlvgl._dispatch_callback(None)
    raise AssertionError("non-callable callback was accepted")
except TypeError:
    pass

print("exception-hook: ok")
