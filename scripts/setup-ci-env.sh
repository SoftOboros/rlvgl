#!/usr/bin/env bash
# One immutable official nightly cohort per workflow; publication stays stable.
set -euo pipefail
python3 - "$0" "$@" <<'COHORT_PY'
import argparse
import datetime
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request

LATEST = "https://static.rust-lang.org/dist/channel-rust-nightly.toml"
CROSS_TARGETS = (
    "thumbv7em-none-eabihf", "riscv32imc-unknown-none-elf",
    "riscv32imafc-unknown-none-elf", "riscv32imac-unknown-none-elf",
    "aarch64-unknown-linux-gnu", "armv7-unknown-linux-gnueabihf",
)
COMPONENTS = ("rustfmt-preview", "clippy-preview", "rust-src", "llvm-tools-preview")
SHA256 = re.compile(r"[0-9a-f]{64}")
GIT_SHA = re.compile(r"[0-9a-f]{40}")
VERSION = re.compile(r"(?P<version>\d+\.\d+\.\d+-nightly) \((?P<commit>[0-9a-f]{7,40}) (?P<date>\d{4}-\d{2}-\d{2})\)")


class CohortError(RuntimeError):
    pass


def require(ok, message):
    if not ok:
        raise CohortError(message)


def digest(body):
    return hashlib.sha256(body).hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def run(command, env=None):
    result = subprocess.run(command, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, check=False)
    if result.returncode:
        sys.stdout.write(result.stdout)
        raise CohortError("Command failed (%s): %s" % (result.returncode, " ".join(command)))
    return result.stdout


def fetch_bytes(url, limit=8 * 1024 * 1024):
    parsed = urllib.parse.urlparse(url)
    require(parsed.scheme == "https" and parsed.netloc == "static.rust-lang.org",
            "Only the official Rust distribution host is admitted")
    with urllib.request.urlopen(url, timeout=60) as response:
        require(urllib.parse.urlparse(response.geturl()).netloc == "static.rust-lang.org",
                "Distribution download redirected outside the official host")
        body = response.read(limit + 1)
    require(len(body) <= limit, "Distribution response exceeded its bounded size")
    return body


def latest_manifest():
    body = fetch_bytes(LATEST)
    checksum = fetch_bytes(LATEST + ".sha256", 4096).decode("ascii").strip().split()
    require(checksum and SHA256.fullmatch(checksum[0]), "Official manifest checksum is malformed")
    require(digest(body) == checksum[0], "Newest manifest/checksum disagree; resolve a fresh workflow cohort")
    return body


def identity(value, label):
    match = VERSION.fullmatch(value or "")
    require(match is not None, "%s advertised nightly identity is malformed" % label)
    datetime.date.fromisoformat(match["date"])
    return match.groupdict()


def archive(manifest, package, target):
    item = manifest.get("pkg", {}).get(package, {}).get("target", {}).get(target)
    require(isinstance(item, dict) and item.get("available") is True,
            "Official component/target unavailable: %s/%s" % (package, target))
    url, checksum = item.get("xz_url"), item.get("xz_hash")
    require(isinstance(url, str) and isinstance(checksum, str) and SHA256.fullmatch(checksum),
            "Official component archive identity is missing: %s/%s" % (package, target))
    parsed = urllib.parse.urlparse(url)
    require(parsed.scheme == "https" and parsed.netloc == "static.rust-lang.org"
            and parsed.path.startswith("/dist/" + manifest["date"] + "/")
            and not parsed.query and not parsed.fragment,
            "Official component archive does not belong to the immutable date")
    return {"url": url, "sha256": checksum}


