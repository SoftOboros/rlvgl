#!/usr/bin/env python3
"""Capture and verify CPY baseline and dependency-firewall evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "docs/cpython/evidence/CPY-BASELINE-2026-08-18.json"
SCHEMA_PATH = REPO_ROOT / "docs/cpython/CPY-BASELINE-MANIFEST.schema.json"
GENERATED_DIR = REPO_ROOT / "docs/cpython/evidence/_generated"
CLOSEOUT_PATH = (
    REPO_ROOT / "docs/cpython/evidence/CPY-COORDINATION-CLOSEOUT-2026-08-20.json"
)

CLOSEOUT_FRONTIER_DELTA = (
    "docs/spec-index/index/_manifest.json",
    "docs/spec-index/index/wld.json",
    "docs/wayland/README.md",
    "docs/wayland/WLD-01-SESSION-SHM-PRESENTATION.md",
    "platform/Cargo.toml",
    "platform/src/wayland/mod.rs",
    "platform/tests/wayland_smoke.rs",
)

REQUIRED_TARGETS = {
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
}
REQUIRED_PROFILES = {
    "host-headless",
    "host-windowed",
    "embedded-linux-direct",
    "embedded-linux-daemon",
}
ALLOWED_HANDOFF_PATHS = {
    "Cargo.toml",
    "Cargo.lock",
    "scripts/publish_changed.sh",
    "runtime-std/",
}
PUBLIC_BASELINE_PATHS = (
    "api/src/lib.rs",
    "api/src/protocol.rs",
    "core/src/lib.rs",
    "core/src/endpoint.rs",
    "core/src/actor.rs",
    "platform/src/lib.rs",
    "micropython/src/lib.rs",
    "scripts/publish_changed.sh",
)


class EvidenceError(RuntimeError):
    """Raised when CPY evidence is incomplete or inconsistent."""


def _run(
    args: list[str],
    *,
    cwd: Path = REPO_ROOT,
    text: bool = True,
) -> str | bytes:
    """Run one checked command and return its standard output."""

    return subprocess.check_output(args, cwd=cwd, text=text)


def _git(*args: str, text: bool = True) -> str | bytes:
    """Run a checked Git query in the rlvgl repository."""

    return _run(["git", *args], text=text)


def _sha256(data: bytes) -> str:
    """Return a manifest-formatted SHA-256 digest."""

    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def _canonical_bytes(value: Any) -> bytes:
    """Encode a value using the canonical JSON form used by CPY evidence."""

    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def _relative(path: str | Path) -> str:
    """Return a repository-relative POSIX path."""

    return Path(path).resolve().relative_to(REPO_ROOT).as_posix()


def _load_json(path: Path) -> dict[str, Any]:
    """Load one UTF-8 JSON object."""

    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise EvidenceError(f"expected JSON object: {_relative(path)}")
    return value


def tracked_cargo_manifest_digest(commit: str) -> tuple[int, str]:
    """Hash every tracked Cargo manifest at an immutable source commit."""

    listing = _git("ls-tree", "-r", "--name-only", commit)
    assert isinstance(listing, str)
    paths = sorted(
        path
        for path in listing.splitlines()
        if path == "Cargo.toml" or path.endswith("/Cargo.toml")
    )
    digest = hashlib.sha256()
    for path in paths:
        data = _git("show", f"{commit}:{path}", text=False)
        assert isinstance(data, bytes)
        digest.update(path.encode("utf-8"))
        digest.update(b"\0")
        digest.update(data)
        digest.update(b"\0")
    return len(paths), f"sha256:{digest.hexdigest()}"


def _assert_current_manifests_match(commit: str) -> None:
    """Refuse capture when the current Cargo manifests differ from the pin."""

    listing = _git("ls-tree", "-r", "--name-only", commit)
    assert isinstance(listing, str)
    pinned = sorted(
        path
        for path in listing.splitlines()
        if path == "Cargo.toml" or path.endswith("/Cargo.toml")
    )
    current = _git("ls-files", "Cargo.toml", "*/Cargo.toml", "**/Cargo.toml")
    assert isinstance(current, str)
    current_paths = sorted(set(current.splitlines()))
    if current_paths != pinned:
        raise EvidenceError("tracked Cargo manifest set differs from the baseline commit")
    for path in pinned:
        source = _git("show", f"{commit}:{path}", text=False)
        assert isinstance(source, bytes)
        if (REPO_ROOT / path).read_bytes() != source:
            raise EvidenceError(f"Cargo manifest differs from {commit[:8]}: {path}")


def load_cargo_metadata(*, locked: bool = True) -> dict[str, Any]:
    """Load Cargo metadata for the current workspace."""

    args = ["cargo", "metadata", "--format-version", "1"]
    if locked:
        args.append("--locked")
    raw = _run(args)
    assert isinstance(raw, str)
    return json.loads(raw)


def normalized_workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    """Project Cargo metadata into a stable workspace graph snapshot."""

    workspace_ids = set(metadata["workspace_members"])
    packages: list[dict[str, Any]] = []
    for package in metadata["packages"]:
        if package["id"] not in workspace_ids:
            continue
        packages.append(
            {
                "dependencies": sorted(
                    {
                        dependency["rename"] or dependency["name"]
                        for dependency in package["dependencies"]
                        if dependency.get("path") is not None
                    }
                ),
                "features": {
                    name: sorted(values)
                    for name, values in sorted(package["features"].items())
                },
                "manifest_path": _relative(package["manifest_path"]),
                "name": package["name"],
                "publish": package.get("publish"),
                "version": package["version"],
            }
        )
    return sorted(packages, key=lambda package: (package["name"], package["manifest_path"]))


def _public_path_records(commit: str) -> list[dict[str, str]]:
    """Hash the public/ownership paths named by CPY-02 at the baseline."""

    records = []
    for path in PUBLIC_BASELINE_PATHS:
        data = _git("show", f"{commit}:{path}", text=False)
        assert isinstance(data, bytes)
        records.append({"path": path, "sha256": _sha256(data)})
    return records


def _publish_order() -> list[str]:
    """Read the governed publication order from its executable authority."""

    output = _run(["bash", "scripts/publish_changed.sh", "--print-order"])
    assert isinstance(output, str)
    return [line for line in output.splitlines() if line]


def capture(manifest_path: Path) -> None:
    """Capture the detached lockfile and normalized graph for one baseline."""

    manifest = _load_json(manifest_path)
    commit = manifest["rlvgl_source"]["commit"]
    _git("cat-file", "-e", f"{commit}^{{commit}}")
    _assert_current_manifests_match(commit)

    count, manifest_digest = tracked_cargo_manifest_digest(commit)
    expected_digest = manifest["rlvgl_source"]["cargo_manifest_set_sha256"]
    if manifest_digest != expected_digest:
        raise EvidenceError(
            f"Cargo manifest digest mismatch: expected {expected_digest}, got {manifest_digest}"
        )

    lock_path = REPO_ROOT / "Cargo.lock"
    if not lock_path.is_file():
        raise EvidenceError("Cargo.lock resolver snapshot is missing")
    metadata = load_cargo_metadata(locked=True)
    packages = normalized_workspace_packages(metadata)

    GENERATED_DIR.mkdir(parents=True, exist_ok=True)
    lock_name = f"CPY-CARGO-LOCK-{commit[:7]}.lock"
    graph_name = f"CPY-GRAPH-{commit[:7]}.json"
    lock_output = GENERATED_DIR / lock_name
    graph_output = GENERATED_DIR / graph_name
    lock_output.write_bytes(lock_path.read_bytes())

    graph = {
        "schema_version": "CPY-GRAPH-1",
        "source_commit": commit,
        "cargo_manifest_count": count,
        "cargo_manifest_set_sha256": manifest_digest,
        "cargo_lock": {
            "path": _relative(lock_output),
            "sha256": _sha256(lock_output.read_bytes()),
            "tracking": "workspace-ignored-resolver-snapshot",
        },
        "cargo_version": manifest["rust"]["cargo_version"],
        "workspace_package_count": len(packages),
        "workspace_packages_sha256": _sha256(_canonical_bytes(packages)),
        "workspace_packages": packages,
        "public_paths": _public_path_records(commit),
        "publish_order": _publish_order(),
    }
    graph_output.write_text(json.dumps(graph, indent=2) + "\n", encoding="utf-8")
    print(f"captured {_relative(lock_output)} ({graph['cargo_lock']['sha256']})")
    print(
        f"captured {_relative(graph_output)} "
        f"({len(packages)} workspace packages, {_sha256(graph_output.read_bytes())})"
    )


def _validate_schema(manifest: dict[str, Any]) -> None:
    """Validate a manifest with the repository's authored JSON Schema."""

    schema = _load_json(SCHEMA_PATH)
    try:
        import jsonschema
    except ImportError:
        errors = _schema_errors(manifest, schema, schema, ())
        if errors:
            raise EvidenceError("baseline schema validation failed:\n  " + "\n  ".join(errors))
    else:  # pragma: no cover - exercised when the optional package is present
        jsonschema.Draft202012Validator.check_schema(schema)
        validator = jsonschema.Draft202012Validator(
            schema,
            format_checker=jsonschema.FormatChecker(),
        )
        errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
        if errors:
            rendered = "\n".join(
                f"  {'.'.join(map(str, error.path)) or '<root>'}: {error.message}"
                for error in errors
            )
            raise EvidenceError(f"baseline schema validation failed:\n{rendered}")


