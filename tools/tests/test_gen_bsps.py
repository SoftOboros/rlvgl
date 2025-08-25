"""Tests for the gen_bsps utility."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


def test_gen_bsps(tmp_path: Path, monkeypatch) -> None:
    """Ensure BSP modules are generated from .ioc files."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    stub = bin_dir / "rlvgl-creator"
    stub.write_text(
        "#!/bin/sh\n"
        "# minimal stub emulating rlvgl-creator\n"
        "board=$4\n"
        "out_dir=''\n"
        "while [ \"$1\" != '' ]; do\n"
        "  if [ \"$1\" = '--bsp-out' ]; then\n"
        "    out_dir=$2; shift 2; continue;\n"
        "  fi\n"
        "  shift\n"
        "done\n"
        "echo '// stub BSP for ' $board > \"$out_dir/$board.rs\"\n"
    )
    stub.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}:{os.environ['PATH']}")
    out_dir = tmp_path / "out"
    subprocess.run(
        ["python", "tools/gen_bsps.py", "--input", "tests/data/gen_bsps", "--output", str(out_dir)],
        check=True,
    )
    assert (out_dir / "f407_demo.rs").exists()
    assert (out_dir / "f429_demo.rs").exists()
