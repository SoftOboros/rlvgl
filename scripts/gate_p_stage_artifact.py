#!/usr/bin/env python3
"""Carry the exact ordered Gate P package set between isolated CI jobs.

The artifact contains only original Cargo archives and a binding manifest.
Restoration verifies every byte, then recreates the extracted-package patch
table at the consumer's local stage path. It never substitutes a workspace
path or a registry package for a staged package.
"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require_sha(value, label):
    if len(value) != 40 and label != "manifest":
        raise ValueError(f"{label}: expected a Git SHA")
    if label == "manifest" and len(value) != 64:
        raise ValueError("manifest: expected SHA-256")
    int(value, 16)
    return value.lower()


def order_and_versions(stage):
    order = (stage / "ordered-crates.txt").read_text().splitlines()
    if len(order) != 31 or len(set(order)) != 31 or any(not name for name in order):
        raise ValueError("Gate P requires exactly 31 distinct ordered crates")
    versions = dict(line.split("\t", 1) for line in (stage / "versions.tsv").read_text().splitlines())
    if any(name not in versions for name in order):
        raise ValueError("missing ordered crate version")
    return order, versions


def pack(args):
    stage, packages, output = map(Path, (args.stage, args.packages, args.output))
    if output.exists() and any(output.iterdir()):
        raise ValueError("artifact output must be empty")
    output.mkdir(parents=True, exist_ok=True)
    order, versions = order_and_versions(stage)
    actual_staged = {path.name for path in (stage / "staged").iterdir() if path.is_dir()}
    expected_staged = {f"{name}-{versions[name]}" for name in order}
    if actual_staged != expected_staged:
        raise ValueError("staged package set differs from publish order")
    rows = []
    for name in order:
        version = versions[name]
        filename = f"{name}-{version}.crate"
        source = packages / filename
        if not source.is_file():
            raise ValueError(f"missing original archive: {filename}")
        shutil.copyfile(source, output / filename)
        rows.append({"name": name, "version": version, "file": filename,
                     "size": source.stat().st_size, "sha256": sha256(source)})
    manifest = {"format": 1, "source_sha": require_sha(args.source_sha, "source"),
                "workflow_sha": require_sha(args.workflow_sha, "workflow"),
                "nightly_manifest_sha256": require_sha(args.manifest_sha, "manifest"),
                "crates": rows}
    (output / "manifest.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")


def safe_extract(archive, destination, expected_prefix):
    with tarfile.open(archive, "r:gz") as stream:
        members = stream.getmembers()
        if not members:
            raise ValueError(f"empty archive: {archive.name}")
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts
                    or path.parts[0] != expected_prefix or member.issym() or member.islnk()
                    or not (member.isfile() or member.isdir())):
                raise ValueError(f"unsafe package member: {member.name}")
        for member in members:
            target = destination.joinpath(*PurePosixPath(member.name).parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                source = stream.extractfile(member)
                if source is None:
                    raise ValueError(f"unreadable package member: {member.name}")
                with source, target.open("xb") as output:
                    shutil.copyfileobj(source, output)


def restore(args):
    artifact, stage = Path(args.artifact), Path(args.stage)
    manifest = json.loads((artifact / "manifest.json").read_text())
    expected_binding = {"format": 1,
                        "source_sha": require_sha(args.source_sha, "source"),
                        "workflow_sha": require_sha(args.workflow_sha, "workflow"),
                        "nightly_manifest_sha256": require_sha(args.manifest_sha, "manifest")}
    for key, expected in expected_binding.items():
        if manifest.get(key) != expected:
            raise ValueError(f"Gate P artifact {key} binding mismatch")
    rows = manifest.get("crates")
    if not isinstance(rows, list) or len(rows) != 31:
        raise ValueError("Gate P artifact must contain 31 ordered crates")
    names = [row["name"] for row in rows]
    if len(set(names)) != 31:
        raise ValueError("duplicate staged crate")
    expected_order = subprocess.check_output(
        [str(Path(args.publisher).resolve()), "--print-order"], text=True).splitlines()
    if names != expected_order:
        raise ValueError("Gate P artifact order differs from publisher order")
    expected_files = {"manifest.json", *(row["file"] for row in rows)}
    if {path.name for path in artifact.iterdir()} != expected_files:
        raise ValueError("Gate P artifact has missing or extra files")
    for row in rows:
        filename = f"{row['name']}-{row['version']}.crate"
        if row["file"] != filename or "/" in filename or ".." in filename:
            raise ValueError("invalid archive identity")
        archive = artifact / filename
        if archive.stat().st_size != row["size"] or sha256(archive) != row["sha256"]:
            raise ValueError(f"Gate P archive mismatch: {filename}")
    if stage.exists() and any(stage.iterdir()):
        raise ValueError("consumer stage directory must be empty")
    (stage / "staged").mkdir(parents=True)
    for row in rows:
        safe_extract(artifact / row["file"], stage / "staged",
                     f"{row['name']}-{row['version']}")
    (stage / "ordered-crates.txt").write_text("\n".join(names) + "\n")
    (stage / "versions.tsv").write_text(
        "".join(f"{row['name']}\t{row['version']}\n" for row in rows))
    (stage / "patch-config.toml").write_text(
        "[patch.crates-io]\n" + "".join(
            f"{row['name']} = {{ path = \"{stage.resolve()}/staged/{row['name']}-{row['version']}\" }}\n"
            for row in rows))


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("pack", "restore"):
        command = sub.add_parser(action)
        command.add_argument("--stage", required=True)
        command.add_argument("--source-sha", required=True)
        command.add_argument("--workflow-sha", required=True)
        command.add_argument("--manifest-sha", required=True)
        if action == "pack":
            command.add_argument("--packages", required=True)
            command.add_argument("--output", required=True)
        else:
            command.add_argument("--artifact", required=True)
            command.add_argument("--publisher", required=True)
    args = parser.parse_args()
    try:
        (pack if args.action == "pack" else restore)(args)
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        parser.exit(1, f"Gate P artifact: {error}\n")


if __name__ == "__main__":
    main()