def _schema_errors(
    value: Any,
    schema: dict[str, Any],
    root: dict[str, Any],
    path: tuple[str | int, ...],
) -> list[str]:
    """Validate the JSON-Schema subset used by the CPY manifest grammar."""

    location = ".".join(map(str, path)) or "<root>"
    if "$ref" in schema:
        reference = schema["$ref"]
        if not reference.startswith("#/"):
            return [f"{location}: unsupported schema reference {reference}"]
        target: Any = root
        for component in reference[2:].split("/"):
            target = target[component.replace("~1", "/").replace("~0", "~")]
        return _schema_errors(value, target, root, path)

    errors: list[str] = []
    expected_type = schema.get("type")
    type_matches = {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "null": value is None,
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
    }
    if expected_type is not None and not type_matches.get(expected_type, False):
        return [f"{location}: expected {expected_type}"]
    if "const" in schema and value != schema["const"]:
        errors.append(f"{location}: expected constant {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{location}: value is not in the allowed set")

    one_of = schema.get("oneOf")
    if one_of is not None:
        matches = sum(not _schema_errors(value, option, root, path) for option in one_of)
        if matches != 1:
            errors.append(f"{location}: expected exactly one oneOf branch to match")
    for clause in schema.get("allOf", []):
        errors.extend(_schema_errors(value, clause, root, path))
    if_clause = schema.get("if")
    if if_clause is not None and not _schema_errors(value, if_clause, root, path):
        errors.extend(_schema_errors(value, schema.get("then", {}), root, path))

    if isinstance(value, dict):
        properties = schema.get("properties", {})
        for required in schema.get("required", []):
            if required not in value:
                errors.append(f"{location}: missing required property {required}")
        if schema.get("additionalProperties") is False:
            for name in value.keys() - properties.keys():
                errors.append(f"{location}: unexpected property {name}")
        for name, child in value.items():
            if name in properties:
                errors.extend(_schema_errors(child, properties[name], root, (*path, name)))

    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{location}: array is shorter than minItems")
        if schema.get("uniqueItems"):
            encoded = [_canonical_bytes(item) for item in value]
            if len(encoded) != len(set(encoded)):
                errors.append(f"{location}: array items are not unique")
        item_schema = schema.get("items")
        if item_schema is not None:
            for index, item in enumerate(value):
                errors.extend(_schema_errors(item, item_schema, root, (*path, index)))
        contains = schema.get("contains")
        if contains is not None and not any(
            not _schema_errors(item, contains, root, (*path, index))
            for index, item in enumerate(value)
        ):
            errors.append(f"{location}: no array item satisfies contains")

    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{location}: string is shorter than minLength")
        if "pattern" in schema and re.search(schema["pattern"], value) is None:
            errors.append(f"{location}: string does not match {schema['pattern']}")
        if schema.get("format") == "date-time":
            try:
                dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
            except ValueError:
                errors.append(f"{location}: invalid date-time")

    if isinstance(value, int) and not isinstance(value, bool):
        if value < schema.get("minimum", value):
            errors.append(f"{location}: integer is below minimum")
    return errors


