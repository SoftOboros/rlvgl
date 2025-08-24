"""Tests for vendor database generation and environment variable wiring."""
import os
import subprocess
import pathlib
import json

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]

def test_build_vendor_exports_env_and_files(tmp_path):
    env = os.environ.copy()
    env["VENDOR_DIR"] = str(REPO_ROOT / "tests/data/chipdb")
    crate_dir = tmp_path / "crate"
    out_dir = tmp_path / "out"
    crate_dir.mkdir()
    env["CRATE_DIR"] = str(crate_dir)
    env["OUT_DIR"] = str(out_dir)
    cmd = "source tools/build_vendor.sh && printf %s \"$RLVGL_CHIP_SRC\""
    res = subprocess.run(
        ["bash", "-c", cmd],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=True,
    )
    assert res.stdout == str(out_dir)
    assert (crate_dir / "LICENSE").exists()
    boards = json.loads((out_dir / "boards.json").read_text())
    assert boards["boards"]["STM32F4DISCOVERY"]["chip"] == "STM32F4"
    assert boards["boards"]["NUCLEO-F401RE"]["chip"] == "STM32F401"
    assert boards["boards"]["STM32F3DISCOVERY"]["chip"] == "STM32F303"
    assert (out_dir / "mcu.json").exists()
    assert (crate_dir / "assets/chipdb.bin.zst").exists()

def test_build_vendor_is_idempotent(tmp_path):
    env = os.environ.copy()
    env["VENDOR_DIR"] = str(REPO_ROOT / "tests/data/chipdb")
    crate_dir = tmp_path / "crate"
    out_dir = tmp_path / "out"
    env["CRATE_DIR"] = str(crate_dir)
    env["OUT_DIR"] = str(out_dir)
    crate_dir.mkdir()
    for _ in range(2):
        subprocess.run(["bash", "tools/build_vendor.sh"], cwd=REPO_ROOT, env=env, check=True)
    license_text = (crate_dir / "LICENSE").read_text(encoding="utf-8")
    assert "STMicroelectronics" in license_text
    boards = json.loads((out_dir / "boards.json").read_text())
    assert boards["boards"]["STM32F4DISCOVERY"]["chip"] == "STM32F4"
    assert boards["boards"]["NUCLEO-F401RE"]["chip"] == "STM32F401"
    assert boards["boards"]["STM32F3DISCOVERY"]["chip"] == "STM32F303"
    assert (out_dir / "mcu.json").exists()
    assert (crate_dir / "assets/chipdb.bin.zst").exists()

