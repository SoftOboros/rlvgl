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

# The generic wrappers contain neutral identity only. Private factories model
# the decoded-result construction path without opening a fake native endpoint.
stage = rlvgl._wrap_stage(7)
assert isinstance(stage, rlvgl.Stage)
assert stage.id == 7
assert stage.stage_id == 7

object_id = (1 << 40) + 3
actor = rlvgl._wrap_actor(stage, object_id, 11)
assert isinstance(actor, rlvgl.Actor)
assert actor.stage is stage
assert actor.id == object_id
assert actor.object_id == object_id
assert actor.type_id == 11

maximum_object_id = (1 << 64) - 1
maximum_actor = rlvgl._wrap_actor(stage, maximum_object_id, 12)
assert maximum_actor.object_id == maximum_object_id

for runtime_owned_type in (rlvgl.Stage, rlvgl.Actor):
    try:
        runtime_owned_type()
        raise AssertionError("runtime-owned wrapper was directly constructed")
    except TypeError:
        pass

# Stage.poll is the same endpoint-wide bounded operation as module poll. With
# no endpoint hook in this host profile, both truthfully report zero work.
default_summary = rlvgl.poll()
assert default_summary == stage.poll()
assert default_summary == rlvgl.poll(max_cues=None)
assert default_summary["budget"] == rlvgl.DEFAULT_POLL_BUDGET == 16
assert default_summary["cues"] == 0
assert default_summary["callbacks"] == 0
assert default_summary["exceptions"] == 0
assert default_summary["deferred_batches"] == 0
assert default_summary["stage_counts"] == ()
assert default_summary["endpoint_connected"] is False

bounded_summary = stage.poll(3)
assert bounded_summary == rlvgl.poll(max_cues=3)
assert bounded_summary["budget"] == 3
assert rlvgl.MAX_POLL_BUDGET == 64

for invalid_budget in (0, -1, 65, True, "1", 1 << 32, (1 << 64) + 1):
    try:
        rlvgl.poll(invalid_budget)
        raise AssertionError("invalid max_cues was accepted")
    except (TypeError, ValueError):
        pass

# Catalog and runtime reads are explicit gaps until endpoint transport lands;
# they must not silently return a placeholder schema or stale value.
for runtime_read in (
    stage.types,
    lambda: stage.describe_type("button"),
    stage.snapshot,
    lambda: actor.get("text"),
    actor.children,
):
    try:
        runtime_read()
        raise AssertionError("runtime read succeeded without an endpoint")
    except rlvgl.UnsupportedError as exception:
        assert isinstance(exception, rlvgl.RlvglError)

for invalid_wrapper in (
    lambda: rlvgl._wrap_stage(0),
    lambda: rlvgl._wrap_stage((1 << 32) + 7),
    lambda: rlvgl._wrap_actor(None, 1, 1),
    lambda: rlvgl._wrap_actor(stage, 0, 1),
    lambda: rlvgl._wrap_actor(stage, 1, 1),
    lambda: rlvgl._wrap_actor(stage, 1 << 32, 1),
    lambda: rlvgl._wrap_actor(stage, 1 << 64, 1),
    lambda: rlvgl._wrap_actor(stage, object_id, 0),
    lambda: rlvgl._wrap_actor(stage, object_id, (1 << 32) + 1),
):
    try:
        invalid_wrapper()
        raise AssertionError("invalid neutral wrapper identity was accepted")
    except (TypeError, ValueError):
        pass

print("binding-facade: ok")