def _verify_authorities(manifest: dict[str, Any]) -> None:
    """Verify every consumed authority against its immutable source commit."""

    for authority in manifest["authorities"]:
        commit = authority["source_commit"]
        path = authority["source_path"]
        raw = _git("show", f"{commit}:{path}")
        assert isinstance(raw, str)
        if not re.search(r"^\*\*Status:\*\*.*Ratified", raw, flags=re.MULTILINE):
            raise EvidenceError(f"authority is not Ratified at its pin: {authority['document_id']}")
        if authority["revision_basis"] == "document-changelog":
            revision = re.escape(authority["revision"])
            if not re.search(rf"^### {revision}\b", raw, flags=re.MULTILINE):
                raise EvidenceError(
                    f"authority revision is absent at its pin: {authority['document_id']} "
                    f"{authority['revision']}"
                )
        elif authority["revision"] is not None:
            raise EvidenceError(
                f"legacy source-commit authority must have null revision: {authority['document_id']}"
            )


def _verify_source(manifest: dict[str, Any]) -> None:
    """Verify source, Cargo-input, and vendored-gitlink pins."""

    source = manifest["rlvgl_source"]
    commit = source["commit"]
    _git("cat-file", "-e", f"{commit}^{{commit}}")
    count, digest = tracked_cargo_manifest_digest(commit)
    if count != source["cargo_manifest_count"]:
        raise EvidenceError(
            f"Cargo manifest count mismatch: expected {source['cargo_manifest_count']}, got {count}"
        )
    if digest != source["cargo_manifest_set_sha256"]:
        raise EvidenceError(
            f"Cargo manifest digest mismatch: expected {source['cargo_manifest_set_sha256']}, "
            f"got {digest}"
        )

    for vendored in manifest["vendored_sources"]:
        raw = _git("ls-tree", commit, vendored["path"])
        assert isinstance(raw, str)
        match = re.match(r"160000 commit ([0-9a-f]{40})\t", raw)
        if match is None or match.group(1) != vendored["commit"]:
            raise EvidenceError(f"vendored gitlink mismatch: {vendored['path']}")


