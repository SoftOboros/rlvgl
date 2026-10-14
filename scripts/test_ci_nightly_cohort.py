"""Newest-nightly cohort refusal controls. No toolchain or registry installation.

The helper's embedded runtime is loaded without main. Genuine temporary Git
proves source/helper provenance; download/rustup fixtures are diagnostic controls,
never claims about actual latest compiler or package qualification.
"""
import argparse
import contextlib
import copy
import hashlib
import io
import json
import os
import re
from pathlib import Path
import subprocess
import tarfile
import tempfile
import types
import unittest
from unittest import mock

import yaml

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "scripts/setup-ci-env.sh"
HOST = "x86_64-unknown-linux-gnu"
RUST_COMMIT = "a" * 40
CARGO_COMMIT = "b" * 40


class UniqueKeyLoader(yaml.SafeLoader):
    def construct_mapping(self, node, deep=False):
        values = {}
        for key_node, value_node in node.value:
            key = self.construct_object(key_node, deep=deep)
            if key in values:
                raise yaml.constructor.ConstructorError(
                    "workflow mapping", node.start_mark,
                    "duplicate key: %s" % key, key_node.start_mark)
            values[key] = self.construct_object(value_node, deep=deep)
        return values


def load_workflow(body):
    return yaml.load(body, Loader=UniqueKeyLoader)


def load_helper():
    text = HELPER.read_text()
    body = text.split("<<'COHORT_PY'\n", 1)[1].rsplit("\nCOHORT_PY", 1)[0]
    module = types.ModuleType("rlvgl_nightly_test_runtime")
    exec(compile(body, str(HELPER), "exec"), module.__dict__)
    return module


def component_archive(binary=b"trusted Cargo executable"):
    stream = io.BytesIO()
    with tarfile.open(fileobj=stream, mode="w:xz") as archive:
        member = tarfile.TarInfo("cargo-nightly-host/cargo/bin/cargo")
        member.size = len(binary)
        archive.addfile(member, io.BytesIO(binary))
    return stream.getvalue()


def manifest(module, date="2026-10-13", cargo_archive=None):
    cargo_hash = hashlib.sha256(cargo_archive or b"archive").hexdigest()
    lines = ['manifest-version = "2"', 'date = "%s"' % date,
             '[pkg.rustc]', 'version = "1.101.0-nightly (aaaaaaaaa 2026-10-12)"',
             'git_commit_hash = "%s"' % RUST_COMMIT,
             '[pkg.cargo]', 'version = "0.102.0-nightly (bbbbbbbbb 2026-10-07)"',
             'git_commit_hash = "%s"' % RUST_COMMIT]
    entries = [("rustc", HOST), ("cargo", HOST)]
    entries += [(name, "*" if name == "rust-src" else HOST) for name in module.COMPONENTS]
    entries += [("rust-std", target) for target in (HOST, *module.CROSS_TARGETS)]
    for package, target in entries:
        table_target = '"*"' if target == "*" else target
        checksum = cargo_hash if package == "cargo" else "c" * 64
        lines += ['[pkg.%s.target.%s]' % (package, table_target), 'available = true',
                  'xz_url = "https://static.rust-lang.org/dist/%s/%s-nightly-%s.tar.xz"' %
                  (date, package, target), 'xz_hash = "%s"' % checksum]
    # Quoted dotted tables and unrelated Rust extension arrays are real shapes.
    lines += ['[pkg.llvm-tools-preview.target."thumbv8m.main-none-eabi"]',
              'available = false', '[[pkg.rust.target.host.extensions]]',
              'pkg = "rust-src"', 'target = "*"']
    return ("\n".join(lines) + "\n").encode()


