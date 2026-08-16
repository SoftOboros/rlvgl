# test_exception_hook.py - Host proof for callback exception containment.

import gc

import rlvgl


events = []
stage = rlvgl._wrap_stage(19)
actor = rlvgl._wrap_actor(stage, (1 << 33) + 5, 2)


def fail_default():
    events.append("default-callback")
    raise ValueError("default-path")


assert rlvgl.set_exception_hook(None) is None
assert rlvgl._dispatch_callback(fail_default) is None
events.append("after-default")
assert events == ["default-callback", "after-default"]
assert rlvgl._in_callback() is False


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
assert rlvgl._in_callback() is False

# Callback Drain Mode leaves immutable wrapper identity available while
# rejecting synchronous endpoint reads with the stable busy exception.
def callback_read_boundary():
    assert rlvgl._in_callback() is True
    assert stage.id == 19
    assert actor.stage is stage
    assert actor.object_id == (1 << 33) + 5
    for runtime_read in (stage.snapshot, lambda: actor.get("text")):
        try:
            runtime_read()
            raise AssertionError("callback-time runtime read was accepted")
        except rlvgl.CallbackBusyError as exception:
            assert isinstance(exception, rlvgl.RlvglError)


assert rlvgl._dispatch_callback(callback_read_boundary) is None
assert rlvgl._in_callback() is False

# The adapter owns a strong reference while a callback ID is active. Releasing
# an ID twice reports completion only once and never replaces a live callable
# on duplicate retention. Successful dispatch after deleting the last Python
# local is the pinned standard variant's strong-reference proof.
assert rlvgl._reset_callbacks() == 0
registry_events = []


class TrackedCallback:
    def __init__(self, tag):
        self.tag = tag

    def __call__(self):
        assert rlvgl._in_callback() is True
        registry_events.append(self.tag)

tracked = TrackedCallback("explicit")
assert rlvgl._retain_callback(31, tracked) is None
assert rlvgl._callback_count() == 1
del tracked
gc.collect()
# Retention is not a binding-local cue queue: an endpoint poll cannot dispatch
# the callable without an actual cue-delivery hook.
assert rlvgl.poll(max_cues=1)["callbacks"] == 0
assert registry_events == []
assert rlvgl._dispatch_registered(31) is None
assert registry_events == ["explicit"]

replacement = TrackedCallback("replacement")
try:
    rlvgl._retain_callback(31, replacement)
    raise AssertionError("duplicate active callback ID was accepted")
except rlvgl.RlvglError:
    pass
del replacement
gc.collect()

assert rlvgl._release_callback(31) is True
assert rlvgl._release_callback(31) is False
assert rlvgl._callback_count() == 0
assert rlvgl._callback_storage_clean() is True
gc.collect()

first = TrackedCallback("reset-first")
second = TrackedCallback("reset-second")
assert rlvgl._retain_callback(41, first) is None
assert rlvgl._retain_callback(42, second) is None
del first
del second
assert rlvgl._reset_callbacks() == 2
assert rlvgl._callback_count() == 0
gc.collect()

try:
    rlvgl._dispatch_registered(31)
    raise AssertionError("released callback remained dispatchable")
except rlvgl.RlvglError:
    pass

for oversized_callback_id in (1 << 32, (1 << 64) + 31):
    try:
        rlvgl._retain_callback(oversized_callback_id, later_callback)
        raise AssertionError("oversized callback ID was truncated")
    except ValueError:
        pass

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