def _verify_evidence_artifacts(manifest: dict[str, Any]) -> dict[str, Path]:
    """Verify every evidence artifact digest and return paths by kind."""

    by_kind: dict[str, Path] = {}
    for artifact in manifest["evidence_artifacts"]:
        path = REPO_ROOT / artifact["path"]
        if not path.is_file():
            raise EvidenceError(f"missing evidence artifact: {artifact['path']}")
        actual = _sha256(path.read_bytes())
        if actual != artifact["sha256"]:
            raise EvidenceError(
                f"evidence digest mismatch for {artifact['path']}: "
                f"expected {artifact['sha256']}, got {actual}"
            )
        if artifact["kind"] in by_kind:
            raise EvidenceError(f"duplicate evidence kind: {artifact['kind']}")
        by_kind[artifact["kind"]] = path
    required = {"cargo-lock", "cargo-graph", "mpy-handoff"}
    if missing := required - set(by_kind):
        raise EvidenceError(f"missing required evidence kinds: {sorted(missing)}")
    return by_kind


def _verify_graph(manifest: dict[str, Any], graph_path: Path) -> None:
    """Verify the historical Cargo graph projection and public path ledger."""

    graph = _load_json(graph_path)
    source = manifest["rlvgl_source"]
    if graph.get("schema_version") != "CPY-GRAPH-1":
        raise EvidenceError("unsupported CPY graph schema")
    if graph.get("source_commit") != source["commit"]:
        raise EvidenceError("graph source commit does not match baseline")
    if graph.get("cargo_manifest_set_sha256") != source["cargo_manifest_set_sha256"]:
        raise EvidenceError("graph Cargo-manifest digest does not match baseline")
    packages = graph.get("workspace_packages", [])
    if graph.get("workspace_package_count") != len(packages):
        raise EvidenceError("graph workspace package count is inconsistent")
    if graph.get("workspace_packages_sha256") != _sha256(_canonical_bytes(packages)):
        raise EvidenceError("graph workspace package projection digest is inconsistent")

    commit = source["commit"]
    expected_public = {record["path"]: record["sha256"] for record in graph["public_paths"]}
    if set(expected_public) != set(PUBLIC_BASELINE_PATHS):
        raise EvidenceError("graph public-path ledger is incomplete")
    for path, expected in expected_public.items():
        data = _git("show", f"{commit}:{path}", text=False)
        assert isinstance(data, bytes)
        if _sha256(data) != expected:
            raise EvidenceError(f"public-path digest mismatch: {path}")

    package_names = {package["name"] for package in packages}
    for required in ("rlvgl-api", "rlvgl-core", "rlvgl-platform", "rlvgl-micropython"):
        if required not in package_names:
            raise EvidenceError(f"baseline graph omits protected package: {required}")
    if "rlvgl-runtime-std" in package_names or "rlvgl-cpython" in package_names:
        raise EvidenceError("baseline graph unexpectedly includes a future CPY crate")