class NightlyCohortTests(unittest.TestCase):
    def setUp(self):
        self.runtime = load_helper()
        self.temp = tempfile.TemporaryDirectory(prefix="rlvgl-cohort-controls-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name).resolve()
        self.repo = self.directory / "source"
        (self.repo / "scripts").mkdir(parents=True)
        self.helper = self.repo / "scripts/setup-ci-env.sh"
        self.helper.write_bytes(HELPER.read_bytes())
        for command in (["git", "init", "-q"],
                        ["git", "add", "scripts/setup-ci-env.sh"],
                        ["git", "-c", "user.name=Cohort Control",
                         "-c", "user.email=cohort-control@example.invalid",
                         "commit", "-qm", "genuine helper fixture"]):
            subprocess.run(command, cwd=self.repo, check=True,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.commit = subprocess.check_output(["git", "rev-parse", "HEAD"],
                                              cwd=self.repo, text=True).strip()
        self.previous_cwd = Path.cwd()
        os.chdir(self.repo)
        self.addCleanup(os.chdir, self.previous_cwd)
        self.environment = mock.patch.dict(os.environ, {
            "PATH": os.environ.get("PATH", ""),
            "HOME": str(self.directory),
            "RUNNER_TEMP": str(self.directory),
            "GITHUB_ACTIONS": "true", "GITHUB_RUN_ID": "777",
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_OUTPUT": str(self.directory / "outputs"),
            "GITHUB_ENV": str(self.directory / "environment"),
            "GITHUB_PATH": str(self.directory / "paths"),
        }, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.archive = component_archive()
        self.body = manifest(self.runtime, cargo_archive=self.archive)
        self.args = argparse.Namespace(source_sha=self.commit, workflow_sha=self.commit,
                                       output_dir=str(self.directory / "cohort"))
        with mock.patch.object(self.runtime, "latest_manifest", return_value=self.body):
            with contextlib.redirect_stdout(io.StringIO()):
                self.runtime.produce(self.args, str(self.helper))
        self.cohort_dir = Path(self.args.output_dir)
        self.check = argparse.Namespace(source_sha=self.commit, workflow_sha=self.commit,
            cohort=str(self.cohort_dir / "cohort.json"),
            manifest=str(self.cohort_dir / "channel-rust-nightly.toml"),
            expected_manifest_sha=self.runtime.digest(self.body),
            output_dir=str(self.directory / "freshness"))
        self.cohort = json.loads(Path(self.check.cohort).read_text())

    def test_resolve_emits_exact_manifest_and_github_outputs(self):
        self.assertEqual(Path(self.check.manifest).read_bytes(), self.body)
        self.assertEqual(self.cohort["github"], {"run_id": "777", "run_attempt": "1"})
        outputs = (self.directory / "outputs").read_text()
        for key in ("date", "manifest_sha256", "helper_sha256", "source_sha", "workflow_sha"):
            self.assertIn(key + "=" + self.cohort[key] + "\n", outputs)
        self.assertEqual(self.runtime.verify(self.check, self.helper)[1], self.cohort)

    def test_resolve_never_overwrites_cohort(self):
        with mock.patch.object(self.runtime, "latest_manifest", return_value=self.body):
            with self.assertRaisesRegex(self.runtime.CohortError, "already exists"):
                self.runtime.produce(self.args, self.helper)
        self.assertEqual(Path(self.check.manifest).read_bytes(), self.body)

    def test_manifest_required_field_refusals(self):
        mutations = (
            self.body.replace(b'manifest-version = "2"', b'manifest-version = "1"'),
            self.body.replace(b'date = "2026-10-13"', b'date = "invalid"'),
            self.body.replace(b'date = "2026-10-13"', b'date = "2026-02-30"'),
            self.body + b'date = "duplicate"\n',  # duplicate inside current table
            self.body.replace(b'1.101.0-nightly', b'1.101.0-stable'),
            self.body.replace(RUST_COMMIT.encode(), b"d" * 40, 1),
            self.body.replace(b'2026-10-13"', b'2026-10-01"', 1),
            self.body.replace(b'available = true', b'available = "true"', 1),
        )
        # Put duplicate date in the actual top-level field scope.
        mutations = mutations[:3] + (b'date = "2026-10-13"\n' + self.body,) + mutations[4:]
        for body in mutations:
            with self.subTest(body=body[:80]), self.assertRaises(self.runtime.CohortError):
                self.runtime.validate_manifest(body)

    def test_component_target_and_archive_refusals(self):
        parsed, _ = self.runtime.validate_manifest(self.body)
        entries = [("cargo", HOST), ("rustc", HOST)]
        entries += [(n, "*" if n == "rust-src" else HOST) for n in self.runtime.COMPONENTS]
        entries += [("rust-std", t) for t in (HOST, *self.runtime.CROSS_TARGETS)]
        for package, target in entries:
            with self.subTest(package=package, target=target):
                changed = copy.deepcopy(parsed)
                changed["pkg"][package]["target"][target]["available"] = False
                with self.assertRaisesRegex(self.runtime.CohortError, "unavailable"):
                    self.runtime.archive(changed, package, target)
        for url in ("https://evil.invalid/dist/2026-10-13/cargo.tar.xz",
                    "http://static.rust-lang.org/dist/2026-10-13/cargo.tar.xz",
                    "https://static.rust-lang.org/dist/2026-10-12/cargo.tar.xz"):
            changed = copy.deepcopy(parsed)
            changed["pkg"]["cargo"]["target"][HOST]["xz_url"] = url
            with self.assertRaises(self.runtime.CohortError):
                self.runtime.archive(changed, "cargo", HOST)

    def test_official_manifest_checksum_mismatch_stops(self):
        def fetch(url, limit=0):
            return self.body if url == self.runtime.LATEST else b"0" * 64
        with mock.patch.object(self.runtime, "fetch_bytes", side_effect=fetch):
            with self.assertRaisesRegex(self.runtime.CohortError, "disagree"):
                self.runtime.latest_manifest()

    def test_raw_body_source_helper_and_run_tamper_refused(self):
        for key, value in (("source_sha", "d" * 40), ("workflow_sha", "d" * 40),
                           ("helper_sha256", "d" * 64),
                           ("github", {"run_id": "777", "run_attempt": "2"}),
                           ("cargo", {"commit": "wrong"})):
            changed = dict(self.cohort, **{key: value})
            Path(self.check.cohort).write_text(json.dumps(changed))
            with self.subTest(key=key), self.assertRaises(self.runtime.CohortError):
                self.runtime.verify(self.check, self.helper)
        Path(self.check.cohort).write_text(json.dumps(self.cohort))
        Path(self.check.manifest).write_bytes(self.body + b"\n")
        with self.assertRaisesRegex(self.runtime.CohortError, "checksum mismatch"):
            self.runtime.verify(self.check, self.helper)

    def test_genuine_transport_helper_and_frozen_source_binding(self):
        transported = self.directory / "transported-setup.sh"
        transported.write_bytes(self.helper.read_bytes())
        self.runtime.verify(self.check, transported)
        # Distinct genuine source and workflow commits retain separate identities.
        (self.repo / "README.md").write_text("frozen source fixture\\n")
        subprocess.run(["git", "add", "README.md"], check=True, stdout=subprocess.PIPE)
        subprocess.run(["git", "-c", "user.name=Cohort Control",
                        "-c", "user.email=cohort-control@example.invalid",
                        "commit", "-qm", "distinct frozen source"], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        source = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        distinct = copy.copy(self.check)
        distinct.source_sha = source
        bound = dict(self.cohort, source_sha=source)
        Path(distinct.cohort).write_text(json.dumps(bound))
        self.assertNotEqual(source, self.commit)
        self.runtime.verify(distinct, transported)
        # Refuse wrong CWD even if the transported cohort was also falsified.
        wrong = copy.copy(distinct)
        wrong.source_sha = "d" * 40
        Path(wrong.cohort).write_text(json.dumps(dict(bound, source_sha=wrong.source_sha)))
        with self.assertRaisesRegex(self.runtime.CohortError, "Current checkout"):
            self.runtime.verify(wrong, transported)
        Path(distinct.cohort).write_text(json.dumps(bound))
        transported.write_bytes(transported.read_bytes() + b"\n")
        with self.assertRaisesRegex(self.runtime.CohortError, "Git blob"):
            self.runtime.verify(distinct, transported)
        wrong = copy.copy(self.check)
        wrong.source_sha = "d" * 40
        with self.assertRaises(self.runtime.CohortError):
            self.runtime.verify(wrong, self.helper)

    def versions(self):
        rust = ("rustc 1.101.0-nightly (aaaaaaaaa 2026-10-12)\n"
                "release: 1.101.0-nightly\ncommit-hash: %s\ncommit-date: 2026-10-12\nhost: %s\n") % (RUST_COMMIT, HOST)
        cargo = ("cargo 1.101.0-nightly (bbbbbbbbb 2026-10-07)\n"
                 "release: 1.101.0-nightly\ncommit-hash: %s\ncommit-date: 2026-10-07\nhost: %s\n") % (CARGO_COMMIT, HOST)
        return rust, cargo

    def test_cargo_own_identity_not_distribution_or_package_version(self):
        rust, cargo = self.versions()
        self.runtime.verify_actual(self.cohort, rust, cargo, HOST)
        for wrong in (cargo.replace(CARGO_COMMIT, RUST_COMMIT),
                      cargo.replace("2026-10-07", "2026-10-08"),
                      cargo.replace("-nightly", ""),
                      cargo.replace(HOST, "wrong-host")):
            with self.assertRaises(self.runtime.CohortError):
                self.runtime.verify_actual(self.cohort, rust, wrong, HOST)
        with self.assertRaises(self.runtime.CohortError):
            self.runtime.verify_actual(self.cohort, rust.replace(RUST_COMMIT, "d" * 40), cargo, HOST)

    def test_genuine_component_alias_and_all_seven_targets_required(self):
        components = ["rustfmt-" + HOST, "clippy-" + HOST, "rust-src", "llvm-tools-" + HOST]
        targets = [HOST, *self.runtime.CROSS_TARGETS]
        self.runtime.verify_inventory("\n".join(components), "\n".join(targets), HOST)
        for member in components:
            with self.assertRaises(self.runtime.CohortError):
                self.runtime.verify_inventory("\n".join(x for x in components if x != member),
                                              "\n".join(targets), HOST)
        for target in targets:
            with self.assertRaises(self.runtime.CohortError):
                self.runtime.verify_inventory("\n".join(components),
                                              "\n".join(t for t in targets if t != target), HOST)

    def test_official_cargo_archive_binds_installed_binary_bytes(self):
        parsed, _ = self.runtime.validate_manifest(self.body)
        metadata = self.runtime.archive(parsed, "cargo", HOST)
        binary = self.directory / "cargo"
        binary.write_bytes(b"trusted Cargo executable")
        with mock.patch.object(self.runtime, "fetch_bytes", return_value=self.archive):
            receipt = self.runtime.verify_cargo_archive(metadata, binary, self.directory)
            self.assertEqual(receipt["binary_sha256"], self.runtime.digest(binary.read_bytes()))
            binary.write_bytes(b"untrusted executable")
            with self.assertRaisesRegex(self.runtime.CohortError, "Installed Cargo bytes"):
                self.runtime.verify_cargo_archive(metadata, binary, self.directory)

    def test_freshness_advance_blocks_without_selecting_toolchain_or_auth(self):
        before = dict(os.environ)
        for current in (True, False):
            args = copy.copy(self.check)
            args.output_dir += "-current" if current else "-advanced"
            body = self.body if current else manifest(self.runtime, "2026-10-14", self.archive)
            with mock.patch.object(self.runtime, "latest_manifest", return_value=body):
                with contextlib.redirect_stdout(io.StringIO()):
                    if current:
                        self.runtime.freshness(args, self.helper)
                    else:
                        with self.assertRaisesRegex(self.runtime.CohortError, "requalification"):
                            self.runtime.freshness(args, self.helper)
            receipt = json.loads((Path(args.output_dir) / "freshness.json").read_text())
            self.assertEqual(receipt["current"], current)
            self.assertEqual(dict(os.environ), before)
        self.assertFalse((self.directory / "environment").exists())
        self.assertFalse((self.directory / "paths").exists())

    def test_prepare_exact_date_private_profile_and_verified_exports(self):
        calls = []
        original_run = self.runtime.run
        rust, cargo = self.versions()
        manager = self.directory / "rustup-manager"
        manager.write_bytes(b"fixture manager identity, never executed")

        def commands(command, env=None):
            calls.append((list(command), dict(env or {})))
            if command[0] == "git":
                return original_run(command, env)
            home = Path(env["RUSTUP_HOME"])
            binaries = home / "toolchains" / ("nightly-2026-10-13-" + HOST) / "bin"
            if command[1:3] == ["toolchain", "install"]:
                binaries.mkdir(parents=True)
                for name in ("rustc", "cargo", "rustdoc"):
                    (binaries / name).write_bytes(b"trusted Cargo executable")
                return "genuine-shaped rustup recorder only\n"
            if command[1] == "which":
                return str(binaries / command[-1]) + "\n"
            if command[-2:] == ["--version", "--verbose"]:
                return cargo if command[0].endswith("/cargo") else rust
            if command[1:3] == ["component", "list"]:
                return "\n".join(["rustfmt-" + HOST, "clippy-" + HOST, "rust-src", "llvm-tools-" + HOST])
            if command[1:3] == ["target", "list"]:
                return "\n".join([HOST, *self.runtime.CROSS_TARGETS])
            self.fail("Unexpected qualification command: %r" % command)

        with mock.patch.object(self.runtime, "run", side_effect=commands), \
             mock.patch.object(self.runtime.shutil, "which", return_value=str(manager)), \
             mock.patch.object(self.runtime, "native_host", return_value=HOST), \
             mock.patch.object(self.runtime, "fetch_bytes", return_value=self.archive), \
             contextlib.redirect_stdout(io.StringIO()):
            self.runtime.prepare(self.check, self.helper)
        installs = [c for c, env in calls if c[1:3] == ["toolchain", "install"]]
        self.assertEqual(len(installs), 1)
        self.assertEqual(installs[0][3], "nightly-2026-10-13")
        self.assertIn("--no-self-update", installs[0])
        self.assertFalse(any("default" in c or "update" in c or "show" in c
                             or c[1:] == ["--version"] for c, env in calls))
        manager_calls = [c for c, env in calls if c[0] == str(manager)]
        self.assertEqual(manager_calls[0][1:3], ["toolchain", "install"])
        exports = dict(line.split("=", 1) for line in (self.directory / "environment").read_text().splitlines())
        for name in ("CARGO_HOME", "RUSTUP_HOME", "RUSTC", "RUSTDOC"):
            self.assertTrue(Path(exports[name]).is_relative_to(self.directory))
        self.assertEqual(exports["RUSTUP_TOOLCHAIN"], "nightly-2026-10-13")
        self.assertNotIn("RUSTUP_TOOLCHAIN", os.environ)
        self.assertTrue(Path(exports["RLVGL_QUALIFICATION_RECEIPT"]).is_file())

    def test_missing_interface_and_untrusted_distribution_refused(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            self.runtime.main([], self.helper)
        os.environ["RUSTUP_DIST_SERVER"] = "https://example.invalid"
        with self.assertRaisesRegex(self.runtime.CohortError, "genuine official"):
            self.runtime.prepare(self.check, self.helper)


class NightlyWorkflowTests(unittest.TestCase):
    def test_duplicate_checkout_keys_are_refused(self):
        with self.assertRaisesRegex(yaml.constructor.ConstructorError, "duplicate key: with"):
            load_workflow("steps:\n- uses: actions/checkout@v4\n  with:\n    ref: exact\n  with:\n    submodules: recursive\n")

    def test_all_callers_share_one_explicit_cohort_and_guard_stable_auth(self):
        invoke = re.compile(r"setup-ci-env\.sh[\"']?\s+(resolve|prepare|freshness)\b")
        for name in ("ci", "crates-ci", "creator-e2e", "publish", "publish-continue", "gate-r"):
            with self.subTest(workflow=name):
                document = load_workflow((ROOT / ".github/workflows" / (name + ".yml")).read_text())
                calls = []
                for job_id, job in document["jobs"].items():
                    for index, step in enumerate(job.get("steps", [])):
                        text = step.get("run", "")
                        for match in invoke.finditer(text):
                            calls.append((match[1], job_id, index, text))
                resolvers = [c for c in calls if c[0] == "resolve"]
                prepares = [c for c in calls if c[0] == "prepare"]
                self.assertEqual(len(resolvers), 1, "Resolve latest once per workflow")
                self.assertTrue(prepares, "Actual qualification consumers must prepare explicit cohort")
                producer = resolvers[0]
                for mode, job_id, index, text in calls:
                    for option in ("--source-sha", "--workflow-sha"):
                        self.assertIn(option, text)
                    if mode != "resolve":
                        for option in ("--cohort", "--manifest", "--expected-manifest-sha"):
                            self.assertIn(option, text)
                    if mode == "prepare":
                        preceding_checkouts = [step for step in document["jobs"][job_id]["steps"][:index]
                                               if step.get("uses", "").startswith("actions/checkout@")]
                        self.assertTrue(preceding_checkouts,
                                        "Source initialization must precede compiler preparation")
                        self.assertEqual(preceding_checkouts[-1]["with"].get("submodules"),
                                         "recursive", "Preserve the original initialization prerequisite")
                        if job_id == producer[1]:
                            self.assertGreater(index, producer[2])
                        else:
                            needs = document["jobs"][job_id].get("needs", [])
                            needs = [needs] if isinstance(needs, str) else needs
                            self.assertIn(producer[1], needs)
                self.assertNotIn("RUSTUP_TOOLCHAIN", document.get("env", {}))
                if name in ("publish", "publish-continue"):
                    triggers = document.get("on", document.get(True))
                    self.assertNotIn("schedule", triggers)
                    publisher = document["jobs"]["publish"]
                    self.assertEqual(publisher["environment"], "release")
                    self.assertEqual(publisher["permissions"]["id-token"], "write")
                    self.assertNotIn("RUSTUP_TOOLCHAIN", publisher.get("env", {}))
                    steps = publisher["steps"]
                    auth = next(i for i, step in enumerate(steps)
                                if step.get("uses", "").startswith("rust-lang/crates-io-auth-action@"))
                    checks = [index for mode, job, index, text in calls
                              if mode == "freshness" and job == "publish"]
                    self.assertEqual(len(checks), 1)
                    self.assertLess(checks[0], auth)
                    stable = next(i for i, step in enumerate(steps)
                                  if step.get("with", {}).get("toolchain") == "stable")
                    self.assertLess(checks[0], stable)
                    self.assertFalse(any(mode == "prepare" and job == "publish"
                                         for mode, job, index, text in calls))
                    self.assertTrue(any(step.get("with", {}).get("toolchain") == "stable"
                                        for step in steps))

    def test_artifact_id_downloads_are_flat_and_bound_to_one_producer(self):
        # The action's ID branch creates a named subdirectory unless merging
        # is explicit, even for one ID. Prepare consumes the two flat files.
        for name in ("ci", "publish", "publish-continue"):
            document = load_workflow((ROOT / ".github/workflows" / (name + ".yml")).read_text())
            producer = {"ci": "nightly-cohort", "publish": "gate-p",
                        "publish-continue": "resolve-release"}[name]
            downloads = [step for job in document["jobs"].values()
                         for step in job.get("steps", [])
                         if step.get("uses", "").startswith("actions/download-artifact@")]
            self.assertTrue(downloads)
            for step in downloads:
                with self.subTest(workflow=name, step=step.get("name")):
                    inputs = step["with"]
                    self.assertIs(inputs.get("merge-multiple"), True)
                    self.assertEqual(inputs["artifact-ids"],
                                     "${{ needs." + producer + ".outputs.artifact_id }}")
                    self.assertNotIn("name", inputs)
                    self.assertNotIn("pattern", inputs)

    def test_release_full_source_guards_precede_stage_on_same_cohort(self):
        owned = (ROOT / "CLAUDE.md").read_text().split("## Pre-Publish Validation", 1)[1]
        block = owned.split("```bash\n", 1)[1].split("```", 1)[0]
        missing = []
        phase = ""
        for line in block.splitlines():
            match = re.match(r"# Phase ([0-9.]+[a-z]?):", line)
            if match:
                phase = match[1]
            if phase in ("2.5", "4.6", "4.7", "4.7b", "4.7c", "4.7d", "4.8", "4.9", "6.5"):
                if line.startswith(("RUSTFLAGS=", "RLVGL_LINT_STRICT=")):
                    missing.append(line)
        self.assertGreater(len(missing), 30)
        for name in ("publish", "publish-continue"):
            document = load_workflow((ROOT / ".github/workflows" / (name + ".yml")).read_text())
            steps = document["jobs"]["gate-p"]["steps"]
            prepares = [i for i, step in enumerate(steps)
                        if re.search(r"setup-ci-env\.sh[\"']?\s+prepare\b", step.get("run", ""))]
            stage = next(i for i, step in enumerate(steps)
                         if step.get("run") == "bash scripts/crates_ci_stage.sh")
            guards = [(i, step["run"]) for i, step in enumerate(steps)
                      if "bash scripts/pre-commit.sh" in step.get("run", "")]
            self.assertEqual(len(prepares), 1)
            self.assertEqual(len(guards), 1)
            index, body = guards[0]
            self.assertLess(prepares[0], index)
            self.assertLess(index, stage)
            commands = {line.strip() for line in body.splitlines()
                        if line.strip() and not line.lstrip().startswith("#")}
            self.assertTrue(set(missing) <= commands)
            self.assertIn("set -euo pipefail", commands)
            self.assertIn("cargo fmt --all -- --check", commands)
            self.assertIn('RUSTDOCFLAGS="--cfg docsrs --cfg nightly" cargo doc --all-features --no-deps --target x86_64-unknown-linux-gnu', commands)
            self.assertFalse(any("|| true" in command for command in commands))

    def test_daily_main_compiles_and_registry_schedule_stays_distinct(self):
        document = load_workflow((ROOT / ".github/workflows/ci.yml").read_text())
        triggers = document.get("on", document.get(True))
        self.assertTrue(triggers.get("schedule"))
        self.assertIn("workflow_dispatch", triggers)
        self.assertIn("main", triggers["pull_request"]["branches"])
        build = document["jobs"]["build"]
        self.assertNotIn("if", build, "Daily main must actually reach required build")
        commands = "\n".join(step.get("run", "") for step in build["steps"])
        for command in ("cargo build --workspace", "cargo fmt --all -- --check",
                        "cargo clippy --workspace", "cargo test --workspace",
                        "cargo doc --all-features", "spec-index-check"):
            self.assertIn(command, commands)
        registry = load_workflow((ROOT / ".github/workflows/gate-r.yml").read_text())
        triggers = registry.get("on", registry.get(True))
        self.assertEqual(triggers["schedule"], [{"cron": "30 16 * * *"}])
        self.assertIn("workflow_run", triggers)
        self.assertIn("workflow_dispatch", triggers)


if __name__ == "__main__":
    unittest.main()
