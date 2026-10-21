"""Run-scoped Gate P handoff checks with small synthetic Cargo archives."""

import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("gate_p_stage_artifact.py")
spec = importlib.util.spec_from_file_location("gate_p_stage_artifact", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
PUBLISHER = Path(__file__).with_name("publish_changed.sh")
SOURCE = "a" * 40
WORKFLOW = "b" * 40
MANIFEST = "c" * 64


class Inputs:
    def __init__(self, **values):
        self.__dict__.update(values)


class GatePArtifact(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.stage = self.root / "producer-stage"
        (self.stage / "staged").mkdir(parents=True)
        self.package = self.root / "original-packages"
        self.package.mkdir()
        self.artifact = self.root / "artifact"
        self.consumer = self.root / "consumer-stage"
        self.order = __import__("subprocess").check_output(
            [str(PUBLISHER), "--print-order"], text=True).splitlines()
        self.assertEqual(len(self.order), 31)
        (self.stage / "ordered-crates.txt").write_text("\n".join(self.order) + "\n")
        (self.stage / "versions.tsv").write_text(
            "".join(f"{name}\t0.0.1\n" for name in self.order))
        for name in self.order:
            directory = f"{name}-0.0.1"
            (self.stage / "staged" / directory).mkdir()
            with tarfile.open(self.package / f"{directory}.crate", "w:gz") as stream:
                body = f"[package]\nname=\"{name}\"\nversion=\"0.0.1\"\n".encode()
                member = tarfile.TarInfo(f"{directory}/Cargo.toml")
                member.size = len(body)
                stream.addfile(member, io.BytesIO(body))
        module.pack(Inputs(stage=self.stage, packages=self.package, output=self.artifact,
                           source_sha=SOURCE, workflow_sha=WORKFLOW, manifest_sha=MANIFEST))

    def restore(self, **overrides):
        values = dict(artifact=self.artifact, stage=self.consumer, publisher=PUBLISHER,
                      source_sha=SOURCE, workflow_sha=WORKFLOW, manifest_sha=MANIFEST)
        values.update(overrides)
        module.restore(Inputs(**values))

    def test_exact_31_archives_restore_as_extracted_patch_set(self):
        self.restore()
        self.assertEqual(len(list((self.consumer / "staged").iterdir())), 31)
        patch = (self.consumer / "patch-config.toml").read_text()
        self.assertEqual(patch.count(" = { path = "), 31)
        self.assertIn(f"{self.consumer.resolve()}/staged/{self.order[0]}-0.0.1", patch)

    def test_binding_or_archive_mutation_refused_before_extract(self):
        with self.assertRaisesRegex(ValueError, "binding mismatch"):
            self.restore(source_sha="d" * 40)
        self.assertFalse(self.consumer.exists())
        first = self.artifact / f"{self.order[0]}-0.0.1.crate"
        first.write_bytes(first.read_bytes() + b"changed")
        with self.assertRaisesRegex(ValueError, "archive mismatch"):
            self.restore()
        self.assertFalse(self.consumer.exists())

    def test_extra_artifact_member_refused(self):
        (self.artifact / "unbound.crate").write_bytes(b"unbound")
        with self.assertRaisesRegex(ValueError, "missing or extra"):
            self.restore()

    def test_no_overwrite_of_existing_consumer_stage(self):
        self.consumer.mkdir()
        (self.consumer / "evidence.txt").write_text("preserve")
        with self.assertRaisesRegex(ValueError, "must be empty"):
            self.restore()
        self.assertEqual((self.consumer / "evidence.txt").read_text(), "preserve")


if __name__ == "__main__":
    unittest.main()