def _verify_handoff(manifest: dict[str, Any], handoff_path: Path) -> None:
    """Verify the initial MPY Safe Point and exact authorized path set."""

    handoff = _load_json(handoff_path)
    if handoff.get("schema_version") != "CPY-HANDOFF-1":
        raise EvidenceError("unsupported CPY handoff schema")
    if handoff.get("source_commit") != manifest["rlvgl_source"]["commit"]:
        raise EvidenceError("handoff source commit does not match baseline")
    if set(handoff.get("authorized_shared_paths", [])) != ALLOWED_HANDOFF_PATHS:
        raise EvidenceError("handoff authorized path set changed")
    if handoff.get("active_mpy_work", {}).get("pcdn") != "PCDN-MPY-04-012":
        raise EvidenceError("handoff does not identify the concurrent MPY slice")
    if not handoff.get("acknowledged"):
        raise EvidenceError("MPY handoff is not acknowledged")


def _verify_closeout(manifest: dict[str, Any], closeout: dict[str, Any]) -> None:
    """Verify the first coordinated migration-wave closeout record."""

    if closeout.get("schema_version") != "CPY-COORDINATION-CLOSEOUT-1":
        raise EvidenceError("unsupported CPY coordination closeout schema")
    if closeout.get("state") != "closed":
        raise EvidenceError("CPY coordination wave is not closed")

    authority = closeout.get("authority", {})
    if authority.get("handoff_source_commit") != manifest["rlvgl_source"]["commit"]:
        raise EvidenceError("closeout handoff source does not match the CPY baseline")
    if set(authority.get("authorized_shared_paths", [])) != ALLOWED_HANDOFF_PATHS:
        raise EvidenceError("closeout authorized path set changed")

    publication = closeout.get("implementation_publication", {})
    commit_fields = (
        "upstream_wayland_commit",
        "pre_rebase_frontier",
        "published_implementation_frontier",
        "implementation_parent_pin_commit",
        "implementation_parent_gitlink",
    )
    for field in commit_fields:
        if re.fullmatch(r"[0-9a-f]{40}", publication.get(field, "")) is None:
            raise EvidenceError(f"closeout has invalid Git commit field: {field}")
    if (
        publication["implementation_parent_gitlink"]
        != publication["published_implementation_frontier"]
    ):
        raise EvidenceError("parent gitlink does not match the published rlvgl frontier")
    _git(
        "merge-base",
        "--is-ancestor",
        publication["published_implementation_frontier"],
        "HEAD",
    )

    provenance = closeout.get("rebase_provenance", {})
    source_mappings = provenance.get("source_commit_mappings", [])
    index_mappings = provenance.get("regenerated_index_commit_mappings", [])
    if len(source_mappings) != 18 or len(index_mappings) != 10:
        raise EvidenceError("closeout rebase map must contain 18 source and 10 index commits")
    mappings = source_mappings + index_mappings
    before = [mapping.get("before") for mapping in mappings]
    after = [mapping.get("after") for mapping in mappings]
    if any(re.fullmatch(r"[0-9a-f]{40}", commit or "") is None for commit in before + after):
        raise EvidenceError("closeout rebase map contains an invalid Git commit")
    if len(set(before)) != 28 or len(set(after)) != 28:
        raise EvidenceError("closeout rebase map contains duplicate commit identities")
    for mapping in mappings:
        subject = _git("show", "-s", "--format=%s", mapping["after"])
        assert isinstance(subject, str)
        if subject.strip() != mapping.get("subject"):
            raise EvidenceError(f"closeout rebase subject mismatch: {mapping['after'][:8]}")
    final_mapping = next(
        (
            mapping
            for mapping in index_mappings
            if mapping.get("before") == publication["pre_rebase_frontier"]
        ),
        None,
    )
    if (
        final_mapping is None
        or final_mapping.get("after") != publication["published_implementation_frontier"]
    ):
        raise EvidenceError("closeout final frontier is absent from the rebase map")
    if tuple(provenance.get("pre_to_post_frontier_tree_delta", [])) != CLOSEOUT_FRONTIER_DELTA:
        raise EvidenceError("closeout pre/post frontier tree delta changed")

    cpy = closeout.get("cpy_disposition", {})
    if set(cpy.get("ratified_phases", [])) != {"CPY-00", "CPY-01", "CPY-02"}:
        raise EvidenceError("closeout overstates the ratified CPY phase set")
    if set(cpy.get("draft_phases", [])) != {f"CPY-{phase:02d}" for phase in range(3, 10)}:
        raise EvidenceError("closeout does not preserve every Draft CPY phase")

    mpy = closeout.get("mpy_disposition", {})
    if set(mpy.get("ratified_phases", [])) != {f"MPY-{phase:02d}" for phase in range(6)}:
        raise EvidenceError("closeout overstates the ratified MPY phase set")
    next_pcdn = mpy.get("next_pcdn", {})
    if next_pcdn.get("id") != "PCDN-MPY-04-017" or next_pcdn.get("state") != "proposal-only":
        raise EvidenceError("closeout must preserve PCDN-MPY-04-017 as proposal-only")
    if mpy.get("coverage_summary") != {"current": 3, "partial": 2, "without_claims": 10}:
        raise EvidenceError("closeout MPY coverage summary changed")


