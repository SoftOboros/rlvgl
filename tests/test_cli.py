"""Integration tests for AFDB CLI commands."""

import json
import subprocess
import sys


def test_cli_import_and_build(tmp_path):
    sample_mcu = "tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml"
    mcu_out = tmp_path / "mcu.json"
    subprocess.run([sys.executable, "-m", "tools.afdb.cli", "import-mcu", "--in", sample_mcu, "--out", str(mcu_out)], check=True)
    assert mcu_out.exists()
    cat_out = tmp_path / "catalog.json"
    subprocess.run([sys.executable, "-m", "tools.afdb.cli", "build-catalog", "--mcu", str(mcu_out), "--out", str(cat_out)], check=True)
    data = json.loads(cat_out.read_text())
    assert "PA0" in data["pins"]
    ir_out = tmp_path / "catalog.bin.zst"
    subprocess.run([sys.executable, "-m", "tools.afdb.cli", "encode-ir", "--catalog", str(cat_out), "--out", str(ir_out)], check=True)
    assert ir_out.exists()
    assert ir_out.stat().st_size < cat_out.stat().st_size