def parse_manifest(text):
    # Rust's distribution schema uses simple string/bool fields here. Parse
    # only that required schema, without adding a TOML runtime dependency.
    # Complex unrelated extension arrays are not interpreted; full raw bytes
    # remain bound to the official checksum in resolve/freshness.
    result, section, tables = {}, (), set()
    fields = {"manifest-version", "date", "version", "git_commit_hash",
              "available", "url", "hash", "xz_url", "xz_hash"}
    for number, original in enumerate(text.splitlines(), 1):
        line = original.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[["):
            # Rust component/extension arrays are not required fields.
            section = ("ignored-array",)
            continue
        if line.startswith("["):
            require(line.endswith("]"), "Malformed distribution table")
            content = line[1:-1]
            token = r'(?:[A-Za-z0-9_*\-]+|"(?:[^"\\]|\\.)*")'
            require(re.fullmatch(token + r"(?:\." + token + r")*", content),
                    "Unsupported distribution table at line %s" % number)
            segments = re.findall(token, content)
            section = tuple(json.loads(x) if x.startswith('"') else x for x in segments)
            require(section not in tables, "Duplicate distribution table")
            tables.add(section)
            continue
        match = re.match(r"([A-Za-z0-9_-]+)\s*=\s*(.*)$", line)
        if not match or match[1] not in fields:
            continue
        key, raw = match.groups()
        relevant = not section or (len(section) >= 2 and section[0] == "pkg"
                                  and (len(section) == 2 or
                                       (len(section) == 4 and section[2] == "target")))
        if not relevant:
            continue
        if key == "available":
            require(raw in ("true", "false"), "Invalid distribution availability")
            value = raw == "true"
        else:
            try:
                value = json.loads(raw)
            except (ValueError, TypeError) as exc:
                raise CohortError("Unsupported required distribution value at line %s" % number) from exc
            require(isinstance(value, str), "Required distribution field is not a string")
        table = result
        for segment in section:
            require(segment not in table or isinstance(table[segment], dict),
                    "Conflicting distribution table")
            table = table.setdefault(segment, {})
        require(key not in table, "Duplicate required distribution field")
        table[key] = value
    return result


def validate_manifest(body):
    try:
        manifest = parse_manifest(body.decode("utf-8"))
        date = manifest.get("date", "")
        require(isinstance(date, str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}", date),
                "Nightly manifest date is missing")
        datetime.date.fromisoformat(date)
        require(manifest.get("manifest-version") == "2", "Unsupported nightly manifest schema")
        rustc = identity(manifest.get("pkg", {}).get("rustc", {}).get("version"), "rustc")
        cargo = identity(manifest.get("pkg", {}).get("cargo", {}).get("version"), "Cargo")
        commit = manifest["pkg"]["rustc"].get("git_commit_hash", "")
        require(GIT_SHA.fullmatch(commit) and commit.startswith(rustc["commit"]),
                "Official rustc full commit does not match advertised compiler")
        require(rustc["date"] <= date and cargo["date"] <= date,
                "Compiler identity dates exceed the distribution cohort")
        # pkg.cargo.git_commit_hash is a distribution commit, not Cargo's own.
        return manifest, {"date": date, "rustc": dict(rustc, full_commit=commit),
                          "cargo": cargo}
    except (UnicodeError, ValueError, KeyError, TypeError) as exc:
        raise CohortError("Malformed official nightly manifest: %s" % exc) from exc


def github_binding():
    active = os.environ.get("GITHUB_ACTIONS") == "true"
    run_id, attempt = os.environ.get("GITHUB_RUN_ID", ""), os.environ.get("GITHUB_RUN_ATTEMPT", "")
    require(not active or (run_id.isdecimal() and attempt.isdecimal()),
            "GitHub cohort requires genuine run ID and attempt")
    return {"run_id": run_id, "run_attempt": attempt}


def provenance(source_sha, workflow_sha, helper):
    require(GIT_SHA.fullmatch(source_sha) and GIT_SHA.fullmatch(workflow_sha),
            "Source and workflow identities must be exact Git commits")
    require(run(["git", "rev-parse", "HEAD"]).strip() == source_sha,
            "Current checkout is not the declared immutable source")
    blob = subprocess.run(["git", "show", workflow_sha + ":scripts/setup-ci-env.sh"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    require(blob.returncode == 0, "Declared workflow setup-helper Git blob is unavailable")
    require(blob.stdout == Path(helper).read_bytes(),
            "Executing setup helper differs from its declared workflow Git blob")
    return digest(blob.stdout)


def produce(args, helper):
    helper_sha = provenance(args.source_sha, args.workflow_sha, helper)
    body = latest_manifest()
    _, advertised = validate_manifest(body)
    out = Path(args.output_dir)
    out.mkdir(parents=True, exist_ok=True)
    require(not (out / "cohort.json").exists() and not (out / "channel-rust-nightly.toml").exists(),
            "An immutable cohort already exists; never overwrite it")
    cohort = dict(schema=1, manifest_sha256=digest(body), helper_sha256=helper_sha,
                  source_sha=args.source_sha, workflow_sha=args.workflow_sha,
                  github=github_binding(), **advertised)
    (out / "channel-rust-nightly.toml").write_bytes(body)
    write_json(out / "cohort.json", cohort)
    outputs = {name: cohort[name] for name in
               ("date", "manifest_sha256", "helper_sha256", "source_sha", "workflow_sha")}
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as stream:
            for key, value in outputs.items():
                stream.write("%s=%s\n" % (key, value))
    print(json.dumps(outputs, sort_keys=True))


def verify(args, helper):
    require(SHA256.fullmatch(args.expected_manifest_sha), "Expected manifest digest is malformed")
    body = Path(args.manifest).read_bytes()
    require(digest(body) == args.expected_manifest_sha, "Transported manifest checksum mismatch")
    manifest, advertised = validate_manifest(body)
    cohort = json.loads(Path(args.cohort).read_text())
    require(isinstance(cohort, dict) and cohort.get("schema") == 1, "Invalid immutable cohort schema")
    for key, value in advertised.items():
        require(cohort.get(key) == value, "Cohort advertised identity mismatch: " + key)
    for key, value in (("manifest_sha256", args.expected_manifest_sha),
                       ("source_sha", args.source_sha), ("workflow_sha", args.workflow_sha),
                       ("github", github_binding())):
        require(cohort.get(key) == value, "Cohort binding mismatch: " + key)
    actual_helper = provenance(args.source_sha, args.workflow_sha, helper)
    require(cohort.get("helper_sha256") == actual_helper, "Cohort helper identity mismatch")
    return manifest, cohort


def compiler_fields(verbose):
    return dict(line.split(": ", 1) for line in verbose.splitlines() if ": " in line)


def verify_actual(cohort, rustc_verbose, cargo_verbose, host):
    rust = compiler_fields(rustc_verbose)
    cargo = compiler_fields(cargo_verbose)
    require(rust.get("host") == host and cargo.get("host") == host,
            "Actual compiler/Cargo host mismatch")
    expected = cohort["rustc"]
    require(rust.get("release") == expected["version"]
            and rust.get("commit-hash") == expected["full_commit"]
            and rust.get("commit-date") == expected["date"],
            "Actual rustc identity differs from immutable manifest")
    expected = cohort["cargo"]
    require(cargo.get("commit-hash", "").startswith(expected["commit"])
            and cargo.get("commit-date") == expected["date"]
            and cargo.get("release", "").endswith("-nightly"),
            "Actual Cargo own commit/date/nightly differs from immutable manifest")
    # Package Cargo version is not its CLI version. The trusted archive below
    # binds the actual executable without inventing a version conversion.


def verify_inventory(components, targets, host):
    installed = set(components.splitlines())
    required = {"rustfmt-" + host, "clippy-" + host, "rust-src", "llvm-tools-" + host}
    require(required <= installed, "Required genuine component inventory is incomplete: " +
            ", ".join(sorted(required - installed)))
    required_targets = {host, *CROSS_TARGETS}
    require(required_targets <= set(targets.splitlines()),
            "Required genuine standard-library target inventory is incomplete")


def verify_cargo_archive(metadata, binary, out):
    body = fetch_bytes(metadata["url"], 128 * 1024 * 1024)
    require(digest(body) == metadata["sha256"], "Official Cargo archive checksum mismatch")
    archive_path = out / "cargo-component.tar.xz"
    archive_path.write_bytes(body)
    with tarfile.open(archive_path, "r:xz") as archive_file:
        matches = [entry for entry in archive_file.getmembers()
                   if entry.isfile() and entry.name.endswith("/cargo/bin/cargo")]
        require(len(matches) == 1, "Official Cargo archive has ambiguous binary contents")
        stream = archive_file.extractfile(matches[0])
        require(stream is not None, "Official Cargo executable missing")
        trusted_binary = digest(stream.read())
    require(digest(Path(binary).read_bytes()) == trusted_binary,
            "Installed Cargo bytes differ from checksum-verified official component")
    return {"archive_sha256": metadata["sha256"], "binary_sha256": trusted_binary}


def native_host():
    # Do not query rustup show/version before installation: those commands
    # can auto-sync a selected compiler and alter a preserved profile.
    machine = platform.machine().lower()
    architecture = {"x86_64": "x86_64", "amd64": "x86_64",
                    "aarch64": "aarch64", "arm64": "aarch64"}.get(machine)
    require(architecture is not None and sys.platform in ("linux", "darwin"),
            "Unsupported native qualification host; an owning profile decision is required")
    suffix = "unknown-linux-gnu" if sys.platform == "linux" else "apple-darwin"
    return architecture + "-" + suffix


def prepare(args, helper):
    manifest, cohort = verify(args, helper)
    require(os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("GITHUB_ENV"),
            "GitHub qualification requires job-scoped GITHUB_ENV")
    require(not os.environ.get("RUSTUP_DIST_SERVER") and not os.environ.get("RUSTUP_UPDATE_ROOT"),
            "Qualification must use the genuine official Rust distribution")
    rustup = shutil.which("rustup")
    require(rustup, "Genuine rustup executable unavailable")
    manager_sha256 = digest(Path(rustup).read_bytes())
    host = native_host()
    requirements = {
        "rustc": archive(manifest, "rustc", host),
        "cargo": archive(manifest, "cargo", host),
        **{name: archive(manifest, name, "*" if name == "rust-src" else host)
           for name in COMPONENTS},
        **{"rust-std/" + target: archive(manifest, "rust-std", target)
           for target in {host, *CROSS_TARGETS}},
    }
    parent = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
    profile = Path(tempfile.mkdtemp(prefix="rlvgl-nightly-", dir=parent)).resolve()
    cargo_home, rustup_home = profile / "cargo-home", profile / "rustup-home"
    cargo_home.mkdir()
    rustup_home.mkdir()
    toolchain = "nightly-" + cohort["date"]
    env = os.environ.copy()
    for key in ("RUSTC", "RUSTDOC", "RUSTUP_TOOLCHAIN", "TRYBUILD",
                "RUSTUP_DIST_SERVER", "RUSTUP_UPDATE_ROOT"):
        env.pop(key, None)
    env.update(CARGO_HOME=str(cargo_home), RUSTUP_HOME=str(rustup_home),
               RUSTUP_TOOLCHAIN=toolchain)
    env["RUSTUP_NO_UPDATE_CHECK"] = "1"
    # Exact date prevents rustup's floating-nightly older-component fallback.
    command = [rustup, "toolchain", "install", toolchain, "--profile", "minimal",
               "--component", "rustfmt,clippy,rust-src,llvm-tools-preview",
               "--target", ",".join(sorted({host, *CROSS_TARGETS})), "--no-self-update"]
    try:
        transcript = run(command, env)
        (profile / "install.log").write_text(transcript)
        print(transcript, end="")
        rustc = run([rustup, "which", "--toolchain", toolchain, "rustc"], env).strip()
        cargo = run([rustup, "which", "--toolchain", toolchain, "cargo"], env).strip()
        rustdoc = run([rustup, "which", "--toolchain", toolchain, "rustdoc"], env).strip()
        require(all(Path(p).resolve().is_relative_to(rustup_home) for p in (rustc, cargo, rustdoc)),
                "Installed compiler executables escaped the new private profile")
        rustc_verbose = run([rustc, "--version", "--verbose"], env)
        cargo_verbose = run([cargo, "--version", "--verbose"], env)
        print(rustc_verbose + cargo_verbose, end="")
        verify_actual(cohort, rustc_verbose, cargo_verbose, host)
        components = run([rustup, "component", "list", "--toolchain", toolchain, "--installed"], env)
        targets = run([rustup, "target", "list", "--toolchain", toolchain, "--installed"], env)
        verify_inventory(components, targets, host)
        cargo_custody = verify_cargo_archive(requirements["cargo"], cargo, profile)
        receipt = dict(cohort=cohort, profile=str(profile), toolchain=toolchain,
                       host=host, rustc=rustc_verbose, cargo=cargo_verbose,
                       components=components, targets=targets, archives=requirements,
                       cargo_custody=cargo_custody, source_sha=args.source_sha,
                       rustup_manager_sha256=manager_sha256)
        write_json(profile / "qualification.json", receipt)
        require(os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("GITHUB_ENV"),
                "GitHub qualification requires job-scoped GITHUB_ENV")
        exports = dict(RUSTUP_TOOLCHAIN=toolchain, CARGO_HOME=str(cargo_home),
                       RUSTUP_HOME=str(rustup_home), RUSTC=rustc, RUSTDOC=rustdoc,
                       RLVGL_NIGHTLY_COHORT_SHA256=cohort["manifest_sha256"],
                       RLVGL_QUALIFICATION_RECEIPT=str(profile / "qualification.json"))
        if os.environ.get("GITHUB_ENV"):
            with open(os.environ["GITHUB_ENV"], "a") as stream:
                for key, value in exports.items():
                    stream.write("%s=%s\n" % (key, value))
        if os.environ.get("GITHUB_PATH"):
            with open(os.environ["GITHUB_PATH"], "a") as stream:
                stream.write(str(Path(rustc).parent) + "\n")
        print(json.dumps(exports, sort_keys=True))
    except Exception as exc:
        write_json(profile / "failure.json", dict(error=str(exc), cohort=cohort,
                                                  command=command, profile=str(profile)))
        raise


def freshness(args, helper):
    _, cohort = verify(args, helper)
    out = Path(args.output_dir)
    out.mkdir(parents=True, exist_ok=True)
    require(not (out / "freshness.json").exists() and
            not (out / "channel-rust-nightly.toml").exists(),
            "Freshness evidence already exists")
    latest = latest_manifest()
    _, advertised = validate_manifest(latest)
    (out / "channel-rust-nightly.toml").write_bytes(latest)
    receipt = dict(qualified_manifest_sha256=cohort["manifest_sha256"],
                   observed_manifest_sha256=digest(latest), qualified_date=cohort["date"],
                   observed_date=advertised["date"], source_sha=args.source_sha,
                   github=github_binding(), observed_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   current=(digest(latest) == cohort["manifest_sha256"]))
    write_json(out / "freshness.json", receipt)
    require(receipt["current"], "Most recent nightly advanced; compiler-dependent requalification required before authentication")
    print(json.dumps(receipt, sort_keys=True))


def main(argv, helper):
    parser = argparse.ArgumentParser(description="Immutable newest-nightly qualification cohort")
    subparsers = parser.add_subparsers(dest="mode", required=True)
    for mode in ("resolve", "prepare", "freshness"):
        sub = subparsers.add_parser(mode)
        sub.add_argument("--source-sha", required=True)
        sub.add_argument("--workflow-sha", required=True)
        if mode in ("resolve", "freshness"):
            sub.add_argument("--output-dir", required=True)
        if mode != "resolve":
            sub.add_argument("--cohort", required=True)
            sub.add_argument("--manifest", required=True)
            sub.add_argument("--expected-manifest-sha", required=True)
    args = parser.parse_args(argv)
    try:
        {"resolve": produce, "prepare": prepare, "freshness": freshness}[args.mode](args, helper)
    except (CohortError, OSError, ValueError, KeyError, TypeError, tarfile.TarError) as exc:
        print("Nightly cohort refused: %s" % exc, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[2:], sys.argv[1]))
COHORT_PY