def validate_closeout(
    manifest: dict[str, Any], closeout_path: Path = CLOSEOUT_PATH
) -> dict[str, Any]:
    """Validate and return the coordinated migration-wave closeout."""

    closeout = _load_json(closeout_path)
    _verify_closeout(manifest, closeout)
    return closeout


def _verify_matrix(manifest: dict[str, Any]) -> None:
    """Verify required interpreter, rootfs, profile, and board selections."""

    builds = manifest["cpython"]["builds"]
    versions = {build["version"] for build in builds}
    if versions != {"3.13.15", "3.14.7"}:
        raise EvidenceError(f"unexpected CPython patch set: {sorted(versions)}")
    selected_pairs = {(build["version"], build["target_triple"]) for build in builds}
    expected_pairs = {(version, target) for version in versions for target in REQUIRED_TARGETS}
    if selected_pairs != expected_pairs:
        raise EvidenceError("CPython build matrix does not cover both minors on every target")
    if any(build["qualification_state"] != "selected" for build in builds):
        raise EvidenceError("baseline must not relabel unexecuted CPython builds as verified")

    rootfs = manifest["root_filesystems"]
    rootfs_pairs = {
        (row["architecture"], package["version"])
        for row in rootfs
        for package in row["python_packages"]
        if package["name"] == "cpython"
    }
    expected_rootfs = {
        (architecture, version)
        for architecture in ("armhf", "arm64")
        for version in versions
    }
    if rootfs_pairs != expected_rootfs:
        raise EvidenceError("rootfs matrix does not cover armhf/arm64 and both CPython minors")
    if any(row["qualification_state"] != "selected" for row in rootfs):
        raise EvidenceError("baseline rootfs rows must remain selected until CPY-06/09 proof")

    profiles = {artifact["profile"] for artifact in manifest["artifacts"]}
    if profiles != REQUIRED_PROFILES:
        raise EvidenceError(f"profile matrix mismatch: {sorted(profiles)}")
    if any(artifact["qualification_state"] != "planned" for artifact in manifest["artifacts"]):
        raise EvidenceError("unbuilt CPY artifacts must remain planned")

    board = manifest["reference_board"]
    rootfs_by_id = {row["id"]: row for row in rootfs}
    if rootfs_by_id[board["rootfs_id"]]["architecture"] != "armhf":
        raise EvidenceError("reference board must select an armhf rootfs")
    if board["kernel_release"] != "6.12.76-bone50":
        raise EvidenceError("reference board must use the proven Bookworm board kernel")
    if "6.19-bone" not in board["excluded_kernel_releases"]:
        raise EvidenceError("stock Trixie kernel exclusion is missing")
    device_states = {device["class"]: device["evidence_state"] for device in board["devices"]}
    if device_states.get("display") != "observed-functional":
        raise EvidenceError("reference display evidence is not recorded")
    if device_states.get("input") != "observed-driver-only":
        raise EvidenceError("touch must remain driver-only while the cape RMA is open")


