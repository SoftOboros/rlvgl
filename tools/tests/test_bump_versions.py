from pathlib import Path
from tools.bump_vendor_versions import bump_manifest


def test_bump_manifest_updates_version(tmp_path):
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname="demo"\nversion="0.1.0"\n')
    new_version = bump_manifest(manifest, dry_run=False)
    assert new_version == "0.1.1"
    assert 'version="0.1.1"' in manifest.read_text()


def test_bump_manifest_dry_run(tmp_path):
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname="demo"\nversion="0.1.0"\n')
    new_version = bump_manifest(manifest, dry_run=True)
    assert new_version == "0.1.1"
    assert 'version="0.1.0"' in manifest.read_text()