def validate_baseline(manifest_path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    """Validate one complete CPY baseline and all local evidence links."""

    manifest = _load_json(manifest_path)
    _validate_schema(manifest)
    _verify_source(manifest)
    _verify_authorities(manifest)
    artifacts = _verify_evidence_artifacts(manifest)
    _verify_graph(manifest, artifacts["cargo-graph"])
    _verify_handoff(manifest, artifacts["mpy-handoff"])
    _verify_matrix(manifest)
    validate_closeout(manifest)
    return manifest


def _package_lookup(metadata: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, set[str]]]:
    """Build package-name and resolved-dependency lookup tables."""

    by_id = {package["id"]: package for package in metadata["packages"]}
    by_name = {package["name"]: package for package in metadata["packages"]}
    edges: dict[str, set[str]] = {package_id: set() for package_id in by_id}
    for node in metadata["resolve"]["nodes"]:
        edges[node["id"]] = {
            dependency["pkg"]
            for dependency in node["deps"]
            if any(kind.get("kind") != "dev" for kind in dependency["dep_kinds"])
        }
    return by_name, edges


def _dependency_closure(package_id: str, edges: dict[str, set[str]]) -> set[str]:
    """Return every package id transitively reachable from one package."""

    pending = list(edges.get(package_id, set()))
    seen: set[str] = set()
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        pending.extend(edges.get(current, set()) - seen)
    return seen


def dependency_firewall_violations(metadata: dict[str, Any]) -> list[str]:
    """Return all CPY-02 dependency-firewall violations in Cargo metadata."""

    by_name, edges = _package_lookup(metadata)
    by_id = {package["id"]: package for package in metadata["packages"]}
    violations: list[str] = []

    def forbidden_name(name: str) -> bool:
        lowered = name.lower().replace("_", "-")
        return (
            lowered.startswith("pyo3")
            or lowered in {"cpython", "python3-sys", "rlvgl-cpython", "rlvgl-micropython"}
            or "micropython" in lowered
        )

    policies = {
        "rlvgl-api": lambda name: forbidden_name(name)
        or name in {"rlvgl-runtime-std", "rlvgl-platform"},
        "rlvgl-core": lambda name: forbidden_name(name)
        or name in {"rlvgl-runtime-std", "rlvgl-platform"},
        "rlvgl-micropython": lambda name: name in {"rlvgl-runtime-std", "rlvgl-cpython"}
        or name.lower().startswith("pyo3"),
        "rlvgl-runtime-std": lambda name: forbidden_name(name),
        "rlvgl-cpython": lambda name: "micropython" in name.lower(),
    }
    for owner, is_forbidden in policies.items():
        package = by_name.get(owner)
        if package is None:
            continue
        closure = _dependency_closure(package["id"], edges)
        bad = sorted({by_id[package_id]["name"] for package_id in closure if is_forbidden(by_id[package_id]["name"])})
        for name in bad:
            violations.append(f"{owner} transitively depends on forbidden package {name}")

    api = by_name.get("rlvgl-api")
    if api is None:
        violations.append("workspace does not contain rlvgl-api")
    else:
        for feature in ("micropython", "cpython", "cm4", "sim"):
            if api["features"].get(feature) != []:
                violations.append(f"rlvgl-api marker feature {feature} is no longer an empty no-op")
        for package in metadata["packages"]:
            for dependency in package["dependencies"]:
                dependency_name = dependency.get("package") or dependency["name"]
                if dependency_name != "rlvgl-api":
                    continue
                selected = set(dependency.get("features", [])) & {"micropython", "cpython", "cm4", "sim"}
                if selected:
                    violations.append(
                        f"{package['name']} enables deprecated rlvgl-api marker features "
                        f"{sorted(selected)}"
                    )
    return sorted(set(violations))


def validate_firewall() -> dict[str, Any]:
    """Run the CPY-02 dependency firewall against the current workspace."""

    metadata = load_cargo_metadata(locked=False)
    violations = dependency_firewall_violations(metadata)
    if violations:
        raise EvidenceError("dependency firewall failed:\n  " + "\n  ".join(violations))
    return metadata


def _parse_args(argv: Iterable[str]) -> argparse.Namespace:
    """Parse the command-line interface."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("capture", "check"):
        command = subparsers.add_parser(name)
        command.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers.add_parser("firewall")
    subparsers.add_parser("all")
    return parser.parse_args(list(argv))


def main(argv: Iterable[str] = sys.argv[1:]) -> int:
    """Run one CPY evidence command."""

    args = _parse_args(argv)
    try:
        if args.command == "capture":
            capture(args.manifest.resolve())
        elif args.command == "check":
            manifest = validate_baseline(args.manifest.resolve())
            print(
                f"CPY baseline valid: {manifest['manifest_id']} at "
                f"{manifest['rlvgl_source']['commit'][:8]}"
            )
        elif args.command == "firewall":
            metadata = validate_firewall()
            print(f"CPY dependency firewall valid: {len(metadata['workspace_members'])} workspace packages")
        else:
            manifest = validate_baseline(DEFAULT_MANIFEST)
            metadata = validate_firewall()
            print(
                f"CPY evidence valid: {manifest['manifest_id']}; dependency firewall valid: "
                f"{len(metadata['workspace_members'])} workspace packages"
            )
    except (EvidenceError, subprocess.CalledProcessError, OSError, json.JSONDecodeError) as exc:
        print(f"CPY evidence error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
